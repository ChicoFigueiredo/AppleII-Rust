//! HU-IMP-080/081: Salvar PDF / Salvar RAW fail visibly (never a panic, never a stuck emulation)
//! and never write outside the chosen destination. No emulator needed: the test plays the card
//! through the capture queue.

use apple2_hostio::{capture_channel, CaptureSender};
use apple2_printer::capture::{CloseReason, JobOrigin, Transport};
use apple2_rust_shell::printer_hub::PrinterHub;
use apple2_rust_shell::printer_model::{PageRange, PdfRequest};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("apple2-rust-expfault-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
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

fn origin() -> JobOrigin {
    JobOrigin {
        machine: "Apple //e Enhanced".into(),
        slot: 1,
    }
}

fn print_job(tx: &CaptureSender, text: &[u8]) {
    let job = tx.open(origin(), Transport::EightBit);
    tx.bytes(job, 0, text.to_vec());
    tx.close(job, CloseReason::Idle);
}

/// A hub with one finished job; returns (hub, sender, work dir, job id).
fn hub_with_job(tag: &str) -> (PrinterHub, CaptureSender, PathBuf, String) {
    let dir = temp_dir(tag);
    let (tx, rx) = capture_channel();
    let mut hub = PrinterHub::start(dir.join("archive"), rx, None, Arc::new(|| {})).unwrap();
    print_job(&tx, b"Linha de teste\r\n");
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().jobs.first().is_some_and(|j| j.pages >= 1)
    }));
    let id = hub.view_model().jobs[0].id.clone();
    (hub, tx, dir, id)
}

fn req(job: &str) -> PdfRequest {
    PdfRequest {
        job: job.into(),
        version: 1,
        range: PageRange::All,
        cover: false,
        title: String::new(),
        note: String::new(),
    }
}

fn has_msg(hub: &PrinterHub, error: bool, starts: &str) -> bool {
    hub.view_model()
        .messages
        .iter()
        .any(|m| m.error == error && m.text.starts_with(starts))
}

fn expect_msg(hub: &mut PrinterHub, error: bool, starts: &str) {
    assert!(
        wait_for(|| {
            hub.poll(Instant::now());
            has_msg(hub, error, starts)
        }),
        "no message {starts:?}: {:?}",
        hub.view_model().messages
    );
}

/// The hub still works after a failed export (no deadlock): a new print arrives.
fn still_alive(hub: &mut PrinterHub, tx: &CaptureSender) {
    let before = hub.view_model().jobs.len();
    print_job(tx, b"depois\r\n");
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model().jobs.len() == before + 1
    }));
}

