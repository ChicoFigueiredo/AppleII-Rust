//! User ROM overrides at the frontend: provenance exposed by the emulator and refusal to start
//! with an invalid file. Firmware here is synthetic, never Apple code.

use apple2_config::roms::RomSource;
use apple2_config::Settings;
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::Emulator;

fn dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("a2user-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn provenance_lists_embedded_roms_by_default() {
    let mut s = Settings::default();
    s.slots.0[6] = apple2_config::SlotCardSpec::Empty;
    let emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    let sys = &emu.rom_report()[0];
    assert!(matches!(
        sys.source,
        RomSource::Embedded("Apple2e_Enhanced.rom")
    ));
    assert_eq!(
        sys.sha256,
        apple2_roms::sha256_hex(apple2_roms::APPLE2E_ENHANCED)
    );
}

#[test]
fn a_user_rom_is_used_and_reported_and_a_bad_one_stops_the_machine() {
    let d = dir("emu");
    let f = d.join("sys.rom");
    // 16K of NOPs: only the provenance matters here, the machine is not run.
    let rom = vec![0xEAu8; 0x4000];
    std::fs::write(&f, &rom).unwrap();
    let mut s = Settings::default();
    s.slots.0[6] = apple2_config::SlotCardSpec::Empty;
    s.roms.apple2e_enhanced = Some(f.clone());
    let emu = Emulator::new(s.clone(), Box::new(DefaultBackend), 48_000).unwrap();
    assert_eq!(emu.rom_report()[0].source, RomSource::File(f.clone()));
    assert_eq!(emu.rom_report()[0].sha256, apple2_roms::sha256_hex(&rom));

    // AppleWin falls back to the default ROM (Memory.cpp:2205-2211): the machine starts with the
    // embedded one, the report says why and the window gets a message.
    std::fs::write(&f, vec![0u8; 100]).unwrap();
    let emu = Emulator::new(s.clone(), Box::new(DefaultBackend), 48_000).unwrap();
    let sys = &emu.rom_report()[0];
    assert!(matches!(sys.source, RomSource::Embedded(_)));
    assert!(sys.fallback.as_deref().unwrap().contains("100 bytes"));
    assert_eq!(
        sys.sha256,
        apple2_roms::sha256_hex(apple2_roms::APPLE2E_ENHANCED)
    );
    assert!(emu.status().message.contains("100 bytes"));
    assert_eq!(
        emu.machine.config().custom_rom,
        None,
        "the embedded ROM is in use"
    );

    s.roms.apple2e_enhanced = Some(d.join("missing.rom"));
    let emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    assert!(emu.rom_report()[0]
        .fallback
        .as_deref()
        .unwrap()
        .contains("file not found"));
    assert!(emu.status().message.contains("file not found"));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_20k_rom_is_accepted_and_its_first_12k_used() {
    // Memory.cpp:2196 reads 12K from any file that is not exactly 16K.
    let d = dir("big");
    let f = d.join("big.rom");
    let big: Vec<u8> = (0..20 * 1024).map(|i| (i % 251) as u8).collect();
    std::fs::write(&f, &big).unwrap();
    let mut s = Settings::default();
    s.slots.0[6] = apple2_config::SlotCardSpec::Empty;
    s.machine.custom_rom = Some(f.clone());
    let emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    let sys = &emu.rom_report()[0];
    assert_eq!(sys.source, RomSource::File(f));
    assert_eq!(sys.size, 0x3000);
    assert_eq!(sys.sha256, apple2_roms::sha256_hex(&big[..0x3000]));
    assert!(emu.status().message.is_empty());
    let _ = std::fs::remove_dir_all(d);
}
