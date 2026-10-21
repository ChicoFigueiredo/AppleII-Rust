use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::{Command, Emulator};
use apple2_rust_shell::keymap::{DbgKey, DebugKey};

fn emulator() -> Emulator {
    let mut s = apple2_config::Settings::default();
    s.slots.0[6] = apple2_config::SlotCardSpec::Empty;
    Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap()
}

fn type_line(e: &mut Emulator, line: &str) {
    for c in line.chars() {
        e.apply(Command::Debug(DebugKey::Char(c)));
    }
    e.apply(Command::Debug(DebugKey::plain(DbgKey::Enter)));
}

#[test]
fn f7_halts_the_machine_and_shows_the_debugger_screen() {
    let mut e = emulator();
    for _ in 0..120 {
        e.run_frame();
    }
    e.apply(Command::ToggleDebugger);
    assert!(e.status().debugger_active);
    assert!(e.debugger_halted());
    let before = e.machine.cycles();
    let (cycles, _) = e.run_frame();
    assert_eq!(cycles, 0);
    assert_eq!(e.machine.cycles(), before);

    // "T" steps one instruction under the debugger
    let pc = e.machine.cpu().pc;
    type_line(&mut e, "T");
    assert!(e.machine.cycles() > before);
    assert_ne!(e.machine.cpu().pc, pc);

    let frame = e.render().clone();
    assert_eq!((frame.width, frame.height), (560, 384));
    assert!(
        frame.rgba.chunks(4).any(|p| p[0] > 100),
        "debugger screen is blank"
    );

    if let Some(p) = std::env::var_os("APPLE2_RUST_TEST_DUMP") {
        apple2_rust_shell::screenshot::save_png(std::path::Path::new(&p), &frame).unwrap();
    }

    // F7 again resumes
    e.apply(Command::ToggleDebugger);
    assert!(!e.status().debugger_active);
    let (cycles, _) = e.run_frame();
    assert!(cycles > 0);
}
