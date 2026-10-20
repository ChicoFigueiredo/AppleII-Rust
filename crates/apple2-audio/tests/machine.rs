//! The audio crate through its public seams: real 6502 code on a [`Machine`] driving the speaker
//! and the Mockingboard family, and the resulting PCM from the [`Mixer`].

use apple2_audio::ssi263::SsiType;
use apple2_audio::{AudioBus, MockingboardCard, Mixer, SamCard, Sockets, Variant};
use apple2_core::{Machine, MachineConfig, Model, VideoStandard};

const RATE: u32 = 48_000;

/// ][+ with a 12K custom ROM: RESET=$0300, IRQ=$0400, NMI=$0500.
fn machine() -> Machine {
    let mut rom = vec![0xEAu8; 0x3000];
    for (vec, target) in [(0xFFFAu16, 0x0500u16), (0xFFFC, 0x0300), (0xFFFE, 0x0400)] {
        let o = vec as usize - 0xD000;
        rom[o] = target as u8;
        rom[o + 1] = (target >> 8) as u8;
    }
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.video_standard = VideoStandard::Ntsc;
    cfg.custom_rom = Some(rom);
    Machine::new(cfg)
}

fn load(m: &mut Machine, addr: u16, code: &[u8]) {
    for (i, b) in code.iter().enumerate() {
        m.poke(addr + i as u16, *b);
    }
}

/// `LDA #v : STA $addr`
fn sta(v: u8, addr: u16) -> [u8; 5] {
    [0xA9, v, 0x8D, addr as u8, (addr >> 8) as u8]
}

fn spin_at(addr: u16) -> [u8; 3] {
    [0x4C, addr as u8, (addr >> 8) as u8]
}

fn setup(variant: Variant) -> (Machine, AudioBus) {
    let bus = AudioBus::new(RATE, VideoStandard::Ntsc);
    let mut m = machine();
    assert!(m.insert_card(4, Box::new(MockingboardCard::new(variant, &bus))));
    (m, bus)
}

/// Runs until the CPU first reaches `pc` (the program's `JMP *` spin).
fn run_to(m: &mut Machine, pc: u16) {
    for _ in 0..100_000 {
        if m.cpu().pc == pc {
            return;
        }
        m.step();
    }
    panic!("never reached {pc:#x}");
}

fn crossings(samples: impl Iterator<Item = f32>) -> usize {
    let (mut last, mut n) = (0i8, 0);
    for s in samples.filter(|s| s.abs() > 0.001) {
        let sign = if s > 0.0 { 1 } else { -1 };
        if last != 0 && sign != last {
            n += 1;
        }
        last = sign;
    }
    n
}

// ---- speaker ----

#[test]
fn speaker_toggle_loop_on_a_machine_is_a_square_wave_of_the_right_pitch() {
    let bus = AudioBus::new(RATE, VideoStandard::Ntsc);
    let mut m = machine();
    // LDA $C030 (4) ; LDX #100 (2) ; DEX (2) ; BNE (3/2) ; JMP (3): one toggle every 508 cycles.
    load(&mut m, 0x300, &[0xAD, 0x30, 0xC0, 0xA2, 100, 0xCA, 0xD0, 0xFD, 0x4C, 0x00, 0x03]);
    let mut mixer = Mixer::new(&bus);
    let mut all = vec![];
    for _ in 0..30 {
        m.run_cycles(17_030);
        all.extend(mixer.produce_machine(&mut m));
    }
    let cyc = m.cycles() as f64;
    let secs = cyc / 1_020_484.45;
    let left: Vec<f32> = all.chunks(2).map(|c| c[0]).collect();
    assert_eq!(all.len() % 2, 0);
    assert!((left.len() as f64 - secs * RATE as f64).abs() < 2.0, "{} frames for {secs}s", left.len());
    let edges = crossings(left.iter().copied()) as f64;
    // Toggle period 508 cycles, one edge per toggle.
    let expected = cyc / 508.0;
    assert!((edges - expected).abs() < 6.0, "{edges} vs {expected}");
    assert!(left.iter().cloned().fold(0.0, f32::max) > 0.9);
    // Right channel carries the same speaker.
    assert_eq!(all[1], all[0]);
}

