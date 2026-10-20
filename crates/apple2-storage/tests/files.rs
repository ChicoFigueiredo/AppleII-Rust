//! Images on the host file system (the `FileStore` adapter and the write-protect rules).

mod common;
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::{Class, DiskImage, ImageError, ImageKind};
use apple2_storage::nibble::NIBBLES_PER_TRACK;
use common::*;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("apple2-storage-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dirty_first_data_nibble(img: &mut DiskImage) {
    let mut buf = vec![0u8; NIBBLES_PER_TRACK];
    let (n, _) = img.read_track(0.0, &mut buf, true);
    let pos = buf[..n].windows(3).position(|w| w == [0xD5, 0xAA, 0xAD]).unwrap() + 3;
    buf[pos] = if buf[pos] == 0x96 { 0x97 } else { 0x96 };
    img.write_track(0.0, &buf, n);
}

#[test]
fn writes_reach_the_file() {
    let dir = TempDir::new("write");
    let path = dir.0.join("work.dsk");
    std::fs::write(&path, disk(DOS33)).unwrap();
    let mut img = DiskImage::open_path(&path, Class::Floppy, false).unwrap();
    assert!(!img.is_write_protected());
    dirty_first_data_nibble(&mut img);
    let on_disk = std::fs::read(&path).unwrap();
    assert_ne!(on_disk, disk(DOS33));
    assert_eq!(on_disk.len(), 143360);
    assert_eq!(on_disk, img.data());
    assert!(img.take_io_error().is_none());
}

#[test]
fn read_only_files_and_forced_protection_never_write() {
    let dir = TempDir::new("ro");
    let path = dir.0.join("ro.dsk");
    std::fs::write(&path, disk(DOS33)).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms).unwrap();
    let mut img = DiskImage::open_path(&path, Class::Floppy, false).unwrap();
    assert!(img.is_write_protected());
    dirty_first_data_nibble(&mut img);
    assert_eq!(std::fs::read(&path).unwrap(), disk(DOS33));

    let rw = dir.0.join("rw.dsk");
    std::fs::write(&rw, disk(DOS33)).unwrap();
    let mut img = DiskImage::open_path(&rw, Class::Floppy, true).unwrap();
    assert!(img.is_write_protected());
    dirty_first_data_nibble(&mut img);
    assert_eq!(std::fs::read(&rw).unwrap(), disk(DOS33));
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&path, perms).unwrap();
}

#[test]
fn zero_length_files_become_blank_images_of_the_extension_type() {
    let dir = TempDir::new("blank");
    for (name, kind, len) in [("a.dsk", ImageKind::Do, 143360), ("a.nib", ImageKind::Nib1, 35 * 0x1A00), ("a.woz", ImageKind::Woz2, 1536)] {
        let path = dir.0.join(name);
        std::fs::write(&path, []).unwrap();
        let img = DiskImage::open_path(&path, Class::Floppy, false).unwrap();
        assert_eq!(img.kind(), kind);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), len as u64, "{name} is resized to its full size");
    }
    let path = dir.0.join("a.po");
    std::fs::write(&path, []).unwrap();
    assert!(matches!(DiskImage::open_path(&path, Class::Floppy, false), Err(ImageError::Unsupported)));
    let ro = dir.0.join("z.dsk");
    std::fs::write(&ro, []).unwrap();
    assert!(matches!(DiskImage::open_path(&ro, Class::Floppy, true), Err(ImageError::ZeroLengthWriteProtected)));
    assert!(matches!(DiskImage::open_path(&dir.0.join("missing.dsk"), Class::Floppy, false), Err(ImageError::Io(_))));
}

#[test]
fn a_card_inserts_files_by_path_and_gives_the_image_back_on_eject() {
    let dir = TempDir::new("card");
    let path = dir.0.join("Game.dsk");
    std::fs::write(&path, disk(DOS33)).unwrap();
    let mut m = apple2_core::Machine::new(apple2_core::MachineConfig::default());
    assert!(m.insert_card(6, Box::new(Disk2Card::new())));
    let c = m.card_mut::<Disk2Card>(6).unwrap();
    c.insert_path(0, &path, false).unwrap();
    assert_eq!((c.base_name(0), c.full_name(0)), ("Game", "Game.dsk"));
    assert!(c.insert_path(1, &dir.0.join("nope.dsk"), false).is_err());
    let image = c.eject_disk(0).unwrap();
    assert_eq!(image.pathname(), path.to_string_lossy());
}
