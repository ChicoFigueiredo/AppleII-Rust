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
        Node::Scalar(_) => Vec::new(), // `State: null`
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

mod mouse {
    use super::*;
    use apple2_cards::pia6821::Pia6821;
    use apple2_cards::{MouseCard, MouseSnapshot};

    /// MouseInterface.cpp:647-697 field by field; negative `m_nX` is saved through `SaveUint`.
    const BODY: &str = "MC6821:
  PRA: 10
  DDRA: 255
  CRA: 4
  PRB: 3
  DDRB: 62
  CRB: 4
  IA: 170
  IB: 192
DataLen: 6
Mode: 0x0F
6821B: 0x7E
6821A: 0x55
Buffer:
  0000: 1011121314150000
Buffer Position: 3
State: 0xA6
X: 4294967294
Y: 300
Btn0: true
Btn1: false
VBL: false
iX: 515
iMinX: 10
iMaxX: 900
iY: 4294967295
iMinY: 20
iMaxY: 800
Button0: false
Button1: true
Enabled: true";

    fn expected() -> MouseSnapshot {
        MouseSnapshot {
            pia: Pia6821 {
                pra: 10,
                ddra: 255,
                cra: 4,
                prb: 3,
                ddrb: 62,
                crb: 4,
                ia: 170,
                ib: 192,
            },
            by_a: 0x55,
            by_b: 0x7E,
            buff: [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0, 0],
            buf_pos: 3,
            data_len: 6,
            mode: 0x0F,
            state: 0xA6,
            n_x: -2,
            n_y: 300,
            i_x: 515,
            min_x: 10,
            max_x: 900,
            i_y: -1,
            min_y: 20,
            max_y: 800,
            btn0: true,
            btn1: false,
            buttons: [false, true],
        }
    }

    #[test]
    fn golden_state() {
        let c = golden("Mouse Card", 4, 1, BODY);
        assert_eq!(
            c.as_any()
                .unwrap()
                .downcast_ref::<MouseCard>()
                .unwrap()
                .snapshot(),
            expected()
        );
    }

    #[test]
    fn writes_the_applewin_keys_in_order() {
        let mut c = MouseCard::new();
        c.apply_snapshot(&expected());
        let body: String = BODY.lines().map(|l| format!("    {l}\n")).collect();
        assert_eq!(
            written(4, &c),
            format!("4:\n  Card: Mouse Card\n  Version: 1\n  State:\n{body}")
        );
    }

    #[test]
    fn rejects_other_versions() {
        let entries = parse_state(BODY);
        let mut host = TestHost::default();
        let mut warnings = Vec::new();
        let mut ctx = LoadCtx {
            host: &mut host,
            model: Model::Apple2eEnhanced,
            warnings: &mut warnings,
        };
        let map = cards::by_aw_name("Mouse Card").unwrap();
        assert!((map.load)(
            &mut ctx,
            4,
            2,
            Map {
                name: "State",
                entries: &entries
            }
        )
        .is_err());
    }

    #[test]
    fn machine_round_trip_with_a_pending_interrupt() {
        let t = round_trip(4, Box::new(MouseCard::new()), |m| {
            let mut s = m.card::<MouseCard>(4).unwrap().snapshot();
            s.mode = 0x0F; // mouse on, all interrupts
            s.i_x = 40;
            s.i_y = 25;
            s.n_x = 30;
            m.card_mut::<MouseCard>(4).unwrap().apply_snapshot(&s);
            apple2_cards::mouse_move(m, 4, 5, -3); // movement interrupt raised
            m.run_frame();
        });
        let s = t.orig.card::<MouseCard>(4).unwrap().snapshot();
        assert!(
            s.state & apple2_cards::mouse::STAT_INT_ALL != 0,
            "interrupt pending: {s:?}"
        );
        assert_eq!(t.orig.save_state(), t.exact.save_state());
        assert_eq!(t.plain.card::<MouseCard>(4).unwrap().snapshot(), s);
        // GH#677: the loader re-asserts the IRQ line.
        assert_eq!(
            t.plain.save_state().interrupts,
            t.orig.save_state().interrupts
        );
    }
}

mod softcard {
    use super::*;
    use apple2_cards::SoftCard;

    const BODY: &str = "Active: 1\nA: 0x11\nB: 0x22\nC: 0x33\nD: 0x44\nE: 0x55\nF: 0x66\nH: 0x77\nL: 0x88\nIX: 0x1234\nIY: 0x5678\nSP: 0x9ABC\nPC: 0xDEF0\nI: 0x12\nR: 0x34\nIFF1: 0x01\nIFF2: 0x00\nIM Mode: 0x02\nA': 0xA1\nB': 0xB2\nC': 0xC3\nD': 0xD4\nE': 0xE5\nF': 0xF6\nH': 0x17\nL': 0x28";

