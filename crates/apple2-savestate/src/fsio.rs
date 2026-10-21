//! Atomic file replacement for save-states (AUD-C-022).
//!
//! AppleWin opens the target with `fopen("wt")` and so truncates the last good save before it
//! writes the new one (`Snapshot_SaveState`, SaveState.cpp); a crash or a full disk in the middle
//! leaves a half-written file. This port writes a temporary file in the same directory, flushes it
//! to the disk (`fsync`) and only then renames it over the target. `rename` replaces an existing
//! file atomically on Unix, and on Windows `std::fs::rename` calls `MoveFileExW` with
//! `MOVEFILE_REPLACE_EXISTING`, which is the portable equivalent of `ReplaceFile`.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Where the temporary file of `target` lives: the same directory, so the rename never crosses a
/// file system.
pub fn temp_path_for(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(".{}.tmp", std::process::id()));
    target.with_file_name(name)
}

/// Replaces `target` with `bytes`, keeping the old file intact unless the whole write succeeded.
pub fn write_atomic(target: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with(target, |f| f.write_all(bytes))
}

/// Like [`write_atomic`] but `fill` writes the content, so a test can inject a failure part of the
/// way through (a simulated crash). On any error the temporary file is removed and `target` is
/// untouched.
pub fn write_atomic_with(
    target: &Path,
    fill: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let tmp = temp_path_for(target);
    let result = (|| {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        fill(&mut f)?;
        f.flush()?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, target)?;
        sync_parent_dir(target);
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Makes the rename durable (a no-op where directories cannot be opened, i.e. Windows).
fn sync_parent_dir(target: &Path) {
    #[cfg(unix)]
    {
        let dir = target
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if let Ok(d) = File::open(dir) {
            let _ = d.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = target;
}
