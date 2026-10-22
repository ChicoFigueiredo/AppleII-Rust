//! Layout tests with exact expected positions (units of 1/2160"): T-002..T-007, T-010 (document
//! level), T-016, T-028 and more. Placeholder glyphs (see `lx310` docs): text runs carry ASCII, the
//! baseline of a line is `y_pos + 180`, and the green-bar printable origin is x = 1620.

mod lx310_common;
use apple2_printer::document::{PageDocument, TextRun};
use apple2_printer::lx310::Lx310Config;
use apple2_printer::paper::PaperKind;
use lx310_common::*;

const X0: u32 = 1620; // (9.5" - 8") / 2
const Y0: u32 = 357; // top of form: pin 1 at the 4.2 mm minimum top margin
const BASE: u32 = Y0 + 210; // text-layer baseline: pin 8 (7 * 30)

fn runs(doc: &PageDocument, page: usize) -> Vec<(u32, u32, String)> {
    doc.pages[page]
        .text
        .iter()
        .map(|t: &TextRun| (t.x, t.y, t.text.clone()))
        .collect()
}

/// Moves expected dots from top-of-form coordinates to page coordinates.
fn lift(v: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    v.into_iter().map(|(x, y)| (x, y + Y0)).collect()
}

fn no_diag(doc: &PageDocument) {
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
}

/// Y of the line where `probe` (a single character) was printed, for each probe in order.
fn ys(doc: &PageDocument) -> Vec<u32> {
    doc.pages
        .iter()
        .flat_map(|p| p.text.iter().map(|t| t.y - BASE))
        .collect()
}

#[test]
fn t002_reset_a_crlf_b_without_literal_esc() {
    let d = run(&[0x1B, 0x40, b'A', 0x0D, 0x0A, b'B']);
    no_diag(&d);
    assert_eq!(d.pages.len(), 1);
    assert_eq!(
        runs(&d, 0),
        vec![(X0, BASE, "A".into()), (X0, BASE + 360, "B".into())]
    );
    assert!(!d.pages[0]
        .text
        .iter()
        .any(|t| t.text.contains(['@', '\u{1b}'])));
}

#[test]
fn t003_cr_lf_and_crlf_are_distinct_without_double_line() {
    let d = run(&[65, 13, 66, 10, 67, 13, 10, b'D']);
    no_diag(&d);
    assert_eq!(
        runs(&d, 0),
        vec![
            (X0, BASE, "A".into()),
            (X0, BASE, "B".into()),       // CR alone: same line, overprinted
            (X0, BASE + 360, "C".into()), // LF moves one line
            (X0, BASE + 720, "D".into()), // CR LF is one line, not two
        ]
    );
}

#[test]
fn t004_form_feed_gives_second_page_in_same_job() {
    let d = run(b"A\x0cB");
    no_diag(&d);
    assert_eq!(d.pages.len(), 2);
    assert_eq!(runs(&d, 0), vec![(X0, BASE, "A".into())]);
    assert_eq!(runs(&d, 1), vec![(X0, BASE, "B".into())]);
    assert!(!d.last_in_progress);
}

#[test]
fn form_feed_is_kept_with_trailing_ff_and_blank_page_between() {
    let d = run(b"A\x0c\x0cB\x0c");
    assert_eq!(d.pages.len(), 3);
    assert!(d.pages[1].dots.is_empty());
}

#[test]
fn t005_line_spacing_commands_measured_in_units() {
    // Each probe 'X' is printed, then the spacing command, then LF.
    let cases: Vec<(&[u8], u32)> = vec![
        (&[0x1B, b'0'], 270),       // 1/8"
        (&[0x1B, b'2'], 360),       // 1/6"
        (&[0x1B, b'3', 36], 360),   // 36/216"
        (&[0x1B, b'3', 18], 180),   // 18/216"
        (&[0x1B, b'3', 0], 0),      // 0/216"
        (&[0x1B, b'3', 255], 2550), // 255/216"
        (&[0x1B, b'A', 12], 360),   // 12/72"
        (&[0x1B, b'A', 85], 2550),  // 85/72"
        (&[0x1B, b'1'], 210),       // 7/72"
    ];
    for (cmd, expect) in cases {
        let mut s = vec![b'X'];
        s.extend_from_slice(cmd);
        s.extend_from_slice(b"\nX");
        let d = run(&s);
        no_diag(&d);
        assert_eq!(ys(&d), vec![0, expect], "{cmd:02X?}");
    }
}

