//! Mouse card (MouseInterface.cpp:605-760), including the MC6821 PIA sub-map.
//!
//! Keys AppleWin saves that our card has no counterpart for: `VBL` (`m_bVBL`, only ever false:
//! it is cleared in `Reset` and nothing sets it) and `Enabled` (`m_bEnabled`, a frontend switch
//! that is true after `Initialize`). They are written as `false` / `true` and ignored on load.

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_cards::pia6821::Pia6821;
use apple2_cards::{MouseCard, MouseSnapshot};
use apple2_core::Card;

/// Unit version (MouseInterface.cpp:668, `Slot(..., 1)`).
const VERSION: u32 = 1;

pub const MAP: Mapping = Mapping { aw_names: &["Mouse Card"], our_name: "Mouse Card", save, load };

fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<MouseCard>()) else { return false };
    let s = c.snapshot();
    w.slot(slot, "Mouse Card", VERSION, |w| {
        // SaveSnapshotMC6821: decimal `SaveUint`.
        w.map("MC6821", |w| {
            let p = &s.pia;
            for (k, v) in [("PRA", p.pra), ("DDRA", p.ddra), ("CRA", p.cra), ("PRB", p.prb), ("DDRB", p.ddrb), ("CRB", p.crb), ("IA", p.ia), ("IB", p.ib)] {
                w.uint(k, v as u64);
            }
        });
        w.line(&format!("DataLen: {}", s.data_len));
        w.hex8("Mode", s.mode as u32);
        w.hex8("6821B", s.by_b as u32);
        w.hex8("6821A", s.by_a as u32);
        w.map("Buffer", |w| w.memory(&s.buff));
        w.line(&format!("Buffer Position: {}", s.buf_pos));
        w.hex8("State", s.state as u32);
        // `m_nX` etc. are ints written with `SaveUint`: negative values print as unsigned.
        for (k, v) in [("X", s.n_x), ("Y", s.n_y)] {
            w.uint(k, v as u32 as u64);
        }
        w.bool("Btn0", s.btn0);
        w.bool("Btn1", s.btn1);
        w.bool("VBL", false);
        for (k, v) in [("iX", s.i_x), ("iMinX", s.min_x), ("iMaxX", s.max_x), ("iY", s.i_y), ("iMinY", s.min_y), ("iMaxY", s.max_y)] {
            w.uint(k, v as u32 as u64);
        }
        w.bool("Button0", s.buttons[0]);
        w.bool("Button1", s.buttons[1]);
        w.bool("Enabled", true);
    });
    true
}

fn load(ctx: &mut LoadCtx, slot: u8, version: u32, st: Map) -> Result<Option<Box<dyn Card>>, Error> {
    if version != VERSION {
        return Err(Error::Format(format!("Version {version} is not supported for card 'Mouse Card'.")));
    }
    let pia = st.sub("MC6821").ok_or_else(|| Error::Format("Card: Expected key: MC6821".into()))?;
    let b = |m: &Map, k: &str| -> Result<u8, Error> { Ok(m.u32(k)? as u8) };
    let mut s = MouseSnapshot {
        pia: Pia6821 {
            pra: b(&pia, "PRA")?,
            ddra: b(&pia, "DDRA")?,
            cra: b(&pia, "CRA")?,
            prb: b(&pia, "PRB")?,
            ddrb: b(&pia, "DDRB")?,
            crb: b(&pia, "CRB")?,
            ia: b(&pia, "IA")?,
            ib: b(&pia, "IB")?,
        },
        data_len: st.u32("DataLen")? as usize,
        mode: b(&st, "Mode")?,
        by_b: b(&st, "6821B")?,
        by_a: b(&st, "6821A")?,
        buff: [0; 8],
        buf_pos: 0,
        state: 0,
        n_x: 0,
        n_y: 0,
        i_x: 0,
        min_x: 0,
        max_x: 0,
        i_y: 0,
        min_y: 0,
        max_y: 0,
        btn0: false,
        btn1: false,
        buttons: [false; 2],
    };
    let buf = st.sub("Buffer").ok_or_else(|| Error::Format("Card: Expected key: Buffer".into()))?;
    buf.load_memory(&mut s.buff)?;
    s.buf_pos = st.u32("Buffer Position")? as usize;
    s.state = b(&st, "State")?;
    s.n_x = st.i32("X")?;
    s.n_y = st.i32("Y")?;
    s.btn0 = st.bool("Btn0")?;
    s.btn1 = st.bool("Btn1")?;
    st.bool("VBL")?;
    s.i_x = st.i32("iX")?;
    s.min_x = st.i32("iMinX")?;
    s.max_x = st.i32("iMaxX")?;
    s.i_y = st.i32("iY")?;
    s.min_y = st.i32("iMinY")?;
    s.max_y = st.i32("iMaxY")?;
    s.buttons = [st.bool("Button0")?, st.bool("Button1")?];
    st.bool("Enabled")?;

    let Some(mut card) = make(ctx, "Mouse Card", slot) else { return Ok(None) };
    let Some(c) = card.as_any_mut().and_then(|a| a.downcast_mut::<MouseCard>()) else {
        return Err(Error::Unsupported("host returned a card that is not a mouse card".into()));
    };
    c.apply_snapshot(&s);
    Ok(Some(card))
}
