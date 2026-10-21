//! Parser tests: T-008, T-009, T-010 (parser level), T-011, T-029, resynchronisation (CA03).

use apple2_printer::command::{Command, Parsed};
use apple2_printer::document::Diagnostic;
use apple2_printer::escp::{Parser, ParserLimits};

fn parse_chunks(chunks: &[&[u8]]) -> (Vec<Parsed>, Vec<Diagnostic>) {
    let mut p = Parser::default();
    let (mut o, mut d) = (Vec::new(), Vec::new());
    let mut off = 0u64;
    for c in chunks {
        p.feed(off, c, &mut o, &mut d);
        off += c.len() as u64;
    }
    p.finish(&mut o, &mut d);
    (o, d)
}

fn parse(data: &[u8]) -> (Vec<Parsed>, Vec<Diagnostic>) {
    parse_chunks(&[data])
}

fn cmds(v: &[Parsed]) -> Vec<Command> {
    v.iter().map(|p| p.command.clone()).collect()
}

fn gfx(density: u8, columns: u16, data: &[u8]) -> Command {
    Command::Graphics {
        density,
        nine_pin: false,
        columns,
        data: data.to_vec(),
    }
}

#[test]
fn t008_esc_k_reads_exactly_eight_columns_with_esc_inside() {
    let stream = [0x1B, b'K', 8, 0, 1, 2, 0x1B, 4, 5, 6, 7, 0x0A, b'Z'];
    let (o, d) = parse(&stream);
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(
        cmds(&o),
        vec![
            gfx(0, 8, &[1, 2, 0x1B, 4, 5, 6, 7, 0x0A]),
            Command::Char(b'Z')
        ]
    );
    // Text resumes at the ninth payload byte's successor (offset 4 + 8 = 12).
    assert_eq!(o[1].offset, 12);
}

#[test]
fn t008_bdd_chunks_of_1_2_and_5_bytes() {
    let stream = [
        0x1B, b'K', 8, 0, 0x11, 0x1B, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, b'A',
    ];
    let whole = parse(&stream);
    for size in [1usize, 2, 5] {
        let chunks: Vec<&[u8]> = stream.chunks(size).collect();
        assert_eq!(parse_chunks(&chunks), whole, "chunk size {size}");
    }
    assert_eq!(whole.0.len(), 2);
}

#[test]
fn t009_esc_star_density_and_data_with_esc_and_lf() {
    // ESC * m=4 (80 dpi) nL=4 nH=0 then 00 FF 1B 0A
    let (o, d) = parse(&[0x1B, b'*', 4, 4, 0, 0x00, 0xFF, 0x1B, 0x0A]);
    assert!(d.is_empty());
    assert_eq!(cmds(&o), vec![gfx(4, 4, &[0x00, 0xFF, 0x1B, 0x0A])]);
}

#[test]
fn graphics_commands_k_l_y_z_map_to_densities_0_to_3() {
    for (cmd, dens) in [(b'K', 0u8), (b'L', 1), (b'Y', 2), (b'Z', 3)] {
        let (o, _) = parse(&[0x1B, cmd, 1, 0, 0xAA]);
        assert_eq!(cmds(&o), vec![gfx(dens, 1, &[0xAA])]);
    }
}

#[test]
fn nine_pin_graphics_reads_two_bytes_per_column() {
    let (o, d) = parse(&[0x1B, b'^', 1, 2, 0, 0xAA, 0x01, 0xBB, 0x00, b'x']);
    assert!(d.is_empty());
    assert_eq!(
        cmds(&o),
        vec![
            Command::Graphics {
                density: 1,
                nine_pin: true,
                columns: 2,
                data: vec![0xAA, 0x01, 0xBB, 0x00]
            },
            Command::Char(b'x')
        ]
    );
}

#[test]
fn t010_every_split_point_gives_the_same_output() {
    let stream: Vec<u8> = [
        &[0x1B, 0x40, b'A', 0x0D, 0x0A][..],
        &[0x1B, b'K', 3, 0, 0x1B, 0x0A, 0xFF],
        &[0x1B, b'D', 8, 16, 0, b'\t', b'B'],
        &[0x1B, b'(', b't', 3, 0, 1, 3, 0],
        &[0x1B, b'C', 0, 11, 0x1B, 0x7A, b'!', 0x0E, b'X', 0x14],
        &[0x1B, b'$', 0x10, 0x00, 0x1B, b'\\', 0xF0, 0xFF],
    ]
    .concat();
    let whole = parse(&stream);
    assert!(!whole.0.is_empty());
    for i in 0..=stream.len() {
        assert_eq!(
            parse_chunks(&[&stream[..i], &stream[i..]]),
            whole,
            "split at {i}"
        );
    }
    for size in 1..=7 {
        let chunks: Vec<&[u8]> = stream.chunks(size).collect();
        assert_eq!(parse_chunks(&chunks), whole, "chunk size {size}");
    }
    // Three-way splits too.
    for i in 0..stream.len() {
        for j in i..stream.len() {
            assert_eq!(
                parse_chunks(&[&stream[..i], &stream[i..j], &stream[j..]]),
                whole
            );
        }
    }
}