#[test]
fn t005_segments_and_esc_j() {
    // ESC J does not move horizontally and does not use the line spacing.
    let d = run(&[b'X', 0x1B, b'J', 54, b'X']);
    assert_eq!(
        runs(&d, 0),
        vec![(X0, BASE, "X".into()), (X0 + 216, BASE + 540, "X".into())]
    );
    // Same stream split in every way gives the same document.
    let s = [
        b'X', 0x1B, b'3', 36, 0x0A, 0x1B, b'0', 0x0A, 0x1B, b'2', 0x0A, b'X',
    ];
    let whole = run(&s);
    assert_eq!(ys(&whole), vec![0, 360 + 270 + 360]);
    for i in 0..=s.len() {
        assert_eq!(
            hash(&run_chunks(
                golden_config(),
                PaperKind::GreenBar,
                &[&s[..i], &s[i..]]
            )),
            hash(&whole)
        );
    }
}

#[test]
fn t006_pitch_advance_by_physical_width() {
    let adv = |prefix: &[u8]| {
        let mut s = prefix.to_vec();
        s.extend_from_slice(b"AB");
        let d = run(&s);
        no_diag(&d);
        // One run when contiguous; recover advance from the run.
        let r = &d.pages[0].text[0];
        assert_eq!(r.text, "AB");
        r.advance
    };
    assert_eq!(adv(&[0x1B, b'P']), 216);
    assert_eq!(adv(&[0x1B, b'M']), 180);
    assert_eq!(adv(&[0x1B, b'g']), 144);
    assert_eq!(adv(&[0x0F]), 126); // 17.14 cpi
    assert_eq!(adv(&[0x1B, b'M', 0x0F]), 108); // 20 cpi
    assert_eq!(adv(&[0x0E]), 432);
    assert_eq!(adv(&[0x1B, b'W', 1]), 432);
    assert_eq!(adv(&[0x0F, 0x0E]), 252);
    assert_eq!(adv(&[0x1B, b' ', 3]), 216 + 54);
    assert_eq!(adv(&[0x1B, b' ', 3, 0x0E]), (216 + 54) * 2);
    assert_eq!(adv(&[0x1B, b'!', 0x01]), 180);
    assert_eq!(adv(&[0x1B, b'!', 0x04]), 126);
    assert_eq!(adv(&[0x1B, b'!', 0x25]), 108 * 2); // 12 cpi + condensed + double width
    assert_eq!(adv(&[0x0F, 0x12]), 216); // DC2 cancels condensed
                                         // Same bytes in the payload regardless of pitch: only positions change.
    let d = run(&[0x1B, b'g', b'A', 0x1B, b'P', b'B']);
    assert_eq!(
        runs(&d, 0),
        vec![(X0, BASE, "A".into()), (X0 + 144, BASE, "B".into())]
    );
}

#[test]
fn t006_columns_by_physical_width_80_at_10cpi_and_no_clipping() {
    let line: Vec<u8> = std::iter::repeat_n(b'M', 80).collect();
    let d = run(&line);
    no_diag(&d);
    assert_eq!(d.pages[0].text.len(), 1);
    assert_eq!(d.pages[0].text[0].text.len(), 80);
    let max_x = d.pages[0].dots.iter().map(|p| p.x).max().unwrap();
    assert!(max_x < X0 + 17_280);
    // The 81st character wraps to the next line.
    let mut l81 = line.clone();
    l81.push(b'M');
    let d = run(&l81);
    assert_eq!(runs(&d, 0)[1], (X0, BASE + 360, "M".into()));
    // 136 columns at 17 cpi fit one line (8" / 126 = 137).
    let mut s = vec![0x0F];
    s.extend(std::iter::repeat_n(b'M', 137));
    let d = run(&s);
    assert_eq!(d.pages[0].text.len(), 1);
}

fn base_dots() -> Vec<(u32, u32)> {
    dots_of(&run(b"X"), 0)
}

#[test]
fn t007_bold_double_strike_underline_affect_dots() {
    let plain = base_dots();
    assert!(plain.len() > 20);

    let bold = dots_of(&run(&[0x1B, b'E', b'X']), 0);
    let mut expect: Vec<(u32, u32)> = plain.clone();
    expect.extend(plain.iter().map(|&(x, y)| (x + 9, y)));
    assert_eq!(bold, expect, "ESC E prints each dot twice, 1/240\" apart");

    let ds = dots_of(&run(&[0x1B, b'G', b'X']), 0);
    let mut expect: Vec<(u32, u32)> = plain.clone();
    expect.extend(plain.iter().map(|&(x, y)| (x, y + 10)));
    assert_eq!(ds, expect, "ESC G prints each dot twice, 1/216\" lower");

    let both = dots_of(&run(&[0x1B, b'E', 0x1B, b'G', b'X']), 0);
    assert_eq!(both.len(), plain.len() * 4);

    let ul = dots_of(&run(&[0x1B, b'-', 1, b'X']), 0);
    let line: Vec<(u32, u32)> = (0..12).map(|k| (X0 + k * 18, Y0 + 240)).collect();
    let mut expect = plain.clone();
    expect.extend(line);
    assert_eq!(ul, expect, "underline on pin 9 across the 10 cpi cell");

    // Cancelling restores plain printing.
    assert_eq!(dots_of(&run(&[0x1B, b'E', 0x1B, b'F', b'X']), 0), plain);
    assert_eq!(dots_of(&run(&[0x1B, b'G', 0x1B, b'H', b'X']), 0), plain);
    assert_eq!(
        dots_of(&run(&[0x1B, b'-', 1, 0x1B, b'-', 0, b'X']), 0),
        plain
    );
    assert_eq!(
        dots_of(&run(&[0x1B, b'-', 49, 0x1B, b'-', 48, b'X']), 0),
        plain
    );
}

