//! Slice 7: every Model with a system ROM boots (no disk) to its BASIC or monitor prompt, with
//! the real ROMs; typing, custom ROMs and save-states work end to end.

use apple2_core::text_screen::{text_screen, row_address};
use apple2_core::{ConfigError, Machine, MachineConfig, Model};

/// Generous budget: the slowest ROM prompts in well under 100 frames (~1.6 s of Apple time).
const BOOT_FRAMES: u32 = 300;

/// What the real firmware leaves on screen with no disk: `(model, banner row 0, prompt row,
/// prompt screen code)`. Banners are as decoded by `text_screen` (clone ROMs print their name
/// in the lowercase half of their own character set (the Latin-looking "prawec" is "ПРАВЕЦ"); the prompt row is where the cursor sits).
/// Prompt codes: `]` ($DD) Applesoft, `*` ($AA) monitor.
const BOOTS: [(Model, &str, usize, u8); 10] = [
    // The original ][ has no autostart: the monitor starts on the last line over a RAM-garbage screen.
    (Model::Apple2, "", 23, 0xAA),
    (Model::Apple2Plus, "APPLE ][", 2, 0xDD),
    (Model::Apple2JPlus, "APPLE ][", 2, 0xDD),
    (Model::Apple2e, "Apple ][", 2, 0xDD),
    (Model::Apple2eEnhanced, "Apple //e", 2, 0xDD),
    // Pravets 82's Applesoft uses its own prompt glyph, $FD.
    (Model::Pravets82, "prawec", 2, 0xFD),
    (Model::Pravets8M, "prawec", 2, 0xDD),
    (Model::Pravets8A, "prawec", 2, 0xDD),
    (Model::Tk3000e, "TK3000//e", 2, 0xDD),
    (Model::Base64A, "BASE 64A", 2, 0xDD),
];

fn booted(model: Model) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(model));
    run_until(&mut m, |m| prompt_row(m).is_some(), BOOT_FRAMES)
        .unwrap_or_else(|| panic!("{model:?} did not reach a prompt:\n{}", text_screen(&m).join("\n")));
    m
}

/// First text row that is exactly a prompt character (`]` or `*`) followed by blanks/cursor.
fn prompt_row(m: &Machine) -> Option<usize> {
    let rows = text_screen(m);
    (0..24).find(|&r| {
        let l = &rows[r];
        (l.starts_with(']') || l.starts_with('*') || l.starts_with('}')) && l[1..].chars().all(|c| matches!(c, ' ' | '_' | '?'))
    })
}

/// Runs frames until `done`, returning the frames used.
fn run_until(m: &mut Machine, done: impl Fn(&Machine) -> bool, max: u32) -> Option<u32> {
    for f in 0..=max {
        if done(m) {
            return Some(f);
        }
        m.run_frame();
    }
    None
}

fn row(m: &Machine, r: usize) -> String {
    text_screen(m)[r].trim().to_string()
}

fn screen_code(m: &Machine, r: usize) -> u8 {
    m.peek_main(row_address(r))
}

#[test]
fn every_model_boots_to_its_prompt() {
    for (model, banner, prompt_row_idx, code) in BOOTS {
        let mut m = Machine::new(MachineConfig::for_model(model));
        let frames = run_until(&mut m, |m| screen_code(m, prompt_row_idx) == code && (banner.is_empty() || row(m, 0) == banner), BOOT_FRAMES)
            .unwrap_or_else(|| panic!("{model:?}: no prompt/banner:\n{}", text_screen(&m).join("\n")));
        println!("{model:?}: frames to prompt {frames}, banner {:?}, pc {:04X}", row(&m, 0), m.cpu().pc);
        // Idle in the keyboard wait (the prompt is stable), not crashed in the debugger or a loop on ROM junk.
        let pc = m.cpu().pc;
        assert!(pc >= 0xC000, "{model:?} pc {pc:04X}");
        m.run_frame();
        assert_eq!(screen_code(&m, prompt_row_idx), code, "{model:?}");
    }
}

