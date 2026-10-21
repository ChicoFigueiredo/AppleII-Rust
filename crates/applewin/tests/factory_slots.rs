//! Factory checks (HU-0601/0602): slot conflicts as AppleWin's CardManager (AUD-P-019/020) and the
//! SNES MAX command-line options (AUD-P-017, factory half).

use apple2_audio::AudioBus;
use apple2_cards::snesmax::{buttons, ButtonMap, HostPad};
use apple2_cards::SnesMax;
use apple2_config::{CardType, Settings, SlotCardSpec};
use apple2_core::VideoStandard;
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::Emulator;
use applewin_shell::factory::{
    build_machine_report, check_slots, parse_controller_mapping_file, snes_max_card,
};
use std::path::PathBuf;

fn base() -> Settings {
    let mut s = Settings::default();
    s.slots.0[6] = SlotCardSpec::Empty;
    s
}

fn bus() -> AudioBus {
    AudioBus::new(48_000, VideoStandard::Ntsc)
}

fn tmp(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("applewin-factory-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

// ------------------------------------------------------------------ slot conflicts

#[test]
fn two_mouse_interfaces_are_an_error_naming_the_slots() {
    let mut s = base();
    s.slots.0[4] = SlotCardSpec::new(CardType::MouseInterface);
    s.slots.0[7] = SlotCardSpec::new(CardType::MouseInterface);
    let e = check_slots(&s).unwrap_err();
    assert!(e.contains("MouseInterface") && e.contains("4, 7"), "{e}");
    assert!(build_machine_report(&s, &bus()).is_err());
    assert!(Emulator::new(s, Box::new(DefaultBackend), 48_000).is_err());
}

#[test]
fn every_single_instance_card_rejects_a_second_copy() {
    for ty in [
        CardType::Ssc,
        CardType::GenericPrinter,
        CardType::Z80,
        CardType::FourPlay,
        CardType::SnesMax,
        CardType::Uthernet,
        CardType::Uthernet2,
        CardType::VidHd,
    ] {
        let mut s = base();
        s.slots.0[2] = SlotCardSpec::new(ty);
        s.slots.0[3] = SlotCardSpec::new(ty);
        assert!(check_slots(&s).is_err(), "{ty:?}");
    }
}

#[test]
fn uthernet_and_uthernet_ii_cannot_coexist() {
    let mut s = base();
    s.slots.0[3] = SlotCardSpec::new(CardType::Uthernet);
    s.slots.0[4] = SlotCardSpec::new(CardType::Uthernet2);
    let e = check_slots(&s).unwrap_err();
    assert!(e.contains("Uthernet") && e.contains("Uthernet II"), "{e}");
}

#[test]
fn a_valid_configuration_has_no_errors_or_warnings() {
    let mut s = base();
    s.slots.0[4] = SlotCardSpec::new(CardType::MouseInterface);
    s.slots.0[7] = SlotCardSpec::new(CardType::Uthernet2);
    assert_eq!(check_slots(&s), Ok(vec![]));
}

#[test]
fn invalid_card_in_slot_0_of_the_plus_is_reported_not_dropped() {
    let mut s = base();
    s.slots.0[0] = SlotCardSpec::new(CardType::Ssc);
    let w = check_slots(&s).unwrap();
    assert!(w.iter().any(|m| m.starts_with("Slot 0: Ssc")), "{w:?}");
    let e = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    assert!(
        e.status().message.contains("Slot 0"),
        "{}",
        e.status().message
    );
}

#[test]
fn language_card_in_a_numbered_slot_is_reported() {
    let mut s = base();
    s.slots.0[3] = SlotCardSpec::new(CardType::LanguageCard);
    let w = check_slots(&s).unwrap();
    assert!(
        w.iter()
            .any(|m| m.contains("Slot 3") && m.contains("slot 0")),
        "{w:?}"
    );
}

// ------------------------------------------------------------------ SNES MAX

const MAPPING: &str = "---
File_hdr:
  Tag: AppleWin Controller Button Remapping
  Version: 1

Unit:
  HID: 054c-09cc
  Description: Test pad
  1: B
  2: A
  3: Y
  4: X
  5: LB
  6: RB
  9: SELECT
  10: START
  12:
";

#[test]
fn user_mapping_file_is_parsed_like_applewin() {
    let map = parse_controller_mapping_file(&tmp("ok.yaml", MAPPING)).unwrap();
    let b = |m: u32| Some(m.trailing_zeros() as u8);
    assert_eq!(map.0[0], b(buttons::B));
    assert_eq!(map.0[1], b(buttons::A));
    assert_eq!(map.0[2], b(buttons::Y));
    assert_eq!(map.0[3], b(buttons::X));
    assert_eq!(map.0[4], b(buttons::L));
    assert_eq!(map.0[5], b(buttons::R));
    assert_eq!(map.0[8], b(buttons::SELECT));
    assert_eq!(map.0[9], b(buttons::START));
    // Missing key and empty value are UNUSED.
    assert_eq!(map.0[6], None);
    assert_eq!(map.0[11], None);
    assert_eq!(map.0[15], None);
}

#[test]
fn user_mapping_errors_use_applewin_messages() {
    let bad_button = MAPPING.replace("1: B", "1: Z");
    let e = parse_controller_mapping_file(&tmp("bad-button.yaml", &bad_button)).unwrap_err();
    assert_eq!(e, "Controller mapping file: Unknown button: Z");
    let e = parse_controller_mapping_file(&tmp(
        "v2.yaml",
        &MAPPING.replace("Version: 1", "Version: 2"),
    ))
    .unwrap_err();
    assert_eq!(e, "Controller mapping file: Version mismatch");
    let e = parse_controller_mapping_file(&tmp("tag.yaml", &MAPPING.replace("Remapping", "Other")))
        .unwrap_err();
    assert!(e.contains("Bad tag"), "{e}");
    let e = parse_controller_mapping_file(&tmp(
        "nohid.yaml",
        &MAPPING.replace("  HID: 054c-09cc\n", ""),
    ))
    .unwrap_err();
    assert!(e.contains("Missing: HID"), "{e}");
    let e = parse_controller_mapping_file(&PathBuf::from("/no/such/file.yaml")).unwrap_err();
    assert_eq!(
        e,
        "Controller mapping file: Failed to initialize parser or open file"
    );
}

#[test]
fn alt_flag_and_user_file_reach_the_card() {
    let mut w = Vec::new();
    let path = tmp("user.yaml", MAPPING);
    let c = snes_max_card([false, false], &[None, Some(path)], &mut w);
    assert!(w.is_empty(), "{w:?}");
    // A valid user file enables the alternative controller for that joystick (CmdLine.cpp).
    assert!(!c.alt_controller(0) && c.alt_controller(1));
    assert_eq!(c.alt_map(1).0[0], Some(buttons::B.trailing_zeros() as u8));
    assert_eq!(*c.alt_map(0), ButtonMap::ALT_DEFAULT);
    let c = snes_max_card([true, false], &[None, None], &mut w);
    assert!(c.alt_controller(0) && !c.alt_controller(1));
}

#[test]
fn a_broken_user_file_warns_and_keeps_the_defaults() {
    let mut w = Vec::new();
    let path = tmp("broken.yaml", &MAPPING.replace("1: B", "1: Q"));
    let c = snes_max_card([false, false], &[Some(path.clone()), None], &mut w);
    assert!(!c.alt_controller(0));
    assert_eq!(w.len(), 1);
    assert!(w[0].starts_with("Error with yaml file: "), "{}", w[0]);
    assert!(w[0].contains("Unknown button: Q"), "{}", w[0]);
}

#[test]
fn factory_builds_the_card_with_the_settings_flags() {
    let mut s = base();
    let path = tmp("factory.yaml", MAPPING);
    s.slots.0[5] = SlotCardSpec::SnesMax {
        alt_controller: [true, false],
        user_mapping: [None, Some(path)],
    };
    let (m, w) = build_machine_report(&s, &bus()).unwrap();
    assert!(w.is_empty(), "{w:?}");
    let c = m.card::<SnesMax>(5).unwrap();
    assert!(c.alt_controller(0) && c.alt_controller(1));
}

#[test]
fn host_pad_goes_through_the_main_and_alt_tables() {
    let mut c = SnesMax::new();
    // Main table (DualShock/DualSense): host bit 0 -> Y, bit 1 -> B.
    let pad = HostPad {
        buttons: 0b11,
        ..HostPad::CENTRED
    };
    c.set_host_pad(0, Some(pad));
    // Alt default (8BitDo NES30 PRO): host bit 0 -> A, bit 1 -> B.
    c.set_alt_controller(1, true);
    c.set_host_pad(1, Some(pad));
    // D-pad by POV hat: up-right = 4500.
    let hat = HostPad {
        pov: 4500,
        ..HostPad::CENTRED
    };
    let m = apple2_cards::snesmax::controller_buttons(&hat, &ButtonMap::MAIN);
    assert_eq!(m, buttons::UP | buttons::RIGHT);
    // Stick below 103 / above 153 on the high byte.
    let stick = HostPad {
        x: 0x6000,
        y: 0xA000,
        ..HostPad::CENTRED
    };
    let m = apple2_cards::snesmax::controller_buttons(&stick, &ButtonMap::MAIN);
    assert_eq!(m, buttons::LEFT | buttons::DOWN);
    assert_eq!(
        apple2_cards::snesmax::controller_buttons(&pad, &ButtonMap::MAIN),
        buttons::Y | buttons::B
    );
    assert_eq!(
        apple2_cards::snesmax::controller_buttons(&pad, &ButtonMap::ALT_DEFAULT),
        buttons::A | buttons::B
    );
}
