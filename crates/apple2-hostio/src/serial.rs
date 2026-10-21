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

    /// Why the last [`open`](Self::open) failed, until one succeeds (AUD-P-030). AppleWin keeps
    /// the card disconnected and only logs; we keep the emulation going the same way, but the
    /// message is also queued in [`crate::diagnostics`] for the frontend.
    fn last_error(&self) -> Option<&str> {
        None
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
            modem: ModemStatus {
                dsr: true,
                dcd: true,
                cts: true,
            },
            ..Default::default()
        }));
        (
            LoopbackSerial {
                state: state.clone(),
                active: false,
            },
            LoopbackHandle(state),
        )
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
    error: Option<String>,
}

impl TcpServerSerial {
    pub fn new(port: u16) -> Self {
        TcpServerSerial {
            port,
            listener: None,
            client: None,
            error: None,
        }
    }

    /// Port actually bound (useful with port 0).
    pub fn local_port(&self) -> Option<u16> {
        self.listener
            .as_ref()
            .and_then(|l| l.local_addr().ok())
            .map(|a| a.port())
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
            let r = TcpListener::bind(("0.0.0.0", self.port)).and_then(|l| {
                l.set_nonblocking(true)?;
                Ok(l)
            });
            let src = format!("serial-tcp:{}", self.port);
            match r {
                Ok(l) => {
                    self.listener = Some(l);
                    self.error = None;
                    crate::diagnostics::resolved(&src);
                }
                Err(e) => {
                    let msg = format!("SSC: cannot listen on TCP port {}: {e}", self.port);
                    crate::diagnostics::report(&src, msg.clone());
                    self.error = Some(msg);
                    return false;
                }
            }
        }
        true
    }

    fn last_error(&self) -> Option<&str> {
        self.error.as_deref()
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
        ModemStatus {
            dsr: up,
            dcd: up,
            cts: up,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Serial port discovery (AUD-P-031)

/// Operating system families the heuristic scan tells apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostOs {
    Linux,
    MacOs,
    Other,
}

impl HostOs {
    pub const CURRENT: HostOs = if cfg!(target_os = "linux") {
        HostOs::Linux
    } else if cfg!(target_os = "macos") {
        HostOs::MacOs
    } else {
        HostOs::Other
    };
}

/// Picks serial devices out of the entries of `/dev` (Unix). Linux: USB adapters (`ttyUSB*`),
/// CDC/ACM modems and boards (`ttyACM*`), on-board UARTs (`ttyS*`, `ttyAMA*`; only when the kernel
/// says a device sits behind them, since `/dev` has 32 `ttyS` nodes on most systems) and
/// Bluetooth RFCOMM links (`rfcomm*`). macOS: the callout devices (`cu.*`) and the USB `tty.*`.
/// `has_device(name)` answers whether `/sys/class/tty/<name>/device` exists. Sorted.
pub fn classify_dev_names<I, S>(
    names: I,
    os: HostOs,
    has_device: &dyn Fn(&str) -> bool,
) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut v: Vec<String> = names
        .into_iter()
        .filter_map(|n| {
            let n = n.as_ref();
            let keep = match os {
                HostOs::Linux => {
                    n.starts_with("ttyUSB")
                        || n.starts_with("ttyACM")
                        || n.starts_with("rfcomm")
                        || ((n.starts_with("ttyS") || n.starts_with("ttyAMA")) && has_device(n))
                }
                HostOs::MacOs => n.starts_with("cu.") || n.starts_with("tty.usb"),
                HostOs::Other => false,
            };
            keep.then(|| format!("/dev/{n}"))
        })
        .collect();
    v.sort();
    v
}

