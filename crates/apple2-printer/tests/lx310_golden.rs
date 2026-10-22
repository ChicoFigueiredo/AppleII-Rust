//! Golden documents (exact dots, text runs and diagnostics, no tolerance) per command or behaviour.
//! Goldens use the placeholder cell, not font glyphs, so they stay valid when `font` lands; the
//! glyph-dot goldens come with the font. Regenerate with `UPDATE_GOLDEN=1` and review the diff.

mod lx310_common;
use apple2_printer::paper::PaperKind;
use lx310_common::*;

struct Case {
    name: &'static str,
    paper: PaperKind,
    data: Vec<u8>,
}

fn case(name: &'static str, data: &[u8]) -> Case {
    Case {
        name,
        paper: PaperKind::GreenBar,
        data: data.to_vec(),
    }
}

fn cases() -> Vec<Case> {
    let mut v = vec![
        case("t002_reset_lf_cr", b"\x1b@A\r\nB"),
        case("t003_cr_lf_crlf", &[65, 13, 66, 10, 67, 13, 10, 68]),
        case("t004_form_feed", b"A\x0cB"),
        case("esc0_one_eighth", b"A\x1b0\nB\nC"),
        case("esc2_one_sixth", b"A\x1b0\n\x1b2\nB\nC"),
        case("esc3_36", b"A\x1b3\x24\nB\nC"),
        case("esc1_seven_72", b"A\x1b1\nB\nC"),
        case(
            "esc_o_cancel_bottom_margin",
            b"\x1bC\x06\x1bN\x02\x1bOA\nB\nC\nD\nE\nF",
        ),
        case(
            "esc_paren_unsupported_skipped",
            b"\x1b(U\x01\x00\x0aA\x1b(C\x02\x00\x01\x02B",
        ),
        case("esc_a_n72", b"A\x1bA\x09\nB\nC"),
        case("esc_j_advance", b"A\x1bJ\x36B\x1bJ\x01C"),
        case("pitch_10_12_15", b"AB\r\n\x1bMAB\r\n\x1bgAB\r\n\x1bPAB"),
        case("pitch_condensed", b"\x0fAB\r\n\x1bM\x0fAB\r\n\x12AB"),
        case(
            "pitch_expanded",
            b"\x0eAB\nAB\r\n\x1bW\x01AB\x1bW\x00AB\x0f\x0eAB",
        ),
        case("esc_space_extra", b"\x1b\x20\x02AB"),
        case(
            "t007_bold_double_strike",
            b"\x1bEA\x1bFB\x1bGC\x1bHD\x1bE\x1bGE",
        ),
        case("top_of_form_first_line", b"\x1bE\x1bW\x01AP\x1bW\x00\x1bF"),
        case("double_width_dots", b"\x1bW\x01X\x1bW\x00X\x0eX"),
        case("line_66_spills_to_next_form", b"\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\nX"),
        case("t007_underline", b"\x1b-\x01AB\x1b-\x00C"),
        case("t007_master_select", b"\x1b!\x88A\x1b!\x00B\x1b!\x01C"),
        case("italic_shear", b"\x1b4A\x1b5B"),
        case(
            "t008_esc_k_with_esc_inside",
            &[
                0x1B, b'K', 8, 0, 0x80, 0x01, 0xFF, 0x1B, 0x0A, 0x00, 0x81, 0x40, b'A',
            ],
        ),
        case("esc_l_120dpi", &[0x1B, b'L', 3, 0, 0x80, 0xFF, 0x01]),
        case(
            "esc_y_no_adjacent",
            &[0x1B, b'Y', 4, 0, 0xFF, 0xFF, 0x00, 0xFF],
        ),
        case("esc_z_240dpi", &[0x1B, b'Z', 4, 0, 0xFF, 0x00, 0xAA, 0x55]),
        case(
            "t009_esc_star_80dpi",
            &[0x1B, b'*', 4, 4, 0, 0x00, 0xFF, 0x1B, 0x0A],
        ),
        case(
            "esc_star_72_90dpi",
            &[
                0x1B, b'*', 5, 2, 0, 0x81, 0x18, 0x0D, 0x1B, b'*', 6, 2, 0, 0x81, 0x18,
            ],
        ),
        case(
            "esc_caret_nine_pin",
            &[0x1B, b'^', 1, 2, 0, 0x80, 0x01, 0x00, 0x01],
        ),
        case(
            "graphics_two_lines",
            &[
                0x1B, b'K', 3, 0, 0x80, 0x40, 0x01, 0x0D, 0x0A, 0x1B, b'K', 3, 0, 0x01, 0x40, 0x80,
            ],
        ),
        case(
            "graphics_right_margin_clip",
            &[
                0x1B, b'Q', 1, 0x1B, b'K', 10, 0, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
                0x80, 0x80,
            ],
        ),
        case("horizontal_tabs_default", b"A\tB\tC"),
        case("esc_d_tabs", b"\x1bD\x05\x0a\x00A\tB\tC"),
        case("t029_esc_d_no_nul", b"\x1bD\x05\x0a"),
        case("esc_b_vertical_tabs", b"\x1bB\x03\x06\x00A\x0bB\x0bC\x0bD"),
        case("esc_dollar_absolute", b"A\x1b$\x3c\x00B"),
        case("esc_backslash_relative", b"A\x1b\\\x78\x00B\x1b\\\xc4\xffC"),
        case("backspace", b"AB\x08C"),
        case("margins_left_right_wrap", b"\x1bl\x02\x1bQ\x07ABCDEFGH"),
        case("esc_c_page_length_lines", b"\x1bC\x03A\nB\nC\nD"),
        case(
            "esc_c_0_inches_bottom_margin",
            b"\x1bC\x00\x01\x1bN\x02A\nB\nC\nD\nE\nF",
        ),
        case(
            "t028_reset_after_two_lines",
            b"\x1bE\x1b-\x01\x1bl\x0aA\r\n\x1b@B\x0cC",
        ),
        case(
            "t011_truncated_graphics",
            &[b'A', 0x1B, b'K', 0x00, 0x10, 1, 2, 3],
        ),
        case("t011_lone_esc", b"A\x1b"),
        case("unknown_esc_resync", b"A\x1bvBC"),
        case("unsupported_known_commands", b"\x1bw\x01\x1bS\x00A\x18"),
        case(
            "state_only_tables_country_nlq",
            b"\x1bt\x00\x1bR\x02\x1bx\x01\x1bU\x01\x1bk\x01\x1b(t\x03\x00\x01\x19\x00A",
        ),
        case("upper_codes_esc6_esc7", b"\x80\x1b7\x80\x1b6\x80"),
        case("double_width_line_cancelled_by_lf", b"\x0eA\nB"),
    ];
    v.push(Case {
        name: "paper_a4_origin",
        paper: PaperKind::A4,
        data: b"A\nB".to_vec(),
    });
    v.push(Case {
        name: "paper_letter_origin",
        paper: PaperKind::Letter,
        data: b"A\x1bK\x02\x00\x80\x01".to_vec(),
    });
    v.push(Case {
        name: "a4_bottom_margin_ignored",
        paper: PaperKind::A4,
        data: b"\x1bC\x03\x1bN\x01A\nB\nC".to_vec(),
    });
    // Long sequences in the no-adjacent modes: a solid stroke, an alternating pattern and
    // pseudo-random bytes (xorshift, fixed seed). Each is 64 columns.
    for (tag, head) in [
        ("esc_y", &[0x1B, b'Y'][..]),
        ("esc_z", &[0x1B, b'Z'][..]),
        ("esc_star2", &[0x1B, b'*', 2][..]),
        ("esc_star3", &[0x1B, b'*', 3][..]),
    ] {
        let mut seed = 0x2545_F491u32;
        let mut random = Vec::new();
        for _ in 0..64 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            random.push(seed as u8);
        }
        let solid = vec![0xFF; 64];
        let alt: Vec<u8> = (0..64)
            .map(|i| if i % 2 == 0 { 0xAA } else { 0x55 })
            .collect();
        for (kind, bytes) in [("solid", solid), ("alternating", alt), ("random", random)] {
            let mut data = head.to_vec();
            data.extend_from_slice(&[64, 0]);
            data.extend_from_slice(&bytes);
            let name: &'static str = Box::leak(format!("long_{tag}_{kind}").into_boxed_str());
            v.push(Case {
                name,
                paper: PaperKind::GreenBar,
                data,
            });
        }
    }
    v
}

#[test]
fn goldens_match_exactly() {
    let all = cases();
    assert!(all.len() >= 20);
    for c in &all {
        let d = run_paper(c.paper, &c.data);
        check_golden(c.name, &render(&d));
    }
}

#[test]
fn goldens_are_identical_fed_one_byte_at_a_time() {
    for c in &cases() {
        let whole = run_paper(c.paper, &c.data);
        let bytes: Vec<&[u8]> = c.data.chunks(1).collect();
        let one = run_chunks(golden_config(), c.paper, &bytes);
        assert_eq!(render(&whole), render(&one), "{}", c.name);
    }
}

#[test]
fn every_golden_file_has_a_case() {
    let dir = golden_path("x");
    let dir = dir.parent().unwrap();
    let names: std::collections::BTreeSet<_> = cases().iter().map(|c| c.name.to_string()).collect();
    for e in std::fs::read_dir(dir).unwrap() {
        let f = e.unwrap().file_name().to_string_lossy().to_string();
        if let Some(n) = f.strip_suffix(".golden") {
            assert!(names.contains(n), "orphan golden {f}");
        }
    }
}
