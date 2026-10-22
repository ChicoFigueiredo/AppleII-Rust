#![cfg(feature = "fs")]
mod common;
use apple2_printer::archive::*;
use apple2_printer::capture::{CaptureEvent, CloseReason};
use apple2_printer::spool::*;
use common::*;
use std::fs;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn lines(n: usize) -> Vec<u8> {
    (0..n)
        .flat_map(|i| format!("linha {i} ção\n").into_bytes())
        .collect()
}

fn feed_in_chunks(s: &mut Spool, job: u64, data: &[u8], chunk: usize) {
    let mut off = 0u64;
    for c in data.chunks(chunk) {
        s.handle(bytes(job, off, c));
        off += c.len() as u64;
    }
}

/// T-018: only the spool runs (no window/consumer): 100 lines later everything is there.
#[test]
fn t018_spool_alone_keeps_everything() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, feeds) = spool_in(dir.path());
    let data = lines(100);
    s.handle(open(1));
    feed_in_chunks(&mut s, 1, &data, 37);
    // Still active: the snapshot carries the document so far.
    s.publish_now();
    let snap = s.snapshot_handle().get();
    let act = snap.active.as_ref().expect("active job");
    assert_eq!(act.bytes, data.len() as u64);
    assert_eq!(
        act.document
            .pages
            .iter()
            .map(|p| p.text.len())
            .sum::<usize>(),
        100
    );
    s.handle(close(1, CloseReason::Idle));

    assert!(
        feeds.load(Ordering::SeqCst) > 10,
        "engine fed incrementally"
    );
    let snap = s.snapshot_handle().get();
    assert!(snap.active.is_none());
    assert_eq!(snap.jobs.len(), 1);
    let j = &snap.jobs[0];
    assert_eq!(j.state, Some(JobState::Completed));
    assert_eq!(j.close_reason, Some(CloseReason::Idle));
    assert_eq!(j.pages, 2);
    assert!(j.title.starts_with("linha 0 ção — 20"), "{}", j.title);
    let raw = fs::read(dir.path().join(&j.id).join("raw.bin")).unwrap();
    assert_eq!(raw, data);
}

#[test]
fn preview_revision_changes_only_when_document_changes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.handle(open(1));
    s.handle(bytes(1, 0, b"abc\n"));
    s.publish_now();
    let a = s.snapshot_handle().get().active.clone().unwrap();
    s.publish_now();
    let b = s.snapshot_handle().get().active.clone().unwrap();
    assert_eq!(a.doc_revision, b.doc_revision);
    assert!(
        std::sync::Arc::ptr_eq(&a.document, &b.document),
        "no copy without change"
    );
    s.handle(bytes(1, 4, b"def\n"));
    s.publish_now();
    let c = s.snapshot_handle().get().active.clone().unwrap();
    assert_ne!(b.doc_revision, c.doc_revision);
}

/// T-019: reset / load / card removal close with the right reason; jobs never mix bytes.
#[test]
fn t019_close_reasons_and_no_mixing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    let reasons = [
        CloseReason::Reset,
        CloseReason::StateLoaded,
        CloseReason::CardRemoved,
    ];
    for (i, r) in reasons.iter().enumerate() {
        let job = i as u64 + 1;
        s.handle(open(job));
        s.handle(bytes(job, 0, format!("job{job}\n").as_bytes()));
        s.handle(close(job, *r));
        // Late bytes of the closed job are dropped with a notice, never appended anywhere.
        s.handle(bytes(job, 5, b"LATE\n"));
    }
    s.publish_now();
    let snap = s.snapshot_handle().get();
    assert_eq!(snap.jobs.len(), 3);
    assert!(snap.notices.iter().any(|n| n.contains("ignorados")));
    for (i, j) in snap.jobs.iter().enumerate() {
        assert_eq!(j.close_reason, Some(reasons[i]));
        let raw = fs::read(dir.path().join(&j.id).join("raw.bin")).unwrap();
        assert_eq!(raw, format!("job{}\n", i + 1).into_bytes());
    }
}

