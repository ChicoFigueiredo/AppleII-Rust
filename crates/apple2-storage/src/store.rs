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

/// A file on the host disk.
pub struct FileStore {
    file: std::fs::File,
}

impl FileStore {
    /// Opens `path` read-write; fails if it is read-only.
    pub fn open(path: &std::path::Path) -> io::Result<Self> {
        Ok(FileStore {
            file: std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)?,
        })
    }
}

impl Store for FileStore {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()> {
        use io::{Seek, SeekFrom, Write};
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)
    }

    fn replace_all(&mut self, data: &[u8]) -> io::Result<()> {
        use io::{Seek, SeekFrom, Write};
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(data)
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
