//! Unit layout (golden structure from AppleWin's writer code) and memory-card round trips.
//!
//! No real AppleWin save-state files are available here, so the fixtures below are transcribed
//! from the C++ writers (CPU.cpp:932, Joystick.cpp:1063, Keyboard.cpp:471, Speaker.cpp:924,
//! Video.cpp:305, Memory.cpp:2840-2990, LanguageCard.cpp:275/:492, YamlHelper.cpp:607).

mod common;
use apple2_core::{AuxCardKind, CardKind, Machine, MachineConfig, Model};
use apple2_savestate::yaml::{self, Map, Node};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, SaveOptions};
use common::*;

fn machine(model: Model, tweak: impl FnOnce(&mut MachineConfig)) -> Machine {
    let mut cfg = MachineConfig::for_model(model);
    tweak(&mut cfg);
    Machine::new(cfg)
}

/// Runs `code` at $0300 for `cycles`.
fn run_code(m: &mut Machine, code: &[u8], cycles: u64) {
    for (i, b) in code.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    m.run_cycles(cycles);
}

fn keys(m: Map) -> Vec<String> {
    m.entries.iter().map(|(k, _)| k.clone()).collect()
}

fn units(top: &[(String, Node)]) -> Vec<Map<'_>> {
    top.iter()
        .filter_map(|(k, n)| match n {
            Node::Map(e) if k == "Unit" => Some(Map {
                name: "Unit",
                entries: e,
            }),
            _ => None,
        })
        .collect()
}

fn roundtrip(m: &Machine, ext: bool) -> Machine {
    let saved = save_aws_yaml(
        m,
        &SaveOptions {
            extension: ext,
            ..Default::default()
        },
    )
    .unwrap();
    let mut host = TestHost::default();
    let loaded = load_aws_yaml(&saved.text, &mut host)
        .unwrap_or_else(|e| panic!("{e}\n{}", &saved.text[..saved.text.len().min(3000)]));
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    loaded.machine
}