    /// The registers AppleWin saves, in its order.
    fn regs(c: &SoftCard) -> Vec<u32> {
        let z = c.z80().unwrap();
        vec![
            c.is_active() as u32,
            z.a as u32,
            z.b as u32,
            z.c as u32,
            z.d as u32,
            z.e as u32,
            z.f as u32,
            z.h as u32,
            z.l as u32,
            z.ix as u32,
            z.iy as u32,
            z.sp as u32,
            z.pc as u32,
            z.i as u32,
            z.r as u32,
            z.iff1 as u32,
            z.iff2 as u32,
            z.im as u32,
            z.a2 as u32,
            z.b2 as u32,
            z.c2 as u32,
            z.d2 as u32,
            z.e2 as u32,
            z.f2 as u32,
            z.h2 as u32,
            z.l2 as u32,
        ]
    }

    const EXPECT: [u32; 26] = [
        1, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x1234, 0x5678, 0x9ABC, 0xDEF0, 0x12,
        0x34, 1, 0, 2, 0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x17, 0x28,
    ];

    fn sample() -> SoftCard {
        let mut c = golden("Z80 SoftCard", 4, 1, BODY);
        let c = c.as_any_mut().unwrap().downcast_mut::<SoftCard>().unwrap();
        let mut n = SoftCard::new();
        n.load(&c.save());
        n
    }

    #[test]
    fn golden_registers_and_old_card_name() {
        for name in ["Z80 SoftCard", "Z80"] {
            let c = golden(name, 4, 1, BODY);
            assert_eq!(
                regs(c.as_any().unwrap().downcast_ref::<SoftCard>().unwrap()),
                EXPECT,
                "{name}"
            );
        }
    }

    #[test]
    fn writes_registers_in_applewin_order() {
        let c = sample();
        assert_eq!(regs(&c), EXPECT);
        let body: String = BODY.lines().map(|l| format!("    {l}\n")).collect();
        assert_eq!(
            written(4, &c),
            format!("4:\n  Card: Z80 SoftCard\n  Version: 1\n  State:\n{body}")
        );
    }

    #[test]
    fn slot_zero_is_refused() {
        let entries = parse_state(BODY);
        let mut host = TestHost::default();
        let mut warnings = Vec::new();
        let mut ctx = LoadCtx {
            host: &mut host,
            model: Model::Apple2eEnhanced,
            warnings: &mut warnings,
        };
        let map = cards::by_aw_name("Z80 SoftCard").unwrap();
        assert!((map.load)(
            &mut ctx,
            0,
            1,
            Map {
                name: "State",
                entries: &entries
            }
        )
        .is_err());
    }

    #[test]
    fn machine_round_trip_with_the_z80_running() {
        let t = round_trip(4, Box::new(SoftCard::new()), |m| {
            m.with_card(4, |c, ctx| c.slot_rom_write(0, 0, ctx)); // hand the CPU to the Z80
            assert_eq!(m.cpu_owner(), Some(4));
            m.card_mut::<SoftCard>(4).unwrap().z80_mut().unwrap().pc = 0x0100;
            for _ in 0..3 {
                m.run_frame();
            }
        });
        assert_eq!(t.orig.cpu_owner(), Some(4));
        assert_eq!(t.orig.save_state(), t.exact.save_state());
        assert_eq!(
            t.plain.cpu_owner(),
            Some(4),
            "Active restores the CPU owner"
        );
        assert_eq!(
            regs(t.plain.card::<SoftCard>(4).unwrap()),
            regs(t.orig.card::<SoftCard>(4).unwrap())
        );
    }
}

mod breakpoint {
    use super::*;
    use apple2_cards::BreakpointCard;

    #[test]
    fn golden_null_state() {
        let c = golden("Breakpoint", 5, 1, "");
        assert_eq!(c.name(), "Breakpoint");
    }

    #[test]
    fn writes_null_state() {
        assert_eq!(
            written(5, &BreakpointCard::new()),
            "5:\n  Card: Breakpoint\n  Version: 1\n  State: null\n"
        );
    }

    #[test]
    fn machine_round_trip() {
        let t = round_trip(5, Box::new(BreakpointCard::new()), |m| {
            m.run_frame();
        });
        assert_eq!(t.orig.save_state(), t.exact.save_state());
        assert!(t.plain.card::<BreakpointCard>(5).is_some());
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

    /// GH#1108: after loading a save-state with a VidHD in slot 3, `$C3nn` reads the card's ROM
    /// bytes (`24 EA 4C`), not the floating bus (SLOTC3ROM on, so the card ROM is visible).
    #[test]
    fn gh1108_slot3_rom_is_set_after_load() {
        use apple2_cpu::Bus;
        let mut t = round_trip(3, Box::new(VidHd::new()), |m| {
            m.board_mut().write(0xC00B, 0); // SLOTC3ROM
        });
        for (name, m) in [
            ("original", &mut t.orig),
            ("loaded", &mut t.exact),
            ("loaded without extension", &mut t.plain),
        ] {
            m.board_mut().write(0xC00B, 0);
            let rom = [0xC300u16, 0xC301, 0xC302].map(|a| m.board_mut().read(a));
            assert_eq!(rom, [0x24, 0xEA, 0x4C], "{name}");
        }
    }
}
