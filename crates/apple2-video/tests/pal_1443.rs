//! AppleWin GH#1443 (AUD-P-001): on a PAL Color TV the colour burst is not killed instantly by a
//! mid-line switch to TEXT (a TV takes many lines to lose it), unlike a PAL composite monitor.
//! The oracle differential (`conformance run`, cases `video-1443-*`) proves the frames equal to
//! AppleWin's; this test pins the behaviour itself, with the scene the differential uses.

#[path = "../tools/scenes.rs"]
mod scenes;

use apple2_core::{Machine, MachineConfig, Model, VideoStandard};
use apple2_video::{RenderConfig, Renderer, VideoType};

fn frame(scene: &str, vt: VideoType, std: VideoStandard) -> Vec<u32> {
    let def = scenes::all()
        .into_iter()
        .find(|s| s.name == scene)
        .expect("scene");
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    cfg.video_standard = std;
    let mut m = Machine::new(cfg);
    for (a, b) in def.main.iter().chain(&[(scenes::ORG, def.code.clone())]) {
        for (i, v) in b.iter().enumerate() {
            m.poke_main(a + i as u16, *v);
        }
    }
    for (a, b) in &def.aux {
        for (i, v) in b.iter().enumerate() {
            m.poke_aux(a + i as u16, *v);
        }
    }
    m.cpu_mut().pc = scenes::ORG;
    let mut r = Renderer::new(RenderConfig {
        video_type: vt,
        ..Default::default()
    });
    r.resync(m.cycles_in_frame());
    for _ in 0..4 {
        r.run_frame(&mut m);
    }
    r.pixels().to_vec()
}

/// True when some pixel of the frame is clearly coloured (not a grey level).
fn has_colour(px: &[u32]) -> bool {
    px.iter().any(|&p| {
        let (r, g, b) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF);
        r.abs_diff(g) > 40 || g.abs_diff(b) > 40
    })
}

#[test]
fn pal_color_tv_keeps_the_color_burst_after_a_switch_to_text() {
    for scene in ["midline-1443-hgr-then-text", "midline-1443-text-then-hgr"] {
        let tv_ntsc = frame(scene, VideoType::ColorTv, VideoStandard::Ntsc);
        let tv_pal = frame(scene, VideoType::ColorTv, VideoStandard::Pal);
        let mon_ntsc = frame(scene, VideoType::ColorMonitorNtsc, VideoStandard::Ntsc);
        let mon_pal = frame(scene, VideoType::ColorMonitorNtsc, VideoStandard::Pal);
        assert!(
            has_colour(&tv_pal),
            "{scene}: the HGR side of the TV is coloured"
        );
        assert!(
            tv_pal == tv_ntsc,
            "{scene}: PAL Color TV must render like NTSC (colour burst not killed, GH#1443)"
        );
        assert!(
            mon_pal != mon_ntsc,
            "{scene}: a PAL composite monitor kills the burst at the mode switch"
        );
    }
}
