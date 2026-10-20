//! Uthernet I (Uthernet1.cpp:1058-1136).

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::Card;
use apple2_hostio::uthernet1::{Uthernet1, Uthernet1Snapshot};

/// `kUNIT_VERSION` (Uthernet1.cpp:1076).
const VERSION: u32 = 1;

pub const MAP: Mapping = Mapping { aw_names: &["Uthernet"], our_name: "Uthernet", save, load };

/// `SaveSnapshot` (:1084). `Started Tx` is a driver latch we do not keep (false) and `Cannot
/// Use` reports whether Npcap is loaded on this PC (false).
fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<Uthernet1>()) else { return false };
    let s = c.snapshot();
    w.slot(slot, "Uthernet", VERSION, |w| {
        w.bool("Enabled", s.enabled);
        w.string("Network Interface", &s.interface);
        w.bool("Started Tx", false);
        w.bool("Cannot Use", false);
        w.hex16("Tx Collect Buffer", s.txcollect_buffer as u32);
        w.hex16("Rx Buffer", s.rx_buffer as u32);
        w.map("CS8900A Registers", |w| w.memory(&s.io));
        w.map("PacketPage Registers", |w| w.memory(&s.packet_page));
    });
    true
}

/// `LoadSnapshot` (:1104). The network back end comes from the host; `Enabled`, the interface
/// name, `Started Tx` and `Cannot Use` are read and dropped (AppleWin does the same for the last).
fn load(ctx: &mut LoadCtx, slot: u8, version: u32, st: Map) -> Result<Option<Box<dyn Card>>, Error> {
    if version != VERSION {
        return Err(Error::Format(format!("Version {version} is not supported for card 'Uthernet'.")));
    }
    let enabled = st.bool("Enabled")?;
    let interface = st.string("Network Interface")?;
    st.bool("Started Tx")?;
    st.bool("Cannot Use")?;
    let txcollect_buffer = st.u32("Tx Collect Buffer")? as u16;
    let rx_buffer = st.u32("Rx Buffer")? as u16;
    let mut io = [0u8; 16];
    st.sub("CS8900A Registers").ok_or_else(|| Error::Format("Card: Expected key: CS8900A Registers".into()))?.load_memory(&mut io)?;
    let mut packet_page = vec![0u8; 0x1000];
    st.sub("PacketPage Registers").ok_or_else(|| Error::Format("Card: Expected key: PacketPage Registers".into()))?.load_memory(&mut packet_page)?;
    let Some(mut card) = make(ctx, "Uthernet", slot) else { return Ok(None) };
    let Some(c) = card.as_any_mut().and_then(|a| a.downcast_mut::<Uthernet1>()) else {
        return Err(Error::Unsupported("host returned a card that is not an Uthernet".into()));
    };
    c.apply_snapshot(&Uthernet1Snapshot { enabled, interface, io, packet_page, txcollect_buffer, rx_buffer });
    Ok(Some(card))
}
