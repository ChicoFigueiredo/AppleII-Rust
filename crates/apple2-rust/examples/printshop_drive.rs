//! Scripted headless driver for printer acceptance with period software (issue #104).
//!
//! `cargo run --release -p apple2-rust --example printshop_drive -- <script> <outdir>`
//!
//! Boots a //e Enhanced with a Disk II in slot 6 and the Generic Printer in slot 1 (Virtual
//! printer only), then runs the script, one command per line (`#` starts a comment):
//!
//! - `disk <drive 1|2> <path>`: inserts a disk image (no reboot);
//! - `writable on`: mounts later disks writable (use copies of the images, never the originals);
//! - `transport auto|8|7`: Transporte of the printer; must come before `boot`;
//! - `boot`: power cycle;
//! - `run <frames>`: runs frames; `key <name>`: one key (a character, `RET`, `ESC`, `SPACE`,
//!   `TAB`, `LEFT`, `RIGHT`, `UP`, `DOWN`, `DEL`, `CTRL-x`), then 6 frames;
//! - `type <text>`: each character as a key, 6 frames apart;
//! - `insert <drive> <path>`: swaps a disk after boot;
//! - `drives`: prints drive status (image, track, activity);
//! - `cpu`: prints the CPU registers (debugging aid);
//! - `peek <hex addr> <len>`: dumps memory;
//! - `shot <name>`: PNG of the screen in `<outdir>`; `text`: 40/80-column text screen on stdout;
//! - `save <name>`: writes the raw bytes captured so far to `<outdir>/<name>`.
//!
//! The raw stream is also written to `<outdir>/raw.bin` at the end. Disks are third-party
//! software and are never part of the repository.