#[test]
fn open_without_close_closes_previous_as_reset() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.handle(open(1));
    s.handle(bytes(1, 0, b"one\n"));
    s.handle(open(2));
    s.handle(bytes(2, 0, b"two\n"));
    s.shutdown();
    let snap = s.snapshot_handle().get();
    assert_eq!(snap.jobs[0].close_reason, Some(CloseReason::Reset));
    assert_eq!(snap.jobs[1].close_reason, Some(CloseReason::Shutdown));
    assert_eq!(snap.jobs[0].total_bytes, 4);
}

#[test]
fn user_finish_closes_with_user_reason() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.handle(open(1));
    s.handle(bytes(1, 0, b"x\n"));
    s.finish_by_user();
    let snap = s.snapshot_handle().get();
    assert_eq!(snap.jobs[0].close_reason, Some(CloseReason::User));
    assert!(snap.active.is_none());
}

#[test]
fn duplicate_bytes_are_dropped_and_gaps_reported() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.handle(open(1));
    s.handle(bytes(1, 0, b"abcd"));
    s.handle(bytes(1, 2, b"cdef")); // 2 repeated
    s.handle(bytes(1, 10, b"Z")); // gap of 4
    s.finish_by_user();
    let snap = s.snapshot_handle().get();
    let raw = fs::read(dir.path().join(&snap.jobs[0].id).join("raw.bin")).unwrap();
    assert_eq!(raw, b"abcdefZ");
    assert!(snap.notices.iter().any(|n| n.contains("repetidos")));
    assert!(snap.notices.iter().any(|n| n.contains("lacuna")));
}

#[test]
fn reprint_adds_version_and_keeps_raw_sha() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    let data = lines(130);
    s.handle(open(1));
    feed_in_chunks(&mut s, 1, &data, 500);
    s.handle(close(1, CloseReason::User));
    let id = s.snapshot_handle().get().jobs[0].id.clone();
    let before = s.archive().manifest(&id).unwrap().sha256.clone();
    let raw_path = dir.path().join(&id).join("raw.bin");
    let mtime = fs::metadata(&raw_path).unwrap().modified().unwrap();

    let v = s.reprint(&id, a4()).unwrap();
    assert_eq!(v, 2);
    let m = s.archive().manifest(&id).unwrap();
    assert_eq!(m.versions.len(), 2);
    assert_eq!(m.versions[1].options, a4());
    assert_eq!(m.versions[1].pages, 3);
    assert_eq!(m.sha256, before);
    assert_eq!(fs::read(&raw_path).unwrap(), data);
    assert_eq!(fs::metadata(&raw_path).unwrap().modified().unwrap(), mtime);
    assert!(s.archive().verify(&id).unwrap());

    let snap = s.snapshot_handle().get();
    let rep = snap.reprint.as_ref().unwrap();
    assert_eq!(rep.version, 2);
    assert_eq!(rep.document.pages[0].width, a4().paper.width);
    // Original version still renders with its own options.
    let orig = s.render_version(&id, 1).unwrap();
    assert_eq!(
        orig.pages[0].width,
        apple2_printer::paper::Paper::default().width
    );
}

#[test]
fn reprint_of_active_or_unknown_job_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.handle(open(1));
    s.handle(bytes(1, 0, b"x\n"));
    let id = s.snapshot_handle().get().active.clone().unwrap().job_id;
    assert!(matches!(
        s.reprint(&id, a4()),
        Err(ArchiveError::JobActive(_))
    ));
    assert!(matches!(s.delete(&id), Err(ArchiveError::JobActive(_))));
    assert!(s.reprint("nope", a4()).is_err());
}

