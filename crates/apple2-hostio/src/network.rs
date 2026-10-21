//! Raw Ethernet frame transport shared by the Uthernet cards (AppleWin `NetworkBackend`,
//! Tfe/NetworkBackend.h) plus the Ethernet/IPv4 helpers of Tfe/IPRaw.cpp.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Tfe/NetworkBackend.h:3-7.
pub const MAX_TXLENGTH: usize = 1518;
pub const MIN_TXLENGTH: usize = 4;
pub const MAX_RXLENGTH: usize = 1518;
pub const MIN_RXLENGTH: usize = 64;

/// Dest MAC + source MAC + EtherType (Uthernet2.cpp:84).
pub const ETH_MINIMUM_SIZE: usize = 6 + 6 + 2;

pub type Mac = [u8; 6];
pub type Ipv4 = [u8; 4];

/// A source and sink of Ethernet frames. Polled, never blocking.
pub trait NetworkBackend: Send {
    /// Sends one frame.
    fn transmit(&mut self, frame: &[u8]);

    /// Takes one received frame, if any (`receive` returning `-1` in AppleWin).
    fn receive(&mut self) -> Option<Vec<u8>>;

    /// Housekeeping once per emulated slice (`update`).
    fn update(&mut self, _cycles: u64) {}

    /// MAC of a host on the local network, for IP RAW sockets (`getMACAddress`). `None` falls back
    /// to broadcast, which is what AppleWin does outside Windows.
    fn mac_for_ip(&mut self, _ip: Ipv4) -> Option<Mac> {
        None
    }

    /// False when the configured interface does not exist (`isValid`).
    fn is_valid(&self) -> bool {
        true
    }

    fn interface_name(&self) -> &str {
        ""
    }
}

/// Backend for a card with no interface selected: drops everything. The Uthernet II still works
/// through its host-socket modes.
#[derive(Default)]
pub struct NullNetwork;

impl NetworkBackend for NullNetwork {
    fn transmit(&mut self, _frame: &[u8]) {}

    fn receive(&mut self) -> Option<Vec<u8>> {
        None
    }

    fn is_valid(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct MockInner {
    rx: VecDeque<Vec<u8>>,
    tx: Vec<Vec<u8>>,
    macs: Vec<(Ipv4, Mac)>,
}

/// Test control of a [`MockNetwork`].
#[derive(Clone, Default)]
pub struct MockNetworkHandle(Arc<Mutex<MockInner>>);

impl MockNetworkHandle {
    /// Queues a frame for the card to receive.
    pub fn push_rx(&self, frame: &[u8]) {
        self.0.lock().unwrap().rx.push_back(frame.to_vec());
    }

    /// Frames the card transmitted so far (drained).
    pub fn take_tx(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.0.lock().unwrap().tx)
    }

    /// Makes `mac_for_ip(ip)` answer `mac`.
    pub fn add_arp(&self, ip: Ipv4, mac: Mac) {
        self.0.lock().unwrap().macs.push((ip, mac));
    }
}

/// In-memory backend for tests.
pub struct MockNetwork(Arc<Mutex<MockInner>>);

impl MockNetwork {
    pub fn new() -> (Self, MockNetworkHandle) {
        let inner = Arc::new(Mutex::new(MockInner::default()));
        (MockNetwork(inner.clone()), MockNetworkHandle(inner))
    }
}

impl NetworkBackend for MockNetwork {
    fn transmit(&mut self, frame: &[u8]) {
        self.0.lock().unwrap().tx.push(frame.to_vec());
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        self.0.lock().unwrap().rx.pop_front()
    }

    fn mac_for_ip(&mut self, ip: Ipv4) -> Option<Mac> {
        self.0
            .lock()
            .unwrap()
            .macs
            .iter()
            .find(|(i, _)| *i == ip)
            .map(|(_, m)| *m)
    }

    fn interface_name(&self) -> &str {
        "mock"
    }
}

/// `crc32_buf` (Tfe/tfesupp.cpp:44-65): the standard Ethernet CRC-32.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Internet checksum (RFC 1071), as IPRaw.cpp `checksum`. Returns the value to store big-endian.
fn ip_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = data
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]) as u32)
        .sum();
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// `createETH2Frame` (IPRaw.cpp:104-132): an Ethernet II frame carrying an IPv4 packet with no
/// options and no fragmentation.
#[allow(clippy::too_many_arguments)]
pub fn create_eth2_frame(
    payload: &[u8],
    source_mac: Mac,
    destination_mac: Mac,
    ttl: u8,
    tos: u8,
    protocol: u8,
    source: Ipv4,
    destination: Ipv4,
) -> Vec<u8> {
    let mut f = Vec::with_capacity(14 + 20 + payload.len());
    f.extend_from_slice(&destination_mac);
    f.extend_from_slice(&source_mac);
    f.extend_from_slice(&0x0800u16.to_be_bytes());
    let mut ip = [0u8; 20];
    ip[0] = 0x45; // IPv4, IHL 5
    ip[1] = tos;
    ip[2..4].copy_from_slice(&((20 + payload.len()) as u16).to_be_bytes());
    ip[8] = ttl;
    ip[9] = protocol;
    ip[12..16].copy_from_slice(&source);
    ip[16..20].copy_from_slice(&destination);
    let sum = ip_checksum(&ip);
    ip[10..12].copy_from_slice(&sum.to_be_bytes());
    f.extend_from_slice(&ip);
    f.extend_from_slice(payload);
    f
}

/// IPv4 payload of an Ethernet frame (`getIPPayload`, IPRaw.cpp:134-162).
#[derive(Debug, PartialEq, Eq)]
pub struct IpPayload<'a> {
    pub protocol: u8,
    pub source: Ipv4,
    pub payload: &'a [u8],
}

pub fn ip_payload(frame: &[u8]) -> Option<IpPayload<'_>> {
    if frame.len() <= ETH_MINIMUM_SIZE + 20 || frame[12..14] != [0x08, 0x00] || frame[14] >> 4 != 4
    {
        return None;
    }
    let header = (frame[14] & 0x0F) as usize * 4;
    let total = u16::from_be_bytes([frame[16], frame[17]]) as usize;
    if total <= header || frame.len() < ETH_MINIMUM_SIZE + total {
        return None;
    }
    Some(IpPayload {
        protocol: frame[23],
        source: [frame[26], frame[27], frame[28], frame[29]],
        payload: &frame[14 + header..14 + total],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn eth2_frame_round_trips_through_ip_payload() {
        let f = create_eth2_frame(
            b"ping",
            [1; 6],
            [2; 6],
            64,
            0,
            1,
            [10, 0, 0, 1],
            [10, 0, 0, 2],
        );
        assert_eq!(f.len(), 14 + 20 + 4);
        assert_eq!(
            ip_checksum(&f[14..34]),
            0,
            "a header with its checksum sums to zero"
        );
        let p = ip_payload(&f).unwrap();
        assert_eq!(
            (p.protocol, p.source, p.payload),
            (1, [10, 0, 0, 1], &b"ping"[..])
        );
    }

    #[test]
    fn ip_payload_rejects_non_ip_and_truncated_frames() {
        let mut f = create_eth2_frame(
            b"ping",
            [1; 6],
            [2; 6],
            64,
            0,
            1,
            [10, 0, 0, 1],
            [10, 0, 0, 2],
        );
        assert!(ip_payload(&f[..f.len() - 1]).is_none());
        f[12] = 0x86; // not IPv4 ethertype
        assert!(ip_payload(&f).is_none());
        assert!(ip_payload(&[0; 10]).is_none());
    }
}
