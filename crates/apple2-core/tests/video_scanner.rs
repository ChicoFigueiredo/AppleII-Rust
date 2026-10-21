//! Video scanner and floating bus through public seams: the `VideoScanner` tables against the
//! hand-written AppleWin NTSC.cpp tables, goldens at known (vert, horz), and real code run on a
//! `Machine` reading the floating bus and $C019 at exact cycles.

mod scanner_ref;

use apple2_core::video_scanner::{Switches, VideoScanner, CYCLES_PER_LINE};
use apple2_core::{AuxCardKind, Machine, MachineConfig, Model, VideoStandard};
use scanner_ref::{HORZ_E, HORZ_II, VERT_HGR, VERT_TXT};

const TEXT: Switches = Switches {
    text: true,
    mixed: false,
    hires: false,
    page2: false,
};
const HGR: Switches = Switches {
    text: false,
    mixed: false,
    hires: true,
    page2: false,
};

fn ntsc(ii: bool) -> VideoScanner {
    VideoScanner::new(VideoStandard::Ntsc, ii)
}

#[test]
fn tables_match_applewin_hand_written_tables_for_a_whole_ntsc_frame() {
    for ii in [false, true] {
        let s = ntsc(ii);
        let horz = if ii { &HORZ_II } else { &HORZ_E };
        for v in 0..262usize {
            for h in 0..65usize {
                let text = VERT_TXT[v / 8] + horz[(v / 64) * 65 + h] + 0x400;
                assert_eq!(
                    s.address_at(v as u32, h as u32, TEXT),
                    text,
                    "text ii={ii} v={v} h={h}"
                );
                let hgr = VERT_HGR[v] + HORZ_E[(v / 64) * 65 + h] + 0x2000;
                assert_eq!(
                    s.address_at(v as u32, h as u32, HGR),
                    hgr,
                    "hgr ii={ii} v={v} h={h}"
                );
            }
        }
    }
}

#[test]
fn golden_addresses_ntsc() {
    let e = ntsc(false);
    let at = |s: &VideoScanner, v, h, sw| s.address_at(v, h, sw);
    // TEXT: row 12 of the page is line 100: (12 % 8) * $80 + $28 (second third) + (29 - 25).
    assert_eq!(at(&e, 100, 29, TEXT), 0x062C);
    assert_eq!(
        at(
            &e,
            100,
            29,
            Switches {
                page2: true,
                ..TEXT
            }
        ),
        0x0A2C
    );
    // LORES uses the text addressing.
    assert_eq!(
        at(
            &e,
            100,
            29,
            Switches {
                text: false,
                ..TEXT
            }
        ),
        0x062C
    );
    // HIRES line 100 (36 within the second third): $1200 + $28 + 4.
    assert_eq!(at(&e, 100, 29, HGR), 0x322C);
    assert_eq!(at(&e, 100, 29, Switches { page2: true, ..HGR }), 0x522C);
    // MIXED: HGR up to line 159, text from line 160.
    let mixed = Switches { mixed: true, ..HGR };
    assert_eq!(at(&e, 159, 25, mixed), 0x3DD0);
    assert_eq!(at(&e, 160, 25, mixed), 0x0650);
    // Last visible HGR byte of the frame.
    assert_eq!(at(&e, 191, 64, HGR), 0x3FF7);
    // Vertical blank keeps counting rows (row 24 onwards): line 200 = row 25.
    assert_eq!(at(&e, 200, 29, TEXT), 0x04FC);
}

#[test]
fn horizontal_blank_adds_1000_on_the_ii_in_text_only() {
    let (ii, e) = (ntsc(true), ntsc(false));
    assert_eq!(ii.address_at(10, 5, TEXT), 0x14EC);
    assert_eq!(e.address_at(10, 5, TEXT), 0x04EC);
    // Visible columns are not affected.
    assert_eq!(ii.address_at(10, 30, TEXT), e.address_at(10, 30, TEXT));
    // HGR never gets it (NTSC.cpp:883).
    assert_eq!(ii.address_at(10, 5, HGR), e.address_at(10, 5, HGR));
    // Mixed text window on the ][ in HBL does.
    let mixed = Switches { mixed: true, ..HGR };
    assert_eq!(ii.address_at(170, 5, mixed) & 0x1000, 0x1000);
    assert_eq!(ii.address_at(100, 5, mixed), e.address_at(100, 5, mixed));
}

