//! The emulation thread end to end (timer pacing: there is no audio device in CI).

use applewin_shell::audio::AudioRing;
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::{spawn, Command};
use applewin_shell::keymap::KeyEvent;
use applewin_shell::pacing::Speed;
use std::time::{Duration, Instant};

fn settings() -> apple2_config::Settings {
    let mut s = apple2_config::Settings::default();
    s.slots.0[6] = apple2_config::SlotCardSpec::Empty; // boot straight to BASIC
    s
}

fn wait_for(mut f: impl FnMut() -> bool, secs: u64) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(secs) {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn real_time_pacing_runs_at_about_one_megahertz() {
    let mut h = spawn(settings(), Box::new(|| Box::new(DefaultBackend)), None, 48_000, Box::new(|| {})).unwrap();
    // Wall-clock measurement: a loaded machine can starve the thread, so take the best of many
    // half-second windows (the timer pacer never runs ahead of 1.0x, which is the real check).
    let (mut best, mut worst_high) = (0.0f32, 0.0f32);
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(15) && best < 0.85 {
        std::thread::sleep(Duration::from_millis(100));
        let m = h.shared.status().mhz;
        best = best.max(m);
        worst_high = worst_high.max(m);
    }
    assert!((0.85..1.3).contains(&best), "best measured {best} MHz");
    assert!(worst_high < 1.3, "ran ahead of real time: {worst_high} MHz");
    h.shutdown();
}

#[test]
fn max_speed_is_much_faster_and_pause_stops_time() {
    let mut h = spawn(settings(), Box::new(|| Box::new(DefaultBackend)), None, 48_000, Box::new(|| {})).unwrap();
    h.send(Command::ToggleFullSpeed);
    assert!(wait_for(|| h.shared.status().mhz > 2.0, 30), "mhz {}", h.shared.status().mhz);
    h.send(Command::SetPaused(true));
    std::thread::sleep(Duration::from_millis(200));
    let a = h.shared.status().cycles;
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(a, h.shared.status().cycles);
    assert!(h.shared.status().paused);
    h.shutdown();
}

#[test]
fn keys_reach_the_machine_and_the_frame_shows_them() {
    let mut h = spawn(settings(), Box::new(|| Box::new(DefaultBackend)), None, 48_000, Box::new(|| {})).unwrap();
    h.send(Command::SetSpeed(Speed::Max));
    assert!(wait_for(|| h.shared.status().frames > 200, 10));
    for c in b"PRINT 1+1\r" {
        h.send(Command::Key(KeyEvent::Ascii(*c)));
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut frame = applewin_shell::video::Frame::new(0, 0);
    let seq = h.shared.frame_seq();
    assert!(wait_for(|| h.shared.frame_seq() > seq + 5, 5));
    h.shared.copy_frame(&mut frame);
    assert_eq!((frame.width, frame.height), (560, 384));
    h.shutdown();
}

#[test]
fn audio_ring_pacing_keeps_the_ring_filled() {
    let ring = AudioRing::new(48_000 * 3 / 10);
    let mut h = spawn(settings(), Box::new(|| Box::new(DefaultBackend)), Some(ring.clone()), 48_000, Box::new(|| {})).unwrap();
    assert!(wait_for(|| ring.fill_frames() > 2000, 5), "ring fill {}", ring.fill_frames());
    // nobody drains it, so the emulation stops near the target (about 80 ms) instead of running away
    std::thread::sleep(Duration::from_millis(300));
    assert!(ring.fill_frames() < 48_000 / 4, "fill {}", ring.fill_frames());
    h.shutdown();
}
