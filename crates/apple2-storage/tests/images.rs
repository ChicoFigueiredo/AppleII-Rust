//! Disk image formats through the public API: detection, track access, write-back and wrappers.

use apple2_storage::image::{Class, DiskImage, ImageError, ImageKind};
use apple2_storage::nibble::NIBBLES_PER_TRACK;
use apple2_storage::store::MemStore;
use std::io::Write;

fn disk(name: &str) -> Vec<u8> {
    let dir = std::env::var("APPLE2_DISKS").unwrap_or_else(|_| format!("{}/../../assets/disks", env!("CARGO_MANIFEST_DIR")));
    std::fs::read(format!("{dir}/{name}")).unwrap_or_else(|e| panic!("{dir}/{name}: {e}"))
}

const DOS33: &str = "DOS 3.3 System Master - 680-0210-A.dsk";

fn track_nibbles(img: &mut DiskImage, phase: f32) -> Vec<u8> {
    let mut buf = vec![0u8; NIBBLES_PER_TRACK];
    let (n, _) = img.read_track(phase, &mut buf, true);
    buf.truncate(n);
    buf
}

#[test]
fn dos33_master_is_dos_order_and_prodos_disk_is_prodos_order() {
    let d = DiskImage::from_bytes("master.dsk", disk(DOS33)).unwrap();
    assert_eq!((d.kind(), d.num_tracks(), d.is_write_protected()), (ImageKind::Do, 35, false));
    let p = DiskImage::from_bytes("ProDOS_2_4_3.po", disk("ProDOS_2_4_3.po")).unwrap();
    assert_eq!(p.kind(), ImageKind::Po);
    // The same ProDOS file named .dsk is still found to be ProDOS order by its block links.
    let p = DiskImage::from_bytes("x.dsk", disk("ProDOS_2_4_3.po")).unwrap();
    assert_eq!(p.kind(), ImageKind::Po);
}

#[test]
fn blank_dsk_is_a_possible_dos_match() {
    let b = DiskImage::from_bytes("BLANK.DSK", disk("BLANK.DSK")).unwrap();
    assert_eq!(b.kind(), ImageKind::Do);
}

#[test]
fn track_zero_starts_with_the_gap_and_first_address_field() {
    let mut d = DiskImage::from_bytes("m.dsk", disk(DOS33)).unwrap();
    let t = track_nibbles(&mut d, 0.0);
    assert_eq!(t.len(), 6384);
    assert!(t[..48].iter().all(|&b| b == 0xFF));
    assert_eq!(&t[48..54], &[0xD5, 0xAA, 0x96, 0xFF, 0xFE, 0xAA]);
    // Half tracks round down: phase 1 (track 0.5) is still track 0, phase 2 is track 1.
    assert_eq!(track_nibbles(&mut d, 1.0), t);
    assert_ne!(track_nibbles(&mut d, 2.0), t);
}

#[test]
fn nib_images_and_wrong_sizes() {
    let mut d = DiskImage::from_bytes("m.dsk", disk(DOS33)).unwrap();
    let mut nib = Vec::new();
    for t in 0..35 {
        let mut b = track_nibbles(&mut d, (t * 2) as f32);
        b.resize(0x1A00, 0xFF);
        nib.extend(b);
    }
    let n = DiskImage::from_bytes("m.nib", nib).unwrap();
    assert_eq!((n.kind(), n.num_tracks()), (ImageKind::Nib1, 35));
    assert!(matches!(DiskImage::from_bytes("m.dsk", vec![1; 1000]), Err(ImageError::Unsupported)));
    assert!(matches!(DiskImage::from_bytes("big.hdv", vec![0; 900 * 1024]), Err(ImageError::UnsupportedHdv)));
}

