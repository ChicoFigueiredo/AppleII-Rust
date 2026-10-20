//! 4Play, SNES MAX, VidHD, Breakpoint card and the game-connector dongles inside a Machine.

use apple2_cards::breakpoint::{Breakpoint, BPTYPE_MEM, BPTYPE_PC};
use apple2_cards::fourplay::Stick;
use apple2_cards::snesmax::buttons;
use apple2_cards::{BreakpointCard, Dongle, DongleKind, FourPlay, SnesMax, VidHd};
use apple2_core::{Card, Machine, MachineConfig, Model};
use apple2_cpu::Bus;

fn plus() -> Machine {
    Machine::new(MachineConfig::for_model(Model::Apple2Plus))
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

fn with<T: 'static, R>(m: &mut Machine, slot: u8, f: impl FnOnce(&mut T, &mut apple2_core::CardCtx) -> R) -> R {
    m.with_card(slot, |c, ctx| f(c.as_any_mut().unwrap().downcast_mut::<T>().unwrap(), ctx)).unwrap()
}

// ---- 4Play ----

#[test]
fn fourplay_ports_read_each_stick() {
    let mut m = plus();
    assert!(m.insert_card(5, Box::new(FourPlay::new())));
    with::<FourPlay, _>(&mut m, 5, |c, _| {
        c.set_stick(0, Stick { up: true, trigger1: true, ..Default::default() });
        c.set_stick(3, Stick { right: true, ..Default::default() });
    });
    assert_eq!(rd(&mut m, 0xC0D0), 0x81 | 0x20);
    assert_eq!(rd(&mut m, 0xC0D1), 0x20);
    assert_eq!(rd(&mut m, 0xC0D3), 0x08 | 0x20);
    assert_eq!(rd(&mut m, 0xC0D4), m.board().floating_bus(), "ports 4+ float");
}

// ---- SNES MAX ----

fn read_bits(m: &mut Machine) -> Vec<u8> {
    // Latch, then clock 17 times reading $C0n0 before each clock.
    wr(m, 0xC0E0, 0);
    let mut out = vec![];
    for _ in 0..17 {
        out.push(rd(m, 0xC0E0) >> 6);
        wr(m, 0xC0E1, 0);
    }
    out
}

#[test]
fn snesmax_shifts_buttons_out_active_low_with_plugged_bit_last() {
    let mut m = plus();
    assert!(m.insert_card(6, Box::new(SnesMax::new())));
    with::<SnesMax, _>(&mut m, 6, |c, _| {
        c.set_controller(0, Some(buttons::B | buttons::START | buttons::R));
        c.set_controller(1, Some(buttons::UP));
    });
    let bits = read_bits(&mut m);
    // b7 = controller 1, b6 = controller 2 (value >> 6 gives c1<<1 | c2).
    let c1: Vec<u8> = bits.iter().map(|b| (b >> 1) & 1).collect();
    let c2: Vec<u8> = bits.iter().map(|b| b & 1).collect();
    assert_eq!(c1[0], 0, "B pressed (active low)");
    assert_eq!(c1[1], 1, "Y not pressed");
    assert_eq!(c1[3], 0, "Start pressed");
    assert_eq!(c1[7], 1);
    assert_eq!(c1[11], 0, "R pressed");
    assert_eq!(c1[16], 0, "controller plugged in");
    assert_eq!(c2[4], 0, "controller 2: Up");
    assert_eq!(c2[0], 1);
}

#[test]
fn snesmax_unplugged_controller_reads_all_ones() {
    let mut m = plus();
    assert!(m.insert_card(6, Box::new(SnesMax::new())));
    with::<SnesMax, _>(&mut m, 6, |c, _| c.set_controller(0, Some(0)));
    let bits = read_bits(&mut m);
    assert!(bits.iter().all(|b| b & 1 == 1), "controller 2 absent");
    assert_eq!(bits[16] >> 1, 0, "controller 1 present");
}

// ---- VidHD ----

