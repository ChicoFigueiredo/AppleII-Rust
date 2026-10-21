//! GH#564: "Save State On Exit" must save the state of the *restarted* machine. A hardware
//! configuration change restarts the emulator (ApplySettings rebuilds the machine); quitting
//! afterwards has to write the new machine, not skip the save or write the old one.
//!
//! C++ (AppleWin a8f0d1d) reproduced: `WM_DESTROY` saves only `if (!g_bRestart)`
//! (WinFrame.cpp:1086-1088, "GH#564: Only save-state on shutdown (not on a restart)"), so a
//! hardware restart writes nothing; `Snapshot_Startup` loads the file once (`bDone`,
//! SaveState.cpp:570-579) so the restart does not reload the old state; `Snapshot_Shutdown`
//! (SaveState.cpp:582-593) saves the machine that is running at the real exit.

use apple2_config::Settings;
use apple2_core::Model;
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::{Command, Emulator};

#[test]
fn gh564_save_state_on_exit_after_a_hardware_restart_saves_the_new_machine() {
    let dir = std::env::temp_dir().join(format!("applewin-gh564-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("session.aws.yaml");
    let mut s = Settings::default();
    s.storage.save_state_on_exit = true;
    s.storage.savestate_filename = Some(path.clone());
    let mut a = Emulator::new(s.clone(), Box::new(DefaultBackend), 48_000).unwrap();
    for _ in 0..30 {
        a.run_frame();
    }
    // Hardware change: another model restarts the machine.
    let mut other = s.clone();
    other.machine.model = Model::Apple2Plus;
    a.apply(Command::ApplySettings(Box::new(other)));
    for _ in 0..30 {
        a.run_frame();
    }
    assert_eq!(a.machine.save_state().config.model, Model::Apple2Plus);
    // the restart itself writes nothing (WinFrame.cpp:1087) and does not reload an old state
    assert!(!path.exists(), "the restart must not save");
    assert!(!a.apply(Command::Shutdown));
    let text = std::fs::read_to_string(&path).expect("o estado foi salvo ao sair");
    assert!(
        text.contains("Model: Apple][+"),
        "{}",
        &text[..200.min(text.len())]
    );
    // And it loads back as the restarted machine.
    let mut b = Emulator::new(Settings::default(), Box::new(DefaultBackend), 48_000).unwrap();
    b.load_state(&path);
    assert_eq!(b.machine.save_state().config.model, Model::Apple2Plus);
    std::fs::remove_dir_all(dir).unwrap();
}
