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
