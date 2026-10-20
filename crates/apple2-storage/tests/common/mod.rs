//! Shared helpers for the integration tests.
#![allow(dead_code)]

use apple2_core::text_screen::text_screen;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;

pub fn disk(name: &str) -> Vec<u8> {
    let dir = std::env::var("APPLE2_DISKS").unwrap_or_else(|_| format!("{}/../../assets/disks", env!("CARGO_MANIFEST_DIR")));
    std::fs::read(format!("{dir}/{name}")).unwrap_or_else(|e| panic!("{dir}/{name}: {e}"))
}

pub const DOS33: &str = "DOS 3.3 System Master - 680-0210-A.dsk";

/// An Enhanced //e with a Disk II in slot 6 holding `image` in drive 1.
pub fn machine_with_disk(image: DiskImage) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut card = Disk2Card::new();
    card.insert_disk(0, image);
    assert!(m.insert_card(6, Box::new(card)));
    m.power_cycle();
    m
}

pub fn screen(m: &Machine) -> String {
    text_screen(m).iter().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

/// Runs frames until `done`, panicking with the screen after `max` frames.
pub fn run_until(m: &mut Machine, max: u32, what: &str, done: impl Fn(&Machine) -> bool) -> u32 {
    for f in 0..=max {
        if done(m) {
            return f;
        }
        m.run_frame();
    }
    panic!("{what}: not reached in {max} frames. PC={:04X}\n{}", m.cpu().pc, screen(m));
}

pub fn screen_has(m: &Machine, text: &str) -> bool {
    screen(m).contains(text)
}

/// Types `text` one key at a time, waiting for the ROM to take each (strobe clear).
pub fn type_text(m: &mut Machine, text: &str) {
    for ch in text.bytes() {
        let mut waited = 0;
        while !m.key_press(if ch == b'\n' { b'\r' } else { ch }) {
            m.run_frame();
            waited += 1;
            assert!(waited < 100);
        }
        for _ in 0..3 {
            m.run_frame();
        }
    }
}

/// Builds a WOZ2 file from a 140K image: each nibble track becomes a bit stream with 10-bit
/// sync bytes (an FF nibble followed by two zero bits), tracks mapped to quarter tracks `4t-1..=4t+1`.
pub fn woz2_from_dsk(dsk: Vec<u8>, boot_sector_format: u8) -> Vec<u8> {
    use apple2_storage::image::DiskImage;
    let mut img = DiskImage::from_bytes("src.dsk", dsk).unwrap();
    let mut tracks: Vec<(Vec<u8>, u32)> = Vec::new();
    for t in 0..35 {
        let mut buf = vec![0u8; 0x1A18];
        let (n, _) = img.read_track((t * 2) as f32, &mut buf, true);
        let mut bits: Vec<bool> = Vec::new();
        for (i, &b) in buf[..n].iter().enumerate() {
            bits.extend((0..8).rev().map(|i| b >> i & 1 != 0));
            // Self-sync bytes (gaps) are FF + two zero bits; an isolated FF (volume 254, a checksum)
            // is a plain nibble.
            if b == 0xFF && matches!(buf.get(i + 1), Some(0xFF | 0xD5)) {
                bits.extend([false, false]);
            }
        }
        let count = bits.len() as u32;
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (i, bit) in bits.iter().enumerate() {
            if *bit {
                bytes[i / 8] |= 0x80 >> (i % 8);
            }
        }
        tracks.push((bytes, count));
    }
    let mut f = apple2_storage::woz::create_empty_woz2();
    f[20 + 38] = boot_sector_format;
    for t in 0..35usize {
        for q in [4 * t, 4 * t + 1] {
            f[88 + q] = t as u8;
        }
        if t > 0 {
            f[88 + 4 * t - 1] = t as u8;
        }
    }
    f.truncate(256 + 1280);
    let mut next_block = 3u16;
    let mut data = Vec::new();
    for (i, (bytes, count)) in tracks.iter().enumerate() {
        let blocks = bytes.len().div_ceil(512) as u16;
        let rec = 256 + i * 8;
        f[rec..rec + 2].copy_from_slice(&next_block.to_le_bytes());
        f[rec + 2..rec + 4].copy_from_slice(&blocks.to_le_bytes());
        f[rec + 4..rec + 8].copy_from_slice(&count.to_le_bytes());
        let mut padded = bytes.clone();
        padded.resize(blocks as usize * 512, 0);
        data.extend(padded);
        next_block += blocks;
    }
    f.extend(&data);
    let size = (f.len() - 256) as u32;
    f[252..256].copy_from_slice(&size.to_le_bytes());
    let largest = tracks.iter().map(|(b, _)| b.len().div_ceil(512)).max().unwrap() as u16;
    f[20 + 44..20 + 46].copy_from_slice(&largest.to_le_bytes());
    apple2_storage::woz::update_crc(&mut f);
    f
}
