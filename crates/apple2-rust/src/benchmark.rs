//! "Benchmark Emulator" (AppleWin `Win32Frame::Benchmark`, Windows/Win32Frame.cpp:246-425).
//!
//! Like AppleWin it measures, one phase after the other (one second each in the UI):
//! pure video FPS in text and hires with half of the video bytes changing every frame, the pure
//! CPU speed with and without video update, and a "realistic" FPS with CPU, video and Disk II
//! running together. It runs on a machine of its own built from the current settings (AppleWin
//! stops the emulated machine and reuses it, which is why it forbids other work meanwhile), so
//! the running session is untouched.

use crate::backend::Backend;
use crate::video::{Frame, FrameSource};
use apple2_config::Settings;
use apple2_core::Machine;
use std::time::{Duration, Instant};

/// `benchopcode` (CPU.cpp:112): the opcodes the benchmark program is made of.
const BENCH_OPCODES: [u8; 33] = [
    0x06, 0x16, 0x24, 0x45, 0x48, 0x65, 0x68, 0x76, 0x84, 0x85, 0x86, 0x91, 0x94, 0xA4, 0xA5, 0xA6,
    0xB1, 0xB4, 0xC0, 0xC4, 0xC5, 0xE6, 0x19, 0x6D, 0x8D, 0x99, 0x9D, 0xAD, 0xB9, 0xBD, 0xDD, 0xED,
    0xEE,
];
const SHORT_OPCODES: usize = 22;

#[derive(Clone, Debug, PartialEq)]
pub struct BenchResult {
    pub cpu_brand: String,
    pub hires_fps: u32,
    pub text_fps: u32,
    /// CPU speed in MHz with the video updated frame by frame.
    pub mhz_video: f64,
    /// CPU speed in MHz, nothing else running.
    pub mhz_full: f64,
    pub realistic_fps: u32,
    /// The program counter left the benchmark code (AppleWin offers to hunt the error down).
    pub cpu_problem: bool,
    pub is_apple2: bool,
}

impl BenchResult {
    /// The text AppleWin shows in its message box (Win32Frame.cpp:403-421).
    pub fn report(&self) -> String {
        let tag = if self.is_apple2 { " (6502)" } else { "" };
        let tenths = |mhz: f64| {
            let t = (mhz * 10.0) as u32;
            format!("{}.{}", t / 10, t % 10)
        };
        let mut s = format!(
            "AppleII-Rust {}\n\nCPU: {}\n\nPure Video FPS:\t{} hires, {} text\nPure CPU MHz:\t{}{tag} (video update)\nPure CPU MHz:\t{}{tag} (full-speed)\n\nEXPECTED AVERAGE VIDEO GAME\nPERFORMANCE: {} FPS",
            env!("CARGO_PKG_VERSION"),
            self.cpu_brand,
            self.hires_fps,
            self.text_fps,
            tenths(self.mhz_video),
            tenths(self.mhz_full),
            self.realistic_fps
        );
        if self.cpu_problem {
            s.push_str(
                "\n\nThe emulator has detected a problem while running the CPU benchmark \
                 (the program counter left the benchmark code).",
            );
        }
        s
    }
}

/// `CpuSetupBenchmark` (CPU.cpp:822-860): registers and the code at `$0300`.
pub fn setup_cpu_benchmark(m: &mut Machine) {
    {
        let c = m.cpu_mut();
        c.a = 0;
        c.x = 0;
        c.y = 0;
        c.pc = 0x300;
        c.sp = 0xFF; // 0x1FF
    }
    let mut addr: u16 = 0x300;
    let mut op = 0usize;
    loop {
        m.poke_main(addr, BENCH_OPCODES[op]);
        m.poke_main(addr + 1, BENCH_OPCODES[op]);
        addr += 2;
        if op >= SHORT_OPCODES {
            m.poke_main(addr, 0);
            addr += 1;
        }
        op += 1;
        if op >= BENCH_OPCODES.len() || (addr & 0x0F) >= 0x0B {
            m.poke_main(addr, 0x4C);
            addr += 1;
            let value = if op >= BENCH_OPCODES.len() {
                0x00
            } else {
                (((addr >> 4) + 1) << 4) as u8
            };
            m.poke_main(addr, value);
            m.poke_main(addr + 1, 0x03);
            addr += 2;
            while addr & 0x0F != 0 {
                addr += 1;
            }
        }
        if op >= BENCH_OPCODES.len() {
            break;
        }
    }
}

