//! `Unit: Miscellaneous` (No Slot Clock) and `Unit: Game I/O Connector` (dongles). Fixtures are
//! transcribed from NoSlotClock.cpp:196-220, SaveState.cpp:540-558 and
//! CopyProtectionDongles.cpp:150-330.

mod common;
use apple2_cards::{Dongle, DongleKind, FixedClock, NoSlotClock, NscSnapshot, WallTime};
use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, SaveOptions};
use common::*;

const NSC_DEVICE: &str = "Unit:
  Type: Miscellaneous
  Version: 1
  State:
    No Slot Clock:
      Clock Register Enabled: true
      Write Enabled: false
      Clock Register Mask: 0x0000000000010000
      Clock Register: 0x1234567890ABCDEF
      Comparison Register Mask: 0x8000000000000000
      Comparison Register: 0x5CA33AC55CA33AC5
";

const NSC_SNAPSHOT: NscSnapshot = NscSnapshot {
    clock_register_enabled: true,
    write_enabled: false,
    clock_register_mask: 0x10000,
    clock_register: 0x1234_5678_90AB_CDEF,
    comparison_register_mask: 0x8000_0000_0000_0000,
    comparison_register: 0x5CA3_3AC5_5CA3_3AC5,
};

fn plain_machine() -> Machine {
    Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced))
}

fn nsc() -> NoSlotClock {
    NoSlotClock::new(Box::new(FixedClock(WallTime::SAMPLE)))
}

