//! Regression guard: rendering must not allocate per frame (the renderer is fed after every
//! opcode, so any allocation in `update` would cost millions of calls a second). Counting
//! allocations is deterministic, unlike a wall-clock threshold on a loaded machine.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_video::{RenderConfig, Renderer, VideoType};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

#[path = "../tools/scenes.rs"]
mod scenes;

fn load(m: &mut Machine, scene: &str) {
    let def = scenes::all()
        .into_iter()
        .find(|s| s.name == scene)
        .expect("scene");
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
}

/// One test fn: the counter is process-wide, so concurrent tests would pollute each other.
#[test]
fn rendering_frames_does_not_allocate() {
    // Every video type, on the boot screen and on busy graphics scenes (mid-line mode changes
    // included, which exercise the most branches of the scanner).
    for scene in [
        None,
        Some("hires-noise"),
        Some("dhires-noise"),
        Some("midline-hgr-text-24"),
    ] {
        for vt in VideoType::ALL {
            let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
            if let Some(s) = scene {
                load(&mut m, s);
            }
            let mut r = Renderer::new(RenderConfig {
                video_type: vt,
                ..Default::default()
            });
            // Warm-up: the first frame configures the scanner; the machine's own growable buffers
            // (speaker toggles of the boot beep) reach their capacity.
            for _ in 0..40 {
                r.run_frame(&mut m);
                m.drain_speaker_toggles().for_each(drop);
            }
            let before = ALLOCS.load(Ordering::Relaxed);
            for _ in 0..6 {
                r.run_frame(&mut m);
                m.drain_speaker_toggles().for_each(drop);
            }
            let n = ALLOCS.load(Ordering::Relaxed) - before;
            assert_eq!(
                n, 0,
                "{scene:?} {vt:?}: {n} allocations over 6 rendered frames"
            );
        }
    }
}
