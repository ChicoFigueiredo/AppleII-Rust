//! Cards whose AppleWin state is empty: 4Play (FourPlay.cpp:152) and SAM (SAM.cpp:94).

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::Card;

fn save_null(w: &mut Writer, slot: u8, name: &str) {
    w.map(&slot.to_string(), |w| {
        w.line(&format!("Card: {name}"));
        w.line("Version: 1");
        w.line("State: null");
    });
}

fn load_stateless(ctx: &mut LoadCtx, slot: u8, version: u32, _st: Map, name: &str) -> Result<Option<Box<dyn Card>>, Error> {
    if !(1..=1).contains(&version) {
        return Err(Error::Format(format!("Version {version} is not supported for card '{name}'.")));
    }
    Ok(make(ctx, name, slot))
}

pub const FOURPLAY: Mapping = Mapping {
    aw_names: &["4Play"],
    our_name: "4Play",
    save: |w, slot, c| {
        save_null(w, slot, c.name());
        true
    },
    load: |ctx, slot, v, st| load_stateless(ctx, slot, v, st, "4Play"),
};

pub const SAM: Mapping = Mapping {
    aw_names: &["SAM"],
    our_name: "SAM",
    save: |w, slot, c| {
        save_null(w, slot, c.name());
        true
    },
    load: |ctx, slot, v, st| load_stateless(ctx, slot, v, st, "SAM"),
};
