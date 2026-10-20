//! Slice 6: keyboard, speaker and game port through the CPU bus and real code of a [`Machine`].

use apple2_core::speaker::CAPACITY;
use apple2_core::{Machine, MachineConfig, Model, NavKey, VideoStandard};
use apple2_cpu::Bus;

/// Custom ROM of NOPs with RESET=$0300 (12K for the ][ family, 16K for the //e family).
fn custom(model: Model) -> Machine {
    let size = if model.is_apple2() { 0x3000 } else { 0x4000 };
    let mut rom = vec![0xEAu8; size];
    rom[size - 4] = 0x00;
    rom[size - 3] = 0x03;
    let mut cfg = MachineConfig::for_model(model);
    cfg.custom_rom = Some(rom);
    cfg.video_standard = VideoStandard::Ntsc;
    Machine::new(cfg)
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

fn load(m: &mut Machine, at: u16, code: &[u8]) {
    for (i, b) in code.iter().enumerate() {
        m.poke(at + i as u16, *b);
    }
}

// ---- Keyboard ----

#[test]
fn ii_strobe_is_cleared_by_c010_read_write_and_any_c01x_read() {
    let mut m = custom(Model::Apple2Plus);
    assert_eq!(rd(&mut m, 0xC000), 0); // reset: keycode 0, strobe off
    assert!(m.key_press(b'a'));
    assert_eq!(rd(&mut m, 0xC000), 0xC1); // uppercased, strobe set
    assert_eq!(rd(&mut m, 0xC00F), 0xC1); // $C000-$C00F all alias
    rd(&mut m, 0xC010);
    assert_eq!(rd(&mut m, 0xC000), 0x41); // keycode stays, strobe off
    m.key_press(b'B');
    wr(&mut m, 0xC010, 0);
    assert_eq!(rd(&mut m, 0xC000), 0x42);
    m.key_press(b'C');
    rd(&mut m, 0xC01B); // any $C01x read clears on the ][ family
    assert_eq!(rd(&mut m, 0xC000), 0x43);
    wr(&mut m, 0xC01F, 0); // any $C01x write too
    m.key_press(b'D');
    wr(&mut m, 0xC01F, 0);
    assert_eq!(rd(&mut m, 0xC000), 0x44);
}

#[test]
fn ii_family_forces_uppercase_and_rejects_some_chars_unless_clone() {
    let mut m = custom(Model::Apple2);
    m.set_caps_lock(false); // no effect on the ][
    m.key_press(b'z');
    assert_eq!(rd(&mut m, 0xC000), 0xDA);
    for k in [b'`', b'{', b'|', b'}', b'~', 0x7F, 0xE9] {
        wr(&mut m, 0xC010, 0);
        assert!(!m.key_press(k), "{k:#x}");
        assert_eq!(rd(&mut m, 0xC000) & 0x80, 0);
    }
    assert!(m.key_press(b'@') && m.key_press(0x0D));
    assert_eq!(rd(&mut m, 0xC000), 0x8D);
    // A clone accepts them (its keyboard has the keys).
    let mut c = custom(Model::Pravets8M);
    assert!(c.key_press(b'~'));
}

#[test]
fn iie_honours_caps_lock_and_rejects_accents() {
    let mut m = custom(Model::Apple2e);
    m.key_press(b'a');
    assert_eq!(rd(&mut m, 0xC000), 0xC1); // caps lock defaults on
    m.set_caps_lock(false);
    m.key_press(b'a');
    assert_eq!(rd(&mut m, 0xC000), 0xE1);
    assert!(m.key_press(b'`') && m.key_press(b'~'));
    wr(&mut m, 0xC010, 0);
    assert!(!m.key_press(0xE9));
    assert_eq!(rd(&mut m, 0xC000) & 0x80, 0);
}

#[test]
fn iie_c010_returns_any_key_down_and_clears_strobe() {
    let mut m = custom(Model::Apple2e);
    m.key_press(b'a');
    m.set_any_key_down(true);
    assert_eq!(rd(&mut m, 0xC010), 0xC1); // keycode | AKD
    assert_eq!(rd(&mut m, 0xC000), 0x41); // strobe cleared
    m.set_any_key_down(false);
    assert_eq!(rd(&mut m, 0xC010), 0x41);
    wr(&mut m, 0xC010, 0);
}

#[test]
fn iie_c011_to_c01f_keep_keycode_in_low_bits() {
    let mut m = custom(Model::Apple2e);
    m.key_press(b'Q');
    // BANK2 is set at power-on: bit 7 = flag, bits 0-6 = keycode; no strobe side effect.
    assert_eq!(rd(&mut m, 0xC011), 0x80 | 0x51);
    assert_eq!(rd(&mut m, 0xC012), 0x51);
    assert_eq!(rd(&mut m, 0xC000), 0xD1);
}

#[test]
fn reset_clears_keycode_and_strobe() {
    let mut m = custom(Model::Apple2e);
    m.key_press(b'Q');
    m.reset();
    assert_eq!(rd(&mut m, 0xC000), 0);
}

#[test]
fn arrow_keys_per_model() {
    let mut e = custom(Model::Apple2e);
    for (k, code) in [(NavKey::Left, 0x08), (NavKey::Up, 0x0B), (NavKey::Right, 0x15), (NavKey::Down, 0x0A), (NavKey::Delete, 0x7F)] {
        assert!(e.key_press_nav(k));
        assert_eq!(rd(&mut e, 0xC000), 0x80 | code);
    }
    let mut ii = custom(Model::Apple2Plus);
    assert!(!ii.key_press_nav(NavKey::Up) && !ii.key_press_nav(NavKey::Delete));
    assert!(ii.key_press_nav(NavKey::Left) && ii.key_press_nav(NavKey::Right));
    assert_eq!(rd(&mut ii, 0xC000), 0x95);
    let mut b = custom(Model::Base64A);
    assert!(b.key_press_nav(NavKey::Delete));
    assert_eq!(rd(&mut b, 0xC000), 0xFF);
}

#[test]
fn paste_queue_advances_one_char_per_strobe_clear_and_collapses_crlf() {
    let mut m = custom(Model::Apple2Plus);
    assert!(m.paste("A\r\nB\n\u{e9}C"));
    assert!(!m.paste("X"), "already pasting");
    assert_eq!(rd(&mut m, 0xC000), 0xC1);
    assert_eq!(rd(&mut m, 0xC000), 0xC1, "reads do not advance");
    wr(&mut m, 0xC010, 0);
    assert_eq!(rd(&mut m, 0xC000), 0x8D, "CRLF is one CR");
    rd(&mut m, 0xC010); // a ][ read of $C01x clears too
    assert_eq!(rd(&mut m, 0xC000), 0xC2);
    wr(&mut m, 0xC010, 0);
    assert_eq!(rd(&mut m, 0xC000), 0x8A, "lone LF is kept; non-ASCII is dropped");
    wr(&mut m, 0xC010, 0);
    assert_eq!(rd(&mut m, 0xC000), 0xC3);
    wr(&mut m, 0xC010, 0);
    assert_eq!(rd(&mut m, 0xC000), 0, "queue empty: real keyboard state again");
    assert!(m.paste("Z"));
}

#[test]
fn paste_on_iie_c010_read_returns_the_consumed_char() {
    let mut m = custom(Model::Apple2e);
    m.paste("HI");
    assert_eq!(rd(&mut m, 0xC000), 0xC8);
    assert_eq!(rd(&mut m, 0xC010), 0xC8);
    assert_eq!(rd(&mut m, 0xC000), 0xC9);
    assert_eq!(rd(&mut m, 0xC010), 0xC9);
    assert_eq!(rd(&mut m, 0xC010), 0, "queue drained: keycode 0 | no AKD");
}

#[test]
fn pravets82_remaps_at_sign() {
    let mut m = custom(Model::Pravets82);
    assert!(m.key_press(b'@'));
    assert_eq!(rd(&mut m, 0xC000), 0xE0); // 64 -> 96
    m.set_caps_lock(false); // cyrillic: '`' -> '^'
    m.key_press(b'`');
    assert_eq!(rd(&mut m, 0xC000), 0xDE);
}

#[test]
fn tk3000_mode_key_remaps_accented_chars() {
    let mut m = custom(Model::Tk3000e);
    assert!(!m.key_press(0xE1), "accents need the |Mode| key");
    m.set_tk3000_mode(true);
    assert!(m.key_press(0xE1)); // a-acute -> '@'
    assert_eq!(rd(&mut m, 0xC000), 0xC0);
    m.key_press(0xC9); // E-acute: uppercase folded to lowercase, then '`'
    assert_eq!(rd(&mut m, 0xC000), 0xE0);
    wr(&mut m, 0xC010, 0);
    assert!(!m.key_press(0xFF), "unmapped >0x7F is dropped");
    m.key_press(b'a');
    assert_eq!(rd(&mut m, 0xC000), 0xC1); // caps lock on
}

#[test]
fn pravets8a_c060_reports_8bit_keyboard_mode() {
    let mut m = custom(Model::Pravets8A);
    // 7-bit mode (default): bit 7 = 1, bit 6 = 0.
    assert_eq!(rd(&mut m, 0xC060) & 0xC0, 0x80);
    wr(&mut m, 0xC060, 1); // allow 8-bit
    m.set_caps_lock(false);
    m.key_press(b'a');
    assert_eq!(rd(&mut m, 0xC060) & 0x80, 0, "lowercase letter: bit 7 clear");
    m.key_press(0x01); // CTRL-A: special key
    assert_eq!(rd(&mut m, 0xC060) & 0x80, 0x80);
    m.key_press(b'A'); // shifted (caps lock off, uppercase typed)
    assert_eq!(rd(&mut m, 0xC060) & 0x80, 0x80);
    wr(&mut m, 0xC060, 0);
    m.key_press(b'a');
    assert_eq!(rd(&mut m, 0xC060) & 0xC0, 0x80);
    wr(&mut m, 0xC060, 1);
    m.reset(); // Pravets::Reset forbids 8-bit again
    assert_eq!(rd(&mut m, 0xC060) & 0xC0, 0x80);
}

#[test]
fn other_models_c060_reads_floating_bus_with_bit7_set() {
    for model in [Model::Apple2Plus, Model::Apple2e, Model::Pravets82] {
        let mut m = custom(model);
        let fb = m.board().floating_bus() & 0x7F;
        let v = rd(&mut m, 0xC060);
        assert_eq!((v & 0x80, v & 0x7F), (0x80, fb), "{model:?}");
        wr(&mut m, 0xC060, 1);
    }
}

// ---- Speaker ----

#[test]
fn speaker_toggles_record_the_cycle_the_opcode_started() {
    // AppleWin runs the I/O handler before adding the opcode's cycles (cpu6502.h `ABS STA CYC(4)`),
    // so a toggle is stamped with the cycle count at the start of the instruction.
    let mut m = custom(Model::Apple2Plus);
    load(&mut m, 0x300, &[0xAD, 0x30, 0xC0, 0x8D, 0x30, 0xC0, 0x2C, 0x30, 0xC0, 0xEA]);
    let c0 = m.cycles();
    for _ in 0..4 {
        m.step();
    }
    let toggles: Vec<u64> = m.drain_speaker_toggles().collect();
    assert_eq!(toggles, [c0, c0 + 4, c0 + 8]);
    assert_eq!(m.drain_speaker_toggles().count(), 0);
}

#[test]
fn speaker_toggle_on_any_c03x_read_or_write_and_returns_floating_bus() {
    let mut m = custom(Model::Apple2e);
    let fb = m.board().floating_bus();
    assert_eq!(rd(&mut m, 0xC03F), fb);
    wr(&mut m, 0xC035, 0);
    assert_eq!(m.drain_speaker_toggles().count(), 2);
    rd(&mut m, 0xC02F);
    rd(&mut m, 0xC040);
    assert_eq!(m.drain_speaker_toggles().count(), 0);
}

#[test]
fn speaker_log_is_bounded_and_drops_the_oldest() {
    let mut m = custom(Model::Apple2);
    load(&mut m, 0x300, &[0xAD, 0x30, 0xC0, 0x4C, 0x00, 0x03]); // LDA $C030 ; JMP $0300 (7 cycles)
    let c0 = m.cycles();
    for _ in 0..2 * (CAPACITY + 10) {
        m.step();
    }
    assert_eq!(m.speaker_toggles_dropped(), 10);
    let v: Vec<u64> = m.drain_speaker_toggles().collect();
    assert_eq!((v.len(), v[0]), (CAPACITY, c0 + 70));
    assert_eq!(m.speaker_toggles_dropped(), 10, "the counter survives draining");
}

// ---- Game port ----

#[test]
fn buttons_combine_joystick_and_apple_keys() {
    let mut m = custom(Model::Apple2e);
    m.set_joystick_attached(1, true);
    for a in [0xC061, 0xC062, 0xC063] {
        assert_eq!(rd(&mut m, a) & 0x80, 0);
    }
    m.set_button(0, true);
    assert_eq!(rd(&mut m, 0xC061) & 0x80, 0x80);
    m.set_button(0, false);
    m.set_open_apple(true);
    assert_eq!(rd(&mut m, 0xC061) & 0x80, 0x80);
    assert_eq!(rd(&mut m, 0xC062) & 0x80, 0);
    m.set_closed_apple(true);
    assert_eq!(rd(&mut m, 0xC062) & 0x80, 0x80);
    m.set_button(2, true);
    assert_eq!(rd(&mut m, 0xC063) & 0x80, 0x80);
    m.set_button(1, true);
    m.set_closed_apple(false);
    assert_eq!(rd(&mut m, 0xC062) & 0x80, 0x80);
    // $C069-$C06B alias $C061-$C063; the low bits are the floating bus.
    let fb = m.board().floating_bus() & 0x7F;
    assert_eq!(rd(&mut m, 0xC069), 0x80 | fb);
}

#[test]
fn default_is_joystick_0_only() {
    let mut ii = custom(Model::Apple2Plus);
    // PB0/PB1 not pressed with joystick 0; PB2 pressed (no joystick 1).
    assert_eq!([0xC061, 0xC062, 0xC063].map(|a| rd(&mut ii, a) >> 7), [0, 0, 1]);
    ii.set_joystick_attached(0, false);
    assert_eq!([0xC061, 0xC062, 0xC063].map(|a| rd(&mut ii, a) >> 7), [1, 1, 1]);
    ii.set_joystick_attached(1, true);
    assert_eq!(rd(&mut ii, 0xC063) >> 7, 0);
    // //e: PB0/PB1 are only the Apple keys even with no joystick 0.
    let mut e = custom(Model::Apple2e);
    e.set_joystick_attached(0, false);
    assert_eq!([0xC061, 0xC062, 0xC063].map(|a| rd(&mut e, a) >> 7), [0, 0, 1]);
}

/// Runs `LDA $C070`, `delay - 4` cycles of filler, then `LDA $C064,X`; true if paddle X read active.
/// `delay` is the distance in cycles between the starts of the two accesses.
fn paddle_active_after(pos: u8, delay: u32, paddle: u8) -> bool {
    let mut m = custom(Model::Apple2Plus);
    m.set_joystick_attached(paddle as usize >> 1, true);
    m.set_paddle(paddle, pos);
    let mut code = vec![0xAD, 0x70, 0xC0];
    let mut left = delay - 4;
    if left % 2 == 1 {
        code.extend([0x24, 0x00]); // BIT $00: 3 cycles
        left -= 3;
    }
    code.extend(std::iter::repeat_n(0xEA, left as usize / 2));
    code.extend([0xBD, 0x64, 0xC0, 0x4C, 0x00, 0x20]); // LDA $C064,X ; JMP $2000
    load(&mut m, 0x300, &code);
    m.cpu_mut().x = paddle;
    while m.cpu().pc != 0x2000 {
        m.step();
    }
    m.cpu().a & 0x80 != 0
}

#[test]
fn paddle_timer_length_is_exact() {
    // 10 * 2816/255 = 110.43 -> 110 cycles after the strobe's opcode started.
    assert!(paddle_active_after(10, 110, 0));
    assert!(!paddle_active_after(10, 111, 0));
    assert!(!paddle_active_after(0, 4, 0), "position 0: already out at the next opcode");
    // 255 counts as 287 (KEGS): 3169 cycles.
    assert!(paddle_active_after(255, 3169, 1));
    assert!(!paddle_active_after(255, 3170, 1));
    assert!(paddle_active_after(254, 2000, 3) && !paddle_active_after(254, 3000, 3));
}

#[test]
fn strobe_while_a_timer_runs_has_no_effect() {
    let mut m = custom(Model::Apple2Plus);
    m.set_paddle(0, 10); // ends 110 cycles after the first strobe
    let mut code = vec![0xAD, 0x70, 0xC0];
    code.extend(std::iter::repeat_n(0xEA, 48)); // second strobe at +100, still running
    code.extend([0xAD, 0x70, 0xC0]);
    code.extend(std::iter::repeat_n(0xEA, 4)); // read at +112
    code.extend([0xAD, 0x64, 0xC0, 0x4C, 0x00, 0x20]);
    load(&mut m, 0x300, &code);
    while m.cpu().pc != 0x2000 {
        m.step();
    }
    assert_eq!(m.cpu().a & 0x80, 0, "not re-armed");
}

#[test]
fn paddles_always_active_without_joystick() {
    let mut m = custom(Model::Apple2e);
    m.set_joystick_attached(0, false);
    assert_eq!([0, 1, 2, 3].map(|n| rd(&mut m, 0xC064 + n) >> 7), [1, 1, 1, 1]);
}

/// The monitor's PREAD ($FB1E) run from the real ROM: Y counts 11-cycle loops until the timer ends.
fn pread(model: Model, pos: u8, attached: bool) -> u8 {
    let mut m = Machine::new(MachineConfig::for_model(model));
    m.set_joystick_attached(0, attached);
    m.set_paddle(0, pos);
    load(&mut m, 0x300, &[0x20, 0x1E, 0xFB, 0x4C, 0x03, 0x03]); // JSR $FB1E ; JMP $0303
    m.cpu_mut().pc = 0x300;
    m.cpu_mut().x = 0;
    while m.cpu().pc != 0x303 {
        m.step();
    }
    m.cpu().y
}

#[test]
fn monitor_pread_returns_paddle_position() {
    for model in [Model::Apple2Plus, Model::Apple2e, Model::Apple2eEnhanced] {
        for pos in [0u8, 128, 255] {
            let y = pread(model, pos, true);
            assert!(y.abs_diff(pos) <= 2, "{model:?} pos {pos}: Y={y}");
        }
        // No joystick: the timer never ends and PREAD saturates at 255.
        assert_eq!(pread(model, 0, false), 255);
    }
}

// ---- Annunciators, J-Plus, //c ----

#[test]
fn annunciators_follow_c058_to_c05f_on_read_and_write() {
    let mut m = custom(Model::Apple2Plus);
    for n in 0..4u16 {
        rd(&mut m, 0xC059 + 2 * n); // on
    }
    assert_eq!(m.board().annunciators, [true; 4]);
    wr(&mut m, 0xC058, 0);
    rd(&mut m, 0xC05E);
    assert_eq!(m.board().annunciators, [false, true, true, false]);
}

#[test]
fn iie_an3_also_drives_dhires() {
    let mut m = custom(Model::Apple2e);
    rd(&mut m, 0xC05E); // clear AN3 = DHIRES on
    assert!(m.board().video.dhires && !m.board().annunciators[3]);
    rd(&mut m, 0xC05F);
    assert!(!m.board().video.dhires && m.board().annunciators[3]);
}

#[test]
fn jplus_an2_selects_ascii_charset() {
    let mut m = custom(Model::Apple2JPlus);
    assert!(!m.board().video.jplus_ascii);
    rd(&mut m, 0xC05D);
    assert!(m.board().video.jplus_ascii);
    wr(&mut m, 0xC05C, 0);
    assert!(!m.board().video.jplus_ascii);
    rd(&mut m, 0xC05D);
    m.reset(); // the ][ family keeps its switches on reset
    assert!(m.board().video.jplus_ascii);
    let mut p = custom(Model::Apple2Plus);
    rd(&mut p, 0xC05D);
    assert!(!p.board().video.jplus_ascii, "only the J-Plus has the Katakana ROM");
}

#[test]
fn iic_ioudis_gates_an3_and_c07e_c07f_read_back() {
    let mut m = custom(Model::Apple2c);
    rd(&mut m, 0xC05E); // IOUDIS off: IOU does not see C05x annunciators; no ANx lines on the //c
    assert!(!m.board().video.dhires);
    assert_eq!(m.board().annunciators, [false; 4]);
    wr(&mut m, 0xC07E, 0); // IOUDIS on
    assert_eq!(rd(&mut m, 0xC07E) & 0x80, 0x80);
    rd(&mut m, 0xC05E);
    assert!(m.board().video.dhires);
    assert_eq!(rd(&mut m, 0xC07F) & 0x80, 0x80, "DHIRES");
    wr(&mut m, 0xC07F, 0); // IOUDIS off
    assert_eq!(rd(&mut m, 0xC07E) & 0x80, 0);
    let mut e = custom(Model::Apple2e); // not the Enhanced: $C07F is plain floating bus
    assert_eq!(rd(&mut e, 0xC07F), e.board().floating_bus());
    wr(&mut e, 0xC07E, 0);
    assert_eq!(e.memmode() & apple2_core::memory::mf::IOUDIS, 0);
}