#[test]
fn t011_lone_esc_at_end_is_diagnosed() {
    let (o, d) = parse(&[b'A', 0x1B]);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].offset, 1);
    assert_eq!(d[0].bytes, vec![0x1B]);
    assert_eq!(o[0].command, Command::Char(b'A'));
    assert!(matches!(o[1].command, Command::Unsupported { .. }));
}

#[test]
fn t011_graphics_declared_longer_than_stream_keeps_received_and_diagnoses() {
    let (o, d) = parse(&[0x1B, b'K', 0x00, 0x10, 1, 2, 3]); // 4096 columns declared, 3 received
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].offset, 0);
    assert!(d[0].message.contains("truncado"));
    assert_eq!(cmds(&o), vec![gfx(0, 3, &[1, 2, 3])]);
}

#[test]
fn t011_parser_state_does_not_leak_into_a_new_job() {
    let mut p = Parser::default();
    let (mut o, mut d) = (Vec::new(), Vec::new());
    p.feed(0, &[0x1B, b'K', 0xFF, 0x1F], &mut o, &mut d);
    assert!(p.in_sequence());
    p.reset();
    p.feed(0, b"hi", &mut o, &mut d);
    assert_eq!(cmds(&o), vec![Command::Char(b'h'), Command::Char(b'i')]);
    assert!(d.is_empty());
}

#[test]
fn t029_esc_d_without_nul_applies_tabs_and_warns() {
    let (o, d) = parse(&[0x1B, b'D', 5, 10, 15]);
    assert_eq!(cmds(&o), vec![Command::HorizontalTabs(vec![5, 10, 15])]);
    assert_eq!(d.len(), 1);
    assert!(d[0].message.contains("sem NUL"));
    assert_eq!(d[0].offset, 0);
}

#[test]
fn esc_d_with_nul_is_syntax_not_a_glyph() {
    let (o, d) = parse(&[0x1B, b'D', 5, 10, 0, b'A']);
    assert!(d.is_empty());
    assert_eq!(
        cmds(&o),
        vec![Command::HorizontalTabs(vec![5, 10]), Command::Char(b'A')]
    );
}

#[test]
fn esc_d_non_ascending_value_terminates_like_nul() {
    let (o, _) = parse(&[0x1B, b'D', 10, 5, b'A']);
    assert_eq!(
        cmds(&o),
        vec![Command::HorizontalTabs(vec![10]), Command::Char(b'A')]
    );
}

#[test]
fn esc_d_stops_after_32_tabs_and_esc_b_after_16() {
    let mut s = vec![0x1B, b'D'];
    s.extend(1..=40u8);
    let (o, _) = parse(&s);
    assert_eq!(o[0].command, Command::HorizontalTabs((1..=32).collect()));
    assert_eq!(o.len(), 1 + 8); // the remaining bytes are text
    let mut s = vec![0x1B, b'B'];
    s.extend(1..=20u8);
    let (o, _) = parse(&s);
    assert_eq!(o[0].command, Command::VerticalTabs((1..=16).collect()));
}

#[test]
fn unknown_esc_drops_exactly_two_bytes_and_resyncs() {
    let (o, d) = parse(&[0x1B, b'v', b'A', b'B']);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].bytes, vec![0x1B, b'v']);
    assert_eq!(
        cmds(&o),
        vec![
            Command::Unsupported {
                bytes: vec![0x1B, b'v']
            },
            Command::Char(b'A'),
            Command::Char(b'B')
        ]
    );
}

#[test]
fn esc_esc_restarts_and_esc_control_keeps_the_control() {
    let (o, d) = parse(&[0x1B, 0x1B, b'@']);
    assert_eq!(d.len(), 1);
    assert_eq!(o.last().unwrap().command, Command::Reset);
    let (o, d) = parse(&[0x1B, 0x0D, b'A']);
    assert_eq!(d.len(), 1);
    assert_eq!(
        cmds(&o),
        vec![
            Command::Unsupported { bytes: vec![0x1B] },
            Command::CarriageReturn,
            Command::Char(b'A')
        ]
    );
}