use apple2_config::card::{PrinterDestination, PrinterSpec, PrinterTransport};
use apple2_config::{Settings, SlotCardSpec};
use apple2_core::keyboard::NavKey;
use apple2_hostio::capture_channel;
use apple2_printer::capture::CaptureEvent;
use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::{Command, Emulator};
use apple2_rust_shell::factory::set_thread_capture;
use apple2_rust_shell::keymap::KeyEvent;
use apple2_rust_shell::screenshot::save_png;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let script = std::fs::read_to_string(args.get(1).ok_or("usage: <script> <outdir>")?)
        .map_err(|e| e.to_string())?;
    let out = PathBuf::from(args.get(2).ok_or("usage: <script> <outdir>")?);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    let mut transport = PrinterTransport::Auto;
    let mut disks: Vec<(usize, PathBuf)> = Vec::new();
    let mut emu: Option<Emulator> = None;
    let (tx, rx) = capture_channel();
    set_thread_capture(Some(tx));
    let mut raw: Vec<u8> = Vec::new();
    let mut read_only = true;

    for (n, line) in script.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (cmd, arg) = line.split_once(' ').unwrap_or((line, ""));
        let at = |m: &str| format!("line {}: {m}", n + 1);
        if cmd == "disk" {
            let (d, p) = arg.split_once(' ').ok_or_else(|| at("disk <n> <path>"))?;
            disks.push((d.parse::<usize>().map_err(|e| e.to_string())? - 1, p.into()));
            continue;
        }
        if cmd == "transport" {
            transport = match arg {
                "8" => PrinterTransport::EightBit,
                "7" => PrinterTransport::SevenBitAppleWin,
                _ => PrinterTransport::Auto,
            };
            continue;
        }
        if cmd == "writable" {
            read_only = arg != "on";
            continue;
        }
        if cmd == "boot" {
            let mut s = Settings::default();
            s.slots.0[1] = SlotCardSpec::GenericPrinter(PrinterSpec {
                destination: PrinterDestination::VirtualOnly,
                transport,
                ..PrinterSpec::default()
            });
            let mut e = Emulator::new(s, Box::new(DefaultBackend), 48_000)?;
            for (d, p) in &disks {
                e.apply(Command::InsertDisk {
                    drive: *d,
                    path: p.clone(),
                    read_only,
                });
            }
            e.apply(Command::PowerCycle);
            emu = Some(e);
            continue;
        }
        let e = emu.as_mut().ok_or_else(|| at("boot first"))?;
        let mut frames = |e: &mut Emulator, k: u32| {
            for _ in 0..k {
                e.run_frame();
                for ev in rx.drain() {
                    if let CaptureEvent::Bytes { data, .. } = ev {
                        raw.extend(data);
                    }
                }
            }
        };
        let press = |e: &mut Emulator, name: &str| -> Result<(), String> {
            let ev = match name {
                "RET" => KeyEvent::Ascii(0x0D),
                "ESC" => KeyEvent::Ascii(0x1B),
                "SPACE" => KeyEvent::Ascii(b' '),
                "TAB" => KeyEvent::Ascii(0x09),
                "LEFT" => KeyEvent::Nav(NavKey::Left),
                "RIGHT" => KeyEvent::Nav(NavKey::Right),
                "UP" => KeyEvent::Nav(NavKey::Up),
                "DOWN" => KeyEvent::Nav(NavKey::Down),
                "DEL" => KeyEvent::Nav(NavKey::Delete),
                s if s.starts_with("CTRL-") && s.len() == 6 => {
                    KeyEvent::Ascii(s.as_bytes()[5].to_ascii_uppercase() & 0x1F)
                }
                s if s.len() == 1 => KeyEvent::Ascii(s.as_bytes()[0]),
                s => return Err(format!("unknown key {s}")),
            };
            e.apply(Command::Key(ev));
            Ok(())
        };
        match cmd {
            "run" => frames(e, arg.parse().map_err(|_| at("run <frames>"))?),
            "key" => {
                press(e, arg).map_err(|m| at(&m))?;
                frames(e, 6);
            }
            "type" => {
                for c in arg.chars() {
                    press(
                        e,
                        &if c == ' ' {
                            "SPACE".into()
                        } else {
                            c.to_string()
                        },
                    )
                    .map_err(|m| at(&m))?;
                    frames(e, 6);
                }
            }
            "shot" => save_png(&out.join(format!("{arg}.png")), &e.redraw_now().clone())?,
            "text" => {
                for l in apple2_core::text_screen::text_screen(&e.machine) {
                    println!("{l}");
                }
            }
            "insert" => {
                let (d, p) = arg.split_once(' ').ok_or_else(|| at("insert <n> <path>"))?;
                e.apply(Command::InsertDisk {
                    drive: d.parse::<usize>().map_err(|e| e.to_string())? - 1,
                    path: p.into(),
                    read_only,
                });
            }
            "drives" => println!("{:?}", e.status().drives),
            "cpu" => println!("{:?}", e.machine.cpu()),
            "peek" => {
                let (a, n) = arg.split_once(' ').unwrap_or((arg, "16"));
                let a = u16::from_str_radix(a.trim_start_matches("0x"), 16)
                    .map_err(|e| e.to_string())?;
                let n: u16 = n.parse().map_err(|_| at("peek <hex addr> <len>"))?;
                let v: Vec<String> = (0..n)
                    .map(|i| format!("{:02X}", e.machine.peek(a.wrapping_add(i))))
                    .collect();
                println!("{a:04X}: {}", v.join(" "));
            }
            "save" => std::fs::write(out.join(arg), &raw).map_err(|e| e.to_string())?,
            _ => return Err(at("unknown command")),
        }
    }
    // Flushes pending writes to the disk images (matters with `writable on`).
    if let Some(e) = emu.as_mut() {
        e.apply(Command::Shutdown);
    }
    std::fs::write(out.join("raw.bin"), &raw).map_err(|e| e.to_string())?;
    println!("raw bytes captured: {}", raw.len());
    Ok(())
}