#[test]
fn pal_has_312_lines_with_their_own_vertical_state() {
    let p = VideoScanner::new(VideoStandard::Pal, false);
    assert_eq!(p.cycles_per_frame(), 20280);
    // Lines up to 255 are the same as NTSC.
    assert_eq!(p.address_at(100, 29, TEXT), 0x062C);
    // After the V state preset at line 256 (V = $100 + line - 312). Line 261: V = $CD, so
    // v0 = 1 ($80) and the sum for h = 64 is 3: $400 + $80 + $18 + 7. Line 300: V = $F4.
    assert_eq!(p.address_at(261, 64, TEXT), 0x049F);
    assert_eq!(p.address_at(300, 29, TEXT), 0x077C);
    assert_eq!(p.address_at(311, 29, TEXT), 0x07FC);
    // PAL VBL starts at line 192 as well.
    assert!(p.vbl_bar(191 * 65 + 64) && !p.vbl_bar(192 * 65));
}

#[test]
fn adjust_steps_back_one_clock_across_lines_and_the_frame() {
    let s = ntsc(false);
    assert_eq!(s.address(100 * 65 + 30, TEXT), s.address_at(100, 29, TEXT));
    assert_eq!(s.address(100 * 65, TEXT), s.address_at(99, 64, TEXT));
    assert_eq!(s.address(0, TEXT), s.address_at(261, 64, TEXT));
}

#[test]
fn vbl_bar_is_set_for_exactly_192_lines_of_every_frame() {
    for (std, frame) in [(VideoStandard::Ntsc, 17030u32), (VideoStandard::Pal, 20280)] {
        let s = VideoScanner::new(std, false);
        assert_eq!(s.cycles_per_frame(), frame);
        assert_eq!(
            (0..frame).filter(|&c| s.vbl_bar(c)).count() as u32,
            192 * CYCLES_PER_LINE
        );
        assert!(
            s.vbl_bar(192 * 65 - 1)
                && !s.vbl_bar(192 * 65)
                && !s.vbl_bar(frame - 1)
                && s.vbl_bar(0)
        );
    }
}

// ---- Through the Machine: real code, exact cycles. ----

fn machine(model: Model, std: VideoStandard) -> Machine {
    let mut cfg = MachineConfig::for_model(model);
    cfg.video_standard = std;
    if model.is_apple2e_family() {
        cfg.aux_card = AuxCardKind::Extended80Col;
    }
    Machine::new(cfg)
}

/// Runs a NOP sled in RAM so that `LDA abs` starts at cycle `target` of the frame, executes it
/// and returns A. The sled is outside the text/hires pages.
fn lda_at(m: &mut Machine, target: u32, abs: u16) -> u8 {
    let frame = if m.config().video_standard == VideoStandard::Ntsc {
        17030
    } else {
        20280
    };
    let target = target % frame;
    let cur = m.cycles_in_frame();
    let mut d = if target >= cur + 3 {
        target - cur
    } else {
        target + frame - cur
    };
    let mut at = 0x6000u16;
    for a in 0x6000..0xA000u16 {
        m.poke_main(a, 0xEA);
    }
    if d % 2 == 1 {
        m.poke_main(at, 0xA5); // LDA $00: 3 cycles to fix the parity
        m.poke_main(at + 1, 0x00);
        at += 2;
        d -= 3;
    }
    let lda = at + (d / 2) as u16;
    m.poke_main(lda, 0xAD);
    m.poke_main(lda + 1, abs as u8);
    m.poke_main(lda + 2, (abs >> 8) as u8);
    m.cpu_mut().pc = 0x6000;
    while m.cpu().pc != lda {
        m.step();
    }
    assert_eq!(m.cycles_in_frame(), target, "positioned");
    m.step();
    m.cpu().a
}

/// Cycle of the frame at which the scanner address for (vert, horz) is fetched: the access sees
/// one clock before the instruction start.
fn start_for(vert: u32, horz: u32) -> u32 {
    vert * 65 + horz + 1
}

fn bus_write(m: &mut Machine, a: u16) {
    use apple2_cpu::Bus;
    m.board_mut().write(a, 0);
}

#[test]
fn floating_bus_reads_the_text_byte_at_the_scanner_address() {
    let mut m = machine(Model::Apple2Plus, VideoStandard::Ntsc);
    m.poke_main(0x062C, 0xA7);
    // $C0E0: slot 6 I/O, nobody home.
    assert_eq!(lda_at(&mut m, start_for(100, 29), 0xC0E0), 0xA7);
    assert_eq!(lda_at(&mut m, start_for(100, 30), 0xC0E0), 0);
}

