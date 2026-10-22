#![cfg(feature = "fs")]
//! R09 / HU-IMP-081: what the Apple II prints is data. Nothing on the virtual printer's path
//! (printer crate, capture queue, the Central's hub/window/model) starts a process or a shell.
//! The legacy `dump_to_printer` ("copy file prn", Windows only, AppleWin's option) lives in
//! `apple2-hostio/src/printer.rs` (`FileSink::close`), acts on the path the user configured and is
//! never reached from the virtual path.
mod common;
use apple2_printer::archive::Archive;
use apple2_printer::capture::CloseReason;
use apple2_printer::lx310::{Lx310, Lx310Config};
use apple2_printer::paper::RenderOptions;
use apple2_printer::spool::{EngineFactory, Spool};
use common::*;
use std::path::{Path, PathBuf};

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn virtual_printer_sources_never_spawn_processes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    rs_files(&root.join("apple2-printer/src"), &mut files);
    files.push(root.join("apple2-hostio/src/printer_capture.rs"));
    for f in [
        "printer_hub.rs",
        "printer_window.rs",
        "printer_model.rs",
        "printer_panels.rs",
    ] {
        files.push(root.join("apple2-rust/src").join(f));
    }
    let banned = [
        "process::Command",
        "Command::new",
        "std::process::",
        "ShellExecute",
        "xdg-open",
        "libc::system",
    ];
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for b in banned {
                // `std::process::id()` is only used to make unique temp names.
                if line.contains(b) && !line.contains("process::id()") {
                    panic!("{}:{}: {b}: {line}", f.display(), n + 1);
                }
            }
        }
    }
}

#[test]
fn shell_looking_bytes_are_just_printed_and_touch_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let f: EngineFactory = Box::new(|| Box::new(Lx310::new(Lx310Config::default())));
    let mut s = Spool::new(
        Archive::open(dir.path()).unwrap(),
        f,
        RenderOptions::default(),
    );
    let sentinel = dir.path().join("pwned");
    let p = sentinel.display();
    let mut payload =
        format!("; touch {p}\r\n$(touch {p})\r\n`touch {p}`\r\n| touch {p}\r\n& touch {p}\r\n")
            .into_bytes();
    payload.extend_from_slice(b"\x1b@\x1b!\xff\x1b\x1b(\x1bP\x1b%\x1b[\x00\x1b|\r\n");
    s.handle(open(1));
    s.handle(bytes(1, 0, &payload));
    s.handle(close(1, CloseReason::Idle));
    assert!(!sentinel.exists());
    let snap = s.snapshot_handle().get();
    let raw = std::fs::read(dir.path().join(&snap.jobs[0].id).join("raw.bin")).unwrap();
    assert_eq!(raw, payload);
    // The archive root holds only the job folder.
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
