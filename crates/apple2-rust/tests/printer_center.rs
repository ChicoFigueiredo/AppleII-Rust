//! Printer Center integration (no GPU): real emulator thread -> capture queue -> hub (bridge +
//! spool thread + Lx310) -> view model, PDF export, reprint and ordered shutdown.

use apple2_config::card::{PrinterDestination, PrinterPaper, PrinterSpec};
use apple2_config::{Settings, SlotCardSpec};
use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::{spawn, Command, EmuHandle};
use apple2_rust_shell::printer_hub::{PrinterConfig, PrinterHub};
use apple2_rust_shell::printer_model::{CenterEvent, PageRange, PdfRequest, Status};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("apple2-rust-center-{tag}-{}", std::process::id()));
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

fn start(tag: &str, spec: PrinterSpec) -> (EmuHandle, PrinterHub, PathBuf, Arc<AtomicUsize>) {
    let dir = temp_dir(tag);
    let s = settings(PrinterSpec {
        filename: Some(dir.join("Printer.txt")),
        ..spec
    });
    let emu = spawn(
        s.clone(),
        Box::new(|| Box::new(DefaultBackend)),
        None,
        48_000,
        Box::new(|| {}),
    )
    .unwrap();
    let wakes = Arc::new(AtomicUsize::new(0));
    let w = wakes.clone();
    let hub = PrinterHub::start(
        dir.join("archive"),
        emu.printer_events(),
        PrinterConfig::from_settings(&s),
        Arc::new(move || {
            w.fetch_add(1, Ordering::SeqCst);
        }),
    )
    .unwrap();
    (emu, hub, dir, wakes)
}