#[test]
fn t007_master_select_combines_and_resets_modes() {
    // ESC ! 0x88 = bold + underline; equals the separate commands.
    let a = run(&[0x1B, b'!', 0x88, b'X']);
    let b = run(&[0x1B, b'E', 0x1B, b'-', 1, b'X']);
    assert_eq!(render(&a), render(&b));
    // ESC ! 0 cancels everything that was on.
    let c = run(&[0x1B, b'E', 0x1B, b'-', 1, 0x0F, 0x1B, b'!', 0, b'X']);
    assert_eq!(dots_of(&c, 0), base_dots());
    assert_eq!(c.pages[0].text[0].advance, 216);
    // Bit 0 = 12 cpi, bit 5 = double width.
    let d = run(&[0x1B, b'!', 0x21, b'X']);
    assert_eq!(d.pages[0].text[0].advance, 360);
}

#[test]
fn t008_esc_k_eight_columns_exact_dots_and_text_resumes() {
    // Columns: 80 (top pin), 01 (pin 8), FF, 1B, 0A, 00, 81, 40.
    let d = run(&[
        0x1B, b'K', 8, 0, 0x80, 0x01, 0xFF, 0x1B, 0x0A, 0x00, 0x81, 0x40, b'A',
    ]);
    no_diag(&d);
    // col 3 (0x1B): pins 3,4,6,7; col 4 (0x0A): pins 4,6.
    let full: Vec<(u32, u32)> = vec![
        (X0, 0),
        (X0 + 36, 210),
        (X0 + 72, 0),
        (X0 + 72, 30),
        (X0 + 72, 60),
        (X0 + 72, 90),
        (X0 + 72, 120),
        (X0 + 72, 150),
        (X0 + 72, 180),
        (X0 + 72, 210),
        (X0 + 108, 90),
        (X0 + 108, 120),
        (X0 + 108, 180),
        (X0 + 108, 210),
        (X0 + 144, 120),
        (X0 + 144, 180),
    ];
    assert_eq!(&dots_of(&d, 0)[..full.len()], &lift(full)[..]);
    // col 5 is 0x00; col 6 is 0x81: pins 0 and 7; col 7 is 0x40: pin 1.
    let all = dots_of(&d, 0);
    assert!(all.contains(&(X0 + 216, Y0)) && all.contains(&(X0 + 216, Y0 + 210)));
    assert!(all.contains(&(X0 + 252, Y0 + 30)));
    // No fictitious command: the 0x1B and 0x0A inside were data, so no line feed happened and the
    // text after the eighth column prints right after the graphic (8 * 36 = 288).
    assert_eq!(runs(&d, 0), vec![(X0 + 288, BASE, "A".into())]);
}

#[test]
fn t009_esc_star_payload_with_esc_and_lf_respects_density() {
    for (m, step) in [
        (0u8, 36u32),
        (1, 18),
        (2, 18),
        (3, 9),
        (4, 27),
        (5, 30),
        (6, 24),
    ] {
        let d = run(&[0x1B, b'*', m, 4, 0, 0x00, 0xFF, 0x1B, 0x0A, b'B']);
        no_diag(&d);
        let all = dots_of(&d, 0);
        // Column 1 (0xFF): 8 pins.
        let c1: Vec<_> = all.iter().filter(|p| p.0 == X0 + step).collect();
        let c2: Vec<_> = all.iter().filter(|p| p.0 == X0 + 2 * step).collect();
        let c3: Vec<_> = all.iter().filter(|p| p.0 == X0 + 3 * step).collect();
        assert_eq!(c1.len(), 8, "m={m}");
        // 0x1B 0x0A would be a bare ESC and a line feed; as data they are ordinary columns
        // (0x1B: 4 pins, 0x0A: 2 pins). ESC Y/Z (m = 2, 3) ignore a dot right after a printed one.
        if m == 2 || m == 3 {
            assert_eq!((c2.len(), c3.len()), (0, 0), "m={m}");
        } else {
            assert_eq!((c2.len(), c3.len()), (4, 2), "m={m}");
        }
        assert_eq!(d.pages[0].text[0].x, X0 + 4 * step, "m={m}");
        assert_eq!(d.pages[0].text[0].y, BASE, "m={m}: no line feed happened");
    }
}

