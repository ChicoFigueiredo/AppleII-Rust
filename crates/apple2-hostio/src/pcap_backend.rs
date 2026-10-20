//! Raw Ethernet through libpcap/Npcap (AppleWin `PCapBackend`, Tfe/PCapBackend.cpp and
//! Tfe/tfearch.cpp). Enabled by the `pcap` feature; needs libpcap (Linux/macOS) or Npcap (Windows)
//! installed on the host, and privileges to capture.

use crate::network::{Ipv4, Mac, NetworkBackend, MAX_RXLENGTH};
use pcap::{Active, Capture, Device, Linktype};

/// `tfe_arch_enumadapter*`: (name, description) of every capture-capable interface.
pub fn list_interfaces() -> Vec<(String, String)> {
    Device::list()
        .unwrap_or_default()
        .into_iter()
        .map(|d| (d.name, d.desc.unwrap_or_default()))
        .collect()
}

pub struct PcapNetwork {
    name: String,
    capture: Option<Capture<Active>>,
}

impl PcapNetwork {
    /// `TfePcapOpenAdapter` (tfearch.cpp:309-370): promiscuous, 1700-byte snaplen, 20 ms timeout,
    /// non-blocking, Ethernet only. An unknown or non-Ethernet interface yields an invalid
    /// backend (`is_valid` false), never a fallback to the first adapter.
    pub fn open(interface_name: &str) -> Self {
        let capture = Device::list()
            .ok()
            .and_then(|l| l.into_iter().find(|d| d.name == interface_name))
            .and_then(|d| Capture::from_device(d).ok())
            .and_then(|c| c.promisc(true).snaplen(1700).timeout(20).immediate_mode(true).open().ok())
            .and_then(|c| c.setnonblock().ok())
            .filter(|c| c.get_datalink() == Linktype::ETHERNET);
        PcapNetwork { name: interface_name.to_string(), capture }
    }
}

impl NetworkBackend for PcapNetwork {
    fn transmit(&mut self, frame: &[u8]) {
        if let Some(c) = &mut self.capture {
            let _ = c.sendpacket(frame);
        }
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        let c = self.capture.as_mut()?;
        let p = c.next_packet().ok()?;
        let mut v = p.data[..p.data.len().min(MAX_RXLENGTH)].to_vec();
        if v.len() & 1 != 0 {
            v.push(0); // tfe_arch_receive pads to an even length
        }
        Some(v)
    }

    /// AppleWin only resolves MACs on Windows (`SendARP`); elsewhere the caller falls back to
    /// broadcast, as here.
    fn mac_for_ip(&mut self, _ip: Ipv4) -> Option<Mac> {
        None
    }

    fn is_valid(&self) -> bool {
        self.capture.is_some()
    }

    fn interface_name(&self) -> &str {
        &self.name
    }
}
