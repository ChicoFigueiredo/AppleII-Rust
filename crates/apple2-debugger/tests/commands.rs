//! Command-table level tests: every command survives odd arguments, help text exists, and the
//! command parser cases of the AppleWin console behave.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::cmdtable::COMMANDS;
use apple2_debugger::console::line_string;
use apple2_debugger::host::MemHost;
use apple2_debugger::{Debugger, RunMode};

fn machine() -> Machine {
    Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced))
}

fn console_text(d: &Debugger) -> String {
    d.console
        .lines()
        .map(line_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn last_line(d: &Debugger) -> String {
    d.console
        .lines()
        .last()
        .map(line_string)
        .unwrap_or_default()
}

#[test]
fn every_command_survives_odd_arguments() {
    let argsets = [
        "",
        "300",
        "300 301",
        "300:310",
        "300,10",
        "\"x\"",
        "a b c d e f g",
        "= = =",
        "( 300",
        "$",
        "# 5",
        "@ 1",
        "* ",
        "! x",
        "-1",
        "ffff ffff ffff ffff",
        "1 2 3 4 5 6 7 8",
    ];
    let mut m = machine();
    let mut d = Debugger::new();
    d.initialize(&mut m);
    d.enter(&mut m);
    for e in COMMANDS {
        for a in argsets {
            if matches!(e.name, "P" | "T" | "TL" | "RTS") && a.starts_with("ffff") {
                continue;
            }
            if matches!(
                e.name,
                "G" | "GG" | "RUN" | "STARTUP" | "BENCH" | "BENCHMARK"
            ) && !a.is_empty()
                && a.len() > 3
            {
                continue;
            }
            let line = format!("{} {}", e.name, a);
            d.execute(&mut m, &line);
            // back to a stopped debugger so the next command starts clean
            d.mode = RunMode::Debug;
            d.steps = 0;
            d.asm_active = false;
        }
    }
}

#[test]
fn every_command_with_a_table_entry_has_a_summary_or_is_an_alias() {
    let with_help = COMMANDS.iter().filter(|e| e.help.is_some()).count();
    assert!(with_help > 150, "{with_help}");
}

#[test]
fn help_commands() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.enter(&mut m);
    d.execute(&mut m, "help bpm");
    let t = console_text(&d);
    assert!(t.contains("BPM, Add breakpoint on memory access"), "{t}");
    assert!(t.contains("Usage: <range>"), "{t}");
    assert!(t.contains("[s<n>/][<nnn>/][l<1|2>/][rom/]<addr>"), "{t}");
    d.console.clear();
    d.execute(&mut m, "help breakpoints");
    assert!(console_text(&d).contains("Category: BREAKPOINTS"));
    assert!(console_text(&d).contains("BPX, Add breakpoint at current instruction"));
    d.console.clear();
    d.execute(&mut m, "?");
    assert!(console_text(&d).contains("Commands: "));
    d.console.clear();
    d.execute(&mut m, "help operators");
    assert!(console_text(&d).contains("Bit-wise exclusive-or"));
    d.console.clear();
    d.execute(&mut m, "help");
    assert!(
        console_text(&d).contains("BOOKMARKS | BREAKPOINTS"),
        "{}",
        console_text(&d)
    );
    // `BPR` with nothing to do prints its usage
    d.console.clear();
    d.execute(&mut m, "bpr");
    assert!(console_text(&d).contains("Usage: [A|X|Y|PC|S] [op] <range | value>"));
}

#[test]
fn commands_match_by_unique_prefix_and_report_ambiguity() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.enter(&mut m);
    d.execute(&mut m, "reg a 12");
    assert_eq!(
        m.cpu().a,
        0x12,
        "REGISTER is an alias of R but `reg` is a unique prefix"
    );
    d.console.clear();
    d.execute(&mut m, "bp");
    d.console.clear();
    d.execute(&mut m, "sy");
    assert!(
        console_text(&d).contains("Ambiguous"),
        "{}",
        console_text(&d)
    );
    d.console.clear();
    d.execute(&mut m, "BPX 300");
    d.execute(&mut m, "bpl");
    assert!(console_text(&d).contains("#"), "{}", console_text(&d));
}

