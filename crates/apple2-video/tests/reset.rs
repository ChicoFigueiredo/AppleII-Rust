//! AppleWin GH#17 and GH#382: after a Reset the //e is back in 40-column TEXT, so a program that
//! then enters mixed graphics (GR, MIXED) gets 40-column text lines, not stale 80-column ones.
//!
//! `Video::VideoResetState` (Video.cpp:164) resets the video mode to TEXT and `NTSC_SetVideoTextMode(40)`.
//! The oracle has no Reset hook, so the check is by equivalence: the frame after "dirty state, Reset,
//! enter mixed lo-res" equals the frame of a machine that went straight to mixed lo-res, and that
//! straight path is the one the oracle differential proves (case `video-lores-mixed-*`). Only the
//! picture (inside the 20/18-pixel border) is compared: the borders keep residue of the previous
//! mode, as in AppleWin, whose framebuffer is not cleared by a mode change.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_video::{RenderConfig, Renderer, VideoType};

fn stub(switches: &[u16]) -> Vec<u8> {
    let mut v = Vec::new();
    for s in switches {
        v.extend([0x8D, *s as u8, (*s >> 8) as u8]);
    }
    v.extend([0x4C, (0x0300 + v.len()) as u8, 0x03]); // JMP *
    v
}

fn load(m: &mut Machine, at: u16, bytes: &[u8]) {
    for (i, b) in bytes.iter().enumerate() {
        m.poke_main(at + i as u16, *b);
    }
}

fn frame(dirty_first: bool, vt: VideoType) -> Vec<u32> {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    // Normal (non-flashing) characters in both text pages, lo-res colours elsewhere.
    for a in 0x400u16..0xC00 {
        m.poke_main(a, 0xC1 + (a % 26) as u8);
        m.poke_aux(a, 0xC1 + ((a / 3) % 26) as u8);
    }
    let mut r = Renderer::new(RenderConfig {
        video_type: vt,
        ..Default::default()
    });
    r.resync(m.cycles_in_frame());
    if dirty_first {
        // PR#3 then double lo-res mixed: 80COL, AN3 (DHIRES), GR, MIXED, LORES.
        load(
            &mut m,
            0x0300,
            &stub(&[0xC00D, 0xC05E, 0xC050, 0xC053, 0xC056]),
        );
        m.cpu_mut().pc = 0x0300;
        r.run_frame(&mut m);
        m.reset();
    }
    load(&mut m, 0x0300, &stub(&[0xC050, 0xC053, 0xC056]));
    m.cpu_mut().pc = 0x0300;
    for _ in 0..4 {
        r.run_frame(&mut m);
    }
    r.pixels().to_vec()
}

#[test]
fn reset_restores_40_column_text_for_the_mixed_mode_window() {
    for vt in [
        VideoType::ColorTv,
        VideoType::ColorMonitorNtsc,
        VideoType::ColorIdealized,
    ] {
        let (a, b) = (frame(true, vt), frame(false, vt));
        let diff: Vec<usize> = (0..a.len())
            .filter(|&i| {
                a[i] != b[i] && (20..580).contains(&(i % 600)) && (18..402).contains(&(i / 600))
            })
            .collect();
        assert!(
            diff.is_empty(),
            "{vt:?}: Reset must leave 80COL/DHIRES off (GH#382) and no residue (GH#17); {} pixels differ, first at {:?}",
            diff.len(),
            (diff.iter().map(|i| i % 600).min(), diff.iter().map(|i| i % 600).max(), diff.iter().map(|i| i / 600).min(), diff.iter().map(|i| i / 600).max())
        );
    }
}
