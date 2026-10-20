//! Z80 SoftCard (Z80VICE/z80.cpp:6459-6580, `Z80_SaveSnapshot` / `Z80_LoadSnapshot`).
//!
//! Not saved because AppleWin has no such state: our `halted` flag and the MEMPTR register
//! (`wz`), and the sub-cycle remainder of the 2x clock (`carry`); they keep their value from a
//! freshly built card on a plain AppleWin load (the extension carries them).

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_cards::SoftCard;
use apple2_core::Card;

/// Unit version (z80.cpp:6469, `Slot(..., 1)`).
const VERSION: u32 = 1;

pub const MAP: Mapping = Mapping { aw_names: &["Z80 SoftCard", "Z80"], our_name: "Z80 SoftCard", save, load };

fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<SoftCard>()) else { return false };
    let Some(z) = c.z80() else { return false };
    w.slot(slot, "Z80 SoftCard", VERSION, |w| {
        // `Active`: whether the Z80 owns the CPU (z80.cpp:6490).
        w.int("Active", c.is_active() as i64);
        for (k, v) in [("A", z.a), ("B", z.b), ("C", z.c), ("D", z.d), ("E", z.e), ("F", z.f), ("H", z.h), ("L", z.l)] {
            w.hex8(k, v as u32);
        }
        w.hex16("IX", z.ix as u32);
        w.hex16("IY", z.iy as u32);
        w.hex16("SP", z.sp as u32);
        w.hex16("PC", z.pc as u32);
        w.hex8("I", z.i as u32);
        w.hex8("R", z.r as u32);
        w.hex8("IFF1", z.iff1 as u32);
        w.hex8("IFF2", z.iff2 as u32);
        w.hex8("IM Mode", z.im as u32);
        for (k, v) in [("A'", z.a2), ("B'", z.b2), ("C'", z.c2), ("D'", z.d2), ("E'", z.e2), ("F'", z.f2), ("H'", z.h2), ("L'", z.l2)] {
            w.hex8(k, v as u32);
        }
    });
    true
}

fn load(ctx: &mut LoadCtx, slot: u8, version: u32, st: Map) -> Result<Option<Box<dyn Card>>, Error> {
    if slot == 0 {
        return Err(Error::Format("Card 'Z80 SoftCard' cannot be in slot 0".into()));
    }
    if version != VERSION {
        return Err(Error::Format(format!("Version {version} is not supported for card 'Z80 SoftCard'.")));
    }
    let Some(mut card) = make(ctx, "Z80 SoftCard", slot) else { return Ok(None) };
    let Some(c) = card.as_any_mut().and_then(|a| a.downcast_mut::<SoftCard>()) else {
        return Err(Error::Unsupported("host returned a card that is not a Z80 SoftCard".into()));
    };
    let g = |k: &str| st.u32(k);
    {
        let z = c.z80_mut().ok_or_else(|| Error::Unsupported("Z80 SoftCard: CPU is not available".into()))?;
        z.a = g("A")? as u8;
        z.b = g("B")? as u8;
        z.c = g("C")? as u8;
        z.d = g("D")? as u8;
        z.e = g("E")? as u8;
        z.f = g("F")? as u8;
        z.h = g("H")? as u8;
        z.l = g("L")? as u8;
        z.ix = g("IX")? as u16;
        z.iy = g("IY")? as u16;
        z.sp = g("SP")? as u16;
        z.pc = g("PC")? as u16;
        z.i = g("I")? as u8;
        z.r = g("R")? as u8;
        z.iff1 = g("IFF1")? != 0;
        z.iff2 = g("IFF2")? != 0;
        z.im = g("IM Mode")? as u8;
        z.a2 = g("A'")? as u8;
        z.b2 = g("B'")? as u8;
        z.c2 = g("C'")? as u8;
        z.d2 = g("D'")? as u8;
        z.e2 = g("E'")? as u8;
        z.f2 = g("F'")? as u8;
        z.h2 = g("H'")? as u8;
        z.l2 = g("L'")? as u8;
    }
    // The board's CPU owner is restored by the loader (see `load_aws_yaml`).
    c.set_active(g("Active")? != 0);
    Ok(Some(card))
}
