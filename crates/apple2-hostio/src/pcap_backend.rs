//! Raw Ethernet through libpcap/Npcap (AppleWin `PCapBackend`, Tfe/PCapBackend.cpp and
//! Tfe/tfearch.cpp). Enabled by the `pcap` feature; needs libpcap (Linux/macOS) or Npcap (Windows)
//! installed on the host, and privileges to capture.

use crate::network::{Ipv4, Mac, NetworkBackend, MAX_RXLENGTH};
use pcap::{Active, Capture, Device, Linktype};

/// `tfe_arch_enumadapter*`: (name, description) of every capture-capable interface. Empty when
/// libpcap cannot list them; [`try_list_interfaces`] says why.
pub fn list_interfaces() -> Vec<(String, String)> {
    try_list_interfaces().unwrap_or_default()
}

/// Like [`list_interfaces`], with the reason when enumeration fails (libpcap without privileges,
/// Npcap not installed...).
pub fn try_list_interfaces() -> Result<Vec<(String, String)>, String> {
    let mut v: Vec<(String, String)> = Device::list()
        .map_err(|e| format!("cannot list network interfaces with libpcap/Npcap: {e}"))?
        .into_iter()
        .map(|d| (d.name, d.desc.unwrap_or_default()))
        .collect();
    v.sort();
    Ok(v)
}

pub struct PcapNetwork {
    name: String,
    capture: Option<Capture<Active>>,
    error: Option<String>,
}

impl PcapNetwork {
    /// `TfePcapOpenAdapter` (tfearch.cpp:309-370): promiscuous, 1700-byte snaplen, 20 ms timeout,
    /// non-blocking, Ethernet only. An unknown or non-Ethernet interface yields an invalid
    /// backend (`is_valid` false), never a fallback to the first adapter. The reason (no such
    /// interface, permission denied, not Ethernet...) is kept in [`last_error`](NetworkBackend::last_error)
    /// and queued in [`crate::diagnostics`]: AppleWin only logs it.
    pub fn open(interface_name: &str) -> Self {
        let src = format!("pcap:{interface_name}");
        match Self::try_open(interface_name) {
            Ok(n) => {
                crate::diagnostics::resolved(&src);
                n
            }
            Err(msg) => {
                crate::diagnostics::report(&src, msg.clone());
                PcapNetwork {
                    name: interface_name.to_string(),
                    capture: None,
                    error: Some(msg),
                }
            }
        }
    }

    /// The fallible form of [`open`](Self::open).
    pub fn try_open(interface_name: &str) -> Result<Self, String> {
        let name = interface_name;
        let device = Device::list()
            .map_err(|e| {
                format!("Uthernet: cannot list network interfaces with libpcap/Npcap: {e}")
            })?
            .into_iter()
            .find(|d| d.name == name)
            .ok_or_else(|| format!("Uthernet: network interface '{name}' does not exist"))?;
        let inactive = Capture::from_device(device)
            .map_err(|e| format!("Uthernet: cannot use network interface '{name}': {e}"))?;
        let active = inactive
            .promisc(true)
            .snaplen(1700)
            .timeout(20)
            .immediate_mode(true)
            .open()
            .map_err(|e| {
                let text = e.to_string().to_lowercase();
                let denied = ["permission", "not permitted", "denied", "privilege"]
                    .iter()
                    .any(|w| text.contains(w));
                let hint = if denied {
                    " (capturing needs root or CAP_NET_RAW; on Windows, Npcap with administrator rights)"
                } else {
                    ""
                };
                format!("Uthernet: cannot open network interface '{name}': {e}{hint}")
            })?;
        let capture = active
            .setnonblock()
            .map_err(|e| format!("Uthernet: cannot set '{name}' non-blocking: {e}"))?;
        if capture.get_datalink() != Linktype::ETHERNET {
            return Err(format!(
                "Uthernet: network interface '{name}' is not Ethernet (link type {:?})",
                capture.get_datalink()
            ));
        }
        Ok(PcapNetwork {
            name: name.to_string(),
            capture: Some(capture),
            error: None,
        })
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

    fn last_error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_interface_gives_an_invalid_backend_with_a_reason() {
        let n = PcapNetwork::open("apple2_hostio_no_such_if0");
        assert!(!n.is_valid());
        let e = n.last_error().expect("reason kept");
        assert!(e.contains("apple2_hostio_no_such_if0"), "{e}");
        assert!(crate::diagnostics::peek().iter().any(|m| m == e));
        let mut n = n;
        n.transmit(&[0; 60]); // disconnected: no-op, no panic
        assert!(n.receive().is_none());
    }

    #[test]
    fn opening_a_real_interface_without_privileges_explains_the_denial() {
        // Skips where capturing is allowed (root, CAP_NET_RAW, Npcap) or no interface exists.
        let Some((name, _)) = list_interfaces().into_iter().find(|(n, _)| n != "any") else {
            return;
        };
        match PcapNetwork::try_open(&name) {
            Ok(_) => {}
            Err(e) => {
                assert!(e.contains(&name), "{e}");
                let denied = e.to_lowercase().contains("not permitted")
                    || e.to_lowercase().contains("permission");
                assert_eq!(denied, e.contains("CAP_NET_RAW"), "hint iff denied: {e}");
            }
        }
    }
}