#[test]
fn out_of_range_parameters_are_diagnosed_and_do_not_eat_text() {
    for (seq, what) in [
        (vec![0x1B, b'A', 86], "ESC A"),
        (vec![0x1B, b'N', 0], "ESC N"),
        (vec![0x1B, b'Q', 0], "ESC Q"),
        (vec![0x1B, b'R', 13], "ESC R"),
        (vec![0x1B, b'-', 2], "ESC -"),
        (vec![0x1B, b'C', 0, 23], "ESC C 0"),
        (vec![0x1B, b'$', 0, 128], "ESC $"),
    ] {
        let mut s = seq.clone();
        s.push(b'Z');
        let (o, d) = parse(&s);
        assert_eq!(d.len(), 1, "{what}");
        assert_eq!(o.last().unwrap().command, Command::Char(b'Z'), "{what}");
    }
}

#[test]
fn graphics_limit_drops_only_the_header() {
    let mut p = Parser::new(ParserLimits {
        max_graphics_columns: 10,
    });
    let (mut o, mut d) = (Vec::new(), Vec::new());
    p.feed(0, &[0x1B, b'K', 11, 0, b'A'], &mut o, &mut d);
    p.finish(&mut o, &mut d);
    assert_eq!(d.len(), 1);
    assert_eq!(o.last().unwrap().command, Command::Char(b'A'));
}

#[test]
fn esc_star_unknown_mode_is_not_consumed_as_payload() {
    let (o, d) = parse(&[0x1B, b'*', 39, 2, 0, b'A']);
    assert_eq!(d.len(), 1);
    assert_eq!(o.last().unwrap().command, Command::Char(b'A'));
}

#[test]
fn esc_paren_payload_is_skipped_by_declared_length() {
    // ESC ( U 1 0 10 (set unit): unsupported, payload consumed, then text.
    let (o, d) = parse(&[0x1B, b'(', b'U', 1, 0, 10, b'A']);
    assert_eq!(d.len(), 1);
    assert_eq!(o.last().unwrap().command, Command::Char(b'A'));
    // ESC ( t assigns a table.
    let (o, d) = parse(&[0x1B, b'(', b't', 3, 0, 1, 25, 0]);
    assert!(d.is_empty());
    assert_eq!(
        cmds(&o),
        vec![Command::AssignCharTable {
            slot: 1,
            id: 25,
            variant: 0
        }]
    );
}

#[test]
fn offsets_follow_the_feed_offset() {
    let mut p = Parser::default();
    let (mut o, mut d) = (Vec::new(), Vec::new());
    p.feed(100, &[b'A', 0x1B], &mut o, &mut d);
    p.feed(102, b"@", &mut o, &mut d);
    assert_eq!(o[0].offset, 100);
    assert_eq!(o[1].offset, 101);
    assert_eq!(o[1].command, Command::Reset);
}

#[test]
fn control_codes_outside_the_lx310_list_are_diagnosed_not_silent() {
    let (o, d) = parse(&[0x18, 0x7F, 0x11, 0x13, 0x00, 0x07]);
    assert_eq!(d.len(), 4); // CAN, DEL, DC1, DC3; NUL and BEL have no page effect
    assert_eq!(o.len(), 4);
}

#[test]
fn esc_c_and_esc_c_0_parameters() {
    let (o, _) = parse(&[0x1B, b'C', 66, 0x1B, b'C', 0, 11]);
    assert_eq!(
        cmds(&o),
        vec![Command::PageLengthLines(66), Command::PageLengthInches(11)]
    );
}

#[test]
fn line_spacing_commands_normalise_to_216ths() {
    let (o, _) = parse(&[
        0x1B, b'0', 0x1B, b'1', 0x1B, b'2', 0x1B, b'3', 36, 0x1B, b'A', 12,
    ]);
    assert_eq!(
        cmds(&o),
        vec![
            Command::LineSpacing216(27),
            Command::LineSpacing216(21),
            Command::LineSpacing216(36),
            Command::LineSpacing216(36),
            Command::LineSpacing216(36)
        ]
    );
}

#[test]
fn relative_horizontal_is_signed() {
    let (o, _) = parse(&[0x1B, b'\\', 0xF0, 0xFF, 0x1B, b'\\', 0x10, 0x00]);
    assert_eq!(
        cmds(&o),
        vec![
            Command::RelativeHorizontal(-16),
            Command::RelativeHorizontal(16)
        ]
    );
}
