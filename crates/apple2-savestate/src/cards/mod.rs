//! Slot cards of `Unit: Slots` (CardManager.cpp:325 `SaveSnapshot`, Card.cpp:186 `GetCardType`).
//!
//! Each card module exports a [`Mapping`]: how to write the card's `<slot>:` entry from our card
//! and how to build (and fill) our card from an entry. Cards without a mapping are left out of
//! the AppleWin view (their entry would lack keys AppleWin requires and make it refuse the whole
//! file); the extension block still brings them back in our loader.

use crate::host::LoadHost;
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::{Card, Model};

mod disk2;
mod hdd;
mod printer;
mod simple;
mod ssc;
mod uthernet1;

/// Loader context handed to [`Mapping::load`].
pub struct LoadCtx<'a> {
    pub host: &'a mut dyn LoadHost,
    pub model: Model,
    pub warnings: &'a mut Vec<String>,
}

/// [`Mapping::load`]: slot, entry `Version`, `State`.
pub type LoadFn = fn(&mut LoadCtx, u8, u32, Map) -> Result<Option<Box<dyn Card>>, Error>;

pub struct Mapping {
    /// AppleWin card names; the first is the one written.
    pub aw_names: &'static [&'static str],
    /// [`Card::name`] of our card.
    pub our_name: &'static str,
    /// Writes the whole `<slot>:` entry; false if `card` is not this mapping's card.
    pub save: fn(&mut Writer, u8, &dyn Card) -> bool,
    /// Builds the card for `slot` from the entry's `Version` and `State`. `Ok(None)`: skipped
    /// (a warning was pushed).
    pub load: LoadFn,
}

pub const MAPPINGS: &[Mapping] = &[disk2::MAP, hdd::MAP, simple::FOURPLAY, simple::SAM, ssc::MAP, printer::MAP, uthernet1::MAP];

pub fn by_our_name(name: &str) -> Option<&'static Mapping> {
    MAPPINGS.iter().find(|m| m.our_name == name)
}

pub fn by_aw_name(name: &str) -> Option<&'static Mapping> {
    MAPPINGS.iter().find(|m| m.aw_names.contains(&name))
}

/// Writes the entry for the card in `slot`; false if there is no mapping for it.
pub fn save_card(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    by_our_name(card.name()).is_some_and(|m| (m.save)(w, slot, card))
}

/// Creates a card through the host (or `None` with a warning pushed).
pub fn make(ctx: &mut LoadCtx, name: &str, slot: u8) -> Option<Box<dyn Card>> {
    let c = ctx.host.make_card(name, slot, ctx.model);
    if c.is_none() {
        ctx.warnings.push(format!("Slot {slot}: the host cannot provide a '{name}' card; the slot stays empty"));
    }
    c
}