#[test]
fn floating_bus_follows_the_scanner_at_instruction_start_not_at_the_bus_cycle() {
    // LDA abs reads on its 4th cycle, but AppleWin advances the scanner per opcode, so the byte
    // is the one at the instruction's start (minus one clock): neighbours 3 clocks later differ.
    let mut m = machine(Model::Apple2Plus, VideoStandard::Ntsc);
    m.poke_main(0x062C, 0x11);
    m.poke_main(0x062F, 0x22);
    assert_eq!(lda_at(&mut m, start_for(100, 29), 0xC0E0), 0x11);
    assert_ne!(lda_at(&mut m, start_for(100, 29), 0xC0E0), 0x22);
}

#[test]
fn floating_bus_follows_page2_hires_mixed_and_80store() {
    let mut m = machine(Model::Apple2e, VideoStandard::Ntsc);
    for (a, v) in [
        (0x062C, 1),
        (0x0A2C, 2),
        (0x322C, 3),
        (0x522C, 4),
        (0x3DD0, 5),
        (0x0650, 6),
    ] {
        m.poke_main(a, v);
    }
    let read = |m: &mut Machine, v, h| lda_at(m, start_for(v, h), 0xC040);
    assert_eq!(read(&mut m, 100, 29), 1); // TEXT page 1
    bus_write(&mut m, 0xC055); // PAGE2
    assert_eq!(read(&mut m, 100, 29), 2);
    // 80STORE: PAGE2 no longer shows page 2. AppleWin indexes the CPU read image (`mem`, see
    // Memory.cpp:2460 and AUD-C-050), where 80STORE+PAGE2 maps the AUX text page: aux byte.
    m.poke_aux(0x062C, 7);
    bus_write(&mut m, 0xC001);
    assert_eq!(read(&mut m, 100, 29), 7);
    bus_write(&mut m, 0xC000);
    bus_write(&mut m, 0xC054);
    bus_write(&mut m, 0xC050); // graphics (LORES)
    assert_eq!(read(&mut m, 100, 29), 1);
    bus_write(&mut m, 0xC057); // HIRES
    assert_eq!(read(&mut m, 100, 29), 3);
    bus_write(&mut m, 0xC055);
    assert_eq!(read(&mut m, 100, 29), 4);
    bus_write(&mut m, 0xC054);
    bus_write(&mut m, 0xC053); // MIXED
    assert_eq!(read(&mut m, 159, 25), 5);
    assert_eq!(read(&mut m, 160, 25), 6);
}

#[test]
fn floating_bus_in_the_ii_hbl_reads_plus_1000() {
    let mut m = machine(Model::Apple2, VideoStandard::Ntsc);
    m.poke_main(0x14EC, 0x5A);
    m.poke_main(0x04EC, 0xA5);
    assert_eq!(lda_at(&mut m, start_for(10, 5), 0xC0E0), 0x5A);
    let mut e = machine(Model::Apple2e, VideoStandard::Ntsc);
    e.poke_main(0x14EC, 0x5A);
    e.poke_main(0x04EC, 0xA5);
    assert_eq!(lda_at(&mut e, start_for(10, 5), 0xC040), 0xA5);
}

#[test]
fn floating_bus_in_vbl_pal_and_across_the_frame_boundary() {
    let mut m = machine(Model::Apple2Plus, VideoStandard::Ntsc);
    m.poke_main(0x04FC, 0x77);
    m.poke_main(0x079F, 0x88);
    assert_eq!(lda_at(&mut m, start_for(200, 29), 0xC0E0), 0x77);
    assert_eq!(
        lda_at(&mut m, 0, 0xC0E0),
        0x88,
        "cycle 0 sees the last clock of the previous frame"
    );
    let mut p = machine(Model::Apple2Plus, VideoStandard::Pal);
    p.poke_main(0x077C, 0x99);
    assert_eq!(lda_at(&mut p, start_for(300, 29), 0xC0E0), 0x99);
}

#[test]
fn c019_flips_exactly_at_line_192() {
    let mut m = machine(Model::Apple2e, VideoStandard::Ntsc);
    let vbl = 192 * 65;
    assert_eq!(lda_at(&mut m, vbl - 1, 0xC019), 0x80);
    assert_eq!(lda_at(&mut m, vbl, 0xC019), 0x00);
    // The read is on the 4th cycle but the scanner is the opcode's start: this one began on line 191.
    assert_eq!(lda_at(&mut m, vbl - 1, 0xC019) & 0x80, 0x80);
    assert_eq!(lda_at(&mut m, 17029, 0xC019), 0x00);
    assert_eq!(lda_at(&mut m, 17030, 0xC019), 0x80, "next frame, line 0");
}

#[test]
fn ii_c01x_read_returns_the_floating_bus() {
    let mut m = machine(Model::Apple2Plus, VideoStandard::Ntsc);
    m.poke_main(0x062C, 0x3C);
    assert_eq!(lda_at(&mut m, start_for(100, 29), 0xC010), 0x3C);
}