#[test]
fn storage_failure_keeps_preview_and_is_visible() {
    use std::io::Write;
    struct Broken(fs::File);
    impl RawSink for Broken {
        fn write_all(&mut self, d: &[u8]) -> std::io::Result<()> {
            if d.contains(&b'!') {
                Err(std::io::Error::other("disco cheio"))
            } else {
                self.0.write_all(d)
            }
        }
        fn sync(&mut self) -> std::io::Result<()> {
            Ok(())
        }
        fn truncate(&mut self, len: u64) -> std::io::Result<()> {
            self.0.set_len(len)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.archive_mut().set_sink_factory(|f| Box::new(Broken(f)));
    s.handle(open(1));
    s.handle(bytes(1, 0, b"ok\n"));
    s.handle(bytes(1, 3, b"bad!\n"));
    s.handle(bytes(1, 8, b"after\n"));
    s.publish_now();
    let snap = s.snapshot_handle().get();
    let act = snap.active.as_ref().unwrap();
    assert!(act
        .storage_error
        .as_deref()
        .unwrap()
        .contains("disco cheio"));
    assert_eq!(act.document.pages[0].text.len(), 3, "preview keeps working");
    assert!(snap.notices.iter().any(|n| n.contains("falha ao gravar")));
    s.finish_by_user();
    let id = s.snapshot_handle().get().jobs[0].id.clone();
    let m = s.archive().manifest(&id).unwrap();
    assert_eq!(m.total_bytes, 3);
    assert!(m.diagnostics[0].message.contains("armazenamento"));
}

#[test]
fn engine_diagnostics_are_summarised_in_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.handle(open(1));
    s.handle(bytes(1, 0, b"a\xFFb\n"));
    s.finish_by_user();
    let id = s.snapshot_handle().get().jobs[0].id.clone();
    let m = s.archive().manifest(&id).unwrap();
    assert_eq!(m.diagnostics_total, 1);
    assert_eq!(m.diagnostics[0].offset, 1);
}

#[test]
fn reopened_archive_reports_interrupted_job_in_notices() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (mut s, _) = spool_in(dir.path());
        s.archive_mut()
            .set_checkpoint_policy(4, Duration::from_secs(3600));
        s.handle(open(1));
        s.handle(bytes(1, 0, b"abcdefgh\nxx"));
        // crash: spool dropped without closing
    }
    let (s, _) = spool_in(dir.path());
    let snap = s.snapshot_handle().get();
    assert_eq!(snap.jobs[0].state, Some(JobState::Interrupted));
    assert!(snap.notices.iter().any(|n| n.contains("interrompido")));
}

