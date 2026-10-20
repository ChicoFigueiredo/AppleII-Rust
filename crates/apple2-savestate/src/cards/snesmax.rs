//! SNES MAX (SNESMAX.cpp:247-277).

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_cards::SnesMax;
use apple2_core::Card;

/// `kUNIT_VERSION` (SNESMAX.cpp:247).
const VERSION: u32 = 1;

pub const MAP: Mapping = Mapping { aw_names: &["SNES MAX"], our_name: "SNES MAX", save, load };

fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<SnesMax>()) else { return false };
    w.slot(slot, "SNES MAX", VERSION, |w| w.uint("Button Index", c.button_index() as u64));
    true
}

fn load(ctx: &mut LoadCtx, slot: u8, version: u32, st: Map) -> Result<Option<Box<dyn Card>>, Error> {
    if !(1..=VERSION).contains(&version) {
        return Err(Error::Format(format!("Version {version} is not supported for card 'SNES MAX'.")));
    }
    let Some(mut card) = make(ctx, "SNES MAX", slot) else { return Ok(None) };
    let Some(c) = card.as_any_mut().and_then(|a| a.downcast_mut::<SnesMax>()) else {
        return Err(Error::Unsupported("host returned a card that is not a SNES MAX".into()));
    };
    c.restore_button_index(st.u32("Button Index")?);
    Ok(Some(card))
}