#[test]
fn speaker_goes_quiet_after_a_stuck_cone_and_volume_scales_it() {
    let bus = AudioBus::new(RATE, VideoStandard::Ntsc);
    let mut m = machine();
    load(&mut m, 0x300, &[0xAD, 0x30, 0xC0, 0x4C, 0x03, 0x03]); // one toggle, then spin
    let mut mixer = Mixer::new(&bus);
    let out = {
        m.run_cycles(1_200_000);
        mixer.produce_machine(&mut m)
    };
    let at = |t: f64| out[(t * RATE as f64) as usize * 2].abs();
    assert!(at(0.05) > 0.99);
    assert!(at(0.6) > 0.1 && at(0.6) < 0.9); // fading (DC filter)
    assert_eq!(at(1.1), 0.0);
    // Half volume (-6 dB-ish via linear gain) on a second machine run.
    let mut mixer2 = Mixer::new(&bus);
    mixer2.set_speaker_gain(0.5);
    let mut m2 = machine();
    load(&mut m2, 0x300, &[0xAD, 0x30, 0xC0, 0x4C, 0x03, 0x03]);
    m2.run_cycles(100_000);
    let o2 = mixer2.produce_machine(&mut m2);
    assert!((o2[(0.05 * RATE as f64) as usize * 2].abs() - 0.5).abs() < 0.01);
}

#[test]
fn sam_dac_writes_drive_the_cone() {
    let bus = AudioBus::new(RATE, VideoStandard::Ntsc);
    let mut m = machine();
    assert!(m.insert_card(5, Box::new(SamCard::new(&bus))));
    // $C0D0 is slot 5's I/O space.
    let mut code = vec![];
    code.extend(sta(0xFF, 0xC0D0));
    code.extend(spin_at(0x305));
    load(&mut m, 0x300, &code);
    let mut mixer = Mixer::new(&bus);
    m.run_cycles(20_000);
    let out = mixer.produce_machine(&mut m);
    let late = out[out.len() - 2];
    assert!((late - 127.0 / 128.0).abs() < 0.01, "{late}");
}

// ---- 6522 ----

#[test]
fn t1_counter_read_right_after_the_write_matches_appleworks_gh701() {
    // GH#701: T1C := $FFFF, then LDA T1C_L (4 cycles) must read $FC.
    let (mut m, _bus) = setup(Variant::MockingboardC);
    let mut code = vec![0xA9, 0xFF, 0x8D, 0x05, 0xC4]; // LDA #$FF : STA T1CH (latch is $FFFF)
    code.extend([0xAD, 0x04, 0xC4, 0x85, 0x10]); // LDA T1CL : STA $10
    code.extend(spin_at(0x30A));
    load(&mut m, 0x300, &code);
    run_to(&mut m, 0x30A);
    assert_eq!(m.peek(0x10), 0xFC);
}

#[test]
fn typical_mockingboard_detection_succeeds_in_the_card_slot_only() {
    // The classic check: start T1 and read the low counter twice; the second read must be lower
    // by the cycles between the two read cycles (LDA abs + STA zp = 7).
    let detect = |m: &mut Machine, page: u8| {
        let mut code = vec![0xA9, 0xFF, 0x8D, 0x05, page]; // T1CH := $FF
        code.extend([0xAD, 0x04, page, 0x85, 0x10, 0xAD, 0x04, page, 0x85, 0x11]);
        code.extend([0xAD, 0x05, page, 0x85, 0x12]); // high byte still $FF
        code.extend(spin_at(0x310));
        load(m, 0x300, &code);
        m.cpu_mut().pc = 0x300;
        run_to(m, 0x310);
        (m.peek(0x10).wrapping_sub(m.peek(0x11)), m.peek(0x12))
    };
    let (mut m, _bus) = setup(Variant::MockingboardC);
    assert_eq!(detect(&mut m, 0xC4), (7, 0xFF));
    // The second 6522 (+$80) detects as well; an empty slot does not.
    let (mut m, _bus) = setup(Variant::MockingboardC);
    let mut code = vec![0xA9, 0xFF, 0x8D, 0x85, 0xC4, 0xAD, 0x84, 0xC4, 0x85, 0x10, 0xAD, 0x84, 0xC4, 0x85, 0x11];
    code.extend(spin_at(0x30F));
    load(&mut m, 0x300, &code);
    run_to(&mut m, 0x30F);
    assert_eq!(m.peek(0x10).wrapping_sub(m.peek(0x11)), 7);
    let (mut m, _bus) = setup(Variant::MockingboardC);
    let (diff, _) = detect(&mut m, 0xC6); // nothing in slot 6
    assert_ne!(diff, 7);
}