#[test]
fn threaded_service_with_ui_closed() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = spool_in(dir.path());
    let (tx, rx) = mpsc::channel::<CaptureEvent>();
    let svc = SpoolService::spawn(s, rx);
    let h = svc.handle();

    let data = lines(100);
    tx.send(open(1)).unwrap();
    let mut off = 0u64;
    for c in data.chunks(64) {
        tx.send(bytes(1, off, c)).unwrap();
        off += c.len() as u64;
    }
    // User action through the command channel.
    let deadline = Instant::now() + Duration::from_secs(10);
    while h.snapshot().active.as_ref().map(|a| a.bytes) != Some(data.len() as u64) {
        assert!(
            Instant::now() < deadline,
            "spool did not consume the stream"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(h.send(SpoolCommand::FinishActive));
    while h.snapshot().active.is_some() {
        assert!(Instant::now() < deadline, "finish not applied");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(h.snapshot().jobs[0].close_reason, Some(CloseReason::User));

    // Dropping the card side stops the thread and hands the spool back.
    tx.send(open(2)).unwrap();
    tx.send(bytes(2, 0, b"tail\n")).unwrap();
    drop(tx);
    let spool = svc.join().expect("thread ended cleanly");
    let jobs = spool.archive().jobs();
    assert_eq!(jobs.len(), 2);
    assert_eq!(
        jobs[1].manifest().unwrap().close_reason,
        Some(CloseReason::Shutdown)
    );
}

#[test]
fn record_export_command_registers_the_file_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = spool_in(dir.path());
    let (tx, rx) = mpsc::channel::<CaptureEvent>();
    let svc = SpoolService::spawn(s, rx);
    let h = svc.handle();
    tx.send(open(1)).unwrap();
    tx.send(bytes(1, 0, b"a\n")).unwrap();
    tx.send(close(1, CloseReason::Idle)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while h.snapshot().jobs.first().and_then(|j| j.state) != Some(JobState::Completed) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let id = h.snapshot().jobs[0].id.clone();
    assert!(h.send(SpoolCommand::RecordExport {
        job_id: id.clone(),
        version: 1,
        path: "/tmp/saida.pdf".into(),
    }));
    // Visible in the snapshot and on disk without shutting the spool down.
    while h.snapshot().jobs[0].exports.is_empty() {
        assert!(Instant::now() < deadline, "export not recorded");
        std::thread::sleep(Duration::from_millis(5));
    }
    let e = &h.snapshot().jobs[0].exports[0];
    assert_eq!((e.version, e.path.as_str()), (1, "/tmp/saida.pdf"));
    let on_disk = fs::read_to_string(dir.path().join(&id).join("manifest.json")).unwrap();
    assert!(on_disk.contains("/tmp/saida.pdf"), "{on_disk}");
    drop(tx);
    svc.join().unwrap();
}

#[test]
fn job_summary_carries_what_the_central_needs() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.handle(open(1));
    s.handle(bytes(1, 0, b"primeira linha\nsegunda\n"));
    // While active the list already shows a human title and the origin/options.
    s.publish_now();
    let snap = s.snapshot_handle().get();
    let j = &snap.jobs[0];
    assert!(j.title.starts_with("primeira linha — 20"), "{}", j.title);
    assert_eq!(j.origin.as_ref().unwrap().slot, 1);
    assert_eq!(
        j.transport,
        Some(apple2_printer::capture::Transport::EightBit)
    );
    assert_eq!(j.options, apple2_printer::paper::RenderOptions::default());
    assert_eq!(snap.active.as_ref().unwrap().options, j.options);
    assert_eq!(snap.active.as_ref().unwrap().title, j.title);
    s.handle(close(1, CloseReason::Idle));
    let id = s.snapshot_handle().get().jobs[0].id.clone();
    s.set_note(&id, "minha nota").unwrap();
    s.reprint(&id, a4()).unwrap();
    s.record_export(&id, 2, std::path::Path::new("/x/y.pdf"))
        .unwrap();
    let j = s.snapshot_handle().get().jobs[0].clone();
    assert_eq!(j.note, "minha nota");
    assert_eq!(j.versions.len(), 2);
    assert_eq!(j.versions[1].options, a4());
    assert_eq!(j.exports.len(), 1);
    assert!(j.sha256.is_some());
    assert!(!j.cancelled);
    assert!(j.title.starts_with("primeira linha — 20"));
}

#[test]
fn cancel_marks_the_job_and_keeps_its_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.handle(open(1));
    s.handle(bytes(1, 0, b"a\n"));
    let id = s
        .snapshot_handle()
        .get()
        .active
        .as_ref()
        .unwrap()
        .job_id
        .clone();
    s.cancel(&id).unwrap();
    s.handle(close(1, CloseReason::User));
    let j = s.snapshot_handle().get().jobs[0].clone();
    assert!(j.cancelled);
    assert_eq!(j.close_reason, Some(CloseReason::User));
    assert_eq!(
        fs::read(dir.path().join(&id).join("raw.bin")).unwrap(),
        b"a\n"
    );
    // The mark survives a reopen.
    let a = Archive::open(dir.path()).unwrap();
    assert!(a.manifest(&id).unwrap().cancelled);
}

fn open_slot(job: u64, slot: u8) -> CaptureEvent {
    CaptureEvent::Open {
        job,
        origin: apple2_printer::capture::JobOrigin {
            machine: "Apple //e".into(),
            slot,
        },
        transport: apple2_printer::capture::Transport::EightBit,
    }
}

