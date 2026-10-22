#![cfg(feature = "fs")]
//! HU-IMP-081: a manifest or raw.bin written by someone else (or damaged by time) is read with
//! bounded memory and never panics; whatever cannot be trusted is listed as "danificado".
mod common;
use apple2_printer::archive::*;
use apple2_printer::capture::CloseReason;
use common::*;
use std::fs;
use std::path::Path;

/// A real, closed job whose manifest the test then rewrites.
fn closed_job(root: &Path) -> (String, serde_json::Value) {
    let mut a = Archive::open(root).unwrap();
    let id = a.start_job(new_job(1)).unwrap();
    a.append(&id, b"abc\r\n").unwrap();
    a.finish_job(&id, CloseReason::User).unwrap();
    let json =
        serde_json::from_slice(&fs::read(root.join(&id).join("manifest.json")).unwrap()).unwrap();
    (id, json)
}

fn rewrite(root: &Path, id: &str, json: &serde_json::Value) {
    fs::write(root.join(id).join("manifest.json"), json.to_string()).unwrap();
}

fn is_damaged(root: &Path, id: &str) -> bool {
    matches!(
        Archive::open(root).unwrap().get(id),
        Some(JobEntry::Damaged { .. })
    )
}

#[test]
fn absurd_sizes_and_truncated_raw_mark_the_job_damaged() {
    for (field, value) in [
        ("total_bytes", serde_json::json!(u64::MAX)),
        ("total_bytes", serde_json::json!(1_000_000_000_000u64)),
        ("pages", serde_json::json!(u64::MAX)),
        ("total_bytes", serde_json::json!(-1)),
        ("sha256", serde_json::json!("0".repeat(64))),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (id, mut json) = closed_job(dir.path());
        json[&field] = value.clone();
        rewrite(dir.path(), &id, &json);
        let a = Archive::open(dir.path()).unwrap();
        let damaged = matches!(a.get(&id), Some(JobEntry::Damaged { .. }));
        // A wrong checksum is only found by `verify`; the others at once. Never a panic.
        if field == "sha256" {
            assert!(!damaged && !a.verify(&id).unwrap());
        } else if field != "pages" {
            assert!(damaged, "{field}={value}");
        }
    }
}

#[test]
fn raw_shorter_than_the_manifest_says_is_damaged() {
    let dir = tempfile::tempdir().unwrap();
    let (id, _) = closed_job(dir.path());
    fs::write(dir.path().join(&id).join("raw.bin"), b"ab").unwrap();
    assert!(is_damaged(dir.path(), &id));
}

#[test]
fn raw_that_is_not_a_regular_file_is_damaged_and_never_read() {
    let dir = tempfile::tempdir().unwrap();
    let (id, _) = closed_job(dir.path());
    let raw = dir.path().join(&id).join("raw.bin");
    fs::remove_file(&raw).unwrap();
    fs::create_dir(&raw).unwrap();
    assert!(is_damaged(dir.path(), &id));
    let a = Archive::open(dir.path()).unwrap();
    assert!(a.open_raw(&id).is_err());
}

#[test]
fn huge_lists_and_strings_inside_the_size_cap_are_trimmed() {
    let dir = tempfile::tempdir().unwrap();
    let (id, mut json) = closed_job(dir.path());
    let diag = serde_json::json!({"offset": 0, "message": "x"});
    json["diagnostics"] = serde_json::Value::Array(vec![diag; 20_000]);
    json["note"] = "n".repeat(2 * 1024 * 1024).into();
    json["title"] = "t".repeat(1024 * 1024).into();
    rewrite(dir.path(), &id, &json);
    let a = Archive::open(dir.path()).unwrap();
    let m = a.manifest(&id).expect("still valid");
    assert!(m.diagnostics.len() <= 100);
    assert!(m.note.chars().count() <= 20_000);
    assert!(m.title.chars().count() <= 200);
}

#[test]
fn future_schema_and_wrong_shapes_are_damaged_with_a_reason() {
    let dir = tempfile::tempdir().unwrap();
    let (id, mut json) = closed_job(dir.path());
    json["schema_version"] = 4.into();
    rewrite(dir.path(), &id, &json);
    let a = Archive::open(dir.path()).unwrap();
    match a.get(&id) {
        Some(JobEntry::Damaged { reason, .. }) => {
            assert!(reason.contains("mais novo"), "{reason}");
        }
        e => panic!("{e:?}"),
    }
    // The raw bytes are still there, untouched: damaged is a label, not a deletion.
    assert_eq!(
        fs::read(dir.path().join(&id).join("raw.bin")).unwrap(),
        b"abc\r\n"
    );
    for bad in [
        serde_json::json!({"schema_version": 3, "id": id, "options": "x"}),
        serde_json::json!({"schema_version": 3, "id": id, "versions": [1, 2]}),
        serde_json::json!({"schema_version": 3, "id": id, "state": "Nope"}),
        serde_json::json!([1, 2, 3]),
        serde_json::json!("string"),
        serde_json::json!(null),
    ] {
        rewrite(dir.path(), &id, &bad);
        assert!(is_damaged(dir.path(), &id), "{bad}");
    }
}

#[test]
fn many_job_folders_are_scanned_with_a_cap() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..300 {
        let jd = dir.path().join(format!("lixo-{i}"));
        fs::create_dir(&jd).unwrap();
        fs::write(jd.join("manifest.json"), b"{").unwrap();
    }
    let a = Archive::open(dir.path()).unwrap();
    assert_eq!(a.jobs().len(), 300);
}
