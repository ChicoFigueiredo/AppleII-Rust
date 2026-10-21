//! The debugger driving a real `Machine`: breakpoints on real firmware, stepping, tracing,
//! command processing and the screen grid.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use apple2_core::{IrqSource, Machine, MachineConfig, Model};
use apple2_debugger::console::line_string;
use apple2_debugger::display::INFO_COL_1;
use apple2_debugger::{Debugger, RunMode};

/// Machine with a 12K custom ROM of NOPs whose RESET vector is $0300 (RAM zeroed = BRK).
fn bare(model: Model) -> Machine {
    let mut rom = vec![0xEAu8; 0x3000];
    for (vec, target) in [(0xFFFAu16, 0x0500u16), (0xFFFC, 0x0300), (0xFFFE, 0x0400)] {
        let o = vec as usize - 0xD000;
        rom[o] = target as u8;
        rom[o + 1] = (target >> 8) as u8;
    }
    let mut cfg = MachineConfig::for_model(model);
    cfg.custom_rom = Some(rom);
    Machine::new(cfg)
}

fn load(m: &mut Machine, at: u16, bytes: &[u8]) {
    for (i, b) in bytes.iter().enumerate() {
        m.poke(at + i as u16, *b);
    }
}

fn console_text(d: &Debugger) -> String {
    d.console
        .lines()
        .map(line_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn debugger(m: &mut Machine) -> Debugger {
    let mut d = Debugger::new();
    d.initialize(m);
    d.enter(m);
    d
}

/// Runs the stepping loop until the debugger stops (or the cycle budget is gone).
fn go(d: &mut Debugger, m: &mut Machine, budget: u64) -> u64 {
    assert!(
        d.is_stepping(),
        "G did not start stepping:\n{}",
        console_text(d)
    );
    d.run(m, budget)
}

#[test]
fn pc_breakpoint_on_cout_during_boot() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    let mut d = debugger(&mut m);
    d.execute(&mut m, "bp FDED");
    assert_eq!(d.breakpoints.count, 1);
    d.execute(&mut m, "g");
    go(&mut d, &mut m, 3_000_000);
    assert_eq!(d.mode, RunMode::Debug, "{}", console_text(&d));
    assert_eq!(m.cpu().pc, 0xFDED);
    let text = console_text(&d);
    assert!(
        text.contains("Stop reason: [B#0] Register PC matches value"),
        "{text}"
    );
    // The code window is centered on COUT and shows its symbol from APPLE2E.SYM.
    let g = d.render(&m);
    assert!(
        g.text()
            .lines()
            .any(|l| l.starts_with("FDED:") && l.contains("COUT")),
        "{}",
        g.text()
    );
    // Hit counters are tracked and the breakpoint list shows it.
    d.execute(&mut m, "bpl");
    assert!(
        console_text(&d).contains("#0 E  S    -  *   00000001"),
        "{}",
        console_text(&d)
    );
}

#[test]
fn memory_write_breakpoint_on_the_text_page() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut d = debugger(&mut m);
    d.execute(&mut m, "bpmw 400:7FF");
    d.execute(&mut m, "g");
    go(&mut d, &mut m, 3_000_000);
    assert_eq!(d.mode, RunMode::Debug);
    let text = console_text(&d);
    assert!(text.contains("Write access at"), "{text}");
    // AppleWin stops *before* the instruction that is about to write: the opcode at PC is a store.
    let op = m.peek(m.cpu().pc);
    let info = apple2_debugger::opcodes::OPCODES_65C02[op as usize];
    assert!(
        info.mem & apple2_debugger::opcodes::MEM_W != 0,
        "PC=${:04X} {} {:?}",
        m.cpu().pc,
        info.mnemonic,
        info.mode
    );
}

