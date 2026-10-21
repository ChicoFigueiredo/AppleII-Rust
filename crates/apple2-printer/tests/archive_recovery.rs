#![cfg(feature = "fs")]
mod common;
use apple2_printer::archive::*;
use apple2_printer::capture::CloseReason;
use common::*;
use std::fs;
use std::time::Duration;

/// T-021: crash after blocks were persisted -> interrupted, truncated at the checkpoint.
#[test]
fn t021_crash_recovers_interrupted_truncated_to_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let id;
    {
        let mut a = Archive::open(dir.path()).unwrap();
        a.set_checkpoint_policy(20, Duration::from_secs(3600));
        id = a.start_job(new_job(1)).unwrap();
        a.append(&id, b"0123456789ABCDEFGHIJ").unwrap(); // checkpoint here
        a.append(&id, b"tail-not-confirmed").unwrap();
        // Crash: the archive is dropped without closing the job.
    }
    let raw = dir.path().join(&id).join("raw.bin");
    assert_eq!(fs::metadata(&raw).unwrap().len(), 20 + 18);

    let a = Archive::open(dir.path()).unwrap();
    let m = a.manifest(&id).unwrap();
    assert_eq!(m.state, JobState::Interrupted);
    assert_eq!(m.total_bytes, 20);
    assert_eq!(fs::read(&raw).unwrap(), b"0123456789ABCDEFGHIJ");
    assert_eq!(
        m.sha256.as_deref(),
        Some(sha_hex(b"0123456789ABCDEFGHIJ").as_str())
    );
    assert!(a.verify(&id).unwrap());
    assert!(a.recovery().iter().any(|r| matches!(
        r,
        RecoveryEvent::Interrupted {
            found_bytes: 38,
            kept_bytes: 20,
            ..
        }
    )));
    // Recovery is persisted: a third open does not repeat it.
    let b = Archive::open(dir.path()).unwrap();
    assert!(b.recovery().is_empty());
    assert_eq!(b.manifest(&id).unwrap().state, JobState::Interrupted);
}

#[test]
fn recovery_never_invents_bytes_when_raw_is_shorter_than_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    a.set_checkpoint_policy(4, Duration::from_secs(3600));
    let id = a.start_job(new_job(1)).unwrap();
    a.append(&id, b"abcd").unwrap();
    drop(a);
    fs::write(dir.path().join(&id).join("raw.bin"), b"ab").unwrap();
    let a = Archive::open(dir.path()).unwrap();
    let m = a.manifest(&id).unwrap();
    assert_eq!((m.total_bytes, m.state), (2, JobState::Interrupted));
}

fn put_manifest(dir: &std::path::Path, name: &str, body: &[u8]) {
    let jd = dir.join(name);
    fs::create_dir(&jd).unwrap();
    fs::write(jd.join("raw.bin"), b"").unwrap();
    fs::write(jd.join("manifest.json"), body).unwrap();
}

fn damaged_ids(a: &Archive) -> Vec<String> {
    a.jobs()
        .iter()
        .filter(|j| matches!(j, JobEntry::Damaged { .. }))
        .map(|j| j.id().to_string())
        .collect()
}

#[test]
fn hostile_manifests_become_damaged_without_panic() {
    let dir = tempfile::tempdir().unwrap();
    put_manifest(dir.path(), "garbage", b"\x00\xff not json {{{");
    put_manifest(
        dir.path(),
        "huge",
        &vec![b' '; (MAX_MANIFEST_BYTES + 1) as usize],
    );
    put_manifest(dir.path(), "deep", "[".repeat(100_000).as_bytes());
    put_manifest(dir.path(), "future", br#"{"schema_version": 99}"#);
    put_manifest(dir.path(), "noschema", br#"{"id":"noschema"}"#);
    put_manifest(
        dir.path(),
        "wrongtypes",
        br#"{"schema_version":2,"id":"wrongtypes","total_bytes":"x"}"#,
    );
    let mut good = Archive::open(dir.path()).unwrap();
    let mut got = damaged_ids(&good);
    got.sort();
    assert_eq!(
        got,
        [
            "deep",
            "future",
            "garbage",
            "huge",
            "noschema",
            "wrongtypes"
        ]
    );
    assert!(good.recovery().len() >= 6);
    // Damaged jobs are listed and can be deleted explicitly.
    good.delete("garbage").unwrap();
    assert!(!dir.path().join("garbage").exists());
}

#[test]
fn manifest_id_must_match_folder_so_nothing_points_outside() {
    let dir = tempfile::tempdir().unwrap();
    let id = "20260101-000000-bbbbbb";
    let mut a = Archive::open(dir.path()).unwrap();
    let real = a.start_job(new_job(1)).unwrap();
    a.finish_job(&real, CloseReason::User).unwrap();
    let mut json: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join(&real).join("manifest.json")).unwrap())
            .unwrap();
    json["id"] = "../../etc".into();
    let jd = dir.path().join(id);
    fs::create_dir(&jd).unwrap();
    fs::write(jd.join("raw.bin"), b"").unwrap();
    fs::write(jd.join("manifest.json"), json.to_string()).unwrap();
    let a = Archive::open(dir.path()).unwrap();
    assert!(matches!(a.get(id), Some(JobEntry::Damaged { .. })));
    assert!(a.manifest(&real).is_some());
}

#[test]
fn exported_path_in_manifest_is_data_only() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    let id = a.start_job(new_job(1)).unwrap();
    a.finish_job(&id, CloseReason::User).unwrap();
    a.record_export(&id, 1, &outside.path().join("../../evil.pdf"))
        .unwrap();
    drop(a);
    let a = Archive::open(dir.path()).unwrap();
    assert!(a.manifest(&id).is_some());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn symlinks_are_never_followed() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.txt"), b"keep").unwrap();

    // A job directory that is a symlink to an outside folder.
    symlink(outside.path(), dir.path().join("20260101-000000-cccccc")).unwrap();
    // A job whose manifest.json and raw.bin are symlinks.
    let jd = dir.path().join("20260101-000000-dddddd");
    fs::create_dir(&jd).unwrap();
    symlink(outside.path().join("secret.txt"), jd.join("manifest.json")).unwrap();
    symlink(outside.path().join("secret.txt"), jd.join("raw.bin")).unwrap();

    let mut a = Archive::open(dir.path()).unwrap();
    let mut got = damaged_ids(&a);
    got.sort();
    assert_eq!(got, ["20260101-000000-cccccc", "20260101-000000-dddddd"]);
    // Deleting the symlinked entry must not remove the outside folder's content.
    let _ = a.delete("20260101-000000-cccccc");
    assert_eq!(
        fs::read(outside.path().join("secret.txt")).unwrap(),
        b"keep"
    );
}

#[test]
fn completed_job_with_wrong_raw_length_is_damaged() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    let id = a.start_job(new_job(1)).unwrap();
    a.append(&id, b"abc").unwrap();
    a.finish_job(&id, CloseReason::User).unwrap();
    fs::write(dir.path().join(&id).join("raw.bin"), b"abcdef").unwrap();
    let a = Archive::open(dir.path()).unwrap();
    assert!(matches!(a.get(&id), Some(JobEntry::Damaged { .. })));
}
