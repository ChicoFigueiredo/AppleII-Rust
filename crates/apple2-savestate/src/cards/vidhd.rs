//! VidHD (VidHD.cpp:177-230).

use super::{make, save_env, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_cards::{VidHd, VidHdSnapshot};
use apple2_core::Card;

/// `kUNIT_VERSION` (VidHD.cpp:177).
const VERSION: u32 = 1;
/// `TEXT_PAGE1_BEGIN` and `SHR_MEMORY_END + 1`: the aux dump covers `[$0400, $A000)`.
const AUX_BEGIN: usize = 0x400;
const AUX_END: usize = 0xA000;

pub const MAP: Mapping = Mapping {
    aw_names: &["VidHD"],
    our_name: "VidHD",
    save,
    load,
};

fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<VidHd>()) else {
        return false;
    };
    let s = c.snapshot();
    let env = save_env();
    w.slot(slot, "VidHD", VERSION, |w| {
        w.hex32("Memory Mode", s.mem_mode);
        w.hex8("Screen Color", s.screen_color as u32);
        w.hex8("New Video", s.new_video as u32);
        w.hex8("Border Color", s.border_color as u32);
        w.hex8("Shadow", s.shadow as u32);
        // II/II+ or //e without aux memory (VidHD.cpp:208). Our machine keeps the ][-family
        // "aux" RAM outside its state, so the dump is written as zeros.
        if env.model.is_apple2() || env.aux_empty {
            w.map("Auxiliary Memory Bank", |w| {
                let zeros = "00".repeat(64);
                for addr in (AUX_BEGIN..AUX_END).step_by(64) {
                    w.line(&format!("{addr:04X}: {zeros}"));
                }
            });
        }
    });
    true
}

fn load(
    ctx: &mut LoadCtx,
    slot: u8,
    version: u32,
    st: Map,
) -> Result<Option<Box<dyn Card>>, Error> {
    if !(1..=VERSION).contains(&version) {
        return Err(Error::Format(format!(
            "Version {version} is not supported for card 'VidHD'."
        )));
    }
    let Some(mut card) = make(ctx, "VidHD", slot) else {
        return Ok(None);
    };
    let Some(c) = card.as_any_mut().and_then(|a| a.downcast_mut::<VidHd>()) else {
        return Err(Error::Unsupported(
            "host returned a card that is not a VidHD".into(),
        ));
    };
    c.apply_snapshot(&VidHdSnapshot {
        mem_mode: st.u32("Memory Mode")?,
        screen_color: st.u32("Screen Color")? as u8,
        new_video: st.u32("New Video")? as u8,
        border_color: st.u32("Border Color")? as u8,
        shadow: st.u32("Shadow")? as u8,
    });
    // The aux dump AppleWin adds for a ][ or a //e without aux memory is validated and dropped.
    if let Some(aux) = st.sub("Auxiliary Memory Bank") {
        aux.load_memory(&mut vec![0u8; AUX_END])?;
    }
    Ok(Some(card))
}