#[test]
fn vidhd_slot_rom_has_the_detection_bytes_and_tracks_iigs_registers() {
    let mut m = plus();
    assert!(m.insert_card(2, Box::new(VidHd::new())));
    assert_eq!([rd(&mut m, 0xC200), rd(&mut m, 0xC201), rd(&mut m, 0xC202)], [0x24, 0xEA, 0x4C]);
    assert!(!with::<VidHd, _>(&mut m, 2, |c, _| c.is_shr()));
    wr(&mut m, 0xC029, 0xC1); // NEWVIDEO: SHR + linear
    wr(&mut m, 0xC034, 0x35);
    wr(&mut m, 0xC035, 0x08);
    wr(&mut m, 0xC022, 0xF0);
    let (shr, border, shadow, screen) = with::<VidHd, _>(&mut m, 2, |c, _| (c.is_shr(), c.border_color(), c.shadow(), c.screen_color()));
    assert_eq!((shr, border, shadow, screen), (true, 5, 8, 0xF0));
    wr(&mut m, 0xC029, 0x41);
    assert!(!with::<VidHd, _>(&mut m, 2, |c, _| c.is_shr()), "both NEWVIDEO b7 and b6 are needed");
    wr(&mut m, 0xC029, 0xC1);
    m.reset();
    assert!(!with::<VidHd, _>(&mut m, 2, |c, _| c.is_shr()), "reset leaves SHR");
}

#[test]
fn vidhd_mirrors_the_memory_switches_on_the_ii() {
    let mut m = plus();
    assert!(m.insert_card(2, Box::new(VidHd::new())));
    assert!(!with::<VidHd, _>(&mut m, 2, |c, _| c.is_write_aux()));
    wr(&mut m, 0xC005, 0); // AUXWRITE on
    assert!(with::<VidHd, _>(&mut m, 2, |c, _| c.is_write_aux()));
    wr(&mut m, 0xC004, 0);
    wr(&mut m, 0xC001, 0); // 80STORE
    rd(&mut m, 0xC055); // PAGE2 (a read is enough)
    assert!(with::<VidHd, _>(&mut m, 2, |c, _| c.is_write_aux()));
    rd(&mut m, 0xC054);
    assert!(!with::<VidHd, _>(&mut m, 2, |c, _| c.is_write_aux()));
}

#[test]
fn vidhd_state_roundtrip() {
    let mut m = plus();
    assert!(m.insert_card(2, Box::new(VidHd::new())));
    wr(&mut m, 0xC029, 0xC0);
    let st = m.save_state();
    wr(&mut m, 0xC029, 0);
    m.load_state(&st).unwrap();
    assert!(with::<VidHd, _>(&mut m, 2, |c, _| c.is_shr()));
    assert!(!VidHd::new().load(&[1]));
}

// ---- Breakpoint card ----

fn push_bp(m: &mut Machine, slot_io: u16, bp: [u8; 6]) {
    for b in bp {
        wr(m, slot_io, b);
    }
}

#[test]
fn breakpoint_card_matches_reported_hits_against_the_fifo() {
    let mut m = plus();
    assert!(m.insert_card(7, Box::new(BreakpointCard::new())));
    assert_eq!(rd(&mut m, 0xC0F0), 1, "empty");
    rd(&mut m, 0xC0F2); // intercept by card
    assert!(with::<BreakpointCard, _>(&mut m, 7, |c, _| c.intercepting()));
    push_bp(&mut m, 0xC0F4, [BPTYPE_PC, 0x34, 0x12, 0, 0, 0]); // PC bp at $1234, read
    assert_eq!(rd(&mut m, 0xC0F0), 0, "not empty");
    with::<BreakpointCard, _>(&mut m, 7, |c, ctx| c.report(Breakpoint { kind: BPTYPE_PC, addr_start: 0x1234, addr_end: 0, access: 0 }, ctx));
    assert!(!m.board().interrupts.irq_line(), "deferred by a cycle");
    m.run_cycles(10);
    assert!(m.board().interrupts.irq_line());
    assert_eq!(rd(&mut m, 0xC0F0), 0x80 | 1, "match, FIFO empty again");
    assert!(!m.board().interrupts.irq_line(), "reading clears the IRQ");
}

#[test]
fn breakpoint_card_flags_mismatch_and_unexpected_hits() {
    let mut m = plus();
    assert!(m.insert_card(7, Box::new(BreakpointCard::new())));
    push_bp(&mut m, 0xC0F4, [BPTYPE_MEM, 0x00, 0x03, 0, 0, 1]);
    with::<BreakpointCard, _>(&mut m, 7, |c, ctx| c.report(Breakpoint { kind: BPTYPE_MEM, addr_start: 0x0300, addr_end: 0, access: 0 }, ctx));
    m.run_cycles(10);
    assert_eq!(rd(&mut m, 0xC0F0) & 0xC0, 0x40, "wrong access: mismatch");
    // FIFO is empty: any further hit is a mismatch.
    with::<BreakpointCard, _>(&mut m, 7, |c, ctx| c.report(Breakpoint { kind: BPTYPE_PC, addr_start: 1, addr_end: 0, access: 0 }, ctx));
    m.run_cycles(10);
    assert_eq!(rd(&mut m, 0xC0F0) & 0xC0, 0x40);
    rd(&mut m, 0xC0F1);
    assert_eq!(rd(&mut m, 0xC0F0), 1, "reset flushes the status");
}