#[test]
fn adjacent_dots_are_dropped_for_esc_y_and_z_only() {
    // Two full columns: with ESC L both print, with ESC Y/Z the second is ignored.
    let l = dots_of(&run(&[0x1B, b'L', 2, 0, 0xFF, 0xFF]), 0);
    let y = dots_of(&run(&[0x1B, b'Y', 2, 0, 0xFF, 0xFF]), 0);
    let z = dots_of(&run(&[0x1B, b'Z', 2, 0, 0xFF, 0xFF]), 0);
    assert_eq!(l.len(), 16);
    assert_eq!(y.len(), 8);
    assert_eq!(z.len(), 8);
    // Non-adjacent columns are all kept.
    assert_eq!(
        dots_of(&run(&[0x1B, b'Y', 3, 0, 0xFF, 0x00, 0xFF]), 0).len(),
        16
    );
}

/// A pin that did not fire in the previous column may fire in this one: a continuous stroke in a
/// no-adjacent mode comes out as every other column (the dot ignored is the second of a pair, and
/// the comparison is against what was PRINTED, not what was asked for).
#[test]
fn continuous_stroke_prints_every_other_column_in_no_adjacent_modes() {
    for (cmd, step) in [
        (&[0x1B, b'Y'][..], 18u32),
        (&[0x1B, b'Z'][..], 9),
        (&[0x1B, b'*', 2][..], 18),
        (&[0x1B, b'*', 3][..], 9),
    ] {
        let mut v = cmd.to_vec();
        v.extend_from_slice(&[8, 0]);
        v.extend_from_slice(&[0xFF; 8]);
        let d = run(&v);
        let dots = dots_of(&d, 0);
        assert_eq!(dots.len(), 4 * 8, "{cmd:?}");
        let xs: std::collections::BTreeSet<_> = dots.iter().map(|p| p.0).collect();
        let want: std::collections::BTreeSet<_> = (0..4).map(|i| X0 + 2 * i * step).collect();
        assert_eq!(xs, want, "{cmd:?}");
        // Alternating pattern: AA 55 AA 55 ... pins that differ never collide.
        let mut v = cmd.to_vec();
        v.extend_from_slice(&[6, 0, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55]);
        assert_eq!(dots_of(&run(&v), 0).len(), 24, "{cmd:?}");
    }
}

#[test]
fn graphics_two_contiguous_lines_no_transposition() {
    // Line 1 at y=0 with pattern, LF 1/6" (360) and line 2; each column keeps its own bits.
    let d = run(&[
        0x1B, b'K', 3, 0, 0x80, 0x40, 0x01, 0x0D, 0x0A, 0x1B, b'K', 3, 0, 0x01, 0x40, 0x80,
    ]);
    assert_eq!(
        dots_of(&d, 0),
        lift(vec![
            (X0, 0),
            (X0 + 36, 30),
            (X0 + 72, 210),
            (X0, 360 + 210),
            (X0 + 36, 360 + 30),
            (X0 + 72, 360),
        ])
    );
}

#[test]
fn nine_pin_graphics_uses_second_byte_for_the_ninth_pin() {
    let d = run(&[0x1B, b'^', 1, 2, 0, 0x80, 0x01, 0x00, 0x01]);
    assert_eq!(
        dots_of(&d, 0),
        lift(vec![(X0, 0), (X0, 240), (X0 + 18, 240)])
    );
}

#[test]
fn graphics_beyond_the_right_margin_is_dropped_and_reported() {
    // ESC Q 1 -> right margin at 1 column (216): only columns with x < 216 print at 60 dpi (6 of 10).
    let d = run(&[
        0x1B, b'Q', 1, 0x1B, b'K', 10, 0, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
        0x80,
    ]);
    assert_eq!(d.pages[0].dots.len(), 216 / 36);
    assert_eq!(d.diagnostics.len(), 1);
}

#[test]
fn t010_document_hash_is_split_independent() {
    let s: Vec<u8> = [
        &b"\x1b@Hello\r\n"[..],
        &[0x1B, b'E'],
        b"World",
        &[0x1B, b'K', 4, 0, 0x1B, 0x0A, 0x0C, 0xFF],
        &[0x1B, b'D', 8, 0, 9],
        b"tab\x0c page two",
        &[0x1B, b'$', 5, 0, b'x', 0x1B, b'v', b'!'],
    ]
    .concat();
    let whole = hash(&run(&s));
    for i in 0..=s.len() {
        assert_eq!(
            hash(&run_chunks(
                golden_config(),
                PaperKind::GreenBar,
                &[&s[..i], &s[i..]]
            )),
            whole,
            "split {i}"
        );
    }
    let one: Vec<&[u8]> = s.chunks(1).collect();
    assert_eq!(
        hash(&run_chunks(golden_config(), PaperKind::GreenBar, &one)),
        whole
    );
}

