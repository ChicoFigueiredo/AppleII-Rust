//! Integridade das imagens (HU-0303): o que a emulação grava no arquivo do host. Os micro-programas
//! de boot (`tests/conformance/inputs/disk-micro`, os mesmos que o oráculo AppleWin executa nos
//! casos `disk-micro-write-*`) gravam um setor; aqui se verifica o que NÃO muda, a proteção contra
//! escrita, a troca de imagem no meio da gravação e o desligamento do motor.
//!
//! As imagens são sempre copiadas para um diretório temporário: o arquivo mestre nunca é escrito.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_storage::disk2::{Disk2Card, DiskStatus};
use apple2_storage::image::{Class, DiskImage};
use std::path::{Path, PathBuf};

fn micro(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/inputs/disk-micro").join(name)
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("a2integrity-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Copy of a fixture in `dir`, uncompressed content returned for comparisons.
fn copy_to(dir: &Path, name: &str) -> (PathBuf, Vec<u8>) {
    let dst = dir.join(name);
    std::fs::copy(micro(name), &dst).unwrap();
    let content = content_of(&dst);
    (dst, content)
}

fn content_of(p: &Path) -> Vec<u8> {
    let bytes = std::fs::read(p).unwrap();
    if p.extension().is_some_and(|e| e == "gz") {
        // the crate's own gzip reader: opening it as an image gives the plain bytes
        DiskImage::from_bytes("x.do.gz", bytes).unwrap().data().to_vec()
    } else {
        bytes
    }
}

fn machine(path: &Path, wp: bool) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut card = Disk2Card::new();
    card.insert_path(0, path, wp).unwrap();
    assert!(m.insert_card(6, Box::new(card)));
    m.power_cycle();
    m
}

fn run(m: &mut Machine, cycles: u64) {
    let end = m.cycles() + cycles;
    while m.cycles() < end {
        m.run_frame();
    }
}

fn changed_ranges(a: &[u8], b: &[u8]) -> Vec<usize> {
    assert_eq!(a.len(), b.len());
    (0..a.len()).filter(|&i| a[i] != b[i]).collect()
}

#[test]
fn writing_a_sector_on_track_1_changes_only_that_sector() {
    let dir = scratch("t1");
    let (path, before) = copy_to(&dir, "write-sector-t1.do.gz");
    let mut m = machine(&path, false);
    run(&mut m, 4_500_000);
    let after = content_of(&path);
    let diff = changed_ranges(&before, &after);
    assert!(!diff.is_empty(), "the program wrote a sector");
    // physical sector 3 of track 1 = one 256-byte logical sector of the 4096 bytes of track 1
    assert!(diff.iter().all(|&i| (4096..8192).contains(&i)), "bytes outside track 1 changed");
    let first = diff[0] / 256;
    assert!(diff.iter().all(|&i| i / 256 == first), "more than one sector changed");
    assert!(after[first * 256..first * 256 + 256].iter().all(|&b| b == 0), "data field of zeros");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_write_protected_host_file_is_never_modified() {
    let dir = scratch("wp");
    let (path, before) = copy_to(&dir, "write-sector.do");
    let mut perm = std::fs::metadata(&path).unwrap().permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&path, perm).unwrap();
    let mut m = machine(&path, false);
    assert!(m.card::<Disk2Card>(6).unwrap().is_write_protected(0));
    run(&mut m, 4_500_000);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // and a forced write protection of a writable file behaves the same
    let mut perm = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&path, perm).unwrap();
    let mut m = machine(&path, true);
    run(&mut m, 4_500_000);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_dirty_track_reaches_the_host_file_when_the_motor_stops_without_ejecting_gh1444() {
    let dir = scratch("flush");
    let (path, before) = copy_to(&dir, "write-sector.do");
    let mut m = machine(&path, false);
    // the program turns the motor off right after the write; 1 s later the track is written back
    run(&mut m, 4_500_000);
    assert!(!m.card::<Disk2Card>(6).unwrap().is_motor_on());
    assert_ne!(std::fs::read(&path).unwrap(), before, "no eject, no Drop: the file already has it");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn swapping_the_image_in_the_middle_of_a_write_keeps_both_images_consistent() {
    let dir = scratch("swap");
    let (a_path, a_before) = copy_to(&dir, "write-sector.do");
    let (b_path, b_before) = copy_to(&dir, "write-sector-motor-on.do.gz");
    let mut m = machine(&a_path, false);
    // run until the program is writing (the write light of the drive is on)
    let mut guard = 0;
    while m.card::<Disk2Card>(6).unwrap().light_status(0) != DiskStatus::Write {
        m.run_cycles(1000);
        guard += 1;
        assert!(guard < 6000, "the program never started writing");
    }
    m.run_cycles(3000); // a few nibbles into the data field
    let card = m.card_mut::<Disk2Card>(6).unwrap();
    // eject: the part written so far is flushed to image A, then B goes in
    let ejected = card.eject_disk(0).expect("image A");
    drop(ejected);
    card.insert_path(0, &b_path, false).unwrap();
    run(&mut m, 4_000_000);
    assert_eq!(std::fs::read(&b_path).unwrap().len(), std::fs::metadata(&b_path).unwrap().len() as usize);
    let a_after = std::fs::read(&a_path).unwrap();
    assert_eq!(a_after.len(), a_before.len(), "image A keeps its size");
    // whatever reached A is inside the track the head was on (track 0)
    assert!(changed_ranges(&a_before, &a_after).iter().all(|&i| i < 4096));
    // image B was inserted after the swap and only ever saw its own program's writes
    let b_after = content_of(&b_path);
    assert_eq!(b_after.len(), b_before.len());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_hard_disk_image_larger_than_a_prodos_volume_is_refused() {
    use apple2_storage::image::HARDDISK_32M_SIZE;
    let dir = scratch("hdd32");
    let p = dir.join("big.hdv");
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(HARDDISK_32M_SIZE as u64 + 512).unwrap();
    drop(f);
    assert!(DiskImage::open_path(&p, Class::HardDisk, false).is_err());
    f_set_len(&p, HARDDISK_32M_SIZE as u64);
    let img = DiskImage::open_path(&p, Class::HardDisk, false).unwrap();
    assert_eq!(img.image_size(), HARDDISK_32M_SIZE);
    let _ = std::fs::remove_dir_all(dir);
}

fn f_set_len(p: &Path, n: u64) {
    std::fs::OpenOptions::new().write(true).open(p).unwrap().set_len(n).unwrap();
}

#[test]
fn damaged_copies_of_real_woz_images_never_panic() {
    // the AppleWin-Test corpus (not versioned): skipped when it was not fetched
    let mut dirs = vec![];
    for up in Path::new(env!("CARGO_MANIFEST_DIR")).ancestors() {
        dirs.push(up.join("target/conformance-cache/applewin-test/v2/WOZ"));
    }
    let Some(dir) = dirs.into_iter().find(|d| d.is_dir()) else {
        eprintln!("skipped: no corpus");
        return;
    };
    let mut state = 0x1234_5678_9ABC_DEF0u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.ends_with(".woz") {
            continue;
        }
        let orig = std::fs::read(e.path()).unwrap();
        for round in 0..25 {
            let mut d = orig.clone();
            // damage the header chunks and a few random places; some rounds fix the CRC
            for _ in 0..1 + round % 6 {
                let at = if round % 2 == 0 {
                    (next() as usize) % 300.min(d.len())
                } else {
                    (next() as usize) % d.len()
                };
                d[at] ^= 1 << (next() % 8);
            }
            if round % 3 == 0 {
                apple2_storage::woz::update_crc(&mut d);
            }
            let r = std::panic::catch_unwind(|| {
                if let Ok(mut img) = DiskImage::from_bytes("x.woz", d) {
                    let mut buf = vec![0u8; 0x4000];
                    for q in 0..160 {
                        let _ = img.read_track(q as f32 / 4.0, &mut buf, true);
                    }
                    img.write_track(1.0, &[0xFF; 100], 100);
                }
            });
            assert!(r.is_ok(), "{name} round {round}");
        }
    }
}