/// Two Generic Printers (slots 1 and 2) printing at once: two complete jobs, no spurious Reset.
#[test]
fn two_slots_interleaved_make_two_complete_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    let a = lines(40);
    let b: Vec<u8> = (0..40)
        .flat_map(|i| format!("outra {i}\n").into_bytes())
        .collect();
    s.handle(open_slot(1, 1));
    s.handle(open_slot(2, 2));
    let (mut oa, mut ob) = (0usize, 0usize);
    while oa < a.len() || ob < b.len() {
        let ea = (oa + 23).min(a.len());
        if oa < ea {
            s.handle(bytes(1, oa as u64, &a[oa..ea]));
            oa = ea;
        }
        let eb = (ob + 17).min(b.len());
        if ob < eb {
            s.handle(bytes(2, ob as u64, &b[ob..eb]));
            ob = eb;
        }
        s.publish_now();
    }
    s.handle(close(2, CloseReason::Idle));
    s.handle(close(1, CloseReason::Idle));
    let snap = s.snapshot_handle().get();
    assert_eq!(snap.jobs.len(), 2);
    assert!(snap.active.is_none());
    for j in &snap.jobs {
        assert_eq!(j.state, Some(JobState::Completed));
        assert_eq!(j.close_reason, Some(CloseReason::Idle), "no spurious Reset");
        let raw = fs::read(dir.path().join(&j.id).join("raw.bin")).unwrap();
        let want = if j.origin.as_ref().unwrap().slot == 1 {
            &a
        } else {
            &b
        };
        assert_eq!(&raw, want);
    }
    assert!(
        !snap
            .notices
            .iter()
            .any(|n| n.contains("ignorados") || n.contains("sem fechamento")),
        "{:?}",
        snap.notices
    );
}

/// The preview follows the last job opened; a quiet job gives it up, a busy one keeps it.
#[test]
fn preview_focus_follows_the_job_that_prints() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, _) = spool_in(dir.path());
    s.handle(open_slot(1, 1));
    s.handle(bytes(1, 0, b"um\n"));
    s.handle(open_slot(2, 2));
    s.publish_now();
    assert_eq!(
        s.snapshot_handle()
            .get()
            .active
            .as_ref()
            .unwrap()
            .capture_job,
        2
    );
    // Job 2 is still fresh: bytes of job 1 do not steal the preview.
    s.handle(bytes(2, 0, b"dois\n"));
    s.handle(bytes(1, 3, b"mais\n"));
    s.publish_now();
    assert_eq!(
        s.snapshot_handle()
            .get()
            .active
            .as_ref()
            .unwrap()
            .capture_job,
        2
    );
    // After a quiet spell job 1 takes it.
    std::thread::sleep(Duration::from_millis(600));
    s.handle(bytes(1, 8, b"tres\n"));
    s.publish_now();
    let snap = s.snapshot_handle().get();
    let act = snap.active.as_ref().unwrap();
    assert_eq!(act.capture_job, 1);
    assert_eq!(act.bytes, 13);
    // Closing the followed job hands the preview to the other one.
    s.handle(close(1, CloseReason::Idle));
    assert_eq!(
        s.snapshot_handle()
            .get()
            .active
            .as_ref()
            .unwrap()
            .capture_job,
        2
    );
    s.handle(close(2, CloseReason::Idle));
    // Same slot again: the earlier job of that slot is closed as Reset.
    s.handle(open_slot(3, 1));
    s.handle(open_slot(4, 1));
    let snap = s.snapshot_handle().get();
    assert!(snap.notices.iter().any(|n| n.contains("sem fechamento")));
    s.shutdown();
    let snap = s.snapshot_handle().get();
    assert!(snap
        .jobs
        .iter()
        .all(|j| j.state == Some(JobState::Completed)));
}