#[test]
fn io_breakpoint_on_the_speaker() {
    let mut m = bare(Model::Apple2Plus);
    load(&mut m, 0x300, &[0xAD, 0x30, 0xC0, 0xEA, 0xEA]); // LDA $C030
    let mut d = debugger(&mut m);
    d.execute(&mut m, "bpio C030");
    d.execute(&mut m, "g");
    go(&mut d, &mut m, 10_000);
    assert_eq!(m.cpu().pc, 0x300, "stops before the access");
    assert!(
        console_text(&d).contains("Memory access at"),
        "{}",
        console_text(&d)
    );
    assert_eq!(m.drain_speaker_toggles().count(), 0);
    d.execute(&mut m, "t");
    assert_eq!(m.cpu().pc, 0x303);
    assert_eq!(m.drain_speaker_toggles().count(), 1);
}

#[test]
fn bus_observer_catches_accesses_the_target_prediction_cannot_see() {
    // An IRQ entry pushes PC on the stack without any instruction naming $01FF.
    let mut m = bare(Model::Apple2Plus);
    load(&mut m, 0x300, &[0x58, 0xEA, 0xEA, 0xEA]); // CLI
    load(&mut m, 0x400, &[0xEA, 0xEA]);
    let mut d = debugger(&mut m);
    d.execute(&mut m, "bpmw 1FC");
    m.board_mut().interrupts.set_irq(IrqSource::Mouse, true);
    d.execute(&mut m, "g");
    go(&mut d, &mut m, 10_000);
    assert_eq!(d.mode, RunMode::Debug);
    assert!(
        console_text(&d).contains("Write access at 01FC"),
        "{}",
        console_text(&d)
    );
    assert_eq!(m.cpu().pc, 0x400, "stopped right after the interrupt entry");
}

#[test]
fn register_breakpoints_and_go_until() {
    let mut m = bare(Model::Apple2Plus);
    load(&mut m, 0x300, &[0xA9, 0x01, 0xA9, 0x42, 0xA9, 0x99, 0xEA]);
    let mut d = debugger(&mut m);
    d.execute(&mut m, "bpr a = 42");
    d.execute(&mut m, "g");
    go(&mut d, &mut m, 1000);
    assert_eq!((m.cpu().a, m.cpu().pc), (0x42, 0x304));
    assert!(console_text(&d).contains("Register A matches value"));
    // `g addr` stops at the address; temp breakpoints and flags are cleared by `bpc *`.
    d.execute(&mut m, "bpc *");
    d.execute(&mut m, "r pc 300");
    d.execute(&mut m, "g 304");
    go(&mut d, &mut m, 1000);
    assert_eq!(m.cpu().pc, 0x304);
    assert!(console_text(&d).contains("Go until"));
}

#[test]
fn step_over_runs_the_subroutine_and_step_out_returns() {
    let mut m = bare(Model::Apple2Plus);
    load(&mut m, 0x300, &[0x20, 0x10, 0x03, 0xEA]); // JSR $310
    load(&mut m, 0x310, &[0xE6, 0x10, 0xE6, 0x10, 0x60]); // INC $10 ; INC $10 ; RTS
    let mut d = debugger(&mut m);
    d.execute(&mut m, "p");
    assert_eq!((m.cpu().pc, m.peek(0x10)), (0x303, 2));
    d.execute(&mut m, "r pc 300");
    d.execute(&mut m, "t"); // into the JSR
    assert_eq!(m.cpu().pc, 0x310);
    d.execute(&mut m, "t");
    d.execute(&mut m, "rts");
    go(&mut d, &mut m, 1000);
    assert_eq!(m.cpu().pc, 0x303);
}

#[test]
fn trace_goes_to_a_writer_in_applewin_format() {
    struct Sink(Rc<RefCell<Vec<u8>>>);
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut m = bare(Model::Apple2Plus);
    load(&mut m, 0x300, &[0xA9, 0x41, 0xAA, 0x8D, 0x00, 0x04, 0xEA]);
    let mut d = debugger(&mut m);
    let out = Rc::new(RefCell::new(Vec::new()));
    d.set_trace_writer(Some(Box::new(Sink(out.clone()))));
    d.execute(&mut m, "t 4");
    go(&mut d, &mut m, 1000);
    let text = String::from_utf8(out.borrow().clone()).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0],
        "Cycles   A: X: Y: SP:  Flags     Addr:Opcode    Mnemonic"
    );
    assert!(
        lines[1].ends_with("FF FF FF 01FC ..R..I..  0300:A9 41     LDA #$41"),
        "{}",
        lines[1]
    );
    assert!(
        lines[2].contains(" 41 FF FF 01FC") && lines[2].contains("0302:AA        TAX"),
        "{}",
        lines[2]
    );
    assert!(
        lines[3].contains("0303:8D 00 04  STA $0400") && lines[3].contains(" 41 41 FF 01FC"),
        "{}",
        lines[3]
    );
}

