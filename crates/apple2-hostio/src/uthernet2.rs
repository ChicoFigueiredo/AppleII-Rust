//! Uthernet II: a WIZnet W5100 with four sockets (Uthernet2.cpp, W5100.h).
//!
//! TCP and UDP sockets are mapped to host sockets (`std::net`), so the card is fully usable
//! without a raw-Ethernet backend; IP RAW and MAC RAW sockets go through the [`NetworkBackend`].
//! "Virtual DNS" (Uthernet2.cpp:32-37) is an AppleWin extension: a socket opened with mode bit 3
//! set resolves the length-prefixed host name stored at `Sn+$2A` and puts the address in `Sn_DIPR`.
//!
//! Extensions over AppleWin, all additive: `LISTEN` (command `$02`) for TCP servers and binding
//! UDP sockets to `Sn_PORT` (AppleWin ignores the port and always uses an ephemeral one); TCP
//! `CONNECT` runs on a helper thread so a slow host cannot freeze the emulator.

use crate::network::{
    create_eth2_frame, ip_payload, Ipv4, Mac, NetworkBackend, ETH_MINIMUM_SIZE, MAX_RXLENGTH,
};
use apple2_core::card::CardCtx;
use apple2_core::Card;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

// Common registers (W5100.h:15-36).
const MR: u16 = 0x0000;
const GAR0: u16 = 0x0001;
const GAR3: u16 = 0x0004;
const SUBR0: u16 = 0x0005;
const SUBR3: u16 = 0x0008;
const SHAR0: u16 = 0x0009;
const SHAR5: u16 = 0x000E;
const SIPR0: u16 = 0x000F;
const SIPR3: u16 = 0x0012;
const RTR0: u16 = 0x0017;
const RTR1: u16 = 0x0018;
const RCR: u16 = 0x0019;
const RMSR: u16 = 0x001A;
const TMSR: u16 = 0x001B;
const PTIMER: u16 = 0x0028;
const UPORT1: u16 = 0x002F;
const S0_BASE: u16 = 0x0400;
const S3_MAX: u16 = 0x07FF;
const TX_BASE: u16 = 0x4000;
const RX_BASE: u16 = 0x6000;
const MEM_MAX: u16 = 0x7FFF;
const MEM_SIZE: usize = 0x8000;

const MR_AI: u8 = 0x02;
const MR_RST: u8 = 0x80;

const SN_MR_PROTO_MASK: u8 = 0x0F;
const SN_MR_MF: u8 = 0x40;
const SN_MR_TCP: u8 = 0x01;
const SN_MR_UDP: u8 = 0x02;
const SN_MR_IPRAW: u8 = 0x03;
const SN_MR_MACRAW: u8 = 0x04;
const SN_VIRTUAL_DNS: u8 = 0x08;

const SN_CR_OPEN: u8 = 0x01;
const SN_CR_LISTEN: u8 = 0x02;
const SN_CR_CONNECT: u8 = 0x04;
const SN_CR_DISCON: u8 = 0x08;
const SN_CR_CLOSE: u8 = 0x10;
const SN_CR_SEND: u8 = 0x20;
const SN_CR_RECV: u8 = 0x40;

// Socket registers, relative to the socket's base.
const SN_MR: u16 = 0x00;
const SN_CR: u16 = 0x01;
const SN_SR: u16 = 0x03;
const SN_PORT0: u16 = 0x04;
const SN_PORT1: u16 = 0x05;
const SN_DHAR0: u16 = 0x06;
const SN_DIPR0: u16 = 0x0C;
const SN_DIPR3: u16 = 0x0F;
const SN_DPORT0: u16 = 0x10;
const SN_DPORT1: u16 = 0x11;
const SN_PROTO: u16 = 0x14;
const SN_TOS: u16 = 0x15;
const SN_TTL: u16 = 0x16;
const SN_TX_FSR0: u16 = 0x20;
const SN_TX_FSR1: u16 = 0x21;
const SN_TX_RD0: u16 = 0x22;
const SN_TX_RD1: u16 = 0x23;
const SN_TX_WR0: u16 = 0x24;
const SN_TX_WR1: u16 = 0x25;
const SN_RX_RSR0: u16 = 0x26;
const SN_RX_RSR1: u16 = 0x27;
const SN_RX_RD0: u16 = 0x28;
const SN_RX_RD1: u16 = 0x29;
const SN_DNS_NAME_LEN: u16 = 0x2A;
const SN_DNS_NAME_BEGIN: u16 = 0x2B;
const SN_DNS_NAME_END: u16 = 0xFF;
const SN_DNS_NAME_CPTY: u16 = SN_DNS_NAME_END - SN_DNS_NAME_BEGIN;