#[test]
fn t011_incomplete_sequences_do_not_panic_and_are_reported() {
    for s in [
        &[0x1Bu8][..],
        &[0x1B, b'K'],
        &[0x1B, b'K', 5],
        &[0x1B, b'K', 5, 0, 1, 2],
        &[0x1B, b'(', b't', 3],
    ] {
        let d = run(s);
        assert!(!d.diagnostics.is_empty(), "{s:02X?}");
        assert_eq!(d.diagnostics[0].offset, 0);
    }
}

#[test]
fn t016_one_thousand_lines_without_drift() {
    let mut s = Vec::new();
    for _ in 0..1000 {
        s.extend_from_slice(b"X\r\n");
    }
    let d = run(&s);
    no_diag(&d);
    // The strip of continuous paper: page * height + y must be exactly n * 360 + baseline
    // (66 lines of 1/6" = one 11" form), so the baseline of line 66 lands on the next form.
    let all: Vec<u32> = d
        .pages
        .iter()
        .flat_map(|p| p.text.iter().map(|t| t.y))
        .collect();
    assert_eq!(all.len(), 1000);
    let h = d.pages[0].height;
    let mut n = 0u32;
    for (i, p) in d.pages.iter().enumerate() {
        for t in &p.text {
            let abs = i as u32 * h + t.y;
            assert_eq!(abs, n * 360 + BASE, "line {n}");
            n += 1;
        }
    }
    assert_eq!(n, 1000);
    assert_eq!(d.pages.len(), 16);
}

#[test]
fn t016_one_thousand_esc_j_and_ff_are_exact() {
    let mut s = Vec::new();
    for _ in 0..1000 {
        s.extend_from_slice(&[0x1B, b'J', 1]); // 1/216" = 10 units each
    }
    s.push(b'X');
    let d = run(&s);
    assert_eq!(d.pages[0].text[0].y, 10_000 + BASE);
    let mut s = Vec::new();
    for _ in 0..1000 {
        s.extend_from_slice(b"X\x0c");
    }
    let d = run(&s);
    assert_eq!(d.pages.len(), 1000);
    assert!(d.pages.iter().all(|p| p.text[0].y == BASE));
}

#[test]
fn page_break_by_length_esc_c_and_bottom_margin() {
    // 3 lines per page (ESC C 3 at 1/6").
    let d = run(b"\x1bC\x03A\nB\nC\nD\nE");
    assert_eq!(d.pages.len(), 2);
    assert_eq!(d.pages[0].text.len(), 3);
    assert_eq!(d.pages[1].text.len(), 2);
    // ESC C 0 1: one inch = 6 lines.
    let d = run(b"\x1bC\x00\x01A\nB\nC\nD\nE\nF\nG");
    assert_eq!(d.pages.len(), 2);
    assert_eq!(d.pages[1].text.len(), 1);
    // ESC N 2 on a 6-line page leaves 4 usable lines (continuous paper).
    let d = run(b"\x1bC\x06\x1bN\x02A\nB\nC\nD\nE");
    assert_eq!(d.pages.len(), 2);
    assert_eq!(d.pages[0].text.len(), 4);
    // ESC O cancels it.
    let d = run(b"\x1bC\x06\x1bN\x02\x1bO A\nB\nC\nD\nE");
    assert_eq!(d.pages[0].text.len(), 5);
    // The bottom margin is ignored on single sheets.
    let d = run_paper(PaperKind::A4, b"\x1bC\x06\x1bN\x02A\nB\nC\nD\nE");
    assert_eq!(d.pages.len(), 1);
}

#[test]
fn t028_reset_restores_format_without_erasing_the_document() {
    let s = b"\x1bE\x1b-\x01\x1bl\x0aA\r\n\x1b@B\x0cC";
    let d = run(s);
    no_diag(&d);
    assert_eq!(d.pages.len(), 2);
    let a = &d.pages[0];
    let a_text: Vec<_> = a.text.iter().map(|t| (t.x, t.y, t.text.as_str())).collect();
    // A: left margin 10 columns, bold and underlined. B: default margin, plain.
    assert_eq!(a_text[0], (X0 + 2160, BASE, "A"));
    assert_eq!(a_text[1], (X0, BASE + 360, "B"));
    let plain_b = run(b"\r\n\r\nB\x0cC");
    let _ = plain_b;
    let a_dots_before = a.dots.len();
    assert!(
        a_dots_before > base_dots().len() * 2,
        "A is bold + underlined"
    );
    // Page 2 'C' has no attributes left.
    assert_eq!(d.pages[1].dots.len(), base_dots().len());
    // The document before ESC @ is intact: same dots as without the later bytes.
    let prefix = run(b"\x1bE\x1b-\x01\x1bl\x0aA\r\n");
    assert_eq!(
        &a.dots[..prefix.pages[0].dots.len()],
        &prefix.pages[0].dots[..]
    );
}