#[test]
fn enhanced_iie_unit_layout_follows_applewin() {
    let mut m = machine(Model::Apple2eEnhanced, |_| {});
    m.cpu_mut().pc = 0x1234;
    let saved = save_aws_yaml(
        &m,
        &SaveOptions {
            date_stamp: "2026-01-01 00:00:00".into(),
            app_version: "test".into(),
            extension: false,
            rgb: Default::default(),
        },
    )
    .unwrap();
    let t = &saved.text;
    assert!(t.starts_with("# Date-stamp: 2026-01-01 00:00:00\n# test\n\n---\nFile_hdr:\n  Tag: AppleWin Save State\n  Version: 2\n\nUnit:\n  Type: Apple2\n  Version: 9\n  State:\n    Model: Enhanced Apple//e\n    CPU:\n      Type: 65C02\n      A: 0x"), "{}", &t[..300]);
    assert!(t.contains("      PC: 0x1234\n"));
    assert!(t.contains("      Defer IRQ By 1 Opcode: false\n"));
    assert!(t.contains("      Joystick1 TrimX: 0 # not implemented yet\n"));

    let top = yaml::parse(t).unwrap();
    assert_eq!(
        top.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        ["File_hdr", "Unit", "Unit", "Unit"]
    );
    let us = units(&top);
    let types: Vec<String> = us.iter().map(|u| u.string("Type").unwrap()).collect();
    // Apple2, aux slot, (no cards: no Slots unit), Game I/O Connector.
    assert_eq!(types, ["Apple2", "Auxiliary Slot", "Game I/O Connector"]);
    let st = us[0].need_sub("State").unwrap();
    assert_eq!(
        keys(st),
        [
            "Model",
            "CPU",
            "Joystick",
            "Keyboard",
            "Speaker",
            "Video",
            "Memory",
            "Main Memory"
        ]
    );
    assert_eq!(
        keys(st.sub("CPU").unwrap()),
        [
            "Type",
            "A",
            "X",
            "Y",
            "P",
            "S",
            "PC",
            "Cumulative Cycles",
            "Defer IRQ By 1 Opcode"
        ]
    );
    assert_eq!(
        keys(st.sub("Joystick").unwrap()),
        [
            "Joystick0 TrimX",
            "Joystick0 TrimY",
            "Joystick1 TrimX",
            "Joystick1 TrimY",
            "Paddle0 Inactive Cycle",
            "Paddle1 Inactive Cycle",
            "Paddle2 Inactive Cycle",
            "Paddle3 Inactive Cycle"
        ]
    );
    assert_eq!(
        keys(st.sub("Keyboard").unwrap()),
        ["Last Key", "Key Waiting"]
    );
    assert_eq!(keys(st.sub("Speaker").unwrap()), ["Last Cycle"]);
    assert_eq!(
        keys(st.sub("Video").unwrap()),
        [
            "Alt Char Set",
            "Video Mode",
            "Cycles This Frame",
            "Video Refresh Rate"
        ]
    );
    assert_eq!(
        keys(st.sub("Memory").unwrap()),
        [
            "Memory Mode",
            "MMU LC Mode",
            "Last RAM Write",
            "IO_SELECT",
            "IO_SELECT_InternalROM",
            "Expansion ROM Type",
            "Peripheral ROM Slot",
            "Last Slot to Set Main Mem LC",
            "Annunciator0",
            "Annunciator1",
            "Annunciator2",
            "Annunciator3"
        ]
    );
    // 64K of main memory in 64-byte lines, upper-case hex (YamlHelper.cpp:607).
    let mem = st.sub("Main Memory").unwrap();
    assert_eq!(mem.entries.len(), 1024);
    assert_eq!(mem.entries[0].0, "0000");
    assert_eq!(mem.entries[1023].0, "FFC0");
    let Node::Scalar(line) = &mem.entries[0].1 else {
        panic!()
    };
    assert_eq!(line.len(), 128);
    // Aux slot unit: Extended 80 Column, v4, a single bank, the RGB adaptor block.
    let aux = us[1].need_sub("State").unwrap();
    assert_eq!(keys(aux), ["Card", "Version", "State"]);
    assert_eq!(aux.string("Card").unwrap(), "Extended 80 Column");
    assert_eq!(aux.u32("Version").unwrap(), 4);
    let inner = aux.need_sub("State").unwrap();
    assert_eq!(
        keys(inner),
        [
            "Num Aux Banks",
            "Active Aux Bank",
            "Auxiliary Memory Bank00",
            "AppleColor RGB Adaptor"
        ]
    );
    assert!(
        t.contains("      Num Aux Banks: 0x001  # [0,1..100] 0=no aux mem, 1=128K system, etc\n")
    );
    assert!(t.contains("      Active Aux Bank: 0x00 # [  0..FF] 0=memaux\n"));
    // Game I/O connector, v4.
    let gio = us[2].need_sub("State").unwrap();
    assert_eq!(us[2].u32("Version").unwrap(), 4);
    assert_eq!(keys(gio), ["Shift Key Mod"]);
}