// Socket status values.
pub const SR_CLOSED: u8 = 0x00;
pub const SR_INIT: u8 = 0x13;
pub const SR_LISTEN: u8 = 0x14;
pub const SR_SYNSENT: u8 = 0x15;
pub const SR_ESTABLISHED: u8 = 0x17;
pub const SR_UDP: u8 = 0x22;
pub const SR_IPRAW: u8 = 0x32;
pub const SR_MACRAW: u8 = 0x42;

/// Poll interval for connection progress when the guest is not looking at the status register.
const POLL_CYCLES: i32 = 1000;
const TOKEN_POLL: u16 = 0;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Host side of a socket.
enum HostSocket {
    None,
    /// `CONNECT` in progress on a helper thread.
    Connecting(Receiver<io::Result<TcpStream>>),
    Tcp(TcpStream),
    Listener(TcpListener),
    Udp(UdpSocket),
}

struct Socket {
    transmit_base: u16,
    transmit_size: u16,
    receive_base: u16,
    receive_size: u16,
    register_address: u16,
    /// Write position inside the receive ring (the chip's internal `Sn_RX_WR`).
    rx_wr: u16,
    rx_rsr: u16,
    status: u8,
    header_size: u8,
    host: HostSocket,
}

impl Socket {
    fn new(register_address: u16) -> Self {
        Socket {
            transmit_base: 0,
            transmit_size: 0,
            receive_base: 0,
            receive_size: 0,
            register_address,
            rx_wr: 0,
            rx_rsr: 0,
            status: SR_CLOSED,
            header_size: 0,
            host: HostSocket::None,
        }
    }

    /// `clearFD`: drops the host socket and closes the W5100 socket.
    fn clear_fd(&mut self) {
        self.host = HostSocket::None;
        self.set_status(SR_CLOSED);
    }

    /// `setStatus`: also fixes the size of the per-packet header the receive buffer carries.
    fn set_status(&mut self, status: u8) {
        self.status = status;
        self.header_size = match status {
            SR_UDP => 4 + 2 + 2,
            SR_IPRAW => 4 + 2,
            SR_MACRAW => 2,
            _ => 0,
        };
    }

    fn is_open(&self) -> bool {
        !matches!(
            self.host,
            HostSocket::None | HostSocket::Listener(_) | HostSocket::Connecting(_)
        ) && (self.status == SR_ESTABLISHED || self.status == SR_UDP)
    }

    /// `isThereRoomFor`: never fills the buffer completely.
    fn is_there_room_for(&self, len: usize) -> bool {
        (self.rx_rsr as usize + len + self.header_size as usize) < self.receive_size as usize
    }

    fn free_room(&self) -> u16 {
        let total = self.receive_size.saturating_sub(self.rx_rsr);
        total.saturating_sub(self.header_size as u16)
    }

