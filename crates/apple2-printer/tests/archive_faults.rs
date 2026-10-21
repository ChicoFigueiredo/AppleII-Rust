#![cfg(feature = "fs")]
mod common;
use apple2_printer::archive::*;
use apple2_printer::capture::CloseReason;
use common::*;
use std::fs::{self, File};
use std::io::{self, Write};

/// Writes at most `left` more bytes, then behaves like a full disk (after a partial write).
struct FullDisk {
    file: File,
    left: usize,
}

impl RawSink for FullDisk {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        if data.len() <= self.left {
            self.left -= data.len();
            return self.file.write_all(data);
        }
        let part = self.left;
        self.left = 0;
        self.file.write_all(&data[..part])?;
        Err(io::Error::other("No space left on device"))
    }
    fn sync(&mut self) -> io::Result<()> {
        self.file.sync_data()
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        self.file.set_len(len)
    }
}

/// T-023: disk full is a visible error, no panic, the prefix stays consistent.
#[test]
fn t023_disk_full_is_error_and_keeps_whole_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    a.set_sink_factory(|file| Box::new(FullDisk { file, left: 8 }));
    let id = a.start_job(new_job(1)).unwrap();
    a.append(&id, b"12345").unwrap();
    let err = a.append(&id, b"6789ABC").unwrap_err();
    assert!(err.to_string().contains("No space"), "{err}");
    let raw = dir.path().join(&id).join("raw.bin");
    assert_eq!(
        fs::read(&raw).unwrap(),
        b"12345",
        "partial tail rolled back"
    );
    a.finish_job(&id, CloseReason::User).unwrap();
    let m = a.manifest(&id).unwrap();
    assert_eq!(m.total_bytes, 5);
    assert_eq!(m.sha256.as_deref(), Some(sha_hex(b"12345").as_str()));
}

#[test]
fn t023_names_with_hash_and_unicode_work_and_nothing_is_overwritten() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("impressões #1 — ação");
    let mut a = Archive::open(&root).unwrap();
    let id1 = a.start_job(new_job(1)).unwrap();
    a.append(&id1, b"first").unwrap();
    a.finish_job(&id1, CloseReason::User).unwrap();
    let id2 = a.start_job(new_job(1)).unwrap();
    a.append(&id2, b"second").unwrap();
    a.finish_job(&id2, CloseReason::User).unwrap();
    assert_ne!(id1, id2);
    assert_eq!(fs::read(root.join(&id1).join("raw.bin")).unwrap(), b"first");
    assert_eq!(
        fs::read(root.join(&id2).join("raw.bin")).unwrap(),
        b"second"
    );
    let b = Archive::open(&root).unwrap();
    assert_eq!(b.jobs().len(), 2);
}

#[cfg(unix)]
#[test]
fn t023_read_only_root_reports_error_without_panic() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
    let writable_anyway = fs::File::create(dir.path().join("probe")).is_ok();
    let result = a.start_job(new_job(1));
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    if writable_anyway {
        return; // running as root: permissions do not apply
    }
    let err = result.unwrap_err();
    assert!(matches!(err, ArchiveError::Io { .. }), "{err}");
    assert!(!err.to_string().is_empty());
    assert!(a.jobs().is_empty(), "failed job is not listed");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0, "no leftovers");
}

#[cfg(unix)]
#[test]
fn t023_read_only_job_dir_makes_checkpoint_fail_visibly() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path()).unwrap();
    let id = a.start_job(new_job(1)).unwrap();
    a.append(&id, b"abc").unwrap();
    let jd = dir.path().join(&id);
    fs::set_permissions(&jd, fs::Permissions::from_mode(0o555)).unwrap();
    let writable_anyway = fs::File::create(jd.join("probe")).is_ok();
    let r = a.checkpoint(&id);
    fs::set_permissions(&jd, fs::Permissions::from_mode(0o755)).unwrap();
    if !writable_anyway {
        assert!(r.is_err());
    }
    // Existing manifest was not clobbered by a half-written tmp.
    assert!(Archive::open(dir.path()).unwrap().manifest(&id).is_some());
}

#[test]
fn open_fails_cleanly_when_root_is_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("not-a-dir");
    fs::write(&f, b"x").unwrap();
    assert!(Archive::open(&f).is_err());
}
