//! Save-states through the app layer: AppleWin `.aws.yaml` round trips, "save state on exit",
//! the `-load-state` / `-screenshot-and-exit` command line.

use apple2_config::{CardType, Settings, SlotCardSpec};
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::{Command, Emulator};
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("applewin-state-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn dos33() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("assets/disks/DOS 3.3 System Master - 680-0210-A.dsk")
}

fn settings_with_disk() -> Settings {
    let mut s = Settings::default();
    s.slots.0[6] = SlotCardSpec::Disk2 {
        firmware_13_sector: false,
        drives: [
            apple2_config::card::DriveSpec {
                image: Some(dos33()),
                write_protect: true,
                connected: true,
            },
            Default::default(),
        ],
    };
    s
}

#[test]
fn save_and_load_round_trip_in_aws_yaml() {
    let dir = temp_dir("rt");
    let path = dir.join("SaveState.aws.yaml");
    let mut emu = Emulator::new(settings_with_disk(), Box::new(DefaultBackend), 48_000).unwrap();
    for _ in 0..400 {
        emu.run_frame();
    }
    emu.apply(Command::SaveState(path.clone()));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.starts_with("# Date-stamp:"),
        "{}",
        &text[..text.len().min(80)]
    );
    assert!(
        text.contains("Tag: AppleWin Save State") && text.contains("Version: 2"),
        "AppleWin file header"
    );
    assert!(text.contains("Type: Apple2") && text.contains("Card: Disk II"));
    let saved = emu.machine.save_state();
    for _ in 0..120 {
        emu.run_frame();
    }
    assert_ne!(emu.machine.save_state(), saved);
    emu.apply(Command::LoadState(path.clone()));
    assert!(
        emu.status().message.starts_with("State loaded"),
        "{}",
        emu.status().message
    );
    assert_eq!(
        emu.machine.save_state(),
        saved,
        "loading restores the saved machine exactly"
    );
    // the machine keeps running from there, with its disk back in drive 1
    assert_eq!(
        emu.status().drives[0]
            .image
            .as_deref()
            .map(|n| n.starts_with("DOS 3.3")),
        Some(true)
    );
    for _ in 0..60 {
        emu.run_frame();
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_state_loads_into_an_emulator_with_other_settings() {
    let dir = temp_dir("other");
    let path = dir.join("a.aws.yaml");
    let mut s = settings_with_disk();
    s.slots.0[2] = SlotCardSpec::Empty; // the default SSC: AppleWin keeps only the first one
    s.slots.0[3] = SlotCardSpec::new(CardType::Ssc);
    let mut a = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    for _ in 0..200 {
        a.run_frame();
    }
    a.apply(Command::SaveState(path.clone()));

    let mut empty = Settings::default();
    empty.slots.0 = std::array::from_fn(|_| SlotCardSpec::Empty);
    empty.machine.model = apple2_core::Model::Apple2Plus;
    let mut b = Emulator::new(empty, Box::new(DefaultBackend), 48_000).unwrap();
    b.apply(Command::LoadState(path));
    assert!(
        b.status().message.starts_with("State loaded"),
        "{}",
        b.status().message
    );
    assert_eq!(b.machine.save_state(), a.machine.save_state());
    assert_eq!(
        b.machine.board().slots.card(3).map(|c| c.name()),
        Some("Super Serial Card")
    );
    assert_eq!(
        b.machine.board().slots.card(6).map(|c| c.name()),
        Some("Disk II")
    );
    assert_eq!(
        b.settings.machine.model,
        apple2_core::Model::Apple2eEnhanced
    );
    // the Disk II sink follows the loaded machine
    assert!(b.status().drives[0].present);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn bad_files_are_reported_not_fatal() {
    let dir = temp_dir("bad");
    let mut emu = Emulator::new(Settings::default(), Box::new(DefaultBackend), 48_000).unwrap();
    let junk = dir.join("junk.aws.yaml");
    std::fs::write(&junk, "not: a state\n").unwrap();
    emu.apply(Command::LoadState(junk));
    assert!(
        emu.status().message.starts_with("Load state failed"),
        "{}",
        emu.status().message
    );
    emu.apply(Command::LoadState(dir.join("missing.aws.yaml")));
    assert!(emu.status().message.starts_with("Load state failed"));
    emu.apply(Command::LoadState(dir.join("old.aws")));
    assert!(
        emu.status().message.contains("v1"),
        "{}",
        emu.status().message
    );
    for _ in 0..10 {
        emu.run_frame();
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_state_on_exit_restores_the_session() {
    let dir = temp_dir("exit");
    let path = dir.join("session.aws.yaml");
    let mut s = Settings::default();
    s.storage.save_state_on_exit = true;
    s.storage.savestate_filename = Some(path.clone());
    let mut a = Emulator::new(s.clone(), Box::new(DefaultBackend), 48_000).unwrap();
    for _ in 0..150 {
        a.run_frame();
    }
    let saved = a.machine.save_state();
    assert!(!a.apply(Command::Shutdown));
    assert!(path.is_file(), "Snapshot_Shutdown saved the state");
    let mut b = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    b.startup_state();
    assert_eq!(
        b.machine.save_state(),
        saved,
        "Snapshot_Startup loaded it back"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn command_line_load_state_and_screenshot_and_exit() {
    let dir = temp_dir("cli");
    let path = dir.join("shot.aws.yaml");
    let mut emu = Emulator::new(Settings::default(), Box::new(DefaultBackend), 48_000).unwrap();
    for _ in 0..200 {
        emu.run_frame();
    }
    emu.apply(Command::SaveState(path.clone()));
    let png = dir.join("out.png");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_applewin"))
        .args([
            "-load-state",
            path.to_str().unwrap(),
            "-screenshot-and-exit",
            png.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let img = image::open(&png).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (560, 384));
    assert!(
        img.pixels().any(|p| p.0[0] > 100),
        "the loaded screen is not blank"
    );

    let bad = std::process::Command::new(env!("CARGO_BIN_EXE_applewin"))
        .args([
            "-load-state",
            dir.join("nope.aws.yaml").to_str().unwrap(),
            "-screenshot-and-exit",
            png.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn current_dir_and_conf_options() {
    let dir = temp_dir("conf");
    let conf = dir.join("my.toml");
    let mut s = Settings::default();
    s.slots.0[6] = SlotCardSpec::Empty;
    s.save(&conf).unwrap();
    // headless run with -conf and -current-dir: the printer card file lands in the current dir
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_applewin"))
        .args([
            "-conf",
            conf.to_str().unwrap(),
            "-current-dir",
            dir.to_str().unwrap(),
            "--headless-frames",
            "300",
            "--dump-text",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Apple //e"), "{text}"); // booted without Disk II (settings from -conf)
    std::fs::remove_dir_all(dir).unwrap();
}
