//! List of host network interfaces for the Uthernet drop-down (AUD-P-032).
//!
//! With the `pcap` feature the list is what libpcap/Npcap can capture on, with the adapter
//! description (AppleWin `tfe_arch_enumadapter*`). Without it, [`portable_interfaces`] asks the
//! operating system through the `if-addrs` crate (Linux, macOS and Windows alike), and builds the
//! description from the addresses. Without libpcap nothing can be attached to those interfaces,
//! so the factory still warns that the `pcap` feature is needed (AUD-P-023).

use std::collections::BTreeMap;
use std::net::IpAddr;

/// (name, description) of every interface the user may pick, sorted by name.
pub fn list_interfaces() -> Vec<(String, String)> {
    #[cfg(feature = "pcap")]
    {
        match crate::pcap_backend::try_list_interfaces() {
            Ok(v) => return v,
            Err(e) => {
                crate::diagnostics::report("pcap-list", format!("Uthernet: {e}"));
            }
        }
    }
    portable_interfaces()
}

/// Interfaces with at least one address, as the OS reports them (no libpcap needed).
pub fn portable_interfaces() -> Vec<(String, String)> {
    let entries = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .map(|i| {
            let up = matches!(i.oper_status, if_addrs::IfOperStatus::Up);
            (i.name.clone(), i.ip(), up)
        });
    group_interfaces(entries)
}

/// One row per interface name: the description lists its addresses, and says "down" when the
/// system reports every address of the interface as not operational.
pub fn group_interfaces(
    entries: impl IntoIterator<Item = (String, IpAddr, bool)>,
) -> Vec<(String, String)> {
    let mut by_name: BTreeMap<String, (Vec<String>, bool)> = BTreeMap::new();
    for (name, ip, up) in entries {
        let e = by_name.entry(name).or_default();
        let text = ip.to_string();
        if !e.0.contains(&text) {
            e.0.push(text);
        }
        e.1 |= up;
    }
    by_name
        .into_iter()
        .map(|(name, (addrs, up))| {
            let mut d = addrs.join(", ");
            if !up {
                d.push_str(" (down)");
            }
            (name, d)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_addresses_by_interface_and_marks_down_ones() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        let got = group_interfaces([
            ("eth0".to_string(), ip("192.168.0.2"), true),
            ("lo".to_string(), ip("127.0.0.1"), true),
            ("eth0".to_string(), ip("fe80::1"), true),
            ("eth0".to_string(), ip("192.168.0.2"), true),
            ("wlan0".to_string(), ip("10.0.0.5"), false),
        ]);
        assert_eq!(
            got,
            vec![
                ("eth0".to_string(), "192.168.0.2, fe80::1".to_string()),
                ("lo".to_string(), "127.0.0.1".to_string()),
                ("wlan0".to_string(), "10.0.0.5 (down)".to_string()),
            ]
        );
    }

    #[test]
    fn the_host_list_is_sorted_and_has_no_empty_names() {
        let v = list_interfaces();
        assert!(v.iter().all(|(n, _)| !n.is_empty()));
        assert!(v.windows(2).all(|w| w[0].0 <= w[1].0));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_always_has_the_loopback_interface() {
        // `lo` has an address in every network namespace that has networking at all; a
        // namespace without any interface returns an empty list, which is also valid.
        let v = portable_interfaces();
        assert!(v.is_empty() || v.iter().any(|(n, _)| n == "lo"), "{v:?}");
    }
}