#[test]
fn breakpoint_fifo_reports_full_after_32_entries() {
    let mut m = plus();
    assert!(m.insert_card(7, Box::new(BreakpointCard::new())));
    for i in 0..32 {
        push_bp(&mut m, 0xC0F4, [BPTYPE_PC, i, 0, 0, 0, 0]);
    }
    assert_eq!(rd(&mut m, 0xC0F0), 2);
    let st = m.save_state();
    rd(&mut m, 0xC0F1);
    m.load_state(&st).unwrap();
    assert_eq!(rd(&mut m, 0xC0F0), 2);
    assert_eq!(rd(&mut m, 0xC0F7), 7, "ID byte");
}

// ---- Dongles ----

fn pb(m: &mut Machine, n: u16) -> bool {
    rd(m, 0xC061 + n) & 0x80 != 0
}

#[test]
fn speedstar_dongle_drives_pb2_from_the_annunciators() {
    let mut m = plus();
    m.set_joystick_attached(1, true); // a real PB2 would read 0
    m.set_game_io(Some(Box::new(Dongle::new(DongleKind::SdsSpeedStar))));
    assert!(pb(&mut m, 2), "AN0 off: high");
    rd(&mut m, 0xC059); // AN0 on
    rd(&mut m, 0xC05B); // AN1 on
    assert!(pb(&mut m, 2), "AN2 still off");
    rd(&mut m, 0xC05D); // AN2 on
    assert!(!pb(&mut m, 2), "NAND output low");
}

#[test]
fn codewriter_key_sequence_through_annunciator_writes() {
    let mut m = plus();
    m.set_game_io(Some(Box::new(Dongle::new(DongleKind::CodeWriter))));
    let mut bits = vec![];
    for _ in 0..8 {
        bits.push(pb(&mut m, 2) as u8);
        rd(&mut m, 0xC05D); // AN2 on
        rd(&mut m, 0xC05C); // AN2 off: clocks the LFSR
    }
    // LFSR 0x6B (1101011), bit0 out, feedback bit0^bit1 into bit 6.
    let mut l = 0x6Bu8;
    let want: Vec<u8> = (0..8)
        .map(|_| {
            let o = l & 1;
            l = (l >> 1) | (((l >> 1) ^ l) & 1) << 6;
            o
        })
        .collect();
    assert_eq!(bits, want);
    rd(&mut m, 0xC05F); // AN3 on resets it
    assert!(pb(&mut m, 2), "initial LFSR bit 0 is 1");
}

#[test]
fn hayden_key_forces_pb1_low_even_without_a_joystick() {
    let mut m = plus();
    m.set_joystick_attached(0, false); // ][ family: PB1 would float high
    assert!(pb(&mut m, 1));
    m.set_game_io(Some(Box::new(Dongle::new(DongleKind::HaydenCompiler))));
    assert!(!pb(&mut m, 1));
}

/// Cycles a paddle reads active after a `$C070` strobe.
fn timeout(m: &mut Machine, pdl: u16) -> u32 {
    for (i, b) in [0x4C, 0x00, 0x03].iter().enumerate() {
        m.poke(0x300 + i as u16, *b); // JMP $0300: 3 cycles per step
    }
    m.cpu_mut().pc = 0x300;
    rd(m, 0xC070);
    let mut n = 0;
    while rd(m, 0xC064 + pdl) & 0x80 != 0 {
        m.step();
        n += 3;
        assert!(n < 10_000);
    }
    n
}