#[test]
fn breakpoint_management_commands() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.enter(&mut m);
    d.execute(&mut m, "bpx 300");
    d.execute(&mut m, "bpmr D000:DFFF");
    d.execute(&mut m, "bpmw 1000,10");
    d.execute(&mut m, "bpv 100");
    d.execute(&mut m, "bpr x >= 80");
    assert_eq!(d.breakpoints.count, 5);
    d.execute(&mut m, "bpd 1");
    assert!(!d.breakpoints.slots[1].enabled);
    d.execute(&mut m, "bpe 1");
    assert!(d.breakpoints.slots[1].enabled);
    d.execute(&mut m, "bpchange 0 T s");
    assert!(d.breakpoints.slots[0].temp && !d.breakpoints.slots[0].stop);
    d.console.clear();
    d.execute(&mut m, "bpl");
    let t = console_text(&d);
    assert!(
        t.contains("0300")
            && t.contains("D000:DFFF")
            && t.contains("1000:100F")
            && t.contains("R  ")
            && t.contains("W  ")
            && t.contains("Vid"),
        "{t}"
    );
    d.execute(&mut m, "bpc 2");
    assert_eq!(d.breakpoints.count, 4);
    d.execute(&mut m, "bpc *");
    assert_eq!(d.breakpoints.count, 0);
    // prefixes: aux bank / language card / ROM
    d.execute(&mut m, "bpmw 01/L1/D000,1000");
    d.execute(&mut m, "bpmr ROM/FF58");
    let b = &d.breakpoints.slots[0];
    assert_eq!(
        (b.prefix.bank, b.prefix.lang_card, b.length),
        (Some(1), Some(1), 0x1000)
    );
    assert!(d.breakpoints.slots[1].prefix.is_rom);
    d.console.clear();
    d.execute(&mut m, "bpl");
    assert!(
        console_text(&d).contains("01/l1/D000:DFFF"),
        "{}",
        console_text(&d)
    );
    // the script that recreates them
    let script = d.breakpoints_script();
    assert!(
        script.contains("BPMW 01/l1/D000:DFFF") && script.contains("BPMR ROM/FF58"),
        "{script}"
    );
}

#[test]
fn watches_zero_page_and_bookmarks() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.enter(&mut m);
    d.execute(&mut m, "w 300");
    d.execute(&mut m, "wa 5 400");
    assert_eq!(d.watches.count, 2);
    d.execute(&mut m, "w C030");
    assert!(console_text(&d).contains("cannot watch an I/O"));
    d.execute(&mut m, "wd 0");
    d.execute(&mut m, "wc 5");
    assert_eq!(d.watches.count, 1);
    d.execute(&mut m, "zp 50");
    d.execute(&mut m, "zp 2 60");
    d.execute(&mut m, "p3 70");
    assert_eq!(d.zero_page.count, 3);
    d.execute(&mut m, "p3");
    assert!(!d.zero_page.slots[3].enabled);
    d.execute(&mut m, "bm 300");
    d.execute(&mut m, "bma 5 FA62");
    assert_eq!(d.bookmarks[0], Some(0x300));
    assert_eq!(d.bookmarks[5], Some(0xFA62));
    d.execute(&mut m, "bmg 5");
    assert_eq!(d.disasm_cur, 0xFA62);
    d.execute(&mut m, "bmc *");
    assert!(d.bookmarks.iter().all(|b| b.is_none()));
}

#[test]
fn file_commands_use_the_host() {
    let mut m = machine();
    let mut d = Debugger::new();
    let mut host = MemHost::default();
    host.files.insert(
        "script.txt".into(),
        b"// comment\nr a 77\nr x 88\necho done\n".to_vec(),
    );
    d.set_host(Box::new(host));
    d.enter(&mut m);
    d.execute(&mut m, "300: 01 02 03 04");
    d.execute(&mut m, "bsave \"t.bin\",300,4");
    d.execute(&mut m, "f 300 303 ff");
    d.execute(&mut m, "bload \"t.bin\",300");
    assert_eq!(
        (m.peek(0x300), m.peek(0x303)),
        (1, 4),
        "{}",
        console_text(&d)
    );
    d.execute(&mut m, "run \"script.txt\"");
    assert_eq!((m.cpu().a, m.cpu().x), (0x77, 0x88));
    assert!(console_text(&d).contains("done"));
    d.execute(&mut m, "bpx 1234");
    d.execute(&mut m, "bpsave \"bps.txt\"");
    d.execute(&mut m, "bpc *");
    d.execute(&mut m, "run \"bps.txt\"");
    assert_eq!(d.breakpoints.count, 1);
    assert_eq!(d.breakpoints.slots[0].address, 0x1234);
    d.execute(&mut m, "tsave \"scr.txt\"");
    assert!(
        console_text(&d).contains("Saved: scr.txt"),
        "{}",
        console_text(&d)
    );
}

#[test]
fn calc_print_and_printf() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.enter(&mut m);
    d.execute(&mut m, "r a 41");
    d.execute(&mut m, "calc a");
    assert!(
        last_line(&d).starts_with("$0041  0z01000001     65  'A'"),
        "{}",
        last_line(&d)
    );
    d.execute(&mut m, "print \"A:\",A,\" hi\"");
    assert_eq!(last_line(&d), "A:0041 hi");
    d.execute(&mut m, "printf \"Dec:%d Hex:%x Bin:%z Chr:%c\",A,A,A,A");
    assert_eq!(last_line(&d), "Dec:65 Hex:0041 Bin:01000001 Chr:A");
    d.execute(&mut m, "echo hello  world");
    assert_eq!(last_line(&d), "hello  world");
    d.execute(&mut m, "log");
    assert!(last_line(&d).contains("Verbosity level set to"));
}

