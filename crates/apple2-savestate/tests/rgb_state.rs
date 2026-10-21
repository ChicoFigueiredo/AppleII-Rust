//! AUD-P-005: the RGB card state (`RGB_SaveSnapshot`, RGBMonitor.cpp:1311) goes through the
//! save-state with AppleWin's field names, and an AppleWin-written file is read back.

mod common;
use apple2_core::{AuxCardKind, Machine, MachineConfig, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, RgbCardState, SaveOptions};
use common::*;

fn ext80() -> Machine {
    let mut cfg = MachineConfig::for_model(Model::Apple2e);
    cfg.aux_card = AuxCardKind::Extended80Col;
    Machine::new(cfg)
}

fn rgb() -> RgbCardState {
    RgbCardState {
        flags: 0x03,
        mode: 0x02,
        prev_an3: 0x5E,
        invert_bit7: true,
    }
}

#[test]
fn rgb_state_is_written_with_applewin_field_names() {
    let saved = save_aws_yaml(
        &ext80(),
        &SaveOptions {
            extension: false,
            rgb: rgb(),
            ..Default::default()
        },
    )
    .unwrap();
    let t = &saved.text;
    assert!(t.contains("AppleColor RGB Adaptor:\n"));
    for line in [
        "RGB mode flags: 0x03\n",
        "RGB mode: 0x02\n",
        "Previous AN3: 0x5E\n",
        "80COL changed: false\n",
        "Invert bit7: true\n",
    ] {
        assert!(t.contains(line), "faltou {line:?} em:\n{t}");
    }
}

#[test]
fn rgb_state_round_trips() {
    let saved = save_aws_yaml(
        &ext80(),
        &SaveOptions {
            extension: false,
            rgb: rgb(),
            ..Default::default()
        },
    )
    .unwrap();
    let loaded = load_aws_yaml(&saved.text, &mut TestHost::default()).unwrap();
    assert_eq!(loaded.rgb, Some(rgb()));
    // Saving again from the loaded values is byte-identical (same date stamp).
    let again = save_aws_yaml(
        &loaded.machine,
        &SaveOptions {
            extension: false,
            rgb: loaded.rgb.unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(again.text, saved.text);
}

#[test]
fn applewin_written_rgb_block_is_read() {
    // Real AppleWin file (Apple //e, Extended 80 Column): flags 0, mode 0, Previous AN3 0x5F.
    let path = format!(
        "{}/../../tests/conformance/inputs/applewin-real/apple2ee-applesoft-prompt.aws.yaml",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).unwrap();
    let loaded = load_aws_yaml(&text, &mut TestHost::default()).unwrap();
    assert_eq!(
        loaded.rgb,
        Some(RgbCardState {
            flags: 0,
            mode: 0,
            prev_an3: 0x5F,
            invert_bit7: false
        })
    );
    // The same file with another RGB state: the fields are honoured.
    let edited = text
        .replace("RGB mode flags: 0x00", "RGB mode flags: 0x01")
        .replace("RGB mode: 0x00", "RGB mode: 0x03")
        .replace("Invert bit7: false", "Invert bit7: true");
    let loaded = load_aws_yaml(&edited, &mut TestHost::default()).unwrap();
    let r = loaded.rgb.unwrap();
    assert_eq!((r.flags, r.mode, r.invert_bit7), (1, 3, true));
}

#[test]
fn machines_without_the_extended_aux_card_carry_no_rgb_block() {
    let m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    let saved = save_aws_yaml(
        &m,
        &SaveOptions {
            extension: false,
            rgb: rgb(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!saved.text.contains("AppleColor RGB Adaptor"));
    let loaded = load_aws_yaml(&saved.text, &mut TestHost::default()).unwrap();
    assert_eq!(loaded.rgb, None);
}
