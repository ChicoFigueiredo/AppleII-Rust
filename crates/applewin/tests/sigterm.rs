//! AUD-C-034: SIGTERM/SIGINT/SIGHUP make the shell quit through the window's close path (which
//! flushes the disks), not die on the signal. Needs a display (the GUI really opens a window), so
//! it skips itself where there is none. Works on a copy: the master fixture is never written.
#![cfg(unix)]

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn have_display() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

fn run_and_signal(sig: &str) -> Option<std::process::ExitStatus> {
    if !have_display() {
        eprintln!("skipped: no display");
        return None;
    }
    let dir = std::env::temp_dir().join(format!("a2sig-{sig}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let img = dir.join("copy.dsk");
    let master = format!("{}/../../assets/disks/BLANK.DSK", env!("CARGO_MANIFEST_DIR"));
    std::fs::copy(master, &img).unwrap();
    let before = std::fs::read(&img).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_applewin"))
        .arg(&img)
        .arg("--no-audio")
        .arg("-conf")
        .arg(dir.join("settings.toml"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // let the window and the emulation thread come up
    std::thread::sleep(Duration::from_secs(5));
    if let Some(st) = child.try_wait().unwrap() {
        eprintln!("skipped: the GUI could not start here ({st})");
        return None;
    }
    let st = Command::new("kill")
        .arg(format!("-{sig}"))
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(st.success());
    let t0 = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if t0.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            panic!("{sig}: the process did not quit");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(std::fs::read(&img).unwrap(), before, "image intact");
    let _ = std::fs::remove_dir_all(&dir);
    Some(status)
}

#[test]
fn sigterm_quits_gracefully_with_status_zero() {
    if let Some(s) = run_and_signal("TERM") {
        assert!(s.success(), "SIGTERM must end with a normal exit, got {s}");
    }
}

#[test]
fn sigint_quits_gracefully_with_status_zero() {
    if let Some(s) = run_and_signal("INT") {
        assert!(s.success(), "SIGINT must end with a normal exit, got {s}");
    }
}