/// Types `text` through `key_press`, one key at a time, waiting for the ROM to take each one
/// (the strobe in $C000 bit 7 clears when the ROM reads $C010).
fn type_text(m: &mut Machine, text: &str) {
    use apple2_cpu::Bus;
    for b in text.bytes() {
        assert!(m.key_press(b), "key {b:#x} refused");
        for _ in 0..20 {
            m.run_cycles(2000);
            if m.board_mut().read(0xC000) & 0x80 == 0 {
                break;
            }
        }
        assert_eq!(m.board_mut().read(0xC000) & 0x80, 0, "ROM never consumed {b:#x}");
    }
    m.run_cycles(200_000);
}

#[test]
fn print_2_plus_2_on_enhanced_and_plus() {
    for model in [Model::Apple2eEnhanced, Model::Apple2Plus] {
        let mut m = booted(model);
        type_text(&mut m, "PRINT 2+2\r");
        let rows = text_screen(&m);
        assert_eq!(rows[2].trim_end_matches(['_', '?', ' ']), "]PRINT 2+2", "{model:?}");
        assert_eq!(rows[3].trim(), "4", "{model:?}:\n{}", rows.join("\n"));
        assert_eq!(rows[5].chars().next(), Some(']'), "{model:?}:\n{}", rows.join("\n"));
    }
}

#[test]
fn paste_call_minus_151_reaches_the_monitor() {
    for model in [Model::Apple2eEnhanced, Model::Apple2Plus] {
        let mut m = booted(model);
        assert!(m.paste("CALL -151\r"));
        let frames = run_until(&mut m, |m| text_screen(m).iter().any(|l| l.starts_with('*')), 60);
        assert!(frames.is_some(), "{model:?}:\n{}", text_screen(&m).join("\n"));
        // And the monitor is alive: examine zero page byte $33 (PROMPT, now `*` = $AA).
        assert!(m.paste("33\r"));
        m.run_cycles(300_000);
        assert!(text_screen(&m).iter().any(|l| l.starts_with("0033- AA")), "{model:?}:\n{}", text_screen(&m).join("\n"));
    }
}

#[test]
fn custom_rom_12k_and_16k_boot() {
    // 12K: the ][+ image on an original ][ model boots like a ][+ (autostart, Applesoft).
    let mut cfg = MachineConfig::for_model(Model::Apple2);
    cfg.custom_rom = Some(apple2_roms::APPLE2_PLUS.to_vec());
    let mut m = Machine::new(cfg);
    run_until(&mut m, |m| screen_code(m, 2) == 0xDD, BOOT_FRAMES).expect("12K custom ROM");
    assert_eq!(row(&m, 0), "APPLE ][");
    // 16K: the //e image (4K internal Cx + 12K) on the Enhanced //e model.
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    cfg.custom_rom = Some(apple2_roms::APPLE2E.to_vec());
    let mut m = Machine::new(cfg);
    run_until(&mut m, |m| screen_code(m, 2) == 0xDD, BOOT_FRAMES).expect("16K custom ROM");
    assert_eq!(row(&m, 0), "Apple ][");
}

#[test]
fn custom_rom_size_is_validated() {
    // AppleWin (Memory.cpp:2105-2118): 16K = //e (Cx + 12K), else whole 12K pages, at most 4.
    for ok in [0x3000, 0x4000, 0x6000, 0x9000, 0xC000] {
        let cfg = MachineConfig { custom_rom: Some(vec![0xEA; ok]), ..Default::default() };
        assert_eq!(cfg.validate(), Ok(()), "{ok:#x}");
        assert!(Machine::try_new(cfg).is_ok());
    }
    for bad in [0, 1, 0x2FFF, 0x3001, 0x3800, 0x5000, 0xF000, 0x10000] {
        let cfg = MachineConfig { custom_rom: Some(vec![0xEA; bad]), ..Default::default() };
        assert_eq!(cfg.validate(), Err(ConfigError::BadRomSize(bad)), "{bad:#x}");
        assert!(matches!(Machine::try_new(cfg), Err(ConfigError::BadRomSize(n)) if n == bad));
    }
}

#[test]
fn cpu_override_forces_variant() {
    use apple2_cpu::Variant;
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    assert_eq!(Machine::new(cfg.clone()).cpu().variant, Variant::Cmos65C02);
    cfg.cpu_cmos = Some(false);
    assert_eq!(Machine::new(cfg).cpu().variant, Variant::Nmos6502);
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.cpu_cmos = Some(true);
    let mut m = Machine::new(cfg);
    assert_eq!(m.cpu().variant, Variant::Cmos65C02);
    // it keeps booting with the other CPU
    run_until(&mut m, |m| screen_code(m, 0) != 0xA0, BOOT_FRAMES).expect("boots as 65C02");
}

