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

/// #107: on Windows release builds only `apple2-rust` is a GUI-subsystem program; its console
/// twin (shipped as `apple2-rust.com`) is not.
#[cfg(windows)]
#[test]
fn only_the_gui_binary_uses_the_windows_subsystem() {
    const GUI: u16 = 2;
    const CONSOLE: u16 = 3;
    let subsystem = |exe: &str| {
        let b = std::fs::read(exe).unwrap();
        let pe = u32::from_le_bytes(b[0x3C..0x40].try_into().unwrap()) as usize;
        assert_eq!(&b[pe..pe + 4], b"PE\0\0", "{exe}");
        u16::from_le_bytes(b[pe + 0x5C..pe + 0x5E].try_into().unwrap())
    };
    let gui = if cfg!(debug_assertions) { CONSOLE } else { GUI };
    assert_eq!(subsystem(env!("CARGO_BIN_EXE_apple2-rust")), gui);
    assert_eq!(
        subsystem(env!("CARGO_BIN_EXE_apple2-rust-console")),
        CONSOLE
    );
}

/// #107: cmd's `>` reaches the console binary (it never reached the GUI one).
#[cfg(windows)]
#[test]
fn cmd_redirection_reaches_the_console_binary() {
    use std::os::windows::process::CommandExt;
    let out = std::env::temp_dir().join(format!("apple2-rust-help-{}.txt", std::process::id()));
    let status = Command::new("cmd")
        .raw_arg(format!(
            "/d /c \"\"{}\" --help > \"{}\"\"",
            env!("CARGO_BIN_EXE_apple2-rust-console"),
            out.display()
        ))
        .status()
        .unwrap();
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    assert!(status.success());
    assert!(text.contains("Usage: apple2-rust"), "{text:?}");
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