#[test]
fn flags_and_stack_commands() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.enter(&mut m);
    d.execute(&mut m, "r p 20");
    d.execute(&mut m, "sec");
    d.execute(&mut m, "sei");
    assert_eq!(m.cpu().p, 0x25);
    d.execute(&mut m, "clc");
    d.execute(&mut m, "se n v");
    assert_eq!(m.cpu().p, 0xE4);
    d.execute(&mut m, "cl n");
    assert_eq!(m.cpu().p & 0x80, 0);
    d.execute(&mut m, "push 12 34");
    assert_eq!(m.cpu().sp, 0xFA);
    d.execute(&mut m, "pop a");
    assert_eq!((m.cpu().a, m.cpu().sp), (0x34, 0xFB));
    d.execute(&mut m, "jsr 300");
    assert_eq!(m.cpu().pc, 0x300);
    assert_eq!(m.cpu().sp, 0xF9);
}

#[test]
fn input_line_editing_and_history() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.enter(&mut m);
    for c in "r a 99".chars() {
        d.console.input_char(c);
    }
    d.input_enter(&mut m);
    assert_eq!(m.cpu().a, 0x99);
    assert_eq!(d.console.history(), ["r a 99"]);
    d.console.history_prev();
    assert_eq!(d.console.input(), "r a 99");
}

#[test]
fn profile_cycles_and_exit() {
    let mut m = machine();
    for (i, b) in [0xA9u8, 0x01, 0xAA, 0xE8, 0xE8, 0xEA, 0x4C, 0x00, 0x03]
        .iter()
        .enumerate()
    {
        m.poke(0x300 + i as u16, *b);
    }
    let mut d = Debugger::new();
    d.enter(&mut m);
    d.execute(&mut m, "r pc 300");
    d.execute(&mut m, "profile");
    d.execute(&mut m, "t 7");
    d.run(&mut m, 1000);
    d.console.clear();
    d.execute(&mut m, "profile list");
    let t = console_text(&d);
    assert!(t.contains("Immediate") && t.contains("Total:"), "{t}");
    assert!(t.contains("INX") && t.contains("(implied)"), "{t}");
    // CYCLES REL shows the last instruction; the video scanner moves with the machine.
    d.execute(&mut m, "cycles rel");
    let before = m.cycles();
    d.execute(&mut m, "t");
    let g = d.render(&m);
    let last = m.cycles() - before;
    assert!(
        g.text().contains(&format!("cycles:{last:08X}")),
        "last instruction took {last} cycles:\n{}",
        g.text()
    );
    // Leaving the debugger with no breakpoints runs free; with one it keeps stepping.
    d.exit(&mut m);
    assert_eq!(d.mode, RunMode::Running);
    d.enter(&mut m);
    d.execute(&mut m, "bpx 305");
    d.exit(&mut m);
    assert_eq!(d.mode, RunMode::Stepping);
    d.run(&mut m, 100);
    assert_eq!((d.mode, m.cpu().pc), (RunMode::Debug, 0x305));
    // `ADDRG` leaves the debugger at an address.
    d.execute(&mut m, "bpc *");
    d.execute(&mut m, "400G");
    assert_eq!((d.mode, m.cpu().pc), (RunMode::Running, 0x400));
}

#[test]
fn auto_named_symbols_and_video_breakpoints() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.enter(&mut m);
    d.execute(&mut m, "sym @ = 1234");
    assert_eq!(
        d.symbols.find_address("_1234").map(|x| x.1),
        Some(apple2_debugger::symbols::Table::User2)
    );
    d.execute(&mut m, "bpv 40");
    d.execute(&mut m, "g");
    d.run(&mut m, 100_000);
    assert_eq!(d.mode, RunMode::Debug);
    assert_eq!(m.video_position().0, 0x40, "{}", console_text(&d));
    assert!(console_text(&d).contains("Video scanner position matches"));
}

#[test]
fn data_size_variants_and_word_rounding() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.enter(&mut m);
    for (i, b) in [1u8, 2, 3, 4, 5, 6, 7, 8, 9].iter().enumerate() {
        m.poke(0x2000 + i as u16, *b);
    }
    d.execute(&mut m, "db4 tbl = 2000:2007");
    d.execute(&mut m, "u 2000");
    let t = d.render(&m).text();
    assert!(
        t.contains("2000:01 02 03") && t.contains("tbl") && t.contains("db"),
        "{t}"
    );
    d.execute(&mut m, "dw 2000");
    assert_eq!(
        d.data.find(0x2001).map(|x| x.end),
        Some(0x2001),
        "DW rounds the length up to a word"
    );
    d.execute(&mut m, "b");
    assert!(console_text(&d).contains("word1"), "{}", console_text(&d));
}