fn text(m: &Machine, extension: bool) -> String {
    let saved = save_aws_yaml(
        m,
        &SaveOptions {
            extension,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
    saved.text
}

fn load(t: &str) -> (Machine, Vec<String>) {
    let l = load_aws_yaml(t, &mut TestHost::default()).unwrap_or_else(|e| panic!("{e}"));
    (l.machine, l.warnings)
}

fn nsc_snapshot(m: &mut Machine) -> Option<NscSnapshot> {
    m.rom_tap_mut()?
        .as_any_mut()?
        .downcast_ref::<NoSlotClock>()
        .map(|n| n.snapshot())
}

/// The state of an NSC left half-way through a read.
fn busy_nsc() -> NoSlotClock {
    use apple2_core::hooks::RomTap;
    let mut c = nsc();
    let mut d = 0;
    c.access(0xC304, false, &mut d);
    for i in 0..64u64 {
        c.access(
            0xC300 | ((0x5CA3_3AC5_5CA3_3AC5u64 >> i) & 1) as u16,
            false,
            &mut d,
        );
    }
    for _ in 0..20 {
        c.access(0xC304, false, &mut d);
    }
    c
}

mod no_slot_clock {
    use super::*;

    #[test]
    fn golden_unit_sets_the_registers() {
        let t = format!("{}\n{NSC_DEVICE}", text(&plain_machine(), false));
        let (mut m, w) = load(&t);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(nsc_snapshot(&mut m), Some(NSC_SNAPSHOT));
    }

    #[test]
    fn no_misc_unit_means_no_clock_from_unit_version_6() {
        let (mut m, _) = load(&text(&plain_machine(), false));
        assert!(m.rom_tap_mut().is_none());
    }

    #[test]
    fn writes_the_unit_after_game_io_with_applewin_keys() {
        let mut m = plain_machine();
        let mut c = nsc();
        assert!(apple2_core::hooks::RomTap::load(&mut c, &busy_nsc_state()));
        m.set_rom_tap(Some(Box::new(c)));
        let t = text(&m, false);
        let s = busy_nsc().snapshot();
        let expect = format!(
            "\nUnit:\n  Type: Miscellaneous\n  Version: 1\n  State:\n    No Slot Clock:\n      Clock Register Enabled: {}\n      Write Enabled: {}\n      Clock Register Mask: 0x{:016X}\n      Clock Register: 0x{:016X}\n      Comparison Register Mask: 0x{:016X}\n      Comparison Register: 0x{:016X}\n",
            s.clock_register_enabled, s.write_enabled, s.clock_register_mask, s.clock_register, s.comparison_register_mask, s.comparison_register
        );
        assert!(t.ends_with(&expect), "{t}");
        assert!(t.contains("Type: Game I/O Connector"));
        assert!(t.find("Game I/O Connector").unwrap() < t.find("Miscellaneous").unwrap());
    }

    fn busy_nsc_state() -> Vec<u8> {
        apple2_core::hooks::RomTap::save(&busy_nsc())
    }

    #[test]
    fn machine_round_trip_mid_read() {
        let mut m = plain_machine();
        m.set_rom_tap(Some(Box::new(busy_nsc())));
        let mut exact = load(&text(&m, true)).0;
        assert_eq!(m.save_state(), exact.save_state());
        let mut plain = load(&text(&m, false)).0;
        assert_eq!(nsc_snapshot(&mut plain), nsc_snapshot(&mut m));
        assert_eq!(nsc_snapshot(&mut exact), nsc_snapshot(&mut m));
    }
}

mod game_io {
    use super::*;

    const KINDS: [(DongleKind, &str); 6] = [
        (DongleKind::SdsSpeedStar, "SDS SpeedStar dongle"),
        (
            DongleKind::CodeWriter,
            "Cortechs Corp - CodeWriter protection key",
        ),
        (
            DongleKind::Robocom500,
            "Robocom Ltd - Robo 500 Interface Module",
        ),
        (
            DongleKind::Robocom1000,
            "Robocom Ltd - Robo 1000 Interface Module",
        ),
        (
            DongleKind::Robocom1500,
            "Robocom Ltd - Robo 1500 Interface Module",
        ),
        (
            DongleKind::HaydenCompiler,
            "Hayden - Applesoft Compiler protection key",
        ),
    ];

    fn with_dongle(kind: DongleKind, lfsr: u8) -> Machine {
        let mut d = Dongle::new(kind);
        d.set_lfsr(lfsr);
        let mut m = plain_machine();
        m.set_game_io(Some(Box::new(d)));
        m
    }

    fn dongle(m: &Machine) -> Option<(DongleKind, u8)> {
        m.game_io()?
            .as_any()?
            .downcast_ref::<Dongle>()
            .map(|d| (d.kind(), d.lfsr()))
    }

    fn game_io_unit(t: &str) -> &str {
        let i = t.find("\nUnit:\n  Type: Game I/O Connector").unwrap();
        &t[i..]
    }

    #[test]
    fn writes_the_device_name_and_the_codewriter_lfsr() {
        for (kind, name) in KINDS {
            let t = text(&with_dongle(kind, 0x2A), false);
            let lfsr = if kind == DongleKind::CodeWriter {
                "    LFSR: 0x2A\n"
            } else {
                ""
            };
            let expect = format!("\nUnit:\n  Type: Game I/O Connector\n  Version: 4\n  State:\n    Shift Key Mod: false\n    Device: {name}\n{lfsr}");
            assert_eq!(game_io_unit(&t), expect, "{name}");
        }
    }

    #[test]
    fn without_a_dongle_only_the_shift_key_mod_is_written() {
        let t = text(&plain_machine(), false);
        assert_eq!(game_io_unit(&t), "\nUnit:\n  Type: Game I/O Connector\n  Version: 4\n  State:\n    Shift Key Mod: false\n");
    }

    #[test]
    fn golden_loads_each_device() {
        for (kind, name) in KINDS {
            let t = text(&plain_machine(), false).replace(
                "    Shift Key Mod: false\n",
                &format!("    Shift Key Mod: true\n    Device: {name}\n    LFSR: 0x35\n"),
            );
            let (m, w) = load(&t);
            assert!(w.is_empty(), "{w:?}");
            let expect_lfsr = if kind == DongleKind::CodeWriter {
                0x35
            } else {
                0x6B
            };
            assert_eq!(dongle(&m), Some((kind, expect_lfsr)), "{name}");
        }
    }

    #[test]
    fn old_unit_versions_have_no_shift_key_mod() {
        let t = text(&plain_machine(), false).replace(
            "  Version: 4\n  State:\n    Shift Key Mod: false\n",
            "  Version: 3\n  State:\n    Device: Hayden - Applesoft Compiler protection key\n",
        );
        let (m, w) = load(&t);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(dongle(&m).map(|d| d.0), Some(DongleKind::HaydenCompiler));
    }

    #[test]
    fn future_unit_version_is_refused() {
        let t = text(&plain_machine(), false).replace(
            "  Type: Game I/O Connector\n  Version: 4",
            "  Type: Game I/O Connector\n  Version: 5",
        );
        assert!(load_aws_yaml(&t, &mut TestHost::default()).is_err());
    }

    #[test]
    fn unknown_device_is_a_warning() {
        let t = text(&plain_machine(), false).replace(
            "    Shift Key Mod: false\n",
            "    Shift Key Mod: false\n    Device: Mystery key\n",
        );
        let (m, w) = load(&t);
        assert!(w.iter().any(|w| w.contains("Mystery key")), "{w:?}");
        assert!(dongle(&m).is_none());
    }

    #[test]
    fn machine_round_trip() {
        for (kind, _) in KINDS {
            let m = with_dongle(kind, 0x2A);
            let exact = load(&text(&m, true)).0;
            assert_eq!(m.save_state(), exact.save_state(), "{kind:?}");
            let plain = load(&text(&m, false)).0;
            // AppleWin keeps the LFSR of the CodeWriter key only; the others are stateless.
            let lfsr = if kind == DongleKind::CodeWriter {
                0x2A
            } else {
                0x6B
            };
            assert_eq!(dongle(&plain), Some((kind, lfsr)), "{kind:?}");
        }
    }
}