    /// `Socket::process` (Uthernet2.cpp:269-306) plus the LISTEN accept.
    fn process(&mut self) {
        match &mut self.host {
            HostSocket::Connecting(rx) => match rx.try_recv() {
                Ok(Ok(stream)) => {
                    if stream.set_nonblocking(true).is_ok() {
                        let _ = stream.set_nodelay(true);
                        self.host = HostSocket::Tcp(stream);
                        self.set_status(SR_ESTABLISHED);
                    } else {
                        self.clear_fd();
                    }
                }
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => self.clear_fd(),
                Err(TryRecvError::Empty) => {}
            },
            HostSocket::Listener(l) => {
                if let Ok((stream, _)) = l.accept() {
                    if stream.set_nonblocking(true).is_ok() {
                        let _ = stream.set_nodelay(true);
                        self.host = HostSocket::Tcp(stream);
                        self.set_status(SR_ESTABLISHED);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Appends to the socket's receive ring (`write8`/`writeData`, Uthernet2.cpp:119-141).
fn ring_write(s: &mut Socket, mem: &mut [u8], data: &[u8]) {
    for &b in data {
        mem[(s.receive_base + s.rx_wr) as usize] = b;
        s.rx_wr = (s.rx_wr + 1) % s.receive_size;
        s.rx_rsr += 1;
    }
}

/// Big-endian register word (`readNetworkWord`).
fn be16(mem: &[u8], a: u16) -> u16 {
    u16::from_be_bytes([mem[a as usize], mem[a as usize + 1]])
}

enum PacketDestination {
    Host,
    Broadcast,
    Other,
}

/// The state AppleWin saves for the Uthernet II (`Uthernet2::SaveSnapshot`, Uthernet2.cpp:1538).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uthernet2Snapshot {
    /// `getInterfaceName()` (host side, ignored by [`Uthernet2::apply_snapshot`]).
    pub interface: String,
    pub virtual_dns: bool,
    pub mode_register: u8,
    pub data_address: u16,
    /// The 32 KB W5100 memory. AppleWin keeps only the common registers (`0x0000-0x002F`), the
    /// socket registers (`0x0400-0x07FF`) and the TX/RX memory (`0x4000-0x7FFF`); the rest is
    /// reserved and reads as zero.
    pub memory: Vec<u8>,
    /// Per socket: `(Sn_RX_WR, Sn_RX_RSR, status)`; only RAW sockets keep a status.
    pub sockets: [(u16, u16, u8); 4],
}

pub struct Uthernet2 {
    backend: Box<dyn NetworkBackend>,
    virtual_dns: bool,
    mode_register: u8,
    data_address: u16,
    memory: Vec<u8>,
    sockets: Vec<Socket>,
    arp_cache: HashMap<Ipv4, Mac>,
    dns_cache: HashMap<String, Ipv4>,
}

impl Uthernet2 {
    /// `virtual_dns` enables the name-resolution extension (on by default in AppleWin).
    pub fn new(backend: Box<dyn NetworkBackend>, virtual_dns: bool) -> Self {
        let mut c = Uthernet2 {
            backend,
            virtual_dns,
            mode_register: 0,
            data_address: 0,
            memory: Vec::new(),
            sockets: Vec::new(),
            arp_cache: HashMap::new(),
            dns_cache: HashMap::new(),
        };
        c.reset_chip(true);
        c
    }

    /// `Reset` (:1362-1409). A power cycle also clears the data address and the caches.
    fn reset_chip(&mut self, power_cycle: bool) {
        self.mode_register = 0;
        if power_cycle {
            self.data_address = 0; // NOT reset otherwise (Uthernet II p.10)
            self.arp_cache.clear();
            self.dns_cache.clear();
        }
        self.memory = vec![0; MEM_SIZE];
        self.sockets = (0..4)
            .map(|i| Socket::new(S0_BASE + ((i as u16) << 8)))
            .collect();
        for i in 0..4 {
            self.reset_rx_tx_buffers(i);
            let base = self.sockets[i].register_address;
            for k in 0..6 {
                self.memory[(base + SN_DHAR0 + k) as usize] = 0xFF;
            }
            self.memory[(base + SN_TTL) as usize] = 0x80;
        }
        self.memory[RTR0 as usize] = 0x07;
        self.memory[RTR1 as usize] = 0xD0;
        self.memory[RCR as usize] = 0x08;
        self.set_rx_sizes(0x55);
        self.set_tx_sizes(0x55);
        if !self.virtual_dns {
            self.memory[PTIMER as usize] = 0x28; // 0 if Virtual DNS is supported
        }
    }

    pub fn snapshot(&self) -> Uthernet2Snapshot {
        let mut memory = vec![0u8; MEM_SIZE];
        for r in [
            0..UPORT1 as usize + 1,
            S0_BASE as usize..S3_MAX as usize + 1,
            TX_BASE as usize..MEM_SIZE,
        ] {
            memory[r.clone()].copy_from_slice(&self.memory[r]);
        }
        let mut sockets = [(0, 0, SR_CLOSED); 4];
        for (o, s) in sockets.iter_mut().zip(&self.sockets) {
            *o = (
                s.rx_wr,
                s.rx_rsr,
                if matches!(s.status, SR_MACRAW | SR_IPRAW) {
                    s.status
                } else {
                    SR_CLOSED
                },
            );
        }
        Uthernet2Snapshot {
            interface: self.backend.interface_name().to_string(),
            virtual_dns: self.virtual_dns,
            mode_register: self.mode_register,
            data_address: self.data_address,
            memory,
            sockets,
        }
    }

    /// `LoadSnapshot` (:1566): resets the chip with the saved Virtual DNS setting, then restores
    /// registers and memory. RAW sockets are restored, others come back closed (:337-363).
    pub fn apply_snapshot(&mut self, s: &Uthernet2Snapshot) {
        self.virtual_dns = s.virtual_dns;
        self.reset_chip(true);
        self.mode_register = s.mode_register;
        self.data_address = s.data_address;
        for r in [
            0..UPORT1 as usize + 1,
            S0_BASE as usize..S3_MAX as usize + 1,
            TX_BASE as usize..MEM_SIZE,
        ] {
            let end = r.end.min(s.memory.len());
            if r.start < end {
                self.memory[r.start..end].copy_from_slice(&s.memory[r.start..end]);
            }
        }
        let (r, t) = (self.memory[RMSR as usize], self.memory[TMSR as usize]);
        self.set_rx_sizes(r);
        self.set_tx_sizes(t);
        for (sock, &(wr, rsr, status)) in self.sockets.iter_mut().zip(&s.sockets) {
            sock.rx_wr = wr;
            sock.rx_rsr = rsr;
            sock.set_status(if matches!(status, SR_MACRAW | SR_IPRAW) {
                status
            } else {
                SR_CLOSED
            });
        }
    }

    fn set_tx_sizes(&mut self, value: u8) {
        self.memory[TMSR as usize] = value;
        let mut v = value;
        let mut base = TX_BASE;
        for s in &mut self.sockets {
            s.transmit_base = base;
            let size = 1u32 << (10 + (v & 3));
            v >>= 2;
            base = (base as u32 + size).min(RX_BASE as u32) as u16;
            s.transmit_size = base - s.transmit_base;
        }
    }

    fn set_rx_sizes(&mut self, value: u8) {
        self.memory[RMSR as usize] = value;
        let mut v = value;
        let mut base = RX_BASE;
        for s in &mut self.sockets {
            s.receive_base = base;
            let size = 1u32 << (10 + (v & 3));
            v >>= 2;
            base = (base as u32 + size).min(MEM_SIZE as u32) as u16;
            s.receive_size = base - s.receive_base;
        }
    }

    fn reg(&self, i: usize, off: u16) -> u8 {
        self.memory[(self.sockets[i].register_address + off) as usize]
    }

    fn set_reg(&mut self, i: usize, off: u16, v: u8) {
        let a = self.sockets[i].register_address + off;
        self.memory[a as usize] = v;
    }

    fn reg_word(&self, i: usize, off: u16) -> u16 {
        be16(&self.memory, self.sockets[i].register_address + off)
    }

    fn dest_ip(&self, i: usize) -> Ipv4 {
        let a = (self.sockets[i].register_address + SN_DIPR0) as usize;
        [
            self.memory[a],
            self.memory[a + 1],
            self.memory[a + 2],
            self.memory[a + 3],
        ]
    }

    fn tx_data_size(&self, i: usize) -> u16 {
        let size = self.sockets[i].transmit_size;
        if size == 0 {
            return 0; // no memory assigned to this socket (Sn size registers): nothing queued
        }
        let mask = size - 1;
        let rd = self.reg_word(i, SN_TX_RD0) & mask;
        let wr = self.reg_word(i, SN_TX_WR0) & mask;
        if wr >= rd {
            wr - rd
        } else {
            wr + size - rd
        }
    }

    /// `updateRSR` (:520-547): resynchronises the received size with the read pointer.
    fn update_rsr(&mut self, i: usize) {
        let size = self.sockets[i].receive_size;
        if size == 0 {
            self.sockets[i].rx_rsr = 0;
            return;
        }
        let mask = size - 1;
        let rd = self.reg_word(i, SN_RX_RD0) & mask;
        let wr = self.sockets[i].rx_wr & mask;
        self.sockets[i].rx_rsr = if wr >= rd { wr - rd } else { wr + size - rd };
    }

    // ---- receive ----

    /// `receiveForMacAddress` (:549-593).
    fn receive_for_mac(&mut self, accept_all: bool) -> Option<(Vec<u8>, PacketDestination)> {
        let mac = &self.memory[SHAR0 as usize..=SHAR5 as usize];
        let mac: [u8; 6] = mac.try_into().unwrap();
        while let Some(frame) = self.backend.receive() {
            if frame.len() >= ETH_MINIMUM_SIZE {
                if frame[..6] == mac {
                    return Some((frame, PacketDestination::Host));
                }
                if frame[..6] == [0xFF; 6] {
                    return Some((frame, PacketDestination::Broadcast));
                }
                if accept_all {
                    return Some((frame, PacketDestination::Other));
                }
            }
            // skip this frame and try another one
        }
        None
    }

    /// `receiveOnePacketRaw` (:595-659): a frame goes to a matching IP RAW socket, else to MAC RAW.
    fn receive_one_packet_raw(&mut self) {
        let mut accept_all = false;
        let mut mac_raw = false;
        if self.sockets[0].status == SR_MACRAW {
            mac_raw = true;
            accept_all = self.reg(0, SN_MR) & SN_MR_MF == 0;
        }
        let Some((frame, dest)) = self.receive_for_mac(accept_all) else {
            return;
        };
        let frame = &frame[..frame.len().min(MAX_RXLENGTH)];
        let ip = ip_payload(frame);
        let mut ip_raw = None;
        if !matches!(dest, PacketDestination::Other) {
            if let Some(p) = &ip {
                ip_raw = (0..self.sockets.len()).find(|&i| {
                    self.sockets[i].status == SR_IPRAW && self.reg(i, SN_PROTO) == p.protocol
                });
            }
        }
        if let (Some(i), Some(p)) = (ip_raw, &ip) {
            let s = &mut self.sockets[i];
            if s.is_there_room_for(p.payload.len()) {
                ring_write(s, &mut self.memory, &p.source);
                ring_write(s, &mut self.memory, &(p.payload.len() as u16).to_be_bytes());
                ring_write(s, &mut self.memory, p.payload);
            }
        } else if mac_raw {
            let s = &mut self.sockets[0];
            if s.is_there_room_for(frame.len()) {
                // The size includes its own two bytes.
                ring_write(
                    s,
                    &mut self.memory,
                    &((frame.len() + 2) as u16).to_be_bytes(),
                );
                ring_write(s, &mut self.memory, frame);
            }
        } // else the packet is dropped
    }

    /// `receiveOnePacketFromSocket` (:704-745): TCP and UDP.
    fn receive_one_packet_from_socket(&mut self, i: usize) {
        if !self.sockets[i].is_open() {
            return;
        }
        let free = self.sockets[i].free_room();
        if free <= 32 {
            return; // avoid meaningless reads
        }
        let mut buf = vec![0u8; free as usize - 1]; // do not fill the buffer completely
        let s = &mut self.sockets[i];
        let mut closed = false;
        let mut header = Vec::new();
        let n = match &mut s.host {
            HostSocket::Tcp(t) => match t.read(&mut buf) {
                Ok(0) => {
                    closed = true;
                    0
                }
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    0
                }
                Err(_) => {
                    closed = true;
                    0
                }
            },
            HostSocket::Udp(u) => match u.recv_from(&mut buf) {
                Ok((n, SocketAddr::V4(src))) => {
                    header.extend_from_slice(&src.ip().octets());
                    header.extend_from_slice(&src.port().to_be_bytes());
                    header.extend_from_slice(&(n as u16).to_be_bytes());
                    n
                }
                Ok(_) => 0,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => 0,
                Err(_) => {
                    closed = true;
                    0
                }
            },
            _ => 0,
        };
        if closed {
            s.clear_fd();
        } else if n > 0 || !header.is_empty() {
            ring_write(s, &mut self.memory, &header);
            ring_write(s, &mut self.memory, &buf[..n]);
        }
    }

    /// `receiveOnePacket` (:747-770), triggered by reading `Sn_RX_RSR`.
    fn receive_one_packet(&mut self, i: usize) {
        match self.sockets[i].status {
            SR_MACRAW | SR_IPRAW => self.receive_one_packet_raw(),
            SR_ESTABLISHED | SR_UDP => self.receive_one_packet_from_socket(i),
            _ => {}
        }
    }

    // ---- send ----

    /// `getMACAddress` (:1487-1525), with the ARP cache.
    fn mac_for(&mut self, ip: Ipv4) -> Mac {
        if let Some(m) = self.arp_cache.get(&ip) {
            return *m;
        }
        let source: Ipv4 = self.memory[SIPR0 as usize..=SIPR3 as usize]
            .try_into()
            .unwrap();
        let mac = if ip == source {
            self.memory[SHAR0 as usize..=SHAR5 as usize]
                .try_into()
                .unwrap()
        } else {
            let mut mac = [0xFF; 6]; // fall back to broadcast
            if ip != [255; 4] {
                let subnet: Ipv4 = self.memory[SUBR0 as usize..=SUBR3 as usize]
                    .try_into()
                    .unwrap();
                let same = (0..4).all(|k| ip[k] & subnet[k] == source[k] & subnet[k]);
                let target = if same {
                    ip // same network: ask for the host itself
                } else {
                    self.memory[GAR0 as usize..=GAR3 as usize]
                        .try_into()
                        .unwrap() // via the gateway
                };
                if let Some(m) = self.backend.mac_for_ip(target) {
                    mac = m;
                }
            }
            mac
        };
        self.arp_cache.insert(ip, mac);
        mac
    }

    /// `sendData` (:844-895): drains the transmit ring and dispatches by socket type.
    fn send_data(&mut self, i: usize) {
        let s = &self.sockets[i];
        let size = s.transmit_size;
        if size == 0 {
            return; // nothing can be queued without transmit memory
        }
        let mask = size - 1;
        let rd = (self.reg_word(i, SN_TX_RD0) & mask) as usize;
        let wr = (self.reg_word(i, SN_TX_WR0) & mask) as usize;
        let base = s.transmit_base as usize;
        let data: Vec<u8> = if rd < wr {
            self.memory[base + rd..base + wr].to_vec()
        } else {
            let mut d = self.memory[base + rd..base + size as usize].to_vec();
            d.extend_from_slice(&self.memory[base..base + wr]);
            d
        };
        // Move the read pointer to the writer.
        self.set_reg(i, SN_TX_RD0, (wr >> 8) as u8);
        self.set_reg(i, SN_TX_RD1, wr as u8);
        match self.sockets[i].status {
            SR_MACRAW => self.backend.transmit(&data),
            SR_IPRAW => {
                let ttl = self.reg(i, SN_TTL);
                let tos = self.reg(i, SN_TOS);
                let protocol = self.reg(i, SN_PROTO);
                let source: Ipv4 = self.memory[SIPR0 as usize..=SIPR3 as usize]
                    .try_into()
                    .unwrap();
                let dest = self.dest_ip(i);
                let smac: Mac = self.memory[SHAR0 as usize..=SHAR5 as usize]
                    .try_into()
                    .unwrap();
                let dmac = self.mac_for(dest);
                let frame = create_eth2_frame(&data, smac, dmac, ttl, tos, protocol, source, dest);
                self.backend.transmit(&frame);
            }
            SR_ESTABLISHED | SR_UDP => self.send_to_socket(i, &data),
            _ => {}
        }
    }

    /// `sendDataToSocket` (:812-842).
    fn send_to_socket(&mut self, i: usize, data: &[u8]) {
        if !self.sockets[i].is_open() {
            return;
        }
        let dest = SocketAddr::from((Ipv4Addr::from(self.dest_ip(i)), self.reg_word(i, SN_DPORT0)));
        let result = match &mut self.sockets[i].host {
            HostSocket::Tcp(t) => write_blocking(t, data),
            HostSocket::Udp(u) => u.send_to(data, dest).map(|_| ()),
            _ => Ok(()),
        };
        if let Err(e) = result {
            if e.kind() != io::ErrorKind::WouldBlock {
                self.sockets[i].clear_fd();
            }
        }
    }

    // ---- commands ----

    fn reset_rx_tx_buffers(&mut self, i: usize) {
        self.sockets[i].rx_wr = 0;
        self.sockets[i].rx_rsr = 0;
        for off in [
            SN_TX_RD0, SN_TX_RD1, SN_TX_WR0, SN_TX_WR1, SN_RX_RD0, SN_RX_RD1,
        ] {
            self.set_reg(i, off, 0);
        }
    }

    /// `openSocket` (:947-1001).
    fn open_socket(&mut self, i: usize) {
        self.sockets[i].clear_fd();
        let mr = self.reg(i, SN_MR);
        let protocol = mr & SN_MR_PROTO_MASK;
        let dns = protocol & SN_VIRTUAL_DNS != 0;
        if dns && !self.virtual_dns {
            return; // requested but not enabled: nothing we can do
        }
        match protocol & !SN_VIRTUAL_DNS {
            SN_MR_IPRAW => self.sockets[i].set_status(SR_IPRAW),
            SN_MR_MACRAW if !dns => self.sockets[i].set_status(SR_MACRAW),
            SN_MR_TCP => self.sockets[i].set_status(SR_INIT),
            SN_MR_UDP => {
                // Bind to the requested port when it is set, else any port (AppleWin uses the latter).
                let port = self.reg_word(i, SN_PORT0);
                let sock =
                    UdpSocket::bind(("0.0.0.0", port)).or_else(|_| UdpSocket::bind(("0.0.0.0", 0)));
                match sock {
                    Ok(u) if u.set_nonblocking(true).is_ok() => {
                        self.sockets[i].host = HostSocket::Udp(u);
                        self.sockets[i].set_status(SR_UDP);
                    }
                    _ => self.sockets[i].clear_fd(),
                }
            }
            _ => {}
        }
        // Only TCP, UDP and IP RAW have a *_DNS mode (openSocket:988-995).
        if dns
            && matches!(
                protocol & !SN_VIRTUAL_DNS,
                SN_MR_IPRAW | SN_MR_TCP | SN_MR_UDP
            )
        {
            self.resolve_dns(i);
        }
        self.reset_rx_tx_buffers(i);
    }

    /// `resolveDNS` (:1012-1037): 0.0.0.0 signals failure.
    fn resolve_dns(&mut self, i: usize) {
        let base = self.sockets[i].register_address;
        let len = self.reg(i, SN_DNS_NAME_LEN) as u16;
        let mut ip = [0u8; 4];
        if len <= SN_DNS_NAME_CPTY {
            let start = (base + SN_DNS_NAME_BEGIN) as usize;
            let name =
                String::from_utf8_lossy(&self.memory[start..start + len as usize]).into_owned();
            ip = match self.dns_cache.get(&name) {
                Some(ip) => *ip,
                None => {
                    let ip = host_by_name(&name);
                    self.dns_cache.insert(name, ip);
                    ip
                }
            };
        }
        for (k, b) in ip.iter().enumerate() {
            self.set_reg(i, SN_DIPR0 + k as u16, *b);
        }
    }

    /// `connectSocket` (:1039-1075).
    fn connect_socket(&mut self, i: usize) {
        if self.sockets[i].status != SR_INIT {
            return;
        }
        let dest = SocketAddr::from((Ipv4Addr::from(self.dest_ip(i)), self.reg_word(i, SN_DPORT0)));
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(TcpStream::connect_timeout(&dest, CONNECT_TIMEOUT));
        });
        self.sockets[i].host = HostSocket::Connecting(rx);
        self.sockets[i].set_status(SR_SYNSENT);
    }

    /// Extension: `LISTEN` on `Sn_PORT`.
    fn listen_socket(&mut self, i: usize) {
        if self.sockets[i].status != SR_INIT {
            return;
        }
        let port = self.reg_word(i, SN_PORT0);
        if let Ok(l) = TcpListener::bind(("0.0.0.0", port)) {
            if l.set_nonblocking(true).is_ok() {
                // Port 0 lets the host pick; the chosen port is written back to `Sn_PORT` so
                // the program (and the tests, which never use fixed ports) can find it.
                if let Ok(a) = l.local_addr() {
                    self.set_reg(i, SN_PORT0, (a.port() >> 8) as u8);
                    self.set_reg(i, SN_PORT0 + 1, a.port() as u8);
                }
                self.sockets[i].host = HostSocket::Listener(l);
                self.sockets[i].set_status(SR_LISTEN);
            }
        }
    }

    /// `setCommandRegister` (:1077-1102). The register itself reads back 0, as the chip clears it.
    fn set_command_register(&mut self, i: usize, value: u8) {
        match value {
            SN_CR_OPEN => self.open_socket(i),
            SN_CR_LISTEN => self.listen_socket(i),
            SN_CR_CONNECT => self.connect_socket(i),
            SN_CR_CLOSE | SN_CR_DISCON => self.sockets[i].clear_fd(),
            SN_CR_SEND => self.send_data(i),
            SN_CR_RECV => self.update_rsr(i),
            _ => {}
        }
    }

    // ---- register file ----

    /// `readSocketRegister` (:1104-1152).
    fn read_socket_register(&mut self, address: u16) -> u8 {
        let i = ((address >> 8) - 4) as usize;
        let loc = address & 0xFF;
        match loc {
            SN_SR => {
                self.sockets[i].process();
                self.sockets[i].status
            }
            SN_TX_FSR0 | SN_TX_FSR1 => {
                let free = self.sockets[i].transmit_size - self.tx_data_size(i);
                if loc == SN_TX_FSR0 {
                    (free >> 8) as u8
                } else {
                    free as u8
                }
            }
            SN_RX_RSR0 | SN_RX_RSR1 => {
                self.receive_one_packet(i);
                let rsr = self.sockets[i].rx_rsr;
                if loc == SN_RX_RSR0 {
                    (rsr >> 8) as u8
                } else {
                    rsr as u8
                }
            }
            _ => self.memory[address as usize],
        }
    }

    /// `readValueAt` (:1154-1183).
    fn read_value_at(&mut self, address: u16) -> u8 {
        if address == MR {
            self.mode_register
        } else if (GAR0..=UPORT1).contains(&address) {
            self.memory[address as usize]
        } else if (S0_BASE..=S3_MAX).contains(&address) {
            self.read_socket_register(address)
        } else {
            // Includes TX/RX memory; unknown locations mirror below 0x8000 (Uthernet II p.13).
            self.memory[(address & MEM_MAX) as usize]
        }
    }

    fn auto_increment(&mut self) {
        if self.mode_register & MR_AI != 0 {
            self.data_address = self.data_address.wrapping_add(1);
            // The TX and RX areas wrap within themselves (Uthernet II p.12).
            if self.data_address == RX_BASE || self.data_address as usize == MEM_SIZE {
                self.data_address -= 0x2000;
            }
        }
    }

    /// `writeSocketRegister` (:1233-1291).
    fn write_socket_register(&mut self, address: u16, value: u8) {
        let i = ((address >> 8) - 4) as usize;
        let loc = address & 0xFF;
        match loc {
            SN_CR => self.set_command_register(i, value),
            SN_MR | SN_PORT0 | SN_PORT1 | SN_DPORT0 | SN_DPORT1 | SN_PROTO | SN_TOS | SN_TTL
            | SN_TX_WR0 | SN_TX_WR1 | SN_RX_RD0 | SN_RX_RD1 => {
                self.memory[address as usize] = value
            }
            SN_DIPR0..=SN_DIPR3 => self.memory[address as usize] = value,
            _ => {
                if self.virtual_dns && loc >= SN_DNS_NAME_LEN {
                    self.memory[address as usize] = value;
                }
            }
        }
    }

    /// `setModeRegister` (:1293-1303).
    fn set_mode_register(&mut self, value: u8) {
        if value & MR_RST != 0 {
            self.reset_chip(false);
        } else {
            self.mode_register = value;
        }
    }

    /// `writeCommonRegister` (:1305-1332).
    fn write_common_register(&mut self, address: u16, value: u8) {
        if address == MR {
            self.set_mode_register(value);
        } else if (GAR0..=GAR3).contains(&address)
            || (SUBR0..=SUBR3).contains(&address)
            || (SHAR0..=SHAR5).contains(&address)
            || (SIPR0..=SIPR3).contains(&address)
        {
            self.memory[address as usize] = value;
        } else if address == RMSR {
            self.set_rx_sizes(value);
        } else if address == TMSR {
            self.set_tx_sizes(value);
        }
    }

    /// `writeValueAt` (:1334-1354).
    fn write_value_at(&mut self, address: u16, value: u8) {
        if address <= UPORT1 {
            self.write_common_register(address, value);
        } else if (S0_BASE..=S3_MAX).contains(&address) {
            self.write_socket_register(address, value);
        } else if (TX_BASE..=MEM_MAX).contains(&address) {
            self.memory[address as usize] = value;
        }
    }

    /// Data port read (`readValue`).
    pub fn read_data(&mut self) -> u8 {
        let v = self.read_value_at(self.data_address);
        self.auto_increment();
        v
    }

    pub fn write_data(&mut self, value: u8) {
        self.write_value_at(self.data_address, value);
        self.auto_increment();
    }

    pub fn data_address(&self) -> u16 {
        self.data_address
    }

    /// Status of socket `i` (0-3), for frontends and tests.
    pub fn socket_status(&self, i: usize) -> u8 {
        self.sockets[i].status
    }

    fn process_sockets(&mut self) {
        for s in &mut self.sockets {
            s.process();
        }
    }
}

/// Blocking write on a socket that is otherwise non-blocking (TCP payloads are small).
fn write_blocking(t: &mut TcpStream, data: &[u8]) -> io::Result<()> {
    t.set_nonblocking(false)?;
    t.set_write_timeout(Some(Duration::from_millis(500)))?;
    let r = t.write_all(data);
    t.set_nonblocking(true)?;
    r
}

/// `getHostByName` (DNS.cpp:31-43): first IPv4 address, or 0.0.0.0.
fn host_by_name(name: &str) -> Ipv4 {
    (name, 0u16)
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| {
            it.find_map(|a| match a {
                SocketAddr::V4(v4) => Some(v4.ip().octets()),
                SocketAddr::V6(_) => None,
            })
        })
        .unwrap_or([0; 4])
}

impl Card for Uthernet2 {
    fn name(&self) -> &'static str {
        "Uthernet II"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    /// `IO_C0` (:1411-1465): only A0 and A1 are decoded.
    fn io_read(&mut self, lo: u8, _ctx: &mut CardCtx) -> u8 {
        match lo & 3 {
            0 => self.mode_register,
            1 => (self.data_address >> 8) as u8,
            2 => self.data_address as u8,
            _ => self.read_data(),
        }
    }

