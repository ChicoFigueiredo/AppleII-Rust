#![cfg(feature = "fs")]
mod common;
use apple2_printer::archive::*;
use apple2_printer::capture::CloseReason;
use common::*;
use std::collections::HashSet;
use std::fs;
use std::time::Duration;

#[test]
fn job_lifecycle_writes_manifest_and_sha() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    let id = a.start_job(new_job(7)).unwrap();
    a.append(&id, b"ab").unwrap();
    a.append(&id, b"c").unwrap();
    a.finish_job(&id, CloseReason::Idle).unwrap();

    let m = a.manifest(&id).unwrap();
    assert_eq!(m.schema_version, SCHEMA_VERSION);
    assert_eq!(m.state, JobState::Completed);
    assert_eq!(m.close_reason, Some(CloseReason::Idle));
    assert_eq!(m.total_bytes, 3);
    assert_eq!(m.profile, "Epson LX-310");
    assert_eq!(m.capture_job, 7);
    assert_eq!(m.versions.len(), 1);
    assert!(m.completed_at.is_some());
    // SHA-256("abc")
    assert_eq!(
        m.sha256.as_deref(),
        Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
    assert_eq!(
        fs::read(dir.path().join(&id).join("raw.bin")).unwrap(),
        b"abc"
    );
    assert!(a.verify(&id).unwrap());

    // The manifest on disk round-trips through a fresh open.
    let b = Archive::open(dir.path()).unwrap();
    assert_eq!(b.manifest(&id).unwrap(), m);
}

#[test]
fn ids_are_safe_and_unique() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    let mut b = Archive::open(dir.path()).unwrap();
    let fixed = chrono::DateTime::parse_from_rfc3339("2026-10-21T14:03:01Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    a.set_clock(move || fixed);
    b.set_clock(move || fixed);
    let mut ids = HashSet::new();
    for i in 0..30 {
        for arch in [&mut a, &mut b] {
            let id = arch.start_job(new_job(i)).unwrap();
            assert!(id.starts_with("20261021-140301-"), "{id}");
            assert!(id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'));
            assert!(ids.insert(id), "duplicate id");
        }
    }
}

#[test]
fn title_note_versions_and_exports_keep_raw_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    let id = a.start_job(new_job(1)).unwrap();
    a.append(&id, b"hello\n").unwrap();
    a.finish_job(&id, CloseReason::User).unwrap();
    let sha = a.manifest(&id).unwrap().sha256.clone();

    a.set_auto_title(&id, "auto").unwrap();
    assert_eq!(a.manifest(&id).unwrap().title, "auto");
    a.set_title(&id, "Meu relatório #1 ção").unwrap();
    a.set_auto_title(&id, "auto 2").unwrap();
    assert_eq!(a.manifest(&id).unwrap().title, "Meu relatório #1 ção");
    a.set_note(&id, "nota pessoal").unwrap();

    let v = a.add_version(&id, a4(), 3, 0).unwrap();
    assert_eq!(v, 2);
    let pdf = a.export_path(&id, v).unwrap();
    assert!(pdf.starts_with(dir.path().join(&id)));
    a.record_export(&id, v, &pdf).unwrap();

    let b = Archive::open(dir.path()).unwrap();
    let m = b.manifest(&id).unwrap();
    assert_eq!(m.note, "nota pessoal");
    assert_eq!(m.versions.len(), 2);
    assert_eq!(m.versions[1].options, a4());
    assert_eq!(m.exports.len(), 1);
    assert_eq!(m.sha256, sha);
    assert!(b.verify(&id).unwrap());
}

#[test]
fn retention_warns_but_never_deletes_and_refuses_active() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    a.set_retention_limit(Some(100));
    let id = a.start_job(new_job(1)).unwrap();
    a.append(&id, &[b'x'; 500]).unwrap();
    a.checkpoint(&id).unwrap();
    assert!(a.total_size() >= 500);
    let w = a.retention_warning().expect("over the limit");
    assert_eq!(w.limit_bytes, 100);

    // Active job cannot be deleted; nothing disappears on its own.
    assert!(matches!(a.delete(&id), Err(ArchiveError::JobActive(_))));
    assert!(dir.path().join(&id).join("raw.bin").exists());

    a.finish_job(&id, CloseReason::User).unwrap();
    assert!(a.retention_warning().is_some());
    assert!(dir.path().join(&id).join("raw.bin").exists());
    a.delete(&id).unwrap();
    assert!(!dir.path().join(&id).exists());
    assert!(a.get(&id).is_none());
    assert!(a.retention_warning().is_none());
}

