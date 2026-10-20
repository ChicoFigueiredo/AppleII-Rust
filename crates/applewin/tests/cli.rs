//! The binary's informational flags must print and exit, never open a window.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn run_with_timeout(args: &[&str]) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_applewin"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn applewin");
    let t0 = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let out = child.wait_with_output().unwrap();
            return (status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned());
        }
        if t0.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            panic!("applewin {args:?} did not exit (hang)");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn help_prints_usage_and_exits() {
    for flag in ["--help", "-h", "-help", "-?"] {
        let (code, out) = run_with_timeout(&[flag]);
        assert_eq!(code, 0, "{flag}");
        assert!(out.contains("Usage: applewin"), "{flag}: {out}");
    }
}

#[test]
fn version_prints_and_exits() {
    for flag in ["--version", "-version", "-V"] {
        let (code, out) = run_with_timeout(&[flag]);
        assert_eq!(code, 0, "{flag}");
        assert!(out.contains("applewin"), "{flag}: {out}");
    }
}