/// Heuristic scan used besides (or instead of) the `serialport` crate: `/dev` on Unix, and on
/// Windows the names `COM1`..`COM32` that exist (opening `\\.\COMn` fails with "not found" for a
/// port that is not there, and with "access denied" for one another program holds).
pub fn scan_serial_ports() -> Vec<String> {
    #[cfg(windows)]
    {
        (1..=32)
            .map(|n| format!("COM{n}"))
            .filter(|name| {
                match std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(format!(r"\\.\{name}"))
                {
                    Ok(_) => true,
                    Err(e) => e.kind() != std::io::ErrorKind::NotFound,
                }
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        let names: Vec<String> = std::fs::read_dir("/dev")
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        classify_dev_names(names, HostOs::CURRENT, &|n| {
            let dir = std::path::Path::new("/sys/class/tty").join(n);
            // Without sysfs (a container, macOS) there is nothing to ask: trust `/dev`.
            !std::path::Path::new("/sys/class/tty").is_dir() || dir.join("device").exists()
        })
    }
}

/// Serial ports to offer in the SSC options (AppleWin `ScanCOMPorts`, SerialComms.cpp:1255).
/// With the `serialport` feature the crate's `available_ports()` comes first (it asks the OS:
/// SetupAPI/registry on Windows, IOKit on macOS, udev or sysfs on Linux) and the heuristic scan
/// adds what it missed; without the feature only the scan runs. No duplicates.
pub fn list_serial_ports() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    #[cfg(feature = "serialport")]
    if let Ok(ports) = serialport::available_ports() {
        v.extend(ports.into_iter().map(|p| p.port_name));
    }
    for p in scan_serial_ports() {
        if !v.contains(&p) {
            v.push(p);
        }
    }
    v.sort();
    v
}

// ---------------------------------------------------------------------------------------------
// Host serial port (feature `serialport`)

/// A real COM/tty port. Reads are non-blocking (`ReadIntervalTimeout=MAXDWORD`, :297-303).
#[cfg(feature = "serialport")]
pub struct HostSerialPort {
    name: String,
    port: Option<Box<dyn serialport::SerialPort>>,
    line: Option<LineConfig>,
    /// AppleWin mirrors DSR to DCD unless the user configured DCD support (GH#553).
    pub support_dcd: bool,
    error: Option<String>,
}

#[cfg(feature = "serialport")]
impl HostSerialPort {
    pub fn new(name: impl Into<String>) -> Self {
        HostSerialPort {
            name: name.into(),
            port: None,
            line: None,
            support_dcd: false,
            error: None,
        }
    }

    fn apply(&mut self) {
        use serialport::{DataBits, Parity as P, StopBits as S};
        let (Some(p), Some(l)) = (self.port.as_mut(), self.line) else {
            return;
        };
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
        let _ = p.set_stop_bits(if l.stop_bits == StopBits::One {
            S::One
        } else {
            S::Two
        });
        let _ = p.write_data_terminal_ready(l.dtr);
        let _ = p.write_request_to_send(l.rts);
    }
}

#[cfg(feature = "serialport")]
impl SerialBackend for HostSerialPort {
    fn open(&mut self) -> bool {
        if self.port.is_none() {
            let r = serialport::new(&self.name, 9600)
                .timeout(std::time::Duration::from_millis(0))
                .open();
            let src = format!("serial:{}", self.name);
            match r {
                Ok(p) => {
                    self.port = Some(p);
                    self.error = None;
                    crate::diagnostics::resolved(&src);
                    self.apply();
                }
                Err(e) => {
                    let msg = format!("SSC: cannot open serial port '{}': {}", self.name, e);
                    crate::diagnostics::report(&src, msg.clone());
                    self.error = Some(msg);
                }
            }
        }
        self.port.is_some()
    }

    fn last_error(&self) -> Option<&str> {
        self.error.as_deref()
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
        self.port
            .as_mut()
            .is_some_and(|p| p.write_all(&[byte]).is_ok())
    }

    fn can_send(&self) -> bool {
        self.port.is_some()
    }

    fn modem_status(&mut self) -> ModemStatus {
        let Some(p) = self.port.as_mut() else {
            return ModemStatus::default();
        };
        let dsr = p.read_data_set_ready().unwrap_or(false);
        let dcd = if self.support_dcd {
            p.read_carrier_detect().unwrap_or(false)
        } else {
            dsr
        };
        ModemStatus {
            dsr,
            dcd,
            cts: p.read_clear_to_send().unwrap_or(false),
        }
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
        assert_eq!(
            s.modem_status(),
            ModemStatus {
                dsr: true,
                dcd: true,
                cts: true
            }
        );

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

    #[test]
    fn tcp_server_bind_failure_is_kept_and_reported() {
        // Hold a port so the card cannot take it.
        let held = TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let port = held.local_addr().unwrap().port();
        let mut b = TcpServerSerial::new(port);
        assert!(!b.open(), "the card stays disconnected");
        assert!(!b.is_active());
        let e = b.last_error().expect("error kept").to_string();
        assert!(
            e.contains(&port.to_string()) && e.starts_with("SSC:"),
            "{e}"
        );
        assert!(crate::diagnostics::peek().contains(&e));
        drop(held);
        assert!(b.open(), "works once the port is free");
        assert!(b.last_error().is_none());
    }

    #[cfg(feature = "serialport")]
    #[test]
    fn missing_host_serial_device_fails_with_a_description() {
        let name = "/dev/apple2_hostio_no_such_tty";
        let mut p = HostSerialPort::new(name);
        assert!(!p.open());
        assert!(!p.is_active() && !p.can_send());
        let e = p.last_error().expect("error kept");
        assert!(
            e.contains(name) && e.contains("cannot open serial port"),
            "{e}"
        );
        assert!(crate::diagnostics::peek().iter().any(|m| m == e));
        // Every later access keeps failing without panicking or flooding the queue.
        let mut rx = VecDeque::new();
        p.poll_rx(&mut rx);
        assert!(!p.write(1) && rx.is_empty());
    }

    #[cfg(all(feature = "serialport", unix))]
    #[test]
    fn host_serial_device_without_permission_fails_with_a_description() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("apple2_hostio_tty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ttyNOPERM");
        std::fs::write(&path, b"").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Root ignores the mode.
        if std::fs::File::open(&path).is_err() {
            let mut p = HostSerialPort::new(path.to_string_lossy());
            assert!(!p.open());
            let e = p.last_error().expect("error kept").to_string();
            assert!(e.contains("ttyNOPERM"), "{e}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn linux_scan_keeps_usb_acm_rfcomm_and_real_uarts() {
        let dev = [
            "null",
            "tty0",
            "ttyS0",
            "ttyS1",
            "ttyS2",
            "ttyUSB1",
            "ttyUSB0",
            "ttyACM0",
            "rfcomm0",
            "ttyAMA0",
            "cu.usbserial",
            "tty.usbserial",
        ];
        let real_uarts = |n: &str| n == "ttyS0" || n == "ttyAMA0";
        assert_eq!(
            classify_dev_names(dev, HostOs::Linux, &real_uarts),
            [
                "/dev/rfcomm0",
                "/dev/ttyACM0",
                "/dev/ttyAMA0",
                "/dev/ttyS0",
                "/dev/ttyUSB0",
                "/dev/ttyUSB1"
            ]
        );
    }

    #[test]
    fn macos_scan_keeps_callout_and_usb_tty_devices() {
        let dev = [
            "cu.Bluetooth-Incoming-Port",
            "cu.usbserial-1410",
            "tty.usbserial-1410",
            "ttyp0",
        ];
        assert_eq!(
            classify_dev_names(dev, HostOs::MacOs, &|_| true),
            [
                "/dev/cu.Bluetooth-Incoming-Port",
                "/dev/cu.usbserial-1410",
                "/dev/tty.usbserial-1410"
            ]
        );
        assert!(classify_dev_names(dev, HostOs::Other, &|_| true).is_empty());
    }

    #[test]
    fn host_port_list_has_no_duplicates() {
        let v = list_serial_ports();
        let mut d = v.clone();
        d.dedup();
        assert_eq!(v, d);
    }
}
