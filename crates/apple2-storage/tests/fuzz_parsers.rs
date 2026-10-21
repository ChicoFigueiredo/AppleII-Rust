//! Fuzzing rápido dos analisadores de imagem (AUD-C-035): WOZ, NIB, DSK/PO/DO, 2MG e HDV.
//! Um gerador xorshift com semente fixa (reprodutível, sem dependência nova) deforma imagens
//! reais e sintéticas; nada pode dar panic nem travar. Um vigia derruba o teste se uma única
//! imagem levar mais de 20 s.

use apple2_storage::image::{Class, DiskImage, HD_BLOCK_SIZE};
use apple2_storage::{new_disk, woz};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::time::Duration;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn asset(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!(
        "{}/../../assets/disks/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .ok()
}

fn corpus(rel: &str) -> Option<Vec<u8>> {
    let mut dirs = Vec::new();
    if let Ok(d) = std::env::var("APPLE2_CORPUS") {
        dirs.push(std::path::PathBuf::from(d));
    }
    // the cache lives in the main checkout's target/, above any worktree
    for up in std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors() {
        dirs.push(up.join("target/conformance-cache/applewin-test"));
    }
    dirs.iter()
        .find_map(|d| std::fs::read(d.join(rel)).ok())
        .filter(|b| b.len() < 2_000_000)
}

fn make_2mg(payload: &[u8], order: u32) -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[0..4].copy_from_slice(b"2IMG");
    h[4..8].copy_from_slice(b"XGS!");
    h[8..10].copy_from_slice(&64u16.to_le_bytes());
    h[10..12].copy_from_slice(&1u16.to_le_bytes());
    h[12..16].copy_from_slice(&order.to_le_bytes());
    h[20..24].copy_from_slice(&((payload.len() / 512) as u32).to_le_bytes());
    h[24..28].copy_from_slice(&64u32.to_le_bytes());
    h[28..32].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    h.extend_from_slice(payload);
    h
}

/// (name used for the extension, bytes, seeds a hard-disk image too)
fn seeds() -> Vec<(&'static str, Vec<u8>)> {
    let mut v: Vec<(&'static str, Vec<u8>)> = Vec::new();
    for (n, f) in [
        ("dos.dsk", "DOS 3.3 System Master - 680-0210-A.dsk"),
        ("pro.po", "ProDOS_2_4_3.po"),
        ("blank.dsk", "BLANK.DSK"),
    ] {
        if let Some(b) = asset(f) {
            v.push((n, b));
        }
    }
    v.push(("empty.woz", woz::create_empty_woz2()));
    for (n, f) in [
        ("g1.woz", "v2/DiskFormat/wozv1-gm.woz"),
        ("g2.woz", "v2/DiskFormat/wozv2-gm.woz"),
        ("z.nib", "v2/DiskFormat/zero.nib"),
        ("s.hdv", "v2/SmartPort-firmware/HDD-small.hdv"),
        ("b.hdv", "v2/HDD-RW-IO-Boundary/HDD-RW-IO-Boundary.hdv"),
    ] {
        if let Some(b) = corpus(f) {
            v.push((n, b));
        }
    }
    v.push(("blank.nib", new_disk::new_blank_disk(232_960, false)));
    v.push(("t.2mg", make_2mg(&new_disk::new_blank_disk(143_360, false), 1)));
    v.push((
        "t.hdv",
        new_disk::new_blank_disk(64 * HD_BLOCK_SIZE, true),
    ));
    v.push((
        "h.2mg",
        make_2mg(&new_disk::new_blank_disk(128 * HD_BLOCK_SIZE, true), 1),
    ));
    v
}

fn mutate(rng: &mut Rng, src: &[u8]) -> Vec<u8> {
    let mut d = src.to_vec();
    match rng.below(6) {
        0 => {
            let n = rng.below(d.len().max(1) + 1);
            d.truncate(n);
        }
        1 => {
            let extra = rng.below(70_000);
            for _ in 0..extra {
                d.push(rng.next() as u8);
            }
        }
        2 => {
            // header-focused flips
            for _ in 0..1 + rng.below(8) {
                let at = rng.below(d.len().min(256));
                if at < d.len() {
                    d[at] = rng.next() as u8;
                }
            }
        }
        3 => {
            // a run of one extreme value
            let at = rng.below(d.len().max(1));
            let n = rng.below(64);
            let b = [0x00, 0xFF, 0x7F, 0x80][rng.below(4)];
            for x in d.iter_mut().skip(at).take(n) {
                *x = b;
            }
        }
        _ => {
            for _ in 0..1 + rng.below(32) {
                if !d.is_empty() {
                    let at = rng.below(d.len());
                    d[at] ^= 1 << rng.below(8);
                }
            }
        }
    }
    d
}

fn exercise(name: &str, bytes: Vec<u8>) {
    for class in [Class::Floppy, Class::HardDisk] {
        if let Ok(mut img) = DiskImage::open_as(class, name, bytes.clone()) {
            let mut buf = vec![0u8; 0x4000];
            for q in (0..160).step_by(3) {
                let _ = img.read_track(q as f32 / 4.0, &mut buf, q % 2 == 0);
            }
            img.write_track(0.0, &[0xD5, 0xAA, 0x96, 0xFF], 4);
            img.write_track(17.25, &[0xFF; 80], 80);
            img.write_track(34.5, &[0x96; 7000], 7000);
            let _ = img.boot_program();
            let mut blk = [0u8; HD_BLOCK_SIZE];
            for b in [0u32, 1, 63, 64, 65_535, 65_536, u32::MAX] {
                let _ = img.read_block(b, &mut blk);
                let _ = img.write_block(b, &[0x5A; HD_BLOCK_SIZE]);
            }
            let _ = (img.num_tracks(), img.image_size(), img.optimal_bit_timing());
        }
    }
}

fn run_fuzz(iterations: usize, seed: u64) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut rng = Rng(seed);
        let seeds = seeds();
        assert!(seeds.len() >= 6, "seeds");
        let mut panics = Vec::new();
        for i in 0..iterations {
            let (name, src) = &seeds[i % seeds.len()];
            let m = mutate(&mut rng, src);
            let copy = m.clone();
            if catch_unwind(AssertUnwindSafe(|| exercise(name, m))).is_err() {
                panics.push((i, name.to_string(), copy.len()));
            }
            tx.send(i).ok();
        }
        tx.send(usize::MAX).ok();
        assert!(panics.is_empty(), "panics: {panics:?}");
    });
    // watchdog: every iteration must complete in 20 s
    loop {
        match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(usize::MAX) => break,
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => panic!("an image took more than 20 s: hang"),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[test]
fn mutated_images_never_panic_or_hang() {
    run_fuzz(400, 0x00C0_FFEE_1234_5678);
}

#[test]
fn mutated_images_never_panic_or_hang_second_seed() {
    run_fuzz(400, 0x9E37_79B9_7F4A_7C15);
}

#[test]
fn pure_random_bytes_never_panic() {
    let mut rng = Rng(42);
    for ext in ["x.dsk", "x.po", "x.do", "x.nib", "x.woz", "x.2mg", "x.hdv", "x.zip", "x.gz"] {
        for len in [0usize, 1, 3, 63, 64, 65, 255, 4096, 143_360, 232_960] {
            let b: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            assert!(catch_unwind(AssertUnwindSafe(|| exercise(ext, b))).is_ok(), "{ext} {len}");
        }
    }
}