fn wait_for(mut f: impl FnMut() -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(20) {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

fn print_hello(emu: &EmuHandle) {
    emu.send(Command::ToggleFullSpeed);
    std::thread::sleep(Duration::from_millis(1500));
    emu.send(Command::Paste("PR#1\rPRINT \"OLA, IMPRESSORA\"\r".into()));
}

fn page_text(hub: &PrinterHub) -> String {
    hub.view_model()
        .document()
        .pages
        .iter()
        .flat_map(|p| p.text.iter().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_print_from_basic_reaches_the_view_model_and_opens_the_window_once() {
    let (mut emu, mut hub, dir, wakes) = start("flow", PrinterSpec::default());
    assert!(hub.available());
    assert_eq!(hub.view_model().status, Status::Offline);
    print_hello(&emu);
    let mut opens = 0;
    let mut activity = false;
    let ok = wait_for(|| {
        let p = hub.poll(Instant::now());
        opens += usize::from(p.auto_open);
        activity |= p.activity;
        page_text(&hub).contains("OLA, IMPRESSORA")
    });
    assert!(
        ok,
        "text: {:?} vm: {:?}",
        page_text(&hub),
        hub.view_model().status
    );
    assert_eq!(opens, 1, "automatic opening fires once per job");
    assert!(activity);
    assert!(wakes.load(Ordering::SeqCst) > 0);
    let vm = hub.view_model();
    assert!(matches!(vm.status, Status::Receiving | Status::Waiting));
    assert_eq!(vm.slot, Some(1));
    assert_eq!(vm.profile, "Epson LX-310");
    assert!(vm.shown.live);
    assert_eq!(vm.jobs.len(), 1);
    assert_eq!(vm.jobs[0].transport, "Automático");
    // idle polls do not redraw
    let rev = vm.revision;
    std::thread::sleep(Duration::from_millis(100));
    hub.poll(Instant::now());
    hub.poll(Instant::now());
    let after = hub.view_model().revision;
    assert!(after <= rev + 2, "no redraw storm: {rev} -> {after}");

    // The list shows a human title (first text line + date), not the archive id, even while
    // the job is open (the real Lx310 fills the TextRun the title comes from).
    let j0 = &hub.view_model().jobs[0];
    // The typed line is echoed to the printer, so it comes first.
    assert!(
        j0.title.starts_with("]PRINT \"OLA, IMPRESSORA\" — "),
        "{}",
        j0.title
    );
    assert_ne!(j0.title, j0.id);

    // Concluir trabalho
    hub.handle(CenterEvent::FinishJob);
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().active_job.is_none()
            && !hub.view_model().shown.loading
            && page_text(&hub).contains("OLA")
    }));
    assert_eq!(
        hub.view_model().jobs[0].state,
        apple2_rust_shell::printer_model::JobState::Finished
    );

    // Salvar PDF with title and note going to the manifest
    let job = hub.view_model().jobs[0].id.clone();
    // APPLE2_CENTRAL_SMOKE_PDF keeps a copy of the exported PDF for manual inspection.
    let pdf = dir.join("saida.pdf");
    hub.export_pdf(
        PdfRequest {
            job: job.clone(),
            version: hub.view_model().shown.version,
            range: PageRange::All,
            cover: true,
            title: "Meu título".into(),
            note: "minha nota".into(),
        },
        pdf.clone(),
    );
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        pdf.exists()
            && hub
                .view_model()
                .messages
                .iter()
                .any(|m| m.text.starts_with("PDF salvo"))
    }));
    // The export is in the manifest at once (summary and disk), long before shutdown.
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().jobs[0]
            .exports
            .iter()
            .any(|(v, p)| *v >= 1 && p.ends_with("saida.pdf"))
    }));
    let on_disk =
        std::fs::read_to_string(dir.join("archive").join(&job).join("manifest.json")).unwrap();
    assert!(on_disk.contains("saida.pdf"), "{on_disk}");
    let bytes = std::fs::read(&pdf).unwrap();
    if let Some(keep) = std::env::var_os("APPLE2_CENTRAL_SMOKE_PDF") {
        std::fs::write(keep, &bytes).unwrap();
    }
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.windows(5).any(|w| w == b"%%EOF"));
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        let j = &hub.view_model().jobs[0];
        j.title == "Meu título" && j.note == "minha nota"
    }));

    // Reprint with another paper creates version 2
    let mut opts = hub.view_model().shown.options;
    opts.paper = apple2_printer::paper::Paper::preset(apple2_printer::paper::PaperKind::A4);
    hub.handle(CenterEvent::Reprint {
        job: job.clone(),
        options: opts,
    });
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        let vm = hub.view_model();
        vm.jobs[0].versions.len() == 2 && vm.shown.version == 2
    }));
    assert_eq!(
        hub.view_model().shown.options.paper.kind,
        apple2_printer::paper::PaperKind::A4
    );

    // Delete the (closed) job
    hub.handle(CenterEvent::DeleteJob(job));
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().jobs.is_empty()
    }));

    emu.shutdown();
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_job_keeps_the_paper_it_started_with_and_closing_the_emulator_closes_it() {
    let (mut emu, mut hub, dir, _) = start(
        "paper",
        PrinterSpec {
            paper: PrinterPaper::BlueBar,
            ..PrinterSpec::default()
        },
    );
    print_hello(&emu);
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        page_text(&hub).contains("OLA")
    }));
    // R05: changing the setup mid-job does not touch the running one
    let mut s = hub.view_model().setup;
    s.options.paper = apple2_printer::paper::Paper::preset(apple2_printer::paper::PaperKind::A4);
    hub.handle(CenterEvent::SetNextSetup(s));
    hub.poll(Instant::now());
    assert_eq!(
        hub.view_model().shown.options.paper.kind,
        apple2_printer::paper::PaperKind::BlueBar
    );
    assert_eq!(
        hub.view_model().setup.options.paper.kind,
        apple2_printer::paper::PaperKind::A4
    );
    // closing: emulator first, then the hub; the job ends as Shutdown with the page saved
    emu.shutdown();
    let root = hub.root().to_path_buf();
    hub.shutdown();
    let mut found = 0;
    for e in std::fs::read_dir(&root).unwrap().flatten() {
        let m = std::fs::read_to_string(e.path().join("manifest.json")).unwrap_or_default();
        if m.contains("\"state\": \"completed\"") || m.contains("\"state\":\"completed\"") {
            assert!(m.contains("shutdown") || m.contains("Shutdown"), "{m}");
            found += 1;
        }
    }
    assert_eq!(found, 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn no_virtual_printer_means_no_button() {
    let spec = PrinterSpec {
        destination: PrinterDestination::FileOnly,
        ..PrinterSpec::default()
    };
    assert!(PrinterConfig::from_settings(&settings(spec)).is_none());
    let mut none = Settings::default();
    none.slots.0[1] = SlotCardSpec::Empty;
    assert!(PrinterConfig::from_settings(&none).is_none());
}

#[test]
fn the_job_table_and_country_change_what_a_job_prints() {
    use apple2_printer::font::CharTable;
    use apple2_printer::paper::RenderOptions;
    use apple2_rust_shell::printer_hub::render_job;
    let dir = temp_dir("table");
    std::fs::create_dir_all(dir.join("j")).unwrap();
    std::fs::write(dir.join("j/raw.bin"), b"[#]{|}@\r\n").unwrap();
    let text = |table, country| {
        let o = RenderOptions {
            table,
            country,
            ..RenderOptions::default()
        };
        let d = render_job(&dir, "j", 9, o).unwrap();
        d.pages[0]
            .text
            .iter()
            .map(|r| r.text.clone())
            .collect::<String>()
    };
    assert_ne!(text(CharTable::Pc437, 0), text(CharTable::Pc437, 2));
    assert!(render_job(&dir, "../x", 1, Default::default()).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn settings_table_and_country_become_render_options_and_back() {
    use apple2_config::card::PrinterTable;
    use apple2_printer::font::CharTable;
    use apple2_rust_shell::printer_hub::{render_options, store_setup};
    let spec = PrinterSpec {
        table: PrinterTable::Abicomp,
        country: 5,
        ..PrinterSpec::default()
    };
    let o = render_options(&spec);
    assert_eq!((o.table, o.country), (CharTable::Abicomp, 5));
    let mut back = PrinterSpec::default();
    store_setup(
        &mut back,
        &apple2_rust_shell::printer_model::PrintSetup { options: o },
    );
    assert_eq!((back.table, back.country), (PrinterTable::Abicomp, 5));
}

#[test]
fn an_old_job_keeps_the_table_it_was_printed_with() {
    use apple2_config::card::PrinterTable;
    use apple2_printer::font::CharTable;
    use apple2_rust_shell::printer_model::PrintSetup;
    let spec = PrinterSpec {
        table: PrinterTable::Brascii,
        country: 2,
        ..PrinterSpec::default()
    };
    let (mut emu, mut hub, dir, _) = start("oldtable", spec);
    print_hello(&emu);
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        page_text(&hub).contains("OLA, IMPRESSORA")
    }));
    hub.handle(CenterEvent::FinishJob);
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().active_job.is_none() && !hub.view_model().shown.loading
    }));
    let mut now = hub.view_model().setup;
    now.options.table = CharTable::Pc850;
    now.options.country = 0;
    hub.handle(CenterEvent::SetNextSetup(PrintSetup {
        options: now.options,
    }));
    hub.poll(Instant::now());
    let vm = hub.view_model();
    assert_eq!(vm.setup.options.table, CharTable::Pc850);
    let job = &vm.jobs[0];
    assert_eq!(job.versions[0].options.table, CharTable::Brascii);
    assert_eq!(job.versions[0].options.country, 2);
    assert_eq!(vm.shown.options.table, CharTable::Brascii);
    emu.shutdown();
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn concluding_a_job_closes_it_on_the_card_and_the_next_print_opens_a_new_one() {
    use apple2_rust_shell::printer_model::JobState;
    let (mut emu, mut hub, dir, _) = start("conclude", PrinterSpec::default());
    print_hello(&emu);
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        page_text(&hub).contains("OLA, IMPRESSORA")
    }));
    hub.handle(CenterEvent::FinishJob);
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().active_job.is_none()
            && hub.view_model().jobs[0].state == JobState::Finished
    }));
    // The card closed it with `User` (not the spool behind its back).
    let id = hub.view_model().jobs[0].id.clone();
    let m = std::fs::read_to_string(dir.join("archive").join(&id).join("manifest.json")).unwrap();
    assert!(m.contains("\"close_reason\": \"User\""), "{m}");
    // The program prints again: a new job, not "bytes ignored".
    emu.send(Command::Paste("PRINT \"SEGUNDA\"\r".into()));
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().jobs.len() == 2 && page_text(&hub).contains("SEGUNDA")
    }));
    assert!(
        !hub.view_model()
            .messages
            .iter()
            .any(|m| m.text.contains("ignorado")),
        "{:?}",
        hub.view_model().messages
    );
    emu.shutdown();
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cancelling_closes_marks_cancelled_and_deletes_nothing() {
    use apple2_rust_shell::printer_model::JobState;
    let (mut emu, mut hub, dir, _) = start("cancel", PrinterSpec::default());
    print_hello(&emu);
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        page_text(&hub).contains("OLA, IMPRESSORA")
    }));
    let id = hub.view_model().jobs[0].id.clone();
    hub.handle(CenterEvent::CancelJob);
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().active_job.is_none()
            && hub.view_model().jobs.first().map(|j| j.state) == Some(JobState::Cancelled)
    }));
    assert_eq!(hub.view_model().jobs.len(), 1, "the job is still listed");
    let job_dir = dir.join("archive").join(&id);
    let raw = std::fs::read(job_dir.join("raw.bin")).unwrap();
    assert!(String::from_utf8_lossy(&raw).contains("OLA, IMPRESSORA"));
    let m = std::fs::read_to_string(job_dir.join("manifest.json")).unwrap();
    assert!(m.contains("\"cancelled\": true"), "{m}");
    emu.shutdown();
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn if_the_card_never_answers_the_spool_closes_the_job_after_the_fallback() {
    use apple2_printer::capture::{JobOrigin, Transport};
    use apple2_rust_shell::printer_hub::CLOSE_FALLBACK;
    let dir = temp_dir("fallback");
    let (tx, rx) = apple2_hostio::capture_channel();
    let mut hub = PrinterHub::start(dir.join("archive"), rx, None, Arc::new(|| {})).unwrap();
    let job = tx.open(
        JobOrigin {
            machine: "Apple //e".into(),
            slot: 1,
        },
        Transport::EightBit,
    );
    tx.bytes(job, 0, b"linha\n".to_vec());
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().active_job.is_some()
    }));
    let t0 = Instant::now();
    hub.handle(CenterEvent::FinishJob);
    // Nobody runs the card: before the fallback nothing changes...
    hub.poll(t0);
    assert!(hub.view_model().active_job.is_some());
    // ...after it the spool closes the job itself.
    assert!(wait_for(|| {
        hub.poll(t0 + CLOSE_FALLBACK + Duration::from_millis(1));
        hub.view_model().active_job.is_none()
    }));
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

/// The version in the PDF metadata is the application's: the printer crate and the app share the
/// workspace version.
#[test]
fn the_printer_crate_and_the_app_share_the_version_written_in_the_pdf() {
    assert_eq!(
        apple2_printer::pdf::PdfMeta::default().app_version,
        env!("CARGO_PKG_VERSION")
    );
}