#[test]
fn delete_rejects_traversal_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    for bad in ["..", "../x", "a/b", "", "ção"] {
        assert!(
            matches!(a.delete(bad), Err(ArchiveError::InvalidName(_))),
            "{bad}"
        );
    }
}

#[test]
fn checkpoint_policy_records_confirmed_offset() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    a.set_checkpoint_policy(10, Duration::from_secs(3600));
    let id = a.start_job(new_job(1)).unwrap();
    a.append(&id, b"12345").unwrap();
    assert_eq!(a.manifest(&id).unwrap().confirmed_offset, 0);
    a.append(&id, b"67890").unwrap();
    assert_eq!(a.manifest(&id).unwrap().confirmed_offset, 10);
}

#[test]
fn default_root_ends_with_impressoes() {
    if let Some(p) = default_root() {
        assert!(p.ends_with("impressoes"));
    }
}

#[test]
fn reads_schema_v1_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let id = "20200101-000000-aaaaaa";
    let jd = dir.path().join(id);
    fs::create_dir(&jd).unwrap();
    fs::write(jd.join("raw.bin"), b"abc").unwrap();
    let v1 = serde_json::json!({
        "schema_version": 1,
        "id": id,
        "capture_job": 3,
        "origin": {"machine": "Apple II+", "slot": 1},
        "transport": "EightBit",
        "profile": "Epson LX-310",
        "options": serde_json::to_value(apple2_printer::paper::RenderOptions::default()).unwrap(),
        "started_at": "2020-01-01T00:00:00Z",
        "completed_at": "2020-01-01T00:00:05Z",
        "close_reason": "Idle",
        "size": 3,
        "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "pages": 1,
        "title": "velho",
        "state": "completed"
    });
    fs::write(jd.join("manifest.json"), v1.to_string()).unwrap();

    let a = Archive::open(dir.path()).unwrap();
    let m = a.manifest(id).expect("v1 manifest must load");
    assert_eq!(m.schema_version, SCHEMA_VERSION);
    assert_eq!(m.total_bytes, 3);
    assert_eq!(m.confirmed_offset, 3);
    assert_eq!(m.title, "velho");
    assert_eq!(m.versions.len(), 1, "original version synthesised");
    assert!(a.verify(id).unwrap());
}

#[test]
fn reads_schema_v2_manifest_without_table_country_or_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let id = "20200102-000000-bbbbbb";
    let jd = dir.path().join(id);
    fs::create_dir(&jd).unwrap();
    fs::write(jd.join("raw.bin"), b"abc").unwrap();
    let mut options =
        serde_json::to_value(apple2_printer::paper::RenderOptions::default()).unwrap();
    let o = options.as_object_mut().unwrap();
    o.remove("table");
    o.remove("country");
    let v2 = serde_json::json!({
        "schema_version": 2,
        "id": id,
        "capture_job": 4,
        "origin": {"machine": "Apple II+", "slot": 1},
        "transport": "EightBit",
        "profile": "Epson LX-310",
        "options": options,
        "started_at": "2020-01-02T00:00:00Z",
        "total_bytes": 3,
        "confirmed_offset": 3,
        "state": "completed"
    });
    fs::write(jd.join("manifest.json"), v2.to_string()).unwrap();
    let a = Archive::open(dir.path()).unwrap();
    let m = a.manifest(id).expect("v2 manifest must load");
    assert_eq!(m.schema_version, 3);
    assert!(!m.cancelled);
    assert_eq!(m.options.table, apple2_printer::font::CharTable::Pc437);
    assert_eq!(m.options.country, 0);
    assert_eq!(
        m.versions[0].options.table,
        apple2_printer::font::CharTable::Pc437
    );
}