/// Free-running T1 at latch `n` with an IRQ handler that counts and acknowledges.
fn irq_machine(n: u16) -> (Machine, AudioBus) {
    let (mut m, bus) = setup(Variant::MockingboardC);
    let mut code = vec![0x58]; // CLI
    code.extend(sta(0x40, 0xC40B)); // ACR: free-run
    code.extend(sta(0xC0, 0xC40E)); // IER: T1
    code.extend(sta(n as u8, 0xC404));
    code.extend(sta((n >> 8) as u8, 0xC405));
    code.extend(spin_at(0x300 + code.len() as u16));
    load(&mut m, 0x300, &code);
    load(&mut m, 0x400, &[0xE6, 0x10, 0xAD, 0x04, 0xC4, 0x40]); // INC $10 : LDA T1CL : RTI
    (m, bus)
}

#[test]
fn free_running_t1_interrupts_every_n_plus_two_cycles() {
    let (mut m, _bus) = irq_machine(0x03FF);
    let mut entries = vec![];
    while m.cycles() < 150_000 {
        let before = m.cpu().pc;
        m.step();
        if m.cpu().pc == 0x400 && before != 0x400 {
            entries.push(m.cycles());
        }
    }
    assert!(entries.len() >= 140, "{}", entries.len());
    let diffs: Vec<i64> = entries.windows(2).map(|w| (w[1] - w[0]) as i64).collect();
    // The event fires at an opcode boundary (JMP = 3 cycles): jitter < 3, mean exactly 1025.
    assert!(diffs.iter().all(|d| (1022..=1028).contains(d)), "{diffs:?}");
    let mean = (entries[entries.len() - 1] - entries[0]) as f64 / (entries.len() - 1) as f64;
    assert!((mean - 1025.0).abs() < 0.05, "{mean}");
    assert_eq!(m.peek(0x10) as usize, entries.len());
}

#[test]
fn one_shot_t1_interrupts_once_at_latch_plus_two_after_the_write() {
    let (mut m, _bus) = setup(Variant::MockingboardC);
    let mut code = vec![0x58];
    code.extend(sta(0xC0, 0xC40E));
    code.extend(sta(0x00, 0xC404));
    code.extend(sta(0x10, 0xC405)); // latch $1000
    code.extend(spin_at(0x300 + code.len() as u16));
    load(&mut m, 0x300, &code);
    load(&mut m, 0x400, &[0xE6, 0x10, 0xAD, 0x04, 0xC4, 0x40]);
    run_to(&mut m, 0x310); // the spin is reached right after STA T1CH completes
    let w_end = m.cycles(); // the STA T1CH just completed
    let mut irq_at = None;
    while m.cycles() < w_end + 20_000 {
        let before = m.cpu().pc;
        m.step();
        if m.cpu().pc == 0x400 && before != 0x400 {
            irq_at.get_or_insert(m.cycles());
        }
    }
    let irq_at = irq_at.expect("no IRQ") - w_end;
    // Edge at 0x1000+2; the CPU takes it at the next opcode boundary (<= 3) + 7-cycle entry.
    assert!((0x1002 + 7..=0x1002 + 7 + 3).contains(&irq_at), "{irq_at:#x}");
    assert_eq!(m.peek(0x10), 1);
}

