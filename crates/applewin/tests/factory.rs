//! Every card type the settings can describe boots in the shell without crashing, and the
//! factory builds the card the type names.

use apple2_config::{CardType, Settings, SlotCardSpec};
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::{Command, Emulator};

fn settings_with(slot: usize, spec: SlotCardSpec) -> Settings {
    let mut s = Settings::default();
    s.slots.0[6] = SlotCardSpec::Empty;
    s.slots.0[slot] = spec;
    s
}

/// The `Card::name` AppleWin's save-state (and our factory) gives each slot card; `None` for the
/// types that live in the core's configuration or are dummies in AppleWin.
fn expected_name(ty: CardType) -> Option<&'static str> {
    use CardType::*;
    Some(match ty {
        Disk2 => "Disk II",
        Ssc => "Super Serial Card",
        MockingboardC => "Mockingboard C",
        Phasor => "Phasor",
        MegaAudio => "MEGA Audio",
        SdMusic => "SD Music",
        GenericPrinter => "Generic Printer",
        GenericHdd => "Hard Disk Controller",
        MouseInterface => "Mouse Card",
        Z80 => "Z80 SoftCard",
        Sam => "SAM",
        Uthernet => "Uthernet",
        Uthernet2 => "Uthernet II",
        FourPlay => "4Play",
        SnesMax => "SNES MAX",
        VidHd => "VidHD",
        BreakpointCard => "Breakpoint",
        Empty | GenericClock | Echo | Col80 | Extended80Col | RamWorksIII | LanguageCard | LanguageCardIIe | Saturn128K => return None,
    })
}

#[test]
fn every_card_type_boots_and_is_built() {
    for v in 0..26 {
        let ty = CardType::from_value(v).unwrap();
        let spec = SlotCardSpec::new(ty);
        let slot = match ty {
            CardType::LanguageCard | CardType::LanguageCardIIe => 0,
            _ => 5,
        };
        let s = settings_with(slot, spec);
        let mut emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap_or_else(|e| panic!("{ty:?}: {e}"));
        for _ in 0..90 {
            emu.run_frame();
        }
        let built = emu.machine.board().slots.card(slot as u8).map(|c| c.name());
        assert_eq!(built, expected_name(ty), "{ty:?}");
    }
}

#[test]
fn all_cards_at_once() {
    let mut s = Settings::default();
    s.slots.0[1] = SlotCardSpec::new(CardType::GenericPrinter);
    s.slots.0[2] = SlotCardSpec::new(CardType::Ssc);
    s.slots.0[3] = SlotCardSpec::new(CardType::Uthernet2);
    s.slots.0[4] = SlotCardSpec::new(CardType::Phasor);
    s.slots.0[5] = SlotCardSpec::new(CardType::VidHd);
    s.slots.0[6] = SlotCardSpec::new(CardType::Disk2);
    s.slots.0[7] = SlotCardSpec::new(CardType::GenericHdd);
    let mut emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    for _ in 0..120 {
        emu.run_frame();
    }
    assert!(emu.machine.cycles() > 1_000_000);
}

#[test]
fn disk2_firmware_13_sector_differs() {
    let boot_rom = |p5: bool| {
        let s = settings_with(6, SlotCardSpec::Disk2 { firmware_13_sector: p5, drives: Default::default() });
        let mut emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
        emu.machine.with_card(6, |c, ctx| (0..=255u8).map(|i| c.slot_rom_read(i, ctx)).collect::<Vec<_>>()).unwrap()
    };
    let (p6, p5) = (boot_rom(false), boot_rom(true));
    assert_ne!(p6, p5, "13-sector (P5A) firmware must differ from the 16-sector one");
    assert_eq!(p6[0], 0xA2);
}

#[test]
fn hard_disk_images_come_from_the_settings() {
    let dir = std::env::temp_dir().join(format!("applewin-hdd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let img = dir.join("work.hdv");
    std::fs::write(&img, vec![0u8; 512 * 64]).unwrap();
    let mut images: [Option<std::path::PathBuf>; 8] = Default::default();
    images[0] = Some(img.clone());
    let s = settings_with(7, SlotCardSpec::GenericHdd { firmware: Default::default(), images });
    let mut emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    let hdd = emu.machine.card_mut::<apple2_storage::hdd::HardDiskCard>(7).expect("HDD card in slot 7");
    assert_eq!(hdd.base_name(0), "work");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn vidhd_makes_the_frame_640_by_400() {
    let s = settings_with(2, SlotCardSpec::VidHd);
    let mut emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    for _ in 0..30 {
        emu.run_frame();
    }
    let f = emu.render();
    assert_eq!((f.width, f.height), (640, 400));
}

#[test]
fn no_slot_clock_follows_the_setting() {
    let mut s = Settings::default();
    assert!(Emulator::new(s.clone(), Box::new(DefaultBackend), 48_000).unwrap().machine.rom_tap_mut().is_some());
    s.machine.no_slot_clock = false;
    assert!(Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap().machine.rom_tap_mut().is_none());
}

#[test]
fn dongle_goes_into_the_game_connector() {
    let mut s = Settings::default();
    s.joystick.dongle = apple2_config::settings::DongleType::Codewriter;
    let emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    assert!(emu.machine.game_io().is_some());
}

#[test]
fn cpu_override_reaches_the_machine() {
    use apple2_config::settings::CpuType;
    let mut s = Settings::default();
    s.machine.cpu = CpuType::Cpu6502;
    let emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    assert_eq!(emu.machine.cpu().variant, apple2_cpu::Variant::Nmos6502);
}

#[test]
fn rom_overrides_from_the_command_line() {
    let dir = std::env::temp_dir().join(format!("applewin-rom-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f8 = dir.join("f8.rom");
    std::fs::write(&f8, vec![0xEAu8; 0x800]).unwrap();
    let cmd = apple2_config::CmdLine::parse(&["-model", "apple2p", "-f8rom", f8.to_str().unwrap()]).unwrap();
    let mut s = Settings::default();
    cmd.apply(&mut s);
    let emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    assert_eq!(emu.machine.peek(0xF800), 0xEA);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn clock_multiplier_sets_the_speed() {
    let mut s = Settings::default();
    s.machine.clock_multiplier = Some(2.5);
    let emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    assert_eq!(emu.speed, applewin_shell::pacing::Speed::Multiplier(2.5));
}

#[test]
fn power_cycle_and_apply_settings_keep_the_cards() {
    let s = settings_with(4, SlotCardSpec::new(CardType::MockingboardC));
    let mut emu = Emulator::new(s.clone(), Box::new(DefaultBackend), 48_000).unwrap();
    emu.apply(Command::PowerCycle);
    emu.apply(Command::ApplySettings(Box::new(s)));
    assert_eq!(emu.machine.board().slots.card(4).map(|c| c.name()), Some("Mockingboard C"));
}
