//! Host side of the Super Serial Card: the [`SerialBackend`] trait and its implementations.
//!
//! AppleWin has two real transports (SerialComms.cpp): a Win32 COM port read by a worker thread
//! (`CommThread`, :1074) and a TCP server socket on port 1977 driven by window messages
//! (`CommTcpSerialAccept/Receive`, :362-396). Here the card polls a non-blocking backend instead
//! (ADR 0001: no threads or host clock in the core), so every backend is `poll`-style.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// `TCP_SERIAL_PORT_DEFAULT` (SerialComms.h:150).
pub const TCP_SERIAL_PORT_DEFAULT: u16 = 1977;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parity {
    None,
    Odd,
    Even,
    Mark,
    Space,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopBits {
    One,
    OnePointFive,
    Two,
}

/// What `UpdateCommState` pushes to the DCB (SerialComms.cpp:187-208).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineConfig {
    pub baud: u32,
    pub data_bits: u8,
    pub parity: Parity,
    pub stop_bits: StopBits,
    /// DTR asserted (command register bit 0, GH#386).
    pub dtr: bool,
    /// RTS asserted (command register transmit-control bits, GH#311).
    pub rts: bool,
}

/// Modem input lines (`MS_DSR_ON`, `MS_RLSD_ON` = DCD, `MS_CTS_ON`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModemStatus {
    pub dsr: bool,
    pub dcd: bool,
    pub cts: bool,
}

/// A non-blocking byte pipe to the outside world.
pub trait SerialBackend: Send {
    /// `CheckComm` (SerialComms.cpp:212): make the transport usable (bind the listener, open the
    /// port). Called lazily at the first register access after a reset. False if it cannot.
    fn open(&mut self) -> bool;

    fn is_active(&self) -> bool;

    /// `CloseComm` (:318). Called on Ctrl-Reset and when the card is dropped.
    fn close(&mut self);

    /// Moves received bytes (if any) to `out` without blocking.
    fn poll_rx(&mut self, out: &mut VecDeque<u8>);

    /// Sends one byte; false if there is nowhere to send it (e.g. no TCP client yet).
    fn write(&mut self, byte: u8) -> bool;

    /// True when a peer can receive (a TCP client is connected, or a COM port is open). Decides
    /// whether `CommTransmit` does anything.
    fn can_send(&self) -> bool;

    fn modem_status(&mut self) -> ModemStatus;

    /// Pushes baud/format/DTR/RTS changes. Backends without lines ignore it.
    fn configure(&mut self, _line: &LineConfig) {}

