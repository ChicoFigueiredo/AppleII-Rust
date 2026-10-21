//! AppleWin GH#241 (Ansi Story), PAL Color Monitor: the first pixel written in frame 1 sits two
//! pixels before the line start (the 50 Hz TEXT -> graphics switch moves the pointer back by 2 on
//! line 0). It is drawn with the signal history `g_nSignalBitsNTSC` had after `NTSC_VideoInit`
//! (`GenerateBaseColors` leaves 0xFFF there), so AppleWin shows a pale blue-white pixel at
//! (18, 18) that the later frames never overwrite; a renderer starting with a clean history
//! drew it black. The oracle differential (case `video-issue-0241-*-pal-monitor`) proves the whole
//! frame equal to AppleWin's; this test pins the pixel.

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

#[test]
fn pal_monitor_first_frame_keeps_the_power_on_signal_history_at_the_left_border() {
    let px = frame(
        "issue-0241-floatbus-dlores-text80",
        VideoType::ColorMonitorNtsc,
        VideoStandard::Pal,
    );
    // Oracle (225, 239, 255); the framebuffer is 600 wide.
    assert_eq!(px[18 * 600 + 18], 0xFFE1_EFFF);
}