#[test]
fn apple2_plus_with_language_card_and_saturn_writes_slot_entries() {
    let mut m = machine(Model::Apple2Plus, |c| {
        c.slots[0] = Some(CardKind::LanguageCard16K);
        c.slots[4] = Some(CardKind::Saturn128K);
    });
    // Enable the slot-0 card for writing, store at $D000 / $FFF0; then bank 3 of the Saturn.
    let code = [
        0xAD, 0x81, 0xC0, 0xAD, 0x81,
        0xC0, // LC slot 0: read/write RAM, bank 2? ($C081 = ROM read, write RAM)
        0xA9, 0x5A, 0x8D, 0x00, 0xD0, 0x8D, 0xF0, 0xFF, 0x00,
    ];
    run_code(&mut m, &code, 200);
    let saved = save_aws_yaml(
        &m,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .unwrap();
    let top = yaml::parse(&saved.text).unwrap();
    let us = units(&top);
    let types: Vec<String> = us.iter().map(|u| u.string("Type").unwrap()).collect();
    assert_eq!(
        types,
        ["Apple2", "Slots", "Game I/O Connector"],
        "no aux unit for the ][ family"
    );
    let a2 = us[0].need_sub("State").unwrap();
    assert_eq!(
        keys(a2.sub("Memory").unwrap()),
        [
            "Memory Mode",
            "IO_SELECT",
            "IO_SELECT_InternalROM",
            "Expansion ROM Type",
            "Peripheral ROM Slot",
            "Last Slot to Set Main Mem LC",
            "Annunciator0",
            "Annunciator1",
            "Annunciator2",
            "Annunciator3"
        ]
    );
    assert_eq!(
        a2.sub("Main Memory").unwrap().entries.len(),
        768,
        "48K for the ][+"
    );
    let slots = us[1].need_sub("State").unwrap();
    assert_eq!(keys(slots), ["0", "4"]);
    let lc = slots.sub("0").unwrap();
    assert_eq!(lc.string("Card").unwrap(), "Language Card");
    assert_eq!(lc.u32("Version").unwrap(), 1);
    assert_eq!(
        keys(lc.need_sub("State").unwrap()),
        ["Memory Mode", "Last RAM Write", "Memory Bank"]
    );
    let sat = slots.sub("4").unwrap();
    assert_eq!(sat.string("Card").unwrap(), "Saturn 128");
    let ss = sat.need_sub("State").unwrap();
    let k = keys(ss);
    assert_eq!(
        &k[..4],
        [
            "Memory Mode",
            "Last RAM Write",
            "Num Saturn Banks",
            "Active Saturn Bank"
        ]
    );
    assert_eq!(k.len(), 4 + 8);
    assert_eq!(k[4], "Memory Bank00");
    assert!(
        saved
            .text
            .contains("      Num Saturn Banks: 0x08   # [1..8] 4=64K, 8=128K card\n")
            || saved.text.contains("Num Saturn Banks: 0x08")
    );
}

#[test]
fn ram_cards_round_trip_with_and_without_the_extension() {
    let cases: Vec<(&str, Machine)> = vec![
        (
            "][+ + LC + Saturn",
            machine(Model::Apple2Plus, |c| {
                c.slots[0] = Some(CardKind::LanguageCard16K);
                c.slots[4] = Some(CardKind::Saturn128K);
            }),
        ),
        (
            "//e RamWorks 8 banks",
            machine(Model::Apple2e, |c| {
                c.aux_card = AuxCardKind::RamWorksIII { banks: 8 }
            }),
        ),
        (
            "//e 80 column 1K",
            machine(Model::Apple2e, |c| c.aux_card = AuxCardKind::Col80_1K),
        ),
        (
            "//e no aux",
            machine(Model::Apple2e, |c| c.aux_card = AuxCardKind::Empty),
        ),
        (
            "//e + Saturn in slot 3",
            machine(Model::Apple2eEnhanced, |c| {
                c.slots[3] = Some(CardKind::Saturn128K)
            }),
        ),
        ("Pravets 82", machine(Model::Pravets82, |_| {})),
        ("original ][", machine(Model::Apple2, |_| {})),
        ("TK3000 //e", machine(Model::Tk3000e, |_| {})),
    ];
    for (name, mut m) in cases {
        // Dirty main/aux/LC RAM with a pattern and set some soft switches from code:
        // LC read RAM + write RAM bank 2 ($C083 x2), 80STORE/PAGE2/HIRES, an annunciator.
        for i in 0..0x4000u16 {
            m.poke_main(0x2000 + i, (i as u8).wrapping_mul(7));
            m.poke_aux(0x400 + (i & 0x3FF), (i as u8) ^ 0xA5);
        }
        let code = [
            0xAD, 0x83, 0xC0, 0xAD, 0x83, 0xC0, 0xA9, 0xC3, 0x8D, 0x00, 0xD1, 0x8D, 0xFE,
            0xFF, // LC RAM + stores
            0x8D, 0x01, 0xC0, 0x8D, 0x55, 0xC0, 0x8D, 0x57, 0xC0, 0xAD, 0x59, 0xC0, 0x00,
        ];
        run_code(&mut m, &code, 400);
        // Saturn bank 3 in slot 4/3 if present.
        for slot in [3u8, 4] {
            if m.board().lc.has_slot(slot) {
                let base = 0xC080 + slot as u16 * 16;
                let c = [
                    0xAD,
                    (base + 0x0B) as u8,
                    (base >> 8) as u8,
                    0xAD,
                    (base + 0x0B) as u8,
                    (base >> 8) as u8,
                    0xA9,
                    0x77,
                    0x8D,
                    0x10,
                    0xD0,
                    0x00,
                ];
                run_code(&mut m, &c, 200);
            }
        }
        let b = roundtrip(&m, true);
        assert_eq!(
            m.save_state(),
            b.save_state(),
            "{name}: extension round trip"
        );
        let c = roundtrip(&m, false);
        let (sa, sc) = (m.save_state(), c.save_state());
        assert_eq!(
            sa.memory.main[..0xC000],
            sc.memory.main[..0xC000],
            "{name}: main RAM"
        );
        assert_eq!(sa.memory.aux, sc.memory.aux, "{name}: aux RAM");
        assert_eq!(sa.memory.lc, sc.memory.lc, "{name}: LC RAM");
        assert_eq!(sa.memory.memmode, sc.memory.memmode, "{name}: memory flags");
        assert_eq!(
            sa.memory.lc_region, sc.memory.lc_region,
            "{name}: LC region"
        );
        assert_eq!(
            sa.memory.aux_bank, sc.memory.aux_bank,
            "{name}: RamWorks bank"
        );
        assert_eq!(sa.annunciators, sc.annunciators, "{name}: annunciators");
        assert_eq!(sa.video, sc.video, "{name}: video switches");
        // AppleWin keeps one "MMU LC Mode" for the //e: when a Saturn set the flags last, the
        // built-in card's own flags are not in the file.
        let infos = |s: &apple2_core::MachineState| {
            let last = s.lc.last_slot();
            s.lc.infos()
                .into_iter()
                .filter(|i| {
                    i.slot != 0
                        || last == 0
                        || i.kind != apple2_core::language_card::LcKind::Builtin
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(infos(&sa), infos(&sc), "{name}: LC cards");
        assert_eq!(sa.lc.last_slot(), sc.lc.last_slot(), "{name}: last LC slot");
        assert_eq!(sa.slots.io_select, sc.slots.io_select);
        assert_eq!(
            (sa.cpu.pc, sa.cpu.sp, sa.cpu.a),
            (sc.cpu.pc, sc.cpu.sp, sc.cpu.a),
            "{name}: registers"
        );
    }
}

#[test]
fn golden_apple2_plus_document_loads() {
    // Hand-written from the writers named in the module docs: a ][+ with a Language Card in
    // slot 0, text mode, PC=$FA62, A=$12, one byte of RAM set at $0400 and $D000 (bank 2 RAM).
    let mut text = String::from(
        "# Date-stamp: 1970-01-01 00:00:00\n# AppleWin 1.30.0\n\n---\nFile_hdr:\n  Tag: AppleWin Save State\n  Version: 2\n\nUnit:\n  Type: Apple2\n  Version: 9\n  State:\n    Model: Apple][+\n    CPU:\n      Type: 6502\n      A: 0x12\n      X: 0x34\n      Y: 0x56\n      P: 0x35\n      S: 0xF0\n      PC: 0xFA62\n      Cumulative Cycles: 0x0000000000012345\n      Defer IRQ By 1 Opcode: false\n    Joystick:\n      Joystick0 TrimX: 0\n      Joystick0 TrimY: 0\n      Joystick1 TrimX: 0 # not implemented yet\n      Joystick1 TrimY: 0 # not implemented yet\n      Paddle0 Inactive Cycle: 0x0000000000000000\n      Paddle1 Inactive Cycle: 0x0000000000000000\n      Paddle2 Inactive Cycle: 0x0000000000000000\n      Paddle3 Inactive Cycle: 0x0000000000000000\n    Keyboard:\n      Last Key: 0x41\n      Key Waiting: true\n    Speaker:\n      Last Cycle: 0x0000000000012000\n    Video:\n      Alt Char Set: false\n      Video Mode: 0x00000040\n      Cycles This Frame: 1234\n      Video Refresh Rate: 2\n    Memory:\n      Memory Mode: 0x00000000\n      IO_SELECT: 0x00\n      IO_SELECT_InternalROM: 0x00\n      Expansion ROM Type: 0\n      Peripheral ROM Slot: 0\n      Last Slot to Set Main Mem LC: 0\n      Annunciator0: false\n      Annunciator1: true\n      Annunciator2: false\n      Annunciator3: false\n    Main Memory:\n",
    );
    for line in 0..768 {
        let mut hex = "00".repeat(64);
        if line == 16 {
            hex.replace_range(0..2, "C1"); // $0400
        }
        text.push_str(&format!("      {:04X}: {hex}\n", line * 64));
    }
    text.push_str("\nUnit:\n  Type: Slots\n  Version: 1\n  State:\n    0:\n      Card: Language Card\n      Version: 1\n      State:\n        Memory Mode: 0x00000430\n        Last RAM Write: 0\n        Memory Bank:\n");
    for line in 0..256 {
        let mut hex = "00".repeat(64);
        if line == 0x40 {
            hex.replace_range(0..2, "D2"); // bank 2 $D000 is at offset $1000 = line 0x40
        }
        text.push_str(&format!("          {:04X}: {hex}\n", line * 64));
    }
    text.push_str(
        "\nUnit:\n  Type: Game I/O Connector\n  Version: 4\n  State:\n    Shift Key Mod: false\n",
    );

    let mut host = TestHost::default();
    let loaded = load_aws_yaml(&text, &mut host).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let m = loaded.machine;
    let s = m.save_state();
    assert_eq!(s.config.model, Model::Apple2Plus);
    assert_eq!(s.config.slots[0], Some(CardKind::LanguageCard16K));
    assert_eq!(
        (s.cpu.a, s.cpu.x, s.cpu.y, s.cpu.sp, s.cpu.pc),
        (0x12, 0x34, 0x56, 0xF0, 0xFA62)
    );
    assert_eq!(s.cpu.p & 0xCF, 0x35 & 0xCF);
    assert_eq!(s.cycles, 0x12345);
    assert_eq!(s.cycles_in_frame, 1234);
    assert_eq!(m.peek_main(0x400), 0xC1);
    assert_eq!(s.keyboard.latch(), (0x41, true));
    assert_eq!(s.annunciators, [false, true, false, false]);
    assert!(s.video.text);
    // LC flags: BANK2 | HIGHRAM | WRITERAM = $430 & mask = RAM read+write, bank 2.
    assert_eq!(s.memory.memmode & 0x430, 0x430);
    assert_eq!(m.peek(0xD000), 0xD2, "bank-2 RAM is visible at $D000");
}

#[test]
fn rejects_what_applewin_rejects() {
    let mut host = TestHost::default();
    let ok = "---\nFile_hdr:\n  Tag: AppleWin Save State\n  Version: 2\n";
    let err = |t: &str| {
        load_aws_yaml(t, &mut TestHost::default())
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
    };
    assert!(err("---\nFile_hdr:\n  Tag: Nope\n  Version: 2\n").contains("Bad tag"));
    assert!(
        err("---\nFile_hdr:\n  Tag: AppleWin Save State\n  Version: 3\n")
            .contains("Version mismatch")
    );
    assert!(err("a: 1\n").contains("Failed to find file header"));
    assert!(err(&format!(
        "{ok}\nUnit:\n  Type: Apple2\n  Version: 10\n  State:\n    Model: Apple][\n"
    ))
    .contains("Version mismatch"));
    assert!(err(&format!(
        "{ok}\nUnit:\n  Type: Apple2\n  Version: 9\n  State:\n    Model: Zorg\n"
    ))
    .contains("Unknown Apple2 type"));
    assert!(err(&format!("{ok}\nUnit:\n  Type: Apple2\n  Version: 9\n  State:\n    Model: Apple][\n    Memory:\n      IO_SELECT: 0x00\n")).contains("Missing"));
    assert!(err(ok).contains("Missing Apple2 unit"));
    let _ = &mut host;
}
