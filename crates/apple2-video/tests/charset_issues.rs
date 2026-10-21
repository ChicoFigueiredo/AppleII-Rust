//! Character-generator regressions reported upstream.

use apple2_core::Model;
use apple2_video::charset::char_sets;

/// GH#354: the Pravets 8M ']' had a superfluous lit dot; the 8A (same Latin layout, checked by
/// the reporter) was fine. The normal ']' ($DD) and its inverse ($5D) must match the 8A's and be
/// the plain bracket: full top and bottom row and a two-dot stem, nothing else.
#[test]
fn gh354_pravets_8m_right_bracket_has_no_extra_dot() {
    let m = char_sets(Model::Pravets8M, false, false)[0];
    let a = char_sets(Model::Pravets8A, false, false)[0];
    assert_eq!(m[0xDD], [0x7C, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7C, 0x00]);
    assert_eq!(m[0xDD], a[0xDD]);
    assert_eq!(m[0x5D], a[0x5D]);
}