    /// True for backends that mirror DSR to DCD like a COM port (GH#553); TCP and the loopback
    /// report the modem lines they are given directly.
    fn mirrors_dsr_to_dcd(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------------------------
// Loopback / in-memory backend (tests and headless use)

#[derive(Default)]
struct MemState {
    rx: VecDeque<u8>,
    tx: Vec<u8>,
    modem: ModemStatus,
    echo: bool,
    line: Option<LineConfig>,
    opens: u32,
}

/// Shared control of a [`LoopbackSerial`] after the card owns it.
#[derive(Clone, Default)]
pub struct LoopbackHandle(Arc<Mutex<MemState>>);

impl LoopbackHandle {
    /// Bytes arriving at the card, as if sent by the remote device.
    pub fn push_rx(&self, data: &[u8]) {
        self.0.lock().unwrap().rx.extend(data);
    }

    /// Bytes the Apple has sent so far (drained).
    pub fn take_tx(&self) -> Vec<u8> {
        std::mem::take(&mut self.0.lock().unwrap().tx)
    }

    pub fn set_modem(&self, m: ModemStatus) {
        self.0.lock().unwrap().modem = m;
    }

    /// The last line configuration the card applied.
    pub fn line(&self) -> Option<LineConfig> {
        self.0.lock().unwrap().line
    }

    /// Times the card opened the transport (AppleWin re-`CheckComm`s after every reset).
    pub fn opens(&self) -> u32 {
        self.0.lock().unwrap().opens
    }
}

/// In-memory serial line: bytes the Apple sends accumulate in a buffer (and come back as input
/// when `echo` is on); the test pushes input through the [`LoopbackHandle`].
pub struct LoopbackSerial {
    state: Arc<Mutex<MemState>>,
    active: bool,
}

impl LoopbackSerial {
    /// A line with DSR/DCD/CTS asserted (a device is attached and ready).
    pub fn new() -> (Self, LoopbackHandle) {
        let state = Arc::new(Mutex::new(MemState {
            modem: ModemStatus { dsr: true, dcd: true, cts: true },
            ..Default::default()
        }));
        (LoopbackSerial { state: state.clone(), active: false }, LoopbackHandle(state))
    }

    /// Like [`new`](Self::new) but every transmitted byte also arrives as input.
    pub fn echo() -> (Self, LoopbackHandle) {
        let (s, h) = Self::new();
        s.state.lock().unwrap().echo = true;
        (s, h)
    }
}

impl SerialBackend for LoopbackSerial {
    fn open(&mut self) -> bool {
        if !self.active {
            self.active = true;
            self.state.lock().unwrap().opens += 1;
        }
        true
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn close(&mut self) {
        self.active = false;
    }

    fn poll_rx(&mut self, out: &mut VecDeque<u8>) {
        out.extend(self.state.lock().unwrap().rx.drain(..));
    }

    fn write(&mut self, byte: u8) -> bool {
        let mut s = self.state.lock().unwrap();
        s.tx.push(byte);
        if s.echo {
            s.rx.push_back(byte);
        }
        true
    }

    fn can_send(&self) -> bool {
        self.active
    }

    fn modem_status(&mut self) -> ModemStatus {
        self.state.lock().unwrap().modem
    }

    fn configure(&mut self, line: &LineConfig) {
        self.state.lock().unwrap().line = Some(*line);
    }
}

// ---------------------------------------------------------------------------------------------
// TCP server (AppleWin's "TCP" serial port choice)

/// Listens on a TCP port and serves the first client that connects (SerialComms.cpp:218-273,
/// :362-396). While a client is connected DSR, DCD and CTS read as asserted (:832-835).
pub struct TcpServerSerial {
    port: u16,
    listener: Option<TcpListener>,
    client: Option<TcpStream>,
}

impl TcpServerSerial {
    pub fn new(port: u16) -> Self {
        TcpServerSerial { port, listener: None, client: None }
    }

    /// Port actually bound (useful with port 0).
    pub fn local_port(&self) -> Option<u16> {
        self.listener.as_ref().and_then(|l| l.local_addr().ok()).map(|a| a.port())
    }

    /// `CommTcpSerialAccept`: one client at a time.
    fn accept(&mut self) {
        if self.client.is_none() {
            if let Some(l) = &self.listener {
                if let Ok((s, _)) = l.accept() {
                    if s.set_nonblocking(true).is_ok() {
                        let _ = s.set_nodelay(true);
                        self.client = Some(s);
                    }
                }
            }
        }
    }
}

impl Default for TcpServerSerial {
    fn default() -> Self {
        Self::new(TCP_SERIAL_PORT_DEFAULT)
    }
}

impl SerialBackend for TcpServerSerial {
    fn open(&mut self) -> bool {
        if self.listener.is_none() {
            match TcpListener::bind(("0.0.0.0", self.port)) {
                Ok(l) if l.set_nonblocking(true).is_ok() => self.listener = Some(l),
                _ => return false,
            }
        }
        true
    }

    fn is_active(&self) -> bool {
        self.listener.is_some()
    }

    fn close(&mut self) {
        // CommTcpSerialClose + closesocket(listen): shutdown so a waiting client sees EOF.
        if let Some(c) = self.client.take() {
            let _ = c.shutdown(std::net::Shutdown::Both);
        }
        self.listener = None;
    }

    fn poll_rx(&mut self, out: &mut VecDeque<u8>) {
        self.accept();
        let mut dead = false;
        if let Some(c) = &mut self.client {
            let mut buf = [0u8; 0x80];
            loop {
                match c.read(&mut buf) {
                    Ok(0) => {
                        dead = true;
                        break;
                    }
                    Ok(n) => out.extend(&buf[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        dead = e.kind() != std::io::ErrorKind::WouldBlock;
                        break;
                    }
                }
            }
        }
        if dead {
            self.client = None;
        }
    }

    fn write(&mut self, byte: u8) -> bool {
        match &mut self.client {
            Some(c) => match c.write(&[byte]) {
                Ok(1) => true,
                Ok(_) => false,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
                Err(_) => {
                    self.client = None;
                    false
                }
            },
            None => false,
        }
    }

    fn can_send(&self) -> bool {
        self.client.is_some()
    }

    fn modem_status(&mut self) -> ModemStatus {
        self.accept();
        let up = self.client.is_some();
        ModemStatus { dsr: up, dcd: up, cts: up }
    }
}

// ---------------------------------------------------------------------------------------------
// Host serial port (feature `serialport`)

/// Names of the serial ports the host reports (AppleWin `ScanCOMPorts`, SerialComms.cpp:1255).
#[cfg(feature = "serialport")]
pub fn list_serial_ports() -> Vec<String> {
    serialport::available_ports().map(|p| p.into_iter().map(|i| i.port_name).collect()).unwrap_or_default()
}

/// A real COM/tty port. Reads are non-blocking (`ReadIntervalTimeout=MAXDWORD`, :297-303).
#[cfg(feature = "serialport")]
pub struct HostSerialPort {
    name: String,
    port: Option<Box<dyn serialport::SerialPort>>,
    line: Option<LineConfig>,
    /// AppleWin mirrors DSR to DCD unless the user configured DCD support (GH#553).
    pub support_dcd: bool,
}

#[cfg(feature = "serialport")]
impl HostSerialPort {
    pub fn new(name: impl Into<String>) -> Self {
        HostSerialPort { name: name.into(), port: None, line: None, support_dcd: false }
    }

    fn apply(&mut self) {
        use serialport::{DataBits, Parity as P, StopBits as S};
        let (Some(p), Some(l)) = (self.port.as_mut(), self.line) else { return };
        let _ = p.set_baud_rate(l.baud);
        let _ = p.set_data_bits(match l.data_bits {
            5 => DataBits::Five,
            6 => DataBits::Six,
            7 => DataBits::Seven,
            _ => DataBits::Eight,
        });
        let _ = p.set_parity(match l.parity {
            Parity::None => P::None,
            Parity::Odd => P::Odd,
            // The crate has no mark/space parity; even is the closest 8-bit-safe fallback.
            Parity::Even | Parity::Mark | Parity::Space => P::Even,
        });
        let _ = p.set_stop_bits(if l.stop_bits == StopBits::One { S::One } else { S::Two });
        let _ = p.write_data_terminal_ready(l.dtr);
        let _ = p.write_request_to_send(l.rts);
    }
}

#[cfg(feature = "serialport")]
impl SerialBackend for HostSerialPort {
    fn open(&mut self) -> bool {
        if self.port.is_none() {
            let r = serialport::new(&self.name, 9600).timeout(std::time::Duration::from_millis(0)).open();
            self.port = r.ok();
            self.apply();
        }
        self.port.is_some()
    }

    fn is_active(&self) -> bool {
        self.port.is_some()
    }

    fn close(&mut self) {
        self.port = None;
    }

    fn poll_rx(&mut self, out: &mut VecDeque<u8>) {
        let Some(p) = self.port.as_mut() else { return };
        let mut buf = [0u8; 0x80];
        while p.bytes_to_read().unwrap_or(0) > 0 {
            match p.read(&mut buf) {
                Ok(n) if n > 0 => out.extend(&buf[..n]),
                _ => break,
            }
        }
    }

    fn write(&mut self, byte: u8) -> bool {
        self.port.as_mut().is_some_and(|p| p.write_all(&[byte]).is_ok())
    }

    fn can_send(&self) -> bool {
        self.port.is_some()
    }

    fn modem_status(&mut self) -> ModemStatus {
        let Some(p) = self.port.as_mut() else { return ModemStatus::default() };
        let dsr = p.read_data_set_ready().unwrap_or(false);
        let dcd = if self.support_dcd { p.read_carrier_detect().unwrap_or(false) } else { dsr };
        ModemStatus { dsr, dcd, cts: p.read_clear_to_send().unwrap_or(false) }
    }

    fn configure(&mut self, line: &LineConfig) {
        self.line = Some(*line);
        self.apply();
    }

    fn mirrors_dsr_to_dcd(&self) -> bool {
        !self.support_dcd
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn poll_until(b: &mut dyn SerialBackend, want: usize) -> Vec<u8> {
        let mut q = VecDeque::new();
        let t = Instant::now();
        while q.len() < want && t.elapsed() < Duration::from_secs(2) {
            b.poll_rx(&mut q);
            std::thread::sleep(Duration::from_millis(2));
        }
        q.into_iter().collect()
    }

    #[test]
    fn tcp_server_waits_for_client_then_exchanges_bytes() {
        let mut s = TcpServerSerial::new(0);
        assert!(s.open() && s.is_active());
        assert!(!s.can_send());
        assert_eq!(s.modem_status(), ModemStatus::default());

        let port = s.local_port().unwrap();
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.write_all(b"hi").unwrap();
        assert_eq!(poll_until(&mut s, 2), b"hi");
        assert!(s.can_send());
        assert_eq!(s.modem_status(), ModemStatus { dsr: true, dcd: true, cts: true });

        assert!(s.write(b'x'));
        let mut b = [0u8; 1];
        c.read_exact(&mut b).unwrap();
        assert_eq!(b[0], b'x');

        drop(c);
        let t = Instant::now();
        while s.can_send() && t.elapsed() < Duration::from_secs(2) {
            s.poll_rx(&mut VecDeque::new()); // sees EOF and drops the client
        }
        assert!(!s.can_send());
    }

    #[test]
    fn loopback_echo_returns_transmitted_bytes() {
        let (mut l, h) = LoopbackSerial::echo();
        assert!(l.open());
        l.write(0x41);
        let mut q = VecDeque::new();
        l.poll_rx(&mut q);
        assert_eq!(q, [0x41]);
        assert_eq!(h.take_tx(), [0x41]);
    }
}
