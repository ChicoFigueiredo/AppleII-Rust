//! The `AppleII-Rust Extension` block.
//!
//! AppleWin's save-state has no place for everything our machine holds (the scheduler queue, the
//! cards' internal registers where the card has no AppleWin unit, the Rust-side counters...). The
//! writer therefore adds one more sub-map at the end of `Unit: Apple2`'s `State`. AppleWin's
//! loader reads only the keys it knows and logs the rest (`YamlHelper::GetMapRemainder`), so the
//! file stays loadable there; our loader uses the block to resume *exactly* where the machine was.
//!
//! The block holds the whole [`MachineState`] as JSON, deflated and hex-encoded in the same
//! `address: bytes` line format as AppleWin's memory dumps (8-digit addresses, since it can pass
//! 64K), plus the name of the card in every slot so cards AppleWin has no unit for can be recreated.

use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::MachineState;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

pub const KEY: &str = "AppleII-Rust Extension";
pub const VERSION: u32 = 1;

/// What the block carries.
pub struct Extension {
    pub state: MachineState,
    /// `(slot, Card::name)` of every inserted card.
    pub cards: Vec<(u8, String)>,
}

pub fn write(w: &mut Writer, state: &MachineState, cards: &[(u8, String)]) -> Result<(), Error> {
    let json = serde_json::to_vec(state).map_err(|e| Error::Unsupported(format!("extension: {e}")))?;
    let mut z = ZlibEncoder::new(Vec::new(), Compression::new(6));
    z.write_all(&json).and_then(|_| z.try_finish()).map_err(|e| Error::Unsupported(format!("extension: {e}")))?;
    let mut data = z.finish().map_err(|e| Error::Unsupported(format!("extension: {e}")))?;
    let len = data.len();
    data.resize(len.div_ceil(8) * 8, 0);
    w.map(KEY, |w| {
        w.uint("Version", VERSION as u64);
        w.string("Format", "json+zlib");
        w.uint("Length", len as u64);
        w.map("Cards", |w| {
            for (slot, name) in cards {
                w.string(&slot.to_string(), name);
            }
        });
        w.map("Data", |w| w.memory_wide(&data));
    });
    Ok(())
}

/// Reads the block of an `Apple2` unit state, if there is one.
pub fn read(apple2_state: Map) -> Result<Option<Extension>, Error> {
    let Some(m) = apple2_state.sub(KEY) else { return Ok(None) };
    if m.u32("Version")? != VERSION || m.string("Format")? != "json+zlib" {
        return Ok(None); // written by another version: fall back to the AppleWin units
    }
    let len = m.u64("Length")? as usize;
    let mut buf = vec![0u8; len.div_ceil(8) * 8];
    m.need_sub("Data")?.load_memory(&mut buf)?;
    buf.truncate(len);
    let mut json = Vec::new();
    ZlibDecoder::new(&buf[..]).read_to_end(&mut json).map_err(|e| Error::Format(format!("{KEY}: {e}")))?;
    let state: MachineState = serde_json::from_slice(&json).map_err(|e| Error::Format(format!("{KEY}: {e}")))?;
    let mut cards = Vec::new();
    if let Some(c) = m.sub("Cards") {
        for (k, _) in c.entries {
            if let (Ok(slot), Ok(name)) = (k.parse::<u8>(), c.string(k)) {
                cards.push((slot, name));
            }
        }
    }
    Ok(Some(Extension { state, cards }))
}
