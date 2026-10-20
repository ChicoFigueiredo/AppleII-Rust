//! Smoke check that per-opcode rendering is fast enough for real time (60 frames/s needs
//! about 1 ms of video work per frame on top of emulation).

use apple2_core::{Machine, MachineConfig, Model};
use apple2_video::{RenderConfig, Renderer, VideoType};

#[test]
#[ignore = "timing; run with --release -- --ignored --nocapture"]
fn frames_per_second() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut r = Renderer::new(RenderConfig { video_type: VideoType::ColorTv, ..Default::default() });
    let t = std::time::Instant::now();
    for _ in 0..600 {
        r.run_frame(&mut m);
    }
    let dt = t.elapsed();
    println!("600 frames in {dt:?} = {:.0} fps", 600.0 / dt.as_secs_f64());
    assert!(dt.as_secs_f64() < 10.0);
}