fn entries(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn pdf_to_a_missing_folder_or_a_directory_is_a_visible_error() {
    let (mut hub, tx, dir, id) = hub_with_job("missing");
    hub.export_pdf(req(&id), dir.join("nao/existe/a.pdf"));
    expect_msg(&mut hub, true, "Falha ao salvar o PDF");
    assert!(!dir.join("nao").exists());
    hub.export_pdf(req(&id), dir.clone());
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        hub.view_model()
            .messages
            .iter()
            .filter(|m| m.error && m.text.starts_with("Falha ao salvar o PDF"))
            .count()
            >= 2
            || has_msg(&hub, true, "Falha ao salvar o PDF")
    }));
    still_alive(&mut hub, &tx);
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn pdf_and_raw_to_a_read_only_folder_are_visible_errors() {
    use std::os::unix::fs::PermissionsExt;
    let (mut hub, tx, dir, id) = hub_with_job("readonly");
    let ro = dir.join("ro");
    std::fs::create_dir(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::File::create(ro.join("probe")).is_ok() {
        // Running as root: the mode is ignored, nothing to check.
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
        hub.shutdown();
        let _ = std::fs::remove_dir_all(dir);
        return;
    }
    hub.export_pdf(req(&id), ro.join("a.pdf"));
    expect_msg(&mut hub, true, "Falha ao salvar o PDF");
    hub.save_raw(&id, ro.join("a.bin"));
    expect_msg(&mut hub, true, "Falha ao salvar os bytes brutos");
    assert!(entries(&ro).is_empty());
    still_alive(&mut hub, &tx);
    hub.shutdown();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn odd_names_work_and_dotdot_stays_where_it_resolves() {
    let (mut hub, tx, dir, id) = hub_with_job("names");
    let sub = dir.join("pasta com espaço");
    std::fs::create_dir(&sub).unwrap();
    let names = ["a #1 ção ★.pdf", "100%.pdf", "  espaço.pdf"];
    for n in names {
        hub.export_pdf(req(&id), sub.join(n));
    }
    // `..` resolves like any path: the file lands in `dir`, not above it.
    hub.export_pdf(req(&id), sub.join("../via-dotdot.pdf"));
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        names.iter().all(|n| sub.join(n).exists()) && dir.join("via-dotdot.pdf").exists()
    }));
    for n in names {
        assert!(std::fs::read(sub.join(n)).unwrap().starts_with(b"%PDF-"));
    }
    hub.save_raw(&id, sub.join("raw #2 ção.bin"));
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        sub.join("raw #2 ção.bin").exists()
    }));
    assert_eq!(
        std::fs::read(sub.join("raw #2 ção.bin")).unwrap(),
        b"Linha de teste\r\n"
    );
    // Nothing leaked next to the work dir.
    let parent = dir.parent().unwrap();
    assert!(!parent.join("via-dotdot.pdf").exists());
    still_alive(&mut hub, &tx);
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn a_symlink_as_destination_is_replaced_never_followed() {
    let (mut hub, tx, dir, id) = hub_with_job("symlink");
    let outside = temp_dir("symlink-outside");
    let secret = outside.join("secret.txt");
    std::fs::write(&secret, b"SEGREDO").unwrap();
    let link = dir.join("link.pdf");
    std::os::unix::fs::symlink(&secret, &link).unwrap();
    hub.export_pdf(req(&id), link.clone());
    hub.save_raw(&id, dir.join("link.pdf"));
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        has_msg(&hub, false, "PDF salvo") || has_msg(&hub, true, "Falha")
    }));
    std::thread::sleep(Duration::from_millis(300));
    hub.poll(Instant::now());
    assert_eq!(std::fs::read(&secret).unwrap(), b"SEGREDO", "outside file written");
    still_alive(&mut hub, &tx);
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(outside);
}

#[test]
fn save_raw_of_an_unsafe_job_id_is_a_visible_error_and_reads_nothing_outside() {
    let (mut hub, tx, dir, _id) = hub_with_job("unsafe-id");
    std::fs::write(dir.join("raw.bin"), b"FORA").unwrap();
    for bad in ["../..", "..", "a/b", ""] {
        hub.save_raw(bad, dir.join("copia.bin"));
    }
    expect_msg(&mut hub, true, "Falha ao salvar os bytes brutos");
    assert!(!dir.join("copia.bin").exists());
    hub.export_pdf(req("../.."), dir.join("x.pdf"));
    expect_msg(&mut hub, true, "Falha ao salvar o PDF");
    assert!(!dir.join("x.pdf").exists());
    still_alive(&mut hub, &tx);
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_damaged_job_cannot_be_exported_and_says_why() {
    let dir = temp_dir("damaged");
    let bad = dir.join("archive").join("20260101-000000-aaaaaa");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("manifest.json"), b"{ not json").unwrap();
    std::fs::write(bad.join("raw.bin"), b"dados").unwrap();
    let (_tx, rx) = capture_channel();
    let mut hub = PrinterHub::start(dir.join("archive"), rx, None, Arc::new(|| {})).unwrap();
    assert!(wait_for(|| {
        hub.poll(Instant::now());
        !hub.view_model().jobs.is_empty()
    }));
    hub.export_pdf(req("20260101-000000-aaaaaa"), dir.join("d.pdf"));
    hub.save_raw("20260101-000000-aaaaaa", dir.join("d.bin"));
    expect_msg(&mut hub, true, "Falha ao salvar o PDF: o trabalho está danificado");
    expect_msg(&mut hub, true, "Falha ao salvar os bytes brutos: o trabalho está danificado");
    assert!(!dir.join("d.pdf").exists() && !dir.join("d.bin").exists());
    hub.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}
