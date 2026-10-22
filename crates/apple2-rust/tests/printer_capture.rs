//! Printer Card routing (fase A): legacy file and/or virtual printer, per `PrinterSpec`, and the
//! job-closing reasons when the emulator replaces the machine.

use apple2_config::card::{PrinterDestination, PrinterSpec, PrinterTransport};
use apple2_config::{Settings, SlotCardSpec};
use apple2_core::Model;
use apple2_hostio::{capture_channel, CaptureReceiver};
use apple2_printer::capture::{CaptureEvent, CloseReason, Transport};
use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::{spawn, Command, Emulator};
use apple2_rust_shell::factory::{printer_card_for, set_thread_capture};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("apple2-rust-pcap-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn settings(spec: PrinterSpec) -> Settings {
    let mut s = Settings::default();
    s.slots.0[6] = SlotCardSpec::Empty;
    s.slots.0[1] = SlotCardSpec::GenericPrinter(spec);
    s
}

fn spec_in(dir: &std::path::Path, destination: PrinterDestination) -> PrinterSpec {
    PrinterSpec {
        filename: Some(dir.join("Printer.txt")),
        destination,
        ..PrinterSpec::default()
    }
}

fn print(emu: &mut Emulator, bytes: &[u8]) {
    for &b in bytes {
        emu.machine.with_card(1, |c, ctx| c.io_write(0, b, ctx));
    }
}

fn bytes_of(evs: &[CaptureEvent]) -> Vec<u8> {
    evs.iter()
        .flat_map(|e| match e {
            CaptureEvent::Bytes { data, .. } => data.clone(),
            _ => vec![],
        })
        .collect()
}