#[test]
fn horizontal_tabs_default_and_esc_d() {
    let d = run(b"A\tB");
    assert_eq!(runs(&d, 0)[1].0, X0 + 8 * 216);
    let d = run(b"\x1bD\x05\x0a\x00A\tB\tC");
    let r = runs(&d, 0);
    assert_eq!(r[1].0, X0 + 5 * 216);
    assert_eq!(r[2].0, X0 + 10 * 216);
    // No tab to the right: HT ignored.
    let d = run(b"\x1bD\x05\x00\t\tA");
    assert_eq!(runs(&d, 0)[0].0, X0 + 5 * 216);
    // ESC D NUL cancels all tabs.
    let d = run(b"\x1bD\x00\tA");
    assert_eq!(runs(&d, 0)[0].0, X0);
    // The tab column is in the pitch of the moment it was set (condensed: 126 per column).
    let d = run(b"\x0f\x1bD\x05\x00\tA");
    assert_eq!(runs(&d, 0)[0].0, X0 + 5 * 126);
}

#[test]
fn t029_esc_d_without_nul_applies_tabs_and_warns_with_offset() {
    let d = run(b"\x1bD\x05\x0a");
    assert_eq!(d.diagnostics.len(), 1);
    assert_eq!(d.diagnostics[0].offset, 0);
    assert!(d.diagnostics[0].message.contains("NUL"));
    // The tabs were applied in the incremental state before the end: feeding text later in the
    // same job sees them.
    let mut e = apple2_printer::lx310::Lx310::new(golden_config());
    use apple2_printer::engine::PrinterEngine;
    e.begin(options(PaperKind::GreenBar));
    e.feed(0, b"\x1bD\x05\x0a");
    assert!(
        e.document().diagnostics.is_empty(),
        "no verdict before the stream ends"
    );
    e.feed(4, b"\x00\tA");
    e.finish();
    assert_eq!(e.document().pages[0].text[0].x, X0 + 5 * 216);
}

#[test]
fn vertical_tabs() {
    // No ESC B: VT behaves as LF.
    assert_eq!(ys(&run(b"A\x0bB")), vec![0, 360]);
    // ESC B 3 6 NUL: tabs at lines 3 and 6 from top of form.
    assert_eq!(
        ys(&run(b"\x1bB\x03\x06\x00A\x0bB\x0bC")),
        vec![0, 3 * 360, 6 * 360]
    );
    // Past the last tab: next page.
    let d = run(b"\x1bB\x03\x00A\x0bB\x0bC");
    assert_eq!(d.pages.len(), 2);
    // ESC B NUL: VT is CR.
    assert_eq!(ys(&run(b"\x1bB\x00A\x0bB")), vec![0, 0]);
}

#[test]
fn margins_and_wrap() {
    // Right margin 5 columns: 7 characters wrap after the fifth.
    let d = run(b"\x1bQ\x05ABCDEFG");
    assert_eq!(
        runs(&d, 0),
        vec![(X0, BASE, "ABCDE".into()), (X0, BASE + 360, "FG".into())]
    );
    // Left margin 3 columns moves the line start; CR returns to it.
    let d = run(b"\x1bl\x03A\rB");
    let r = runs(&d, 0);
    assert_eq!(r[0].0, X0 + 3 * 216);
    assert_eq!(r[1].0, X0 + 3 * 216);
    // Left >= right is rejected with a diagnostic.
    let d = run(b"\x1bQ\x05\x1bl\x05A");
    assert_eq!(d.diagnostics.len(), 1);
    assert_eq!(runs(&d, 0)[0].0, X0);
    // Right margin beyond the printable width is rejected.
    let d = run(b"\x1bQ\xffA");
    assert_eq!(d.diagnostics.len(), 1);
}

#[test]
fn absolute_and_relative_horizontal_positions() {
    // ESC $ 1/60": 60 -> 1 inch.
    let d = run(b"\x1b$\x3c\x00A");
    assert_eq!(runs(&d, 0)[0].0, X0 + 2160);
    // ESC \ 1/120": +120 -> 1 inch, then -60 -> half inch back.
    let d = run(b"\x1b\\\x78\x00\x1b\\\xc4\xffA");
    assert_eq!(runs(&d, 0)[0].0, X0 + 2160 - 1080);
    // Out of the printable area: ignored with diagnostic.
    let d = run(b"\x1b$\xff\x7fA");
    assert_eq!(d.diagnostics.len(), 1);
    assert_eq!(runs(&d, 0)[0].0, X0);
    let d = run(b"\x1b\\\x00\xf0A");
    assert_eq!(d.diagnostics.len(), 1);
}

