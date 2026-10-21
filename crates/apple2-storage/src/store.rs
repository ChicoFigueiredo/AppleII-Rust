//! Where an image lives. The emulation works on in-memory copies; a [`Store`] receives the
//! changes (AppleWin writes through a file handle in `CImageBase::WriteImageData`,
//! DiskImageHelper.cpp:207). No store means a purely in-memory image.

use std::io;
use std::sync::{Arc, Mutex};

pub trait Store {
    /// Overwrites `data.len()` bytes at `offset`, growing the file if needed.
    fn write_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()>;

    /// Replaces the whole content (used by gzip/zip wrapped images, which are rewritten as a
    /// whole each time).
    fn replace_all(&mut self, data: &[u8]) -> io::Result<()>;
}

/// Files up to this size are rewritten atomically on every change (temporary file, `fsync`,
/// `rename`); larger ones (hard disks, up to 32 MB) are updated in place, one block at a time,
/// and flushed with `sync_data`, because rewriting 32 MB per 512-byte block is not reasonable.
pub const ATOMIC_REWRITE_LIMIT: u64 = 2 * 1024 * 1024;

/// Where an injected failure strikes inside [`write_atomic`] (tests only).
#[derive(Clone, Copy, Debug)]
pub enum Fault {
    /// The write to the temporary file fails after this many bytes.
    AfterBytes(usize),
}

/// Replaces the content of `path` atomically: the new content goes to a temporary file in the
/// same directory, is flushed to the device and then renamed over the target. A crash or an
/// error at any point leaves the previous file intact (AUD-C-033).
pub fn write_atomic(path: &std::path::Path, data: &[u8]) -> io::Result<()> {
    write_atomic_with(path, data, None)
}

pub(crate) fn write_atomic_with(
    path: &std::path::Path,
    data: &[u8],
    fault: Option<Fault>,
) -> io::Result<()> {
    use io::Write;
    // Follow symbolic links: the link itself must survive, the file it names is replaced.
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = target
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp_name = target.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(".{}.tmp", std::process::id()));
    let tmp = dir.join(tmp_name);
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        match fault {
            Some(Fault::AfterBytes(n)) => {
                f.write_all(&data[..n.min(data.len())])?;
                return Err(io::Error::other("injected write failure"));
            }
            None => f.write_all(data)?,
        }
        if let Ok(meta) = std::fs::metadata(&target) {
            let _ = f.set_permissions(meta.permissions());
        }
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, &target)?;
        // Make the rename itself durable (not possible on Windows, harmless to try elsewhere).
        #[cfg(unix)]
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// A file on the host disk.
pub struct FileStore {
    path: std::path::PathBuf,
    fault: Option<Fault>,
}

impl FileStore {
    /// Checks that `path` can be written; fails if it is read-only.
    pub fn open(path: &std::path::Path) -> io::Result<Self> {
        std::fs::OpenOptions::new().read(true).write(true).open(path)?;
        Ok(FileStore {
            path: path.to_path_buf(),
            fault: None,
        })
    }

    /// Makes the next atomic rewrite fail half-way (crash simulation for tests).
    pub fn inject_fault(&mut self, fault: Option<Fault>) {
        self.fault = fault;
    }
}

impl Store for FileStore {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()> {
        use io::{Seek, SeekFrom, Write};
        let len = std::fs::metadata(&self.path)?.len();
        let end = offset + data.len() as u64;
        if len.max(end) <= ATOMIC_REWRITE_LIMIT {
            let mut all = std::fs::read(&self.path)?;
            if all.len() < end as usize {
                all.resize(end as usize, 0);
            }
            all[offset as usize..end as usize].copy_from_slice(data);
            return write_atomic_with(&self.path, &all, self.fault);
        }
        let mut file = std::fs::OpenOptions::new().write(true).open(&self.path)?;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)?;
        file.sync_data()
    }

    fn replace_all(&mut self, data: &[u8]) -> io::Result<()> {
        write_atomic_with(&self.path, data, self.fault)
    }
}

/// A shared in-memory file: the frontend (or a test) keeps a clone to inspect or save what the
/// emulation wrote.
#[derive(Clone, Default)]
pub struct MemStore(pub Arc<Mutex<Vec<u8>>>);

impl MemStore {
    pub fn new(data: Vec<u8>) -> Self {
        MemStore(Arc::new(Mutex::new(data)))
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.0.lock().unwrap().clone()
    }
}

impl Store for MemStore {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()> {
        let mut v = self.0.lock().unwrap();
        let end = offset as usize + data.len();
        if v.len() < end {
            v.resize(end, 0);
        }
        v[offset as usize..end].copy_from_slice(data);
        Ok(())
    }

    fn replace_all(&mut self, data: &[u8]) -> io::Result<()> {
        *self.0.lock().unwrap() = data.to_vec();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("a2store-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn crash_in_the_middle_of_a_rewrite_keeps_the_previous_image() {
        let dir = tmpdir("crash");
        let p = dir.join("a.dsk");
        std::fs::write(&p, vec![0xAA; 4096]).unwrap();
        let mut s = FileStore::open(&p).unwrap();
        s.inject_fault(Some(Fault::AfterBytes(100)));
        assert!(s.replace_all(&[0x55; 4096]).is_err());
        assert!(s.write_at(10, &[1, 2, 3]).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), vec![0xAA; 4096]);
        // no temporary file is left behind
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        s.inject_fault(None);
        s.write_at(10, &[1, 2, 3]).unwrap();
        let got = std::fs::read(&p).unwrap();
        assert_eq!(&got[10..13], &[1, 2, 3]);
        assert_eq!(got[9], 0xAA);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn large_files_are_updated_in_place() {
        let dir = tmpdir("big");
        let p = dir.join("h.hdv");
        std::fs::write(&p, vec![0u8; ATOMIC_REWRITE_LIMIT as usize + 1024]).unwrap();
        let mut s = FileStore::open(&p).unwrap();
        s.write_at(512, &[9; 512]).unwrap();
        let got = std::fs::read(&p).unwrap();
        assert_eq!(got.len(), ATOMIC_REWRITE_LIMIT as usize + 1024);
        assert_eq!(got[512], 9);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