#[test]
fn polling_ifr_sees_the_timer_flag_without_interrupts() {
    // One-shot T1 with IER off: IFR bit 6 must read set once the timer ran out (cpp:392-398).
    let (mut m, _bus) = setup(Variant::MockingboardC);
    let mut code = sta(0x20, 0xC404).to_vec();
    code.extend(sta(0x00, 0xC405));
    // Poll: LDA IFR ; AND #$40 ; BEQ poll ; (flag seen) ; STA $10 ; spin
    code.extend([0xAD, 0x0D, 0xC4, 0x29, 0x40, 0xF0, 0xF9, 0x85, 0x10]);
    code.extend(spin_at(0x300 + code.len() as u16));
    load(&mut m, 0x300, &code);
    run_to(&mut m, 0x313);
    assert_eq!(m.peek(0x10), 0x40);
}

// ---- AY8910 through the 6522 ----

/// Emits the Mockingboard AY register write sequence for the 6522 at `base`.
/// `orb_hi`: extra ORB bits (Phasor chip select, b4:b3).
fn ay_write(code: &mut Vec<u8>, base: u16, orb_hi: u8, reg: u8, val: u8) {
    code.extend(sta(reg, base + 1)); // ORA := reg
    code.extend(sta(orb_hi | 0x07, base)); // LATCH
    code.extend(sta(orb_hi | 0x04, base)); // INACTIVE (reset released)
    code.extend(sta(val, base + 1));
    code.extend(sta(orb_hi | 0x06, base)); // WRITE
    code.extend(sta(orb_hi | 0x04, base));
}

fn ay_init(code: &mut Vec<u8>, base: u16) {
    code.extend(sta(0xFF, base + 3)); // DDRA
    code.extend(sta(0x07, base + 2)); // DDRB
    code.extend(sta(0x04, base)); // reset released
}

fn tone_program(base: u16, orb_hi: u8, period: u8) -> Vec<u8> {
    tone_program_at(0x300, base, orb_hi, period)
}

fn tone_program_at(org: u16, base: u16, orb_hi: u8, period: u8) -> Vec<u8> {
    let mut c = vec![];
    ay_init(&mut c, base);
    ay_write(&mut c, base, orb_hi, 0, period);
    ay_write(&mut c, base, orb_hi, 1, 0);
    ay_write(&mut c, base, orb_hi, 7, 0x3E);
    ay_write(&mut c, base, orb_hi, 8, 0x0F);
    c.extend(spin_at(org + c.len() as u16));
    c
}

fn render(m: &mut Machine, bus: &AudioBus, cycles: u64) -> Vec<f32> {
    let mut mixer = Mixer::new(bus);
    m.run_cycles(cycles);
    mixer.produce_machine(m)
}

#[test]
fn mockingboard_left_6522_plays_the_left_channel_only_at_the_programmed_pitch() {
    let (mut m, bus) = setup(Variant::MockingboardC);
    load(&mut m, 0x300, &tone_program(0xC400, 0, 100));
    let out = render(&mut m, &bus, 1_020_484);
    let l = out.iter().step_by(2).copied();
    let r = out.iter().skip(1).step_by(2).copied();
    assert_eq!(r.clone().fold(0.0f32, |a, b| a.max(b.abs())), 0.0);
    let peak = l.clone().fold(0.0f32, f32::max);
    assert!(peak > 0.2 && peak < 0.25, "{peak}"); // 10752 * 2/3 / 32768
    // Period 100: 637.8 Hz = ~1275 edges in one second.
    let edges = crossings(l);
    assert!((1270..=1282).contains(&edges), "{edges}");
}

#[test]
fn mockingboard_right_6522_plays_the_right_channel() {
    let (mut m, bus) = setup(Variant::MockingboardC);
    load(&mut m, 0x300, &tone_program(0xC480, 0, 80));
    let out = render(&mut m, &bus, 300_000);
    assert_eq!(out.iter().step_by(2).fold(0.0f32, |a, &b| a.max(b.abs())), 0.0);
    assert!(out.iter().skip(1).step_by(2).fold(0.0f32, |a, &b| a.max(b)) > 0.2);
}

