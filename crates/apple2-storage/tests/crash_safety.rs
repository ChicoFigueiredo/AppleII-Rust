//! AUD-C-033: a crash (here, an injected error) in the middle of rewriting an image file leaves
//! the previous image intact, for plain and compressed images, and no temporary file behind.
//! Works on copies: the master fixture is never written.

use apple2_storage::image::{Class, DiskImage};
use apple2_storage::store::{Fault, FileStore};
use std::path::{Path, PathBuf};

fn micro(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/inputs/disk-micro")
        .join(name)
}

#[test]
fn a_failed_rewrite_leaves_the_previous_image_intact() {
    for name in ["write-sector.do.gz", "write-sector.do"] {
        let dir = std::env::temp_dir().join(format!("a2crash-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::copy(micro(name), &path).unwrap();
        let before = std::fs::read(&path).unwrap();

        let mut img = DiskImage::open_path(&path, Class::Floppy, false).unwrap();
        let mut buf = vec![0u8; 0x4000];
        let (n, _) = img.read_track(0.0, &mut buf, true);
        buf[300] ^= 0x01; // a different nibble somewhere in the track
        let mut store = FileStore::open(&path).unwrap();
        store.inject_fault(Some(Fault::AfterBytes(64)));
        img.set_store(Some(Box::new(store)));
        img.write_track(0.0, &buf[..n], n);

        assert!(img.take_io_error().is_some(), "{name}: the failed write is reported");
        assert_eq!(std::fs::read(&path).unwrap(), before, "{name}: the file did not change");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name() != name)
            .collect();
        assert!(leftovers.is_empty(), "{name}: temporary file left behind");

        // and without the injected failure the same write goes through
        let mut img = DiskImage::open_path(&path, Class::Floppy, false).unwrap();
        img.write_track(0.0, &buf[..n], n);
        assert!(img.take_io_error().is_none());
        assert_ne!(std::fs::read(&path).unwrap(), before, "{name}: the write reached the file");
        let _ = std::fs::remove_dir_all(dir);
    }
}
