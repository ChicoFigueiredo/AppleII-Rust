//! A richer `text_screen`: the text page as strings, honouring 80 columns (aux/main interleave),
//! page 2 and the 80STORE rule. For tests and the debugger; attributes are dropped.

use crate::source::VideoSource;
use apple2_core::memory::mf;
use apple2_core::text_screen::{decode_char, row_address, COLS, ROWS};
use apple2_core::Model;

/// The 24 rows of the displayed text page, 40 or 80 characters each.
pub fn screen_text(s: &dyn VideoSource) -> Vec<String> {
    let model = s.model();
    let lowercase = !model.is_apple2() || matches!(model, Model::Pravets82 | Model::Pravets8M | Model::Pravets8A);
    let mode = s.mode();
    let mm = s.memmode();
    let alt = mode.altcharset && !model.is_apple2();
    let page2 = mm & mf::PAGE2 != 0 && mm & mf::STORE80 == 0;
    let col80 = mode.col80 && !model.is_apple2();
    (0..ROWS)
        .map(|r| {
            let base = row_address(r) + if page2 { 0x400 } else { 0 };
            (0..COLS as u16)
                .flat_map(|c| {
                    let m = decode_char(s.main(base + c), lowercase, alt);
                    if col80 {
                        vec![decode_char(s.aux(base + c), lowercase, alt), m]
                    } else {
                        vec![m]
                    }
                })
                .collect()
        })
        .collect()
}