#[test]
fn phasor_native_mode_runs_the_ays_at_double_clock_with_interleaved_6522_decoding() {
    let freq = |mode_switch: bool| {
        let (mut m, bus) = setup(Variant::Phasor);
        let mut code = vec![];
        let base = if mode_switch {
            code.extend([0xAD, 0xC5, 0xC0]); // LDA $C0C5: slot 4 -> Phasor native mode
            0xC410u16 // 6522 A is at $Cn10-$Cn1F; AY1 is selected by ORB b4:3 = 01
        } else {
            0xC400
        };
        let orb_hi = if mode_switch { 0x08 } else { 0 };
        let mut c = vec![];
        ay_init(&mut c, base);
        ay_write(&mut c, base, orb_hi, 0, 100);
        ay_write(&mut c, base, orb_hi, 1, 0);
        ay_write(&mut c, base, orb_hi, 7, 0x3E);
        ay_write(&mut c, base, orb_hi, 8, 0x0F);
        code.extend(c);
        code.extend(spin_at(0x300 + code.len() as u16));
        load(&mut m, 0x300, &code);
        let out = render(&mut m, &bus, 1_020_484);
        crossings(out.iter().step_by(2).copied())
    };
    let normal = freq(false) as f64;
    let native = freq(true) as f64;
    assert!((native / normal - 2.0).abs() < 0.02, "{native} / {normal}");
}

#[test]
fn megaaudio_and_sdmusic_variants_make_sound_too() {
    for v in [Variant::MegaAudio, Variant::SdMusic] {
        let (mut m, bus) = setup(v);
        load(&mut m, 0x300, &tone_program(0xC400, 0, 100));
        let out = render(&mut m, &bus, 200_000);
        assert!(out.iter().step_by(2).fold(0.0f32, |a, &b| a.max(b)) > 0.2, "{v:?}");
    }
}

#[test]
fn volume_handle_scales_and_mutes_a_card() {
    let bus = AudioBus::new(RATE, VideoStandard::Ntsc);
    let mut m = machine();
    let card = MockingboardCard::new(Variant::MockingboardC, &bus);
    let vol = card.volume();
    m.insert_card(4, Box::new(card));
    load(&mut m, 0x300, &tone_program(0xC400, 0, 100));
    let mut mixer = Mixer::new(&bus);
    m.run_cycles(100_000);
    let full = mixer.produce_machine(&mut m).iter().fold(0.0f32, |a, &b| a.max(b));
    vol.set_gain(0.5);
    m.run_cycles(100_000);
    let half = mixer.produce_machine(&mut m).iter().fold(0.0f32, |a, &b| a.max(b));
    assert!((half / full - 0.5).abs() < 0.02, "{half} {full}");
    vol.set_mute(true);
    m.run_cycles(50_000);
    assert_eq!(mixer.produce_machine(&mut m).iter().fold(0.0f32, |a, &b| a.max(b.abs())), 0.0);
}

// ---- SSI263 ----

fn speech_program(phoneme_byte: u8) -> Vec<u8> {
    let mut c = vec![0x58]; // CLI
    c.extend(sta(0x00, 0xC444)); // SSI reg 4: filter frequency (power-on value silences it)
    c.extend(sta(phoneme_byte, 0xC440)); // reg 0: duration/mode + phoneme (standby: no play yet)
    c.extend(sta(0x0F, 0xC443)); // reg 3: CTL=0 (out of power-down), amplitude 15
    c.extend(spin_at(0x300 + c.len() as u16));
    c
}

#[test]
fn ssi263_phoneme_is_audible_and_sets_ifr_bit1_when_complete() {
    let (mut m, bus) = setup(Variant::MockingboardC);
    load(&mut m, 0x300, &speech_program(0xC0 | 0x0A));
    let out = render(&mut m, &bus, 100_000);
    let (l, r): (Vec<f32>, Vec<f32>) = (out.iter().step_by(2).copied().collect(), out.iter().skip(1).step_by(2).copied().collect());
    assert_eq!(l, r); // mono speech in both channels
    assert!(l.iter().fold(0.0f32, |a, &b| a.max(b.abs())) > 0.02);
    // The 6522 at $C480 receives the SSI263's A/!R on CA1 => IFR bit 1.
    load(&mut m, 0x320, &[0xAD, 0x8D, 0xC4, 0x85, 0x10, 0x4C, 0x25, 0x03]); // LDA IFR : STA $10
    m.cpu_mut().pc = 0x320;
    run_to(&mut m, 0x325);
    assert_eq!(m.peek(0x10) & 0x02, 0x02);
}