#[test]
fn track_writes_reach_the_store_and_survive_reopening() {
    let store = MemStore::new(disk(DOS33));
    let mut d = DiskImage::from_bytes("m.dsk", disk(DOS33)).unwrap().with_store(Box::new(store.clone()));
    let mut t = track_nibbles(&mut d, 6.0); // track 3
    // Corrupt-free change: rewrite the track unchanged, then change a data nibble pattern via a sector.
    d.write_track(6.0, &t, t.len());
    assert_eq!(store.bytes(), disk(DOS33), "an unchanged track leaves the file identical");
    // Patch the first data field's first nibble: sector data changes, so the image bytes change.
    let pos = t.windows(3).position(|w| w == [0xD5, 0xAA, 0xAD]).unwrap() + 3;
    t[pos] = if t[pos] == 0x96 { 0x97 } else { 0x96 };
    d.write_track(6.0, &t, t.len());
    assert_ne!(store.bytes(), disk(DOS33));
    assert_eq!(store.bytes().len(), 143360);
    let again = DiskImage::from_bytes("m.dsk", store.bytes()).unwrap();
    assert_eq!(again.data(), d.data());
}

#[test]
fn write_protected_images_do_not_write() {
    let store = MemStore::new(disk(DOS33));
    let mut d = DiskImage::from_bytes("m.dsk", disk(DOS33)).unwrap().with_store(Box::new(store.clone()));
    d.set_write_protected(true);
    let mut t = track_nibbles(&mut d, 0.0);
    let pos = t.windows(3).position(|w| w == [0xD5, 0xAA, 0xAD]).unwrap() + 3;
    t[pos] ^= 0x01;
    d.write_track(0.0, &t, t.len());
    assert_eq!(store.bytes(), disk(DOS33));
}

fn two_img(format: u32, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[0..4].copy_from_slice(b"2IMG");
    h[4..8].copy_from_slice(b"TEST");
    h[8..10].copy_from_slice(&64u16.to_le_bytes());
    h[10..12].copy_from_slice(&1u16.to_le_bytes());
    h[12..16].copy_from_slice(&format.to_le_bytes());
    h[16..20].copy_from_slice(&flags.to_le_bytes());
    h[20..24].copy_from_slice(&((body.len() / 512) as u32).to_le_bytes());
    h[24..28].copy_from_slice(&64u32.to_le_bytes());
    h[28..32].copy_from_slice(&(body.len() as u32).to_le_bytes());
    [h, body.to_vec()].concat()
}

#[test]
fn two_img_wrapper_gives_volume_number_and_lock() {
    let f = two_img(0, 0x8000_0000 | 0x100 | 7, &disk(DOS33));
    let mut d = DiskImage::from_bytes("m.2mg", f).unwrap();
    assert_eq!(d.kind(), ImageKind::Do);
    assert!(d.is_write_protected());
    assert_eq!(d.image_size(), 143360);
    // Volume 7 is in the address field: 4-and-4 of 7 = AB AF.
    let t = track_nibbles(&mut d, 0.0);
    assert_eq!(&t[51..53], &[0xAB, 0xAF]);
    let po = DiskImage::from_bytes("p.2mg", two_img(1, 0, &disk("ProDOS_2_4_3.po"))).unwrap();
    assert_eq!(po.kind(), ImageKind::Po);
}

#[test]
fn gz_and_zip_wrappers_round_trip_with_write_back() {
    let raw = disk(DOS33);
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&raw).unwrap();
    let gz = gz.finish().unwrap();
    let store = MemStore::new(gz.clone());
    let mut d = DiskImage::from_bytes("m.dsk.gz", gz).unwrap().with_store(Box::new(store.clone()));
    assert_eq!(d.kind(), ImageKind::Do);
    let mut t = track_nibbles(&mut d, 0.0);
    let pos = t.windows(3).position(|w| w == [0xD5, 0xAA, 0xAD]).unwrap() + 3;
    t[pos] = if t[pos] == 0x96 { 0x97 } else { 0x96 };
    d.write_track(0.0, &t, t.len());
    let again = DiskImage::from_bytes("m.dsk.gz", store.bytes()).unwrap();
    assert_eq!(again.data(), d.data());
    assert_ne!(again.data(), &raw[..]);

    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    z.start_file("inside.dsk", zip::write::SimpleFileOptions::default()).unwrap();
    z.write_all(&raw).unwrap();
    let zbytes = z.finish().unwrap().into_inner();
    let zstore = MemStore::new(zbytes.clone());
    let mut d = DiskImage::from_bytes("m.zip", zbytes).unwrap().with_store(Box::new(zstore.clone()));
    assert_eq!((d.kind(), d.name_in_zip(), d.num_valid_images_in_zip(), d.is_write_protected()), (ImageKind::Do, Some("inside.dsk"), 1, false));
    d.write_track(0.0, &t, t.len());
    assert_eq!(DiskImage::from_bytes("m.zip", zstore.bytes()).unwrap().data(), d.data());
}

