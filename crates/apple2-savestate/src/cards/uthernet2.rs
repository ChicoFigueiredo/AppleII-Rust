//! Uthernet II (Uthernet2.cpp:330-365 `Socket::SaveSnapshot`, :1538-1640 `Uthernet2`).

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::Card;
use apple2_hostio::uthernet2::{Uthernet2, Uthernet2Snapshot};

/// `kUNIT_VERSION` (Uthernet2.cpp:1531): 2 added Virtual DNS.
const VERSION: u32 = 2;

// W5100.h:30-36.
const COMMON: std::ops::Range<usize> = 0..0x2F + 1;
const SOCKETS: std::ops::Range<usize> = 0x0400..0x07FF + 1;
const TX: std::ops::Range<usize> = 0x4000..0x6000;
const RX: std::ops::Range<usize> = 0x6000..0x8000;

pub const MAP: Mapping = Mapping {
    aw_names: &["Uthernet II"],
    our_name: "Uthernet II",
    save,
    load,
};

/// `SaveSnapshot` (:1538) with `Socket::SaveSnapshot` (:330) per socket.
fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<Uthernet2>()) else {
        return false;
    };
    let s = c.snapshot();
    w.slot(slot, "Uthernet II", VERSION, |w| {
        w.string("Network Interface", &s.interface);
        w.bool("Virtual DNS", s.virtual_dns);
        w.hex8("Mode Register", s.mode_register as u32);
        w.hex16("Data Address", s.data_address as u32);
        w.map("Common Registers", |w| w.memory(&s.memory[COMMON]));
        w.map("Socket Registers", |w| w.memory(&s.memory[SOCKETS]));
        w.map("TX Memory", |w| w.memory(&s.memory[TX]));
        w.map("RX Memory", |w| w.memory(&s.memory[RX]));
        for (i, &(wr, rsr, status)) in s.sockets.iter().enumerate() {
            w.map(&format!("Socket {i}"), |w| {
                w.hex16("RX Write Register", wr as u32);
                w.hex16("RX Size Register", rsr as u32);
                w.hex8("Socket Register", status as u32);
            });
        }
    });
    true
}

/// `LoadSnapshot` (:1566), unit versions 1 and 2. Missing memory sub-maps are skipped as in
/// AppleWin. The network back end comes from the host.
fn load(
    ctx: &mut LoadCtx,
    slot: u8,
    version: u32,
    st: Map,
) -> Result<Option<Box<dyn Card>>, Error> {
    if !(1..=VERSION).contains(&version) {
        return Err(Error::Format(format!(
            "Version {version} is not supported for card 'Uthernet II'."
        )));
    }
    let interface = st.string("Network Interface")?;
    let virtual_dns = if version >= 2 {
        st.bool("Virtual DNS")?
    } else {
        false
    };
    let mode_register = st.u32("Mode Register")? as u8;
    let data_address = st.u32("Data Address")? as u16;
    let mut memory = vec![0u8; 0x8000];
    for (key, range) in [
        ("Common Registers", COMMON),
        ("Socket Registers", SOCKETS),
        ("TX Memory", TX),
        ("RX Memory", RX),
    ] {
        if let Some(m) = st.sub(key) {
            m.load_memory(&mut memory[range])?;
        }
    }
    let mut sockets = [(0u16, 0u16, 0u8); 4];
    for (i, s) in sockets.iter_mut().enumerate() {
        if let Some(m) = st.sub(&format!("Socket {i}")) {
            *s = (
                m.u32("RX Write Register")? as u16,
                m.u32("RX Size Register")? as u16,
                m.u32("Socket Register")? as u8,
            );
        }
    }
    let Some(mut card) = make(ctx, "Uthernet II", slot) else {
        return Ok(None);
    };
    let Some(c) = card
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Uthernet2>())
    else {
        return Err(Error::Unsupported(
            "host returned a card that is not an Uthernet II".into(),
        ));
    };
    c.apply_snapshot(&Uthernet2Snapshot {
        interface,
        virtual_dns,
        mode_register,
        data_address,
        memory,
        sockets,
    });
    Ok(Some(card))
}
