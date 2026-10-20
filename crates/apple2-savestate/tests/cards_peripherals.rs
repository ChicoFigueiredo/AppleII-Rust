//! Mouse, Z80 SoftCard, SNES MAX, VidHD and Breakpoint cards: golden YAML (transcribed from the
//! AppleWin writers), write format and full-machine round trips.

mod common;
use apple2_core::{Card, Machine, MachineConfig, Model};
use apple2_savestate::cards::{self, LoadCtx};
use apple2_savestate::yaml::{self, Map, Node, Writer};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, SaveOptions};
use common::*;

/// Parses a `State:` body (a list of `key: value` lines, indented freely) into nodes.
fn parse_state(body: &str) -> Vec<(String, Node)> {
    let text = format!(
        "State:\n{}",
        body.lines().map(|l| format!("  {l}\n")).collect::<String>()
    );
    match yaml::parse(&text).unwrap().remove(0).1 {
        Node::Map(m) => m,
        Node::Scalar(_) => panic!("State is not a map"),
    }
}

/// Runs the mapping for AppleWin card `aw_name` on a `State:` body.
fn golden(aw_name: &str, slot: u8, version: u32, body: &str) -> Box<dyn Card> {
    let entries = parse_state(body);
    let mut host = TestHost::default();
    let mut warnings = Vec::new();
    let mut ctx = LoadCtx {
        host: &mut host,
        model: Model::Apple2eEnhanced,
        warnings: &mut warnings,
    };
    let map = cards::by_aw_name(aw_name).unwrap_or_else(|| panic!("no mapping for {aw_name}"));
    let card = (map.load)(
        &mut ctx,
        slot,
        version,
        Map {
            name: "State",
            entries: &entries,
        },
    )
    .unwrap()
    .expect("card built");
    assert!(warnings.is_empty(), "{warnings:?}");
    card
}

/// The `<slot>:` entry text our card writes.
fn written(slot: u8, card: &dyn Card) -> String {
    let mut w = Writer::new("d", "v");
    assert!(cards::save_card(&mut w, slot, card));
    w.finish()
        .strip_prefix("# Date-stamp: d\n# v\n\n---\n")
        .unwrap()
        .to_string()
}

/// Original machine, its exact reload (extension) and its AppleWin-only reload.
struct Trip {
    orig: Machine,
    exact: Machine,
    plain: Machine,
}

/// Full machine -> YAML -> machine with `slot` holding `card`.
fn round_trip(slot: u8, card: Box<dyn Card>, run: impl FnOnce(&mut Machine)) -> Trip {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(slot, card));
    m.power_cycle();
    run(&mut m);
    let load = |opts: &SaveOptions| {
        let saved = save_aws_yaml(&m, opts).unwrap();
        assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
        let loaded = load_aws_yaml(&saved.text, &mut TestHost::default()).unwrap();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        loaded.machine
    };
    let exact = load(&SaveOptions::default());
    let plain = load(&SaveOptions {
        extension: false,
        ..Default::default()
    });
    Trip {
        orig: m,
        exact,
        plain,
    }
}

mod snesmax {
    use super::*;
    use apple2_cards::SnesMax;

    #[test]
    fn golden_button_index() {
        let c = golden("SNES MAX", 4, 1, "Button Index: 9");
        assert_eq!(
            c.as_any()
                .unwrap()
                .downcast_ref::<SnesMax>()
                .unwrap()
                .button_index(),
            9
        );
    }

    #[test]
    fn writes_button_index() {
        let mut c = SnesMax::new();
        c.restore_button_index(17);
        assert_eq!(
            written(4, &c),
            "4:\n  Card: SNES MAX\n  Version: 1\n  State:\n    Button Index: 17\n"
        );
    }

    #[test]
    fn machine_round_trip() {
        let mut c = SnesMax::new();
        c.restore_button_index(5);
        let t = round_trip(4, Box::new(c), |_| {});
        assert_eq!(t.orig.save_state(), t.exact.save_state());
        assert_eq!(t.plain.card::<SnesMax>(4).unwrap().button_index(), 5);
    }
}

mod vidhd {
    use super::*;
    use apple2_cards::{VidHd, VidHdSnapshot};
    use apple2_savestate::cards::{with_save_env, SaveEnv};

    const SNAP: VidHdSnapshot = VidHdSnapshot {
        mem_mode: 0x85,
        screen_color: 0x0F,
        new_video: 0xC1,
        border_color: 0x03,
        shadow: 0x08,
    };
    const BODY: &str = "Memory Mode: 0x00000085\nScreen Color: 0x0F\nNew Video: 0xC1\nBorder Color: 0x03\nShadow: 0x08";

    #[test]
    fn golden_registers() {
        let c = golden("VidHD", 2, 1, BODY);
        assert_eq!(
            c.as_any()
                .unwrap()
                .downcast_ref::<VidHd>()
                .unwrap()
                .snapshot(),
            SNAP
        );
    }

    #[test]
    fn golden_apple2_plus_requires_and_skips_the_aux_dump() {
        let body = format!(
            "{BODY}\nAuxiliary Memory Bank:\n  0400: 0102030405060708\n  9FF8: AABBCCDDEEFF0011"
        );
        let c = golden("VidHD", 2, 1, &body);
        assert_eq!(
            c.as_any()
                .unwrap()
                .downcast_ref::<VidHd>()
                .unwrap()
                .snapshot(),
            SNAP
        );
    }

    #[test]
    fn writes_registers_without_aux_dump_on_a_iie_with_aux() {
        let mut c = VidHd::new();
        c.apply_snapshot(&SNAP);
        let expect = format!(
            "2:\n  Card: VidHD\n  Version: 1\n  State:\n{}\n",
            BODY.lines()
                .map(|l| format!("    {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(written(2, &c), expect);
    }

    #[test]
    fn writes_aux_dump_on_apple2_plus() {
        let mut c = VidHd::new();
        c.apply_snapshot(&SNAP);
        let env = SaveEnv {
            model: Model::Apple2Plus,
            aux_empty: false,
        };
        let text = with_save_env(env, || written(2, &c));
        let (head, dump) = text.split_once("    Auxiliary Memory Bank:\n").unwrap();
        assert!(head.ends_with("    Shadow: 0x08\n"));
        let lines: Vec<&str> = dump.lines().collect();
        assert_eq!(lines.len(), 0x9C00 / 64);
        assert_eq!(lines[0], format!("      0400: {}", "00".repeat(64)));
        assert_eq!(
            lines[lines.len() - 1],
            format!("      9FC0: {}", "00".repeat(64))
        );
    }

    #[test]
    fn machine_round_trip() {
        let mut c = VidHd::new();
        c.apply_snapshot(&SNAP);
        let t = round_trip(2, Box::new(c), |_| {});
        assert_eq!(t.orig.save_state(), t.exact.save_state());
        // `reset` (power cycle) cleared NEWVIDEO; the file carries what the card held at save time.
        let held = t.orig.card::<VidHd>(2).unwrap().snapshot();
        assert_eq!(t.plain.card::<VidHd>(2).unwrap().snapshot(), held);
    }
}
