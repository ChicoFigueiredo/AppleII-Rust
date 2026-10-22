//! The binary's informational flags must print and exit, never open a window.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn run_with_timeout(args: &[&str]) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_apple2-rust"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn apple2-rust");
    let t0 = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let out = child.wait_with_output().unwrap();
            return (
                status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout).into_owned(),
            );
        }
        if t0.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            panic!("apple2-rust {args:?} did not exit (hang)");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn help_prints_usage_and_exits() {
    for flag in ["--help", "-h", "-help", "-?"] {
        let (code, out) = run_with_timeout(&[flag]);
        assert_eq!(code, 0, "{flag}");
        assert!(out.contains("Usage: apple2-rust"), "{flag}: {out}");
    }
}

#[test]
fn a_closed_stdout_pipe_ends_quietly_without_a_panic() {
    // `apple2-rust --help | head -1`: the reader goes away before the output is written (#108).
    for args in [
        &["--help"][..],
        &["--version"],
        &["--headless-frames", "1", "--dump-text"],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_apple2-rust"))
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn apple2-rust");
        drop(child.stdout.take());
        let out = child.wait_with_output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!err.contains("panicked"), "{args:?}: {err}");
        assert_eq!(out.status.code(), Some(0), "{args:?}: {err}");
    }
}

#[test]
fn bad_rom_options_exit_with_the_applewin_message_and_no_window() {
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_apple2-rust"))
            .args(args)
            .output()
            .expect("spawn apple2-rust");
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let (code, err) = run(&["-rom", "/nonexistent/x.rom"]);
    assert_eq!(code, Some(1));
    assert!(err.contains("Failed to load custom rom (not found or not exactly 12KiB or 16KiB)"));
    let (code, err) = run(&["-f8rom", "/nonexistent/f8.rom"]);
    assert_eq!(code, Some(1));
    assert!(err.contains("Failed to load custom F8 rom (not found or not exactly 2KiB)"));
}

#[test]
fn version_prints_and_exits() {
    for flag in ["--version", "-version", "-V"] {
        let (code, out) = run_with_timeout(&[flag]);
        assert_eq!(code, 0, "{flag}");
        assert!(out.contains("apple2-rust"), "{flag}: {out}");
    }
}