fn pc_in_range(m: &Machine) -> bool {
    (0x300..=0x400).contains(&m.cpu().pc)
}

/// Runs `body` in a loop for `dur` (at least once); returns the iterations.
fn timed(dur: Duration, mut body: impl FnMut(u32)) -> u32 {
    let start = Instant::now();
    let mut n = 0u32;
    loop {
        body(n);
        n += 1;
        if start.elapsed() >= dur {
            return n;
        }
    }
}

/// Fills `$4000-$7FFF` with the two video patterns of AppleWin (half of the bytes differ).
fn prepare_buffers(m: &mut Machine) {
    for loop_ in 4096u32..6144 {
        let v: u32 = if ((loop_ & 1) ^ ((loop_ & 0x40) >> 6)) != 0 {
            0x1414_1414
        } else {
            0xAAAA_AAAA
        };
        for k in 0..4 {
            m.poke_main((loop_ * 4 + k) as u16, (v >> (8 * k)) as u8);
        }
    }
    for loop_ in 6144u32..8192 {
        let v: u32 = if ((loop_ & 1) ^ ((loop_ & 0x40) >> 6)) != 0 {
            0xAAAA_AAAA
        } else {
            0x1414_1414
        };
        for k in 0..4 {
            m.poke_main((loop_ * 4 + k) as u16, (v >> (8 * k)) as u8);
        }
    }
}

/// Changes half of the page at `base` per frame, like the benchmark does with
/// `memset` / `memcpy` from the two buffers.
fn churn(m: &mut Machine, base: u16, size: u16, cycle: u32) {
    if cycle & 1 != 0 {
        for i in 0..size {
            m.poke_main(base + i, 0x14);
        }
    } else {
        let src = if cycle & 2 != 0 { 0x4000u16 } else { 0x6000 };
        for i in 0..size {
            let b = m.peek_main(src + i);
            m.poke_main(base + i, b);
        }
    }
}

/// Touches a soft switch the way a program would (`LDA $C0xx`), so every flag follows.
fn soft_switch(m: &mut Machine, addr: u16) {
    for (i, b) in [0xAD, addr as u8, (addr >> 8) as u8]
        .into_iter()
        .enumerate()
    {
        m.poke_main(0x2F0 + i as u16, b);
    }
    m.cpu_mut().pc = 0x2F0;
    m.step();
}

/// Text or hires, no mixed mode, page 1.
fn set_mode(m: &mut Machine, text: bool) {
    soft_switch(m, if text { 0xC051 } else { 0xC050 });
    soft_switch(m, if text { 0xC056 } else { 0xC057 });
    soft_switch(m, 0xC052);
    soft_switch(m, 0xC054);
}