#[test]
fn monitor_style_shortcuts_edit_and_list_memory() {
    let mut m = bare(Model::Apple2e);
    let mut d = debugger(&mut m);
    d.execute(&mut m, "300: A9 00 8D 00 04 4C 00 03");
    assert_eq!(
        (m.peek(0x300), m.peek(0x305), m.peek(0x307)),
        (0xA9, 0x4C, 0x03)
    );
    d.execute(&mut m, "300L");
    assert_eq!(d.disasm_top, 0x300);
    let g = d.render(&m);
    assert!(g.text().contains("0300:A9 00"), "{}", g.text());
    d.execute(&mut m, "r a = 5A");
    d.execute(&mut m, "r x 12");
    assert_eq!((m.cpu().a, m.cpu().x), (0x5A, 0x12));
    d.execute(&mut m, "f 2000:20FF 77");
    d.execute(&mut m, "4000<2000.20FFM");
    assert_eq!((m.peek(0x4000), m.peek(0x40FF)), (0x77, 0x77));
    d.execute(&mut m, "s 2000,100 77 77");
    assert!(
        console_text(&d).contains("Total: 255"),
        "{}",
        console_text(&d)
    );
    d.execute(&mut m, "bogus");
    assert!(console_text(&d).contains("Illegal Command"));
}

#[test]
fn assembler_prompt_writes_code_and_resolves_labels() {
    let mut m = bare(Model::Apple2eEnhanced);
    let mut d = debugger(&mut m);
    d.execute(&mut m, "a 300");
    assert_eq!(d.console.prompt(), '!');
    for l in ["LDA #$41", "JSR $FDED", "BRA DONE", "NOP", "DONE RTS", ""] {
        d.execute(&mut m, l);
    }
    assert_eq!(d.console.prompt(), '>');
    let code: Vec<u8> = (0x300..0x30B).map(|a| m.peek(a)).collect();
    assert_eq!(
        code,
        [0xA9, 0x41, 0x20, 0xED, 0xFD, 0x80, 0x01, 0xEA, 0x60, 0x00, 0x00]
    );
    // `a addr instruction` one-liner
    d.execute(&mut m, "a 320 INC");
    assert_eq!(m.peek(0x320), 0x1A);
}

#[test]
fn expression_calc_and_symbols_commands() {
    let mut m = bare(Model::Apple2Plus);
    let mut d = debugger(&mut m);
    d.execute(&mut m, "calc cout + 3");
    assert!(console_text(&d).contains("$FDF0"), "{}", console_text(&d));
    d.execute(&mut m, "sym myvar = 1234");
    d.execute(&mut m, "sym myvar");
    assert!(
        console_text(&d).contains("User1: $1234 myvar"),
        "{}",
        console_text(&d)
    );
    d.execute(&mut m, "sym ! myvar");
    d.execute(&mut m, "sym 1234");
    assert!(console_text(&d).contains("Address not found"));
    d.execute(&mut m, "symmain fded");
    assert!(console_text(&d).contains("Main: $FDED COUT"));
    d.execute(&mut m, "symbasic PRINT.FAC");
    assert!(
        console_text(&d).contains("Basic: $ED2E PRINT.FAC"),
        "{}",
        console_text(&d)
    );
    // The DOS 3.3 .SYM2 script defined symbols and data blocks.
    d.execute(&mut m, "symdos33 DOS.COS0");
    assert!(
        console_text(&d).contains("DOS33: $9EEB DOS.COS0"),
        "{}",
        console_text(&d)
    );
    assert!(d.data.find(0xA884).is_some(), "ASC T_INIT range");
}