#[test]
fn multi_image_zip_is_read_only_and_first_wins() {
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for n in ["a.dsk", "b.po"] {
        z.start_file(n, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(&disk(if n == "a.dsk" { DOS33 } else { "ProDOS_2_4_3.po" })).unwrap();
    }
    let d = DiskImage::from_bytes("two.zip", z.finish().unwrap().into_inner()).unwrap();
    assert_eq!((d.kind(), d.num_valid_images_in_zip(), d.is_write_protected()), (ImageKind::Do, 2, true));
}

#[test]
fn new_blank_images_by_extension() {
    let d = DiskImage::from_bytes("new.dsk", Vec::new()).unwrap();
    assert_eq!((d.kind(), d.data().len()), (ImageKind::Do, 143360));
    let n = DiskImage::from_bytes("new.nib", Vec::new()).unwrap();
    assert_eq!((n.kind(), n.data().len(), n.data()[0], n.data()[1]), (ImageKind::Nib1, 35 * 0x1A00, 0x80, 0x81));
    let w = DiskImage::from_bytes("new.woz", Vec::new()).unwrap();
    assert_eq!((w.kind(), w.is_woz()), (ImageKind::Woz2, true));
    assert!(DiskImage::from_bytes("new.po", Vec::new()).is_err());
}

#[test]
fn woz2_tracks_are_created_on_write_and_read_back() {
    let store = MemStore::new(Vec::new());
    let mut w = DiskImage::from_bytes("new.woz", Vec::new()).unwrap().with_store(Box::new(store.clone()));
    let mut buf = vec![0u8; NIBBLES_PER_TRACK];
    let (n, bits) = w.read_track(0.0, &mut buf, true);
    assert_eq!((n, bits), (6400, 51200), "an unwritten track reads as the 6400-byte empty track");
    let track: Vec<u8> = (0..6000).map(|i| (i * 13) as u8).collect();
    buf[..6000].copy_from_slice(&track);
    w.write_track(4.0, &buf, 6000); // track 2
    let (n, bits) = w.read_track(4.0, &mut buf, true);
    assert_eq!((n, bits), (6000, 48000));
    assert_eq!(&buf[..6000], &track[..]);
    // The neighbouring quarter tracks see the same track; the file stays a valid WOZ.
    assert_eq!(w.read_track(3.5, &mut buf, true).0, 6000);
    let again = DiskImage::from_bytes("new.woz", store.bytes()).unwrap();
    assert_eq!(again.data(), w.data());
    assert_eq!(w.data().len() % 512, 0);
}

#[test]
fn hard_disk_images_need_the_hdv_extension_or_800k() {
    assert!(DiskImage::hard_disk_from_bytes("p.po", disk("ProDOS_2_4_3.po")).is_err());
    let h = DiskImage::hard_disk_from_bytes("p.hdv", disk("ProDOS_2_4_3.po")).unwrap();
    assert_eq!((h.kind(), h.image_size()), (ImageKind::Hdv, 143360));
    let mut blk = [0u8; 512];
    assert!(h.read_block(0, &mut blk) && blk[0] != 0);
    assert!(!h.read_block(280, &mut blk));
    let big = DiskImage::hard_disk_from_bytes("big.po", vec![0; 900 * 1024]).unwrap();
    assert_eq!(big.kind(), ImageKind::Hdv);
    let _ = Class::Floppy;
}