#[test]
fn custom_f8_rom_replaces_last_2k() {
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.custom_f8_rom = Some(vec![0xAB; 0x800]);
    let m = Machine::new(cfg.clone());
    assert_eq!(m.peek(0xF800), 0xAB);
    assert_eq!(m.peek(0xFFFF), 0xAB);
    assert_ne!(m.peek(0xF7FF), 0xAB);
    cfg.custom_f8_rom = Some(vec![0; 10]);
    assert_eq!(cfg.validate(), Err(ConfigError::BadF8RomSize(10)));
}

/// Everything observable that a deterministic continuation must reproduce.
fn fingerprint(m: &Machine) -> (Vec<u8>, Vec<u8>, [u8; 5], u16, u64, u64) {
    let c = m.cpu();
    (
        (0..=0xFFFFu16).map(|a| m.peek_main(a)).collect(),
        (0..=0xFFFFu16).map(|a| m.peek_aux(a)).collect(),
        [c.a, c.x, c.y, c.sp, c.p],
        c.pc,
        m.cycles(),
        m.frames(),
    )
}

#[test]
fn save_state_roundtrip_is_deterministic() {
    const FRAMES: u32 = 40;
    let cont = |m: &mut Machine| {
        assert!(m.paste("PRINT 6*7\rFOR I=1 TO 99:A=A+I:NEXT:PRINT A\r"));
        for _ in 0..FRAMES {
            m.run_frame();
        }
    };
    for model in [Model::Apple2eEnhanced, Model::Apple2Plus] {
        let mut reference = booted(model);
        let mut a = booted(model);
        assert_eq!(a.save_state(), reference.save_state(), "boot is deterministic");
        // Through JSON, as a frontend would store it.
        let json = serde_json::to_string(&a.save_state()).unwrap();
        let state: apple2_core::MachineState = serde_json::from_str(&json).unwrap();
        cont(&mut reference);
        // Mutate: run something else, poke RAM, flip switches.
        assert!(a.paste("HOME\rPRINT 1\r"));
        a.run_cycles(500_000);
        a.poke_main(0x300, 0x55);
        a.set_button(0, true);
        assert_ne!(a.save_state(), state, "the mutation must be visible");
        a.load_state(&state).unwrap();
        cont(&mut a);
        assert_eq!(a.save_state(), reference.save_state(), "{model:?}");
        assert!(fingerprint(&a) == fingerprint(&reference), "{model:?}");
        assert_eq!(row(&a, 3), "42", "{model:?}");
    }
}

#[test]
fn load_state_rejects_a_different_configuration() {
    let state = Machine::new(MachineConfig::for_model(Model::Apple2Plus)).save_state();
    let mut other = Machine::new(MachineConfig::for_model(Model::Apple2e));
    assert!(other.load_state(&state).is_err());
}

/// On-CPU nanoseconds of this thread (Linux), so the benchmark is meaningful on a loaded machine.
fn thread_cpu_ns() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/thread-self/schedstat").ok()?;
    s.split_whitespace().next()?.parse().ok()
}

/// `cargo test --release -p apple2-core --test boot emulated_speed -- --ignored --nocapture`
#[test]
#[ignore]
fn emulated_speed() {
    let mut m = booted(Model::Apple2eEnhanced);
    assert!(m.paste("FOR I=1 TO 1E9:NEXT\r")); // keep the CPU busy in Applesoft
    m.run_frame();
    let cpu0 = thread_cpu_ns();
    let t = std::time::Instant::now();
    let cycles: u64 = (0..600).map(|_| m.run_frame()).sum();
    let secs = t.elapsed().as_secs_f64();
    println!("emulated {:.1} MHz ({:.0}x real time)", cycles as f64 / secs / 1e6, cycles as f64 / secs / 1_020_484.0);
    if let (Some(a), Some(b)) = (cpu0, thread_cpu_ns()) {
        println!("emulated {:.1} MHz on-CPU", cycles as f64 / (b - a) * 1e3);
    }
}