#[test]
fn backspace_moves_one_character_left() {
    let d = run(b"AB\x08C");
    assert_eq!(runs(&d, 0)[1].0, X0 + 216); // A..B then back one cell: C overprints B
    let d = run(b"\x08A");
    assert_eq!(runs(&d, 0)[0].0, X0); // ignored at the left margin
}

#[test]
fn paper_origin_follows_the_preset() {
    for (kind, x0) in [
        (PaperKind::GreenBar, 1620u32),
        (PaperKind::PlainContinuous, 1620),
        (PaperKind::Letter, 540),
        (PaperKind::A4, 289),
    ] {
        let d = run_paper(kind, b"A");
        assert_eq!(d.pages[0].text[0].x, x0, "{kind:?}");
        assert_eq!((d.pages[0].width, d.pages[0].height), {
            let p = apple2_printer::paper::Paper::preset(kind);
            (p.width, p.height)
        });
    }
}

#[test]
fn esc_at_keeps_position_and_open_page_state() {
    let d = run(b"A\n\x1b@B");
    // ESC @ restores format but the paper does not move.
    assert_eq!(
        runs(&d, 0),
        vec![(X0, BASE, "A".into()), (X0, BASE + 360, "B".into())]
    );
}

#[test]
fn unknown_and_unsupported_commands_never_pass_silently() {
    for s in [&b"\x1bvA"[..], b"\x1bwA\x01", b"\x1bSA", b"\x18A"] {
        let d = run(s);
        assert!(!d.diagnostics.is_empty(), "{s:02X?}");
    }
}

#[test]
fn upper_codes_print_by_default_and_esc_7_makes_them_controls() {
    let d = run(b"\x80");
    assert_eq!(d.pages[0].text.len(), 0); // placeholder decodes only ASCII text
    assert!(!d.pages[0].dots.is_empty());
    assert!(d.diagnostics.is_empty());
    let d = run(b"\x1b7\x80\x1b6\x80");
    assert_eq!(d.diagnostics.len(), 1);
    assert_eq!(d.diagnostics[0].offset, 2);
    assert!(!d.pages[0].dots.is_empty());
}

#[test]
fn state_only_commands_are_accepted_without_diagnostics() {
    // ESC t / R / x / k / U / ( t change state the real font will use; no dots of their own.
    let d =
        run(b"\x1bt\x00\x1bR\x02\x1bx\x01\x1bk\x01\x1bU\x01\x1b(t\x03\x00\x01\x03\x00\x1bt\x01A");
    no_diag(&d);
    assert_eq!(d.pages[0].text.len(), 1);
}

#[test]
fn ink_is_black_and_dots_stay_on_the_page() {
    let d = run(b"Hello, world\r\n\x1b*\x00\x05\x00\xff\xff\xff\xff\xff");
    for p in &d.pages {
        assert!(p
            .dots
            .iter()
            .all(|d| d.ink == 0 && d.x < p.width && d.y < p.height));
    }
}

#[test]
fn revision_counts_changes_since_last_call() {
    use apple2_printer::engine::PrinterEngine;
    let mut e = apple2_printer::lx310::Lx310::new(golden_config());
    e.begin(options(PaperKind::GreenBar));
    assert_eq!(e.take_revision(), 0);
    e.feed(0, b"A");
    assert!(e.take_revision() > 0);
    assert_eq!(e.take_revision(), 0);
    e.feed(1, b"\r\n");
    assert_eq!(e.take_revision(), 0, "movement alone changes no ink");
}

#[test]
fn begin_starts_a_fresh_job() {
    use apple2_printer::engine::PrinterEngine;
    let mut e = apple2_printer::lx310::Lx310::new(golden_config());
    e.begin(options(PaperKind::GreenBar));
    e.feed(0, b"\x1bEA\x1bK\x05");
    e.begin(options(PaperKind::A4));
    e.feed(0, b"B");
    e.finish();
    let d = e.document();
    assert_eq!(d.pages.len(), 1);
    assert!(d.diagnostics.is_empty());
    assert_eq!(d.pages[0].text[0].x, 289);
}