#[test]
fn data_directives_show_in_the_listing() {
    let mut m = bare(Model::Apple2Plus);
    load(&mut m, 0x2000, &[0xC8, 0xC5, 0xCC, 0xCC, 0xCF]);
    load(&mut m, 0x2010, &[0x34, 0x12]);
    let mut d = debugger(&mut m);
    d.execute(&mut m, "asc 2000:2004");
    d.execute(&mut m, "dw words = 2010");
    d.execute(&mut m, "u 2000");
    let g = d.render(&m);
    let text = g.text();
    assert!(
        text.contains("2000:C8 C5 CC") && text.contains("ASC \"HELLO\""),
        "{text}"
    );
    d.execute(&mut m, "u 2010");
    let text = d.render(&m).text();
    assert!(
        text.contains("words") && text.contains("dw") && text.contains("1234"),
        "{text}"
    );
    d.execute(&mut m, "x 2000:2004");
    assert!(d.data.find(0x2000).is_none());
}

#[test]
fn screen_grid_snapshot() {
    let mut m = bare(Model::Apple2Plus);
    load(
        &mut m,
        0x300,
        &[0xA9, 0x41, 0x8D, 0x00, 0x04, 0xD0, 0xFB, 0x60],
    );
    let mut d = Debugger::new();
    d.enter(&mut m);
    d.execute(&mut m, "bp 304");
    d.execute(&mut m, "w 400");
    d.execute(&mut m, "md1 300");
    d.execute(&mut m, "zp 10");
    d.execute(&mut m, "t");
    let g = d.render(&m);
    // Golden snapshot of the whole 80x48 screen (UPDATE_GOLDEN=1 regenerates it after a change that
    // was checked by eye against AppleWin's layout).
    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden_screen.txt");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(golden_path, g.text() + "\n").unwrap();
    }
    let golden = std::fs::read_to_string(golden_path).expect("tests/golden_screen.txt");
    assert_eq!(g.text() + "\n", golden);
    let rows: Vec<String> = (0..48).map(|y| g.row_text(y)).collect();
    let pc_row = 21;
    assert!(rows[pc_row].starts_with("0302:8D 00 04") && rows[pc_row].contains("STA $0400"));
    assert!(g.cursor == Some((1, 47)));
    // Colors: the cursor line uses the PC colors (yellow background), breakpoint red, info cyan.
    use apple2_debugger::color::DebugColor::*;
    assert_eq!(g.cell(0, pc_row).bg, d.colors.get(BG_DISASM_PC_C));
    assert_eq!(g.cell(0, pc_row).fg, d.colors.get(FG_DISASM_PC_C));
    assert_eq!(g.cell(INFO_COL_1 + 3, 3).bg, d.colors.get(BG_DATA_1));
    assert_eq!(g.cell(62 + 6, 0).bg, d.colors.get(BG_DISASM_BP_S_C));
}

#[test]
fn io_breakpoint_on_the_speaker_during_a_real_boot() {
    // The autostart ROM beeps (BELL1, $FBDD) before showing the banner: an LDA $C030.
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    let mut d = debugger(&mut m);
    d.execute(&mut m, "bpio C030");
    d.execute(&mut m, "g");
    go(&mut d, &mut m, 3_000_000);
    assert_eq!(d.mode, RunMode::Debug, "{}", console_text(&d));
    let op = m.peek(m.cpu().pc);
    assert_eq!(op, 0xAD, "LDA abs");
    assert_eq!(
        m.peek(m.cpu().pc + 1) as u16 | (m.peek(m.cpu().pc + 2) as u16) << 8,
        0xC030
    );
    assert!(
        console_text(&d).contains("Memory access at") && console_text(&d).contains("C030"),
        "{}",
        console_text(&d)
    );
    assert_eq!(
        m.drain_speaker_toggles().count(),
        0,
        "stops before the toggle"
    );
    d.execute(&mut m, "t");
    assert_eq!(m.drain_speaker_toggles().count(), 1);
}