    fn io_write(&mut self, lo: u8, value: u8, _ctx: &mut CardCtx) {
        match lo & 3 {
            0 => self.set_mode_register(value),
            1 => self.data_address = (self.data_address & 0x00FF) | (value as u16) << 8,
            2 => self.data_address = (self.data_address & 0xFF00) | value as u16,
            _ => self.write_data(value),
        }
    }

    fn slot_rom_read(&mut self, _offset: u8, ctx: &mut CardCtx) -> u8 {
        ctx.floating_bus
    }

    fn reset(&mut self, power_cycle: bool, ctx: &mut CardCtx) {
        self.reset_chip(power_cycle);
        ctx.schedule(TOKEN_POLL, POLL_CYCLES);
    }

    fn update(&mut self, cycles: u64, _ctx: &mut CardCtx) {
        self.backend.update(cycles);
        self.process_sockets();
    }

    fn on_event(&mut self, _token: u16, _ctx: &mut CardCtx) -> i32 {
        self.process_sockets();
        POLL_CYCLES
    }

    /// Registers, RAM and RAW sockets survive; TCP/UDP connections cannot (:337-363).
    fn save(&self) -> Vec<u8> {
        let mut v = vec![
            self.mode_register,
            self.data_address as u8,
            (self.data_address >> 8) as u8,
        ];
        v.extend_from_slice(&self.memory);
        for s in &self.sockets {
            v.extend_from_slice(&s.rx_wr.to_le_bytes());
            v.extend_from_slice(&s.rx_rsr.to_le_bytes());
            v.push(if matches!(s.status, SR_MACRAW | SR_IPRAW) {
                s.status
            } else {
                SR_CLOSED
            });
        }
        v
    }

    fn load(&mut self, state: &[u8]) -> bool {
        if state.len() != 3 + MEM_SIZE + 4 * 5 {
            return false;
        }
        self.reset_chip(true);
        self.mode_register = state[0];
        self.data_address = u16::from_le_bytes([state[1], state[2]]);
        self.memory.copy_from_slice(&state[3..3 + MEM_SIZE]);
        let (r, t) = (self.memory[RMSR as usize], self.memory[TMSR as usize]);
        self.set_rx_sizes(r);
        self.set_tx_sizes(t);
        for (i, s) in self.sockets.iter_mut().enumerate() {
            let o = 3 + MEM_SIZE + i * 5;
            s.rx_wr = u16::from_le_bytes([state[o], state[o + 1]]);
            s.rx_rsr = u16::from_le_bytes([state[o + 2], state[o + 3]]);
            s.set_status(state[o + 4]);
        }
        true
    }
}
