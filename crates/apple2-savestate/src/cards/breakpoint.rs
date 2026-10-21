//! Breakpoint card (Debugger/BreakpointCard.cpp:238-258): `State: null`, no state.

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::Card;

/// `kUNIT_VERSION` (BreakpointCard.cpp:238).
const VERSION: u32 = 1;

pub const MAP: Mapping = Mapping {
    aw_names: &["Breakpoint"],
    our_name: "Breakpoint",
    save: |w: &mut Writer, slot: u8, _c: &dyn Card| {
        w.map(&slot.to_string(), |w| {
            w.line("Card: Breakpoint");
            w.line(&format!("Version: {VERSION}"));
            w.line("State: null");
        });
        true
    },
    load,
};

fn load(
    ctx: &mut LoadCtx,
    slot: u8,
    version: u32,
    _st: Map,
) -> Result<Option<Box<dyn Card>>, Error> {
    if !(1..=VERSION).contains(&version) {
        return Err(Error::Format(format!(
            "Version {version} is not supported for card 'Breakpoint'."
        )));
    }
    Ok(make(ctx, "Breakpoint", slot))
}