#[test]
fn ssi263_phoneme_ends_silent_when_powered_down() {
    let (mut m, bus) = setup(Variant::MockingboardC);
    let mut code = speech_program(0xC0 | 0x0A);
    code.truncate(code.len() - 8); // drop the CTL write and the spin
    code.extend(sta(0x8F, 0xC443)); // CTL=1: stays in power-down, so nothing plays
    code.extend(spin_at(0x300 + code.len() as u16));
    load(&mut m, 0x300, &code);
    let out = render(&mut m, &bus, 100_000);
    assert_eq!(out.iter().fold(0.0f32, |a, &b| a.max(b.abs())), 0.0);
}

#[test]
fn votrax_sc01_phoneme_through_port_b_is_audible() {
    // Original Mockingboard speech: PCR=$B0, DDRB=$FF, phoneme on port B (SC-01 at $Cn00 only).
    let (mut m, bus) = setup(Variant::MockingboardC);
    let mut code = vec![];
    code.extend(sta(0xB0, 0xC40C));
    code.extend(sta(0xFF, 0xC402));
    code.extend(sta(0x2C, 0xC400)); // phoneme E
    code.extend(spin_at(0x300 + code.len() as u16));
    load(&mut m, 0x300, &code);
    let out = render(&mut m, &bus, 60_000);
    assert!(out.iter().fold(0.0f32, |a, &b| a.max(b.abs())) > 0.02);
    // Sockets without the SC-01 stay silent.
    let bus = AudioBus::new(RATE, VideoStandard::Ntsc);
    let mut m = machine();
    let sockets = Sockets { sc01: false, ssi263: [SsiType::Empty, SsiType::Empty], ..Sockets::default() };
    m.insert_card(4, Box::new(MockingboardCard::with_sockets(Variant::MockingboardC, &bus, sockets)));
    load(&mut m, 0x300, &code);
    let out = render(&mut m, &bus, 60_000);
    assert_eq!(out.iter().fold(0.0f32, |a, &b| a.max(b.abs())), 0.0);
}

// ---- state ----

fn card_bytes(m: &Machine, slot: u8) -> Vec<u8> {
    m.board().slots.card(slot).unwrap().save()
}

#[test]
fn machine_save_state_roundtrip_resumes_timers_and_irqs_identically() {
    let (mut m, _bus) = irq_machine(0x0800);
    m.run_cycles(60_000);
    let st = m.save_state();
    let saved_card = card_bytes(&m, 4);
    m.run_cycles(60_000);
    let (a_card, a_count) = (card_bytes(&m, 4), m.peek(0x10));
    m.load_state(&st).unwrap();
    assert_eq!(card_bytes(&m, 4), saved_card);
    m.run_cycles(60_000);
    assert_eq!(card_bytes(&m, 4), a_card);
    assert_eq!(m.peek(0x10), a_count);
}

#[test]
fn card_state_roundtrip_covers_ay_ssi263_and_phasor_mode() {
    let (mut m, bus) = setup(Variant::Phasor);
    load(&mut m, 0x300, &[0xAD, 0xC5, 0xC0, 0x4C, 0x10, 0x03]); // Phasor native mode, then the tone
    load(&mut m, 0x310, &tone_program_at(0x310, 0xC410, 0x08, 90));
    m.run_cycles(40_000);
    let bytes = card_bytes(&m, 4);
    let st = m.save_state();
    let mut m2 = machine();
    assert!(m2.insert_card(4, Box::new(MockingboardCard::new(Variant::Phasor, &bus))));
    m2.load_state(&st).unwrap();
    assert_eq!(card_bytes(&m2, 4), bytes);
    // A card of another variant refuses the bytes.
    let mut m3 = machine();
    m3.insert_card(4, Box::new(MockingboardCard::new(Variant::MockingboardC, &bus)));
    assert!(m3.load_state(&st).is_err());
}
