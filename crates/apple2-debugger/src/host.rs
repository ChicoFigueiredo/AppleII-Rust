//! Host file access for the debugger commands that touch files (`BLOAD`, `BSAVE`, `RUN`, `SAVE`,
//! `LOAD`, `TF`, `SYM ... LOAD`). The default [`FsHost`] uses the file system relative to a
//! directory (`PWD` / `CD`); tests and sandboxes can plug another implementation.

use std::io::{self, Write};
use std::path::PathBuf;

pub trait Host {
    fn read(&mut self, name: &str) -> io::Result<Vec<u8>>;
    fn write(&mut self, name: &str, data: &[u8]) -> io::Result<()>;
    /// Opens `name` for writing (trace files).
    fn create(&mut self, name: &str) -> io::Result<Box<dyn Write>>;
    /// Current debugger directory (`PWD`).
    fn dir(&self) -> String;
    /// `CD`; false if the directory does not exist.
    fn set_dir(&mut self, dir: &str) -> bool;
}

/// `std::fs` relative to `dir`.
pub struct FsHost {
    dir: PathBuf,
}

impl FsHost {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        FsHost { dir: dir.into() }
    }

    fn path(&self, name: &str) -> PathBuf {
        let p = PathBuf::from(name);
        if p.is_absolute() {
            p
        } else {
            self.dir.join(p)
        }
    }
}

impl Host for FsHost {
    fn read(&mut self, name: &str) -> io::Result<Vec<u8>> {
        std::fs::read(self.path(name))
    }

    fn write(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        std::fs::write(self.path(name), data)
    }

    fn create(&mut self, name: &str) -> io::Result<Box<dyn Write>> {
        Ok(Box::new(io::BufWriter::new(std::fs::File::create(self.path(name))?)))
    }

    fn dir(&self) -> String {
        self.dir.display().to_string()
    }

    fn set_dir(&mut self, dir: &str) -> bool {
        let p = self.path(dir);
        if p.is_dir() {
            self.dir = p;
            true
        } else {
            false
        }
    }
}

/// Refuses everything (the default when the frontend gives no host).
#[derive(Default)]
pub struct NoHost;

impl Host for NoHost {
    fn read(&mut self, _: &str) -> io::Result<Vec<u8>> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "no host file access"))
    }

    fn write(&mut self, _: &str, _: &[u8]) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "no host file access"))
    }

    fn create(&mut self, _: &str) -> io::Result<Box<dyn Write>> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "no host file access"))
    }

    fn dir(&self) -> String {
        String::new()
    }

    fn set_dir(&mut self, _: &str) -> bool {
        false
    }
}

/// In-memory files, for tests.
#[derive(Default)]
pub struct MemHost {
    pub files: std::collections::HashMap<String, Vec<u8>>,
}

struct SharedWriter(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Host for MemHost {
    fn read(&mut self, name: &str) -> io::Result<Vec<u8>> {
        self.files.get(name).cloned().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, name.to_string()))
    }

    fn write(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        self.files.insert(name.to_string(), data.to_vec());
        Ok(())
    }

    fn create(&mut self, name: &str) -> io::Result<Box<dyn Write>> {
        // Trace output lands in `files[name]` when the writer is flushed on drop is not needed:
        // the cell is shared through a leak-free Rc held by the returned writer only, so tests
        // that need the content should use `Debugger::set_trace_writer` with their own sink.
        self.files.insert(name.to_string(), Vec::new());
        Ok(Box::new(SharedWriter(Default::default())))
    }

    fn dir(&self) -> String {
        "/".into()
    }

    fn set_dir(&mut self, _: &str) -> bool {
        true
    }
}