/// Runs the whole benchmark with `phase` per measurement (AppleWin: one second).
pub fn run(
    settings: &Settings,
    backend: &mut dyn Backend,
    phase: Duration,
) -> Result<BenchResult, String> {
    let bus = apple2_audio::AudioBus::new(48_000, settings.to_machine_config().video_standard);
    let mut m = backend.build_machine(settings, &bus)?;
    let mut video: Box<dyn FrameSource> = backend.frame_source(settings);
    let mut frame = Frame::new(0, 0);
    let is_apple2 = settings.machine.model.is_apple2();

    prepare_buffers(&mut m);

    // text frames per second
    set_mode(&mut m, true);
    for i in 0..0x400u16 {
        m.poke_main(0x400 + i, 0x14);
    }
    video.redraw(&m);
    let text_fps = timed(phase, |c| {
        churn(&mut m, 0x400, 0x400, c & 3);
        video.render(&m, &mut frame);
    });

    // hires frames per second
    set_mode(&mut m, false);
    for i in 0..0x2000u16 {
        m.poke_main(0x2000 + i, 0x14);
    }
    video.redraw(&m);
    let hires_fps = timed(phase, |c| {
        churn(&mut m, 0x2000, 0x2000, c & 3);
        video.render(&m, &mut frame);
    });

    // CPU speed with the video updated, then without
    let mut mhz = [0.0f64; 2];
    let mut cpu_problem = false;
    for (i, slot) in mhz.iter_mut().enumerate() {
        setup_cpu_benchmark(&mut m);
        let c0 = m.cycles();
        let t0 = Instant::now();
        loop {
            if i == 0 {
                video.run_frame(&mut m);
            } else {
                m.run_cycles(100_000);
            }
            if t0.elapsed() >= phase {
                break;
            }
        }
        let secs = t0.elapsed().as_secs_f64().max(1e-9);
        *slot = (m.cycles() - c0) as f64 / secs / 1e6;
        cpu_problem |= !pc_in_range(&m);
    }

    // realistic: CPU + video together (the first frames run the CPU for real, like AppleWin's
    // first ten iterations)
    set_mode(&mut m, false);
    for i in 0..0x2000u16 {
        m.poke_main(0x2000 + i, 0xAA);
    }
    setup_cpu_benchmark(&mut m);
    video.redraw(&m);
    let realistic_fps = timed(phase, |c| {
        if c < 10 {
            m.run_cycles(100_000);
        }
        churn(&mut m, 0x2000, 0x2000, c & 3);
        video.render(&m, &mut frame);
    });

    Ok(BenchResult {
        cpu_brand: format!("{:?}", m.cpu().variant),
        hires_fps: hires_fps.max(1),
        text_fps: text_fps.max(1),
        mhz_video: mhz[0],
        mhz_full: mhz[1],
        realistic_fps: realistic_fps.max(1),
        cpu_problem,
        is_apple2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::DefaultBackend;

    #[test]
    fn benchmark_program_stays_in_its_page_and_runs() {
        let mut s = Settings::default();
        s.slots.0[6] = apple2_config::SlotCardSpec::Empty;
        let bus = apple2_audio::AudioBus::new(48_000, s.to_machine_config().video_standard);
        let mut m = DefaultBackend.build_machine(&s, &bus).unwrap();
        setup_cpu_benchmark(&mut m);
        // the code ends with JMP $0300 and the whole loop is below $0400
        let c0 = m.cycles();
        m.run_cycles(200_000);
        assert!(m.cycles() - c0 >= 200_000);
        assert!(pc_in_range(&m), "pc = {:04X}", m.cpu().pc);
    }

    #[test]
    fn short_run_reports_all_figures_in_applewin_format() {
        let mut s = Settings::default();
        s.slots.0[6] = apple2_config::SlotCardSpec::Empty;
        let r = run(&s, &mut DefaultBackend, Duration::from_millis(30)).unwrap();
        assert!(r.hires_fps > 0 && r.text_fps > 0 && r.realistic_fps > 0);
        assert!(r.mhz_full > 0.5, "{r:?}");
        assert!(!r.cpu_problem);
        let t = r.report();
        assert!(t.contains("Pure Video FPS:\t"), "{t}");
        assert!(t.contains("hires,") && t.contains("text"));
        assert!(t.contains("(video update)") && t.contains("(full-speed)"));
        assert!(t.contains("EXPECTED AVERAGE VIDEO GAME\nPERFORMANCE: "));
    }
}