#[test]
fn first_line_is_inside_the_page_at_the_top_margin() {
    for kind in [PaperKind::GreenBar, PaperKind::A4, PaperKind::Letter] {
        let d = run_paper(kind, b"\x1bE\x1bW\x01APPLE II-RUST\x1bF");
        no_diag(&d);
        let p = &d.pages[0];
        let min = p.dots.iter().map(|d| d.y).min().unwrap();
        let max = p.dots.iter().map(|d| d.y).max().unwrap();
        assert_eq!(
            min, 357,
            "pin 1 at the 4.2 mm minimum top margin ({kind:?})"
        );
        assert_eq!(max, 357 + 6 * 30, "placeholder box rows 0..6 ({kind:?})");
        assert!(max < p.height);
        assert!(p
            .dots
            .iter()
            .all(|d| d.x >= if kind == PaperKind::A4 { 289 } else { 540 }));
    }
}

#[test]
fn double_width_doubles_every_matrix_column() {
    let plain = dots_of(&run(b"X"), 0);
    for stream in [&b"\x1bW\x01X"[..], b"\x0eX", b"\x1b!\x20X"] {
        let wide = dots_of(&run(stream), 0);
        // Every matrix column becomes two adjacent columns; a dot with a row-mate two matrix
        // columns to its right also fills that empty column pair (continuous strokes).
        let mut expect = Vec::new();
        for &(x, y) in &plain {
            let i = (x - X0) / 18; // matrix column index
            let fill = plain.contains(&(x + 36, y));
            for k in 0..(if fill { 4 } else { 2 }) {
                expect.push((X0 + (2 * i + k) * 18, y));
            }
        }
        expect.sort_unstable();
        expect.dedup();
        let mut wide = wide;
        wide.sort_unstable();
        wide.dedup();
        assert_eq!(wide, expect, "{stream:02X?}");
    }
}

#[test]
fn line_66_of_a_form_prints_on_the_next_form_without_loss() {
    // 66 lines fill an 11" form; the last line's lower pins fall past the form edge and continue
    // on the next form (continuous paper), where they cannot collide with its first line.
    let mut s = Vec::new();
    for _ in 0..65 {
        s.extend_from_slice(b"\r\n");
    }
    s.extend_from_slice(b"X");
    let d = run(&s);
    no_diag(&d);
    let base = dots_of(&run(b"X"), 0);
    let abs: Vec<u32> = d
        .pages
        .iter()
        .enumerate()
        .flat_map(|(i, p)| p.dots.iter().map(move |q| i as u32 * p.height + q.y))
        .collect();
    let mut expect: Vec<u32> = base.iter().map(|&(_, y)| y + 65 * 360).collect();
    let mut abs = abs;
    abs.sort_unstable();
    expect.sort_unstable();
    assert_eq!(abs, expect);
    assert_eq!(d.pages.len(), 2);
    // Single sheets cannot: the dots below the sheet are dropped and reported.
    let d = run_paper(PaperKind::A4, b"\x1bC\x00\x16\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xff\x1bJ\xffX");
    assert!(d
        .pages
        .iter()
        .all(|p| p.dots.iter().all(|q| q.y < p.height)));
}

/// Runs `stream` with the real font.
fn run_font(stream: &[u8]) -> PageDocument {
    run_chunks(Lx310Config::default(), PaperKind::GreenBar, &[stream])
}

#[test]
fn condensed_and_narrow_pitches_keep_every_glyph_inside_its_cell() {
    // The same matrix at a smaller pitch: columns are spaced proportionally, so the dots of one
    // character never reach the next cell (126 units at 17 cpi, 144 at 15 cpi).
    for (pre, adv) in [
        (&b"\x0f"[..], 126u32),
        (b"\x1bP", 216),
        (b"\x1bM", 180),
        (b"\x1bg", 144),
    ] {
        let mut s = pre.to_vec();
        s.extend_from_slice(b"W");
        let d = run_font(&s);
        let xs: Vec<u32> = d.pages[0].dots.iter().map(|q| q.x).collect();
        let (lo, hi) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
        assert!(lo >= X0 && hi < X0 + adv, "{pre:02X?}: {lo}..{hi} vs {adv}");
    }
}

#[test]
fn frame_strokes_continue_through_the_gap_between_lines() {
    // CP437 0xB3 on two consecutive lines at 1/6": the pins above and below the gap are joined
    // by one dot per pin row, so the stroke has a dot every 30 units from top to bottom.
    let d = run_font(b"\x1bt\x01\xb3\r\n\xb3");
    let ys: Vec<u32> = d.pages[0].dots.iter().map(|q| q.y).collect();
    let mut sorted = ys.clone();
    sorted.sort_unstable();
    let expect: Vec<u32> = (0..9 + 12).map(|i| Y0 + 30 * i).collect();
    assert_eq!(sorted, expect);
    // A line pitch over 1/4" is left open.
    let d = run_font(b"\x1bt\x01\x1b3\x60\xb3\r\n\xb3");
    assert_eq!(d.pages[0].dots.len(), 18);
}