#[test]
fn robocom_module_sets_the_pdl3_timer_from_the_annunciators() {
    let mut m = plus();
    m.set_joystick_attached(1, true);
    m.set_game_io(Some(Box::new(Dongle::new(DongleKind::Robocom1000))));
    let t0 = timeout(&mut m, 3);
    let want = (34.0_f64 * 2816.0 / 255.0) as u32;
    assert!(t0.abs_diff(want) <= 4, "mode 0: {t0} vs {want}");
    rd(&mut m, 0xC059); // AN0 on: mode 1 -> 151
    let t1 = timeout(&mut m, 3);
    let want = (151.0_f64 * 2816.0 / 255.0) as u32;
    assert!(t1.abs_diff(want) <= 4, "mode 1: {t1} vs {want}");
    rd(&mut m, 0xC05F); // AN3 on: module off, joystick 1 (centre 127) takes over
    let t2 = timeout(&mut m, 3);
    let want = (127.0_f64 * 2816.0 / 255.0) as u32;
    assert!(t2.abs_diff(want) <= 4, "unpowered: {t2} vs {want}");
}

#[test]
fn dongle_state_survives_save_load() {
    let mut m = plus();
    m.set_game_io(Some(Box::new(Dongle::new(DongleKind::CodeWriter))));
    rd(&mut m, 0xC05D);
    rd(&mut m, 0xC05C);
    let st = m.save_state();
    let l = m.game_io_mut().unwrap().save();
    rd(&mut m, 0xC05D);
    rd(&mut m, 0xC05C);
    assert_ne!(m.game_io_mut().unwrap().save(), l);
    m.load_state(&st).unwrap();
    assert_eq!(m.game_io_mut().unwrap().save(), l);
}

#[test]
fn stateless_card_save_load_is_a_noop() {
    let mut c = FourPlay::new();
    assert!(c.load(&c.save()));
}

#[test]
fn vidhd_on_the_ii_dual_writes_ram_to_aux_while_auxwrite_is_on() {
    // Memory.cpp memVidHD / cpu_general.inl:112 (GH#997).
    let mut m = plus();
    assert!(m.insert_card(2, Box::new(VidHd::new())));
    wr(&mut m, 0x2000, 0x11);
    assert_eq!(m.peek_aux(0x2000), 0, "AUXWRITE off: main only");
    wr(&mut m, 0xC005, 0); // AUXWRITE on
    wr(&mut m, 0x2001, 0x5A);
    assert_eq!((m.peek_main(0x2001), m.peek_aux(0x2001)), (0x5A, 0x5A));
    wr(&mut m, 0xC004, 0); // off
    wr(&mut m, 0x2002, 0x77);
    assert_eq!((m.peek_main(0x2002), m.peek_aux(0x2002)), (0x77, 0));
    // 80STORE + PAGE2 behaves like AUXWRITE (VidHD.cpp:84)
    wr(&mut m, 0xC001, 0);
    rd(&mut m, 0xC055);
    wr(&mut m, 0x2003, 0x33);
    assert_eq!(m.peek_aux(0x2003), 0x33);
}

#[test]
fn without_a_vidhd_the_ii_has_no_aux() {
    let mut m = plus();
    wr(&mut m, 0xC005, 0);
    wr(&mut m, 0x2001, 0x5A);
    assert_eq!(m.peek_aux(0x2001), 0);
}

#[test]
fn vidhd_shr_renders_through_the_video_crate() {
    use apple2_video::{RenderConfig, Renderer, VideoType, WithVidHd};
    let mut m = plus();
    assert!(m.insert_card(2, Box::new(VidHd::new())));
    wr(&mut m, 0xC005, 0); // AUXWRITE: CPU writes land in aux too
    wr(&mut m, 0x2000, 0x12); // line 0: pixels 1,1,2,2 then index 0
    for (a, v) in [(0x9E02, 0x00), (0x9E03, 0x0F), (0x9E04, 0xF0), (0x9E05, 0x00)] {
        wr(&mut m, a, v); // palette 0: entry 1 = $0F00 (red), entry 2 = $00F0 (green)
    }
    wr(&mut m, 0xC029, 0xC1); // NEWVIDEO: SHR
    let mut r = Renderer::new(RenderConfig { video_type: VideoType::MonoWhite, ..Default::default() });
    let card = m.card::<VidHd>(2).unwrap();
    let src = WithVidHd { source: &m, vidhd: card };
    r.redraw(&src);
    assert_eq!(r.pixel(0, 0), [0xF0, 0, 0, 255]);
    assert_eq!(r.pixel(2, 1), [0, 0xF0, 0, 255]);
    assert_eq!(r.pixel(4, 0), [0, 0, 0, 255]);
    assert_eq!(r.size(), (680, 436));
}