#[test]
fn destination_routes_to_file_virtual_or_both() {
    for (dest, file, virt) in [
        (PrinterDestination::VirtualAndFile, true, true),
        (PrinterDestination::FileOnly, true, false),
        (PrinterDestination::VirtualOnly, false, true),
    ] {
        let dir = temp_dir(&format!("{dest:?}"));
        let (tx, rx) = capture_channel();
        let spec = PrinterSpec {
            transport: PrinterTransport::SevenBitAppleWin,
            ..spec_in(&dir, dest)
        };
        let mut m = apple2_core::Machine::new(apple2_core::MachineConfig::for_model(
            Model::Apple2eEnhanced,
        ));
        let card = printer_card_for(&spec, Model::Apple2eEnhanced, false, 1, Some(&tx));
        assert!(m.insert_card(1, Box::new(card)));
        m.with_card(1, |c, ctx| c.io_write(0, 0xC1, ctx)); // 'A' | 0x80
        m.reset();
        let evs = rx.drain();
        assert_eq!(!evs.is_empty(), virt, "{dest:?}");
        if virt {
            assert!(matches!(
                &evs[0],
                CaptureEvent::Open { origin, transport: Transport::SevenBitAppleWin, .. }
                    if origin.slot == 1 && origin.machine == "Apple //e (enhanced)"
            ));
            assert_eq!(bytes_of(&evs), b"A");
        }
        assert_eq!(dir.join("Printer.txt").exists(), file, "{dest:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn without_a_consumer_the_card_keeps_the_legacy_file() {
    let dir = temp_dir("noconsumer");
    let spec = spec_in(&dir, PrinterDestination::VirtualOnly);
    let mut m = apple2_core::Machine::new(apple2_core::MachineConfig::for_model(
        Model::Apple2eEnhanced,
    ));
    let card = printer_card_for(&spec, Model::Apple2eEnhanced, false, 1, None);
    assert!(m.insert_card(1, Box::new(card)));
    m.with_card(1, |c, ctx| c.io_write(0, b'x', ctx));
    m.reset();
    assert_eq!(std::fs::read(dir.join("Printer.txt")).unwrap(), b"x");
    let _ = std::fs::remove_dir_all(dir);
}

fn summary(evs: &[CaptureEvent]) -> Vec<String> {
    evs.iter()
        .map(|e| match e {
            CaptureEvent::Open { job, .. } => format!("open{job}"),
            CaptureEvent::Bytes { job, offset, data } => {
                format!("bytes{job}@{offset}:{}", String::from_utf8_lossy(data))
            }
            CaptureEvent::Close { job, reason } => format!("close{job}:{reason:?}"),
        })
        .collect()
}

#[test]
fn t019_state_load_and_machine_rebuild_close_the_job_with_their_reasons() {
    let dir = temp_dir("t019");
    let spec = spec_in(&dir, PrinterDestination::VirtualAndFile);
    let (tx, rx) = capture_channel();
    set_thread_capture(Some(tx));
    let mut emu = Emulator::new(settings(spec.clone()), Box::new(DefaultBackend), 48_000).unwrap();
    let state = dir.join("S.aws.yaml");
    emu.apply(Command::SaveState(state.clone()));

    print(&mut emu, b"ab");
    emu.apply(Command::LoadState(state));
    print(&mut emu, b"cd"); // the loaded machine's card: a new job from offset 0
    emu.apply(Command::ApplySettings(Box::new(settings(spec))));
    print(&mut emu, b"e");
    drop(emu);
    set_thread_capture(None);

    assert_eq!(
        summary(&rx.drain()),
        [
            "open1",
            "bytes1@0:ab",
            "close1:StateLoaded",
            "open2",
            "bytes2@0:cd",
            "close2:CardRemoved",
            "open3",
            "bytes3@0:e",
            "close3:Shutdown",
        ]
    );
    let _ = std::fs::remove_dir_all(dir);
}

fn wait_for(mut f: impl FnMut() -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(10) {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn the_emulation_thread_exposes_the_events_through_the_handle() {
    let dir = temp_dir("handle");
    let spec = spec_in(&dir, PrinterDestination::VirtualOnly);
    let mut h = spawn(
        settings(spec),
        Box::new(|| Box::new(DefaultBackend)),
        None,
        48_000,
        Box::new(|| {}),
    )
    .unwrap();
    let rx: CaptureReceiver = h.printer_events();
    h.send(Command::ToggleFullSpeed); // let BASIC boot quickly before the paste
    std::thread::sleep(Duration::from_millis(1500));
    h.send(Command::Paste("PR#1\rPRINT \"HI\"\r".into()));
    let mut got = Vec::new();
    let ok = wait_for(|| {
        got.extend(rx.drain());
        String::from_utf8_lossy(&bytes_of(&got).iter().map(|c| c & 0x7F).collect::<Vec<_>>())
            .contains("HI")
    });
    let st = h.shared.status();
    assert!(
        ok,
        "events: {got:?} cycles {:?} full {} mhz {}",
        st.cycles, st.full_speed, st.mhz
    );
    assert!(matches!(got[0], CaptureEvent::Open { .. }));
    h.shutdown();
    got.extend(rx.drain());
    assert!(matches!(
        got.last(),
        Some(CaptureEvent::Close {
            reason: CloseReason::Shutdown,
            ..
        })
    ));
    let _ = std::fs::remove_dir_all(dir);
}

/// Two Generic Printer cards (slots 1 and 2) printing alternately through the same queue: every
/// job has its own id and origin slot, and no byte of one card ever lands in the other's job,
/// neither on the wire nor in the Arquivo de impressões.
#[test]
fn two_cards_printing_alternately_never_mix_their_jobs() {
    use apple2_printer::archive::Archive;
    use apple2_printer::lx310::{Lx310, Lx310Config};
    use apple2_printer::paper::RenderOptions;
    use apple2_printer::spool::Spool;
    use std::collections::HashMap;

    let dir = temp_dir("twocards");
    let (tx, rx) = capture_channel();
    let spec = spec_in(&dir, PrinterDestination::VirtualOnly);
    let mut m = apple2_core::Machine::new(apple2_core::MachineConfig::for_model(
        Model::Apple2eEnhanced,
    ));
    for slot in [1u8, 2] {
        let card = printer_card_for(&spec, Model::Apple2eEnhanced, false, slot, Some(&tx));
        assert!(m.insert_card(slot, Box::new(card)));
    }
    for i in 0..6 {
        for (slot, ch) in [(1u8, b'A'), (2u8, b'B')] {
            m.with_card(slot, |c, ctx| c.io_write(0, ch, ctx));
            let _ = i;
        }
    }
    let evs = rx.drain();

    // Wire level: one job per slot, distinct ids, each carrying only its own letter.
    let mut slot_of: HashMap<u64, u8> = HashMap::new();
    for e in &evs {
        if let CaptureEvent::Open { job, origin, .. } = e {
            assert!(slot_of.insert(*job, origin.slot).is_none(), "id reused");
        }
    }
    assert_eq!(slot_of.len(), 2);
    let mut slots: Vec<u8> = slot_of.values().copied().collect();
    slots.sort();
    assert_eq!(slots, [1, 2]);
    for e in &evs {
        if let CaptureEvent::Bytes { job, data, .. } = e {
            let want = if slot_of[job] == 1 { b'A' } else { b'B' };
            assert!(data.iter().all(|&b| b == want), "mixed bytes in job {job}");
        }
    }

    // Archive level: whatever the spool keeps for a job comes from a single origin.
    let archive = Archive::open(dir.join("archive")).unwrap();
    let mut spool = Spool::new(
        archive,
        Box::new(|| Box::new(Lx310::new(Lx310Config::default()))),
        RenderOptions::default(),
    );
    for e in evs {
        spool.handle(e);
    }
    spool.shutdown();
    let snap = spool.snapshot_handle().get();
    assert!(snap.jobs.len() >= 2);
    let ids: std::collections::HashSet<_> = snap.jobs.iter().map(|j| j.id.clone()).collect();
    assert_eq!(ids.len(), snap.jobs.len(), "archive ids are distinct");
    for j in &snap.jobs {
        let raw = std::fs::read(dir.join("archive").join(&j.id).join("raw.bin")).unwrap();
        let slot = j.origin.as_ref().expect("origin recorded").slot;
        let want = if slot == 1 { b'A' } else { b'B' };
        // The spool keeps one active job at a time (a known limit, see the matrix), so a job may
        // hold less than its card wrote, but never a byte of the other card.
        assert!(raw.iter().all(|&b| b == want), "{raw:?}");
    }
    let _ = std::fs::remove_dir_all(dir);
}
