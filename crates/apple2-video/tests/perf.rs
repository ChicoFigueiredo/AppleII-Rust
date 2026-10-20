//! Smoke check that per-opcode rendering is fast enough for real time (60 frames/s needs
//! about 1 ms of video work per frame on top of emulation).

use apple2_core::{Machine, MachineConfig, Model};
use apple2_video::{RenderConfig, Renderer, VideoType};

/// On-CPU nanoseconds of this thread (Linux), so timings are meaningful on a loaded machine.
fn thread_cpu_ns() -> Option<f64> {
    std::fs::read_to_string("/proc/thread-self/schedstat").ok()?.split_whitespace().next()?.parse().ok()
}

#[test]
#[ignore = "timing; run with --release -- --ignored --nocapture"]
fn frames_per_second() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut r = Renderer::new(RenderConfig { video_type: VideoType::ColorTv, ..Default::default() });
    let cpu0 = thread_cpu_ns();
    let t = std::time::Instant::now();
    let mut cycles = 0u64;
    for _ in 0..600 {
        cycles += r.run_frame(&mut m);
    }
    let dt = t.elapsed();
    println!("600 frames in {dt:?} = {:.0} fps", 600.0 / dt.as_secs_f64());
    if let (Some(a), Some(b)) = (cpu0, thread_cpu_ns()) {
        println!("Machine::step + Renderer::update: {:.1} MHz on-CPU", cycles as f64 / (b - a) * 1e3);
    }
    assert!(dt.as_secs_f64() < 10.0);
}
