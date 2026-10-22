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
