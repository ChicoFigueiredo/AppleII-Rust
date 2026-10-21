//! GH#342: while PAUSED, cycling the video mode (F9) must repaint the screen at once.

use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::{Command, Emulator};

fn boot(video_mode: Option<apple2_config::settings::VideoType>) -> Emulator {
    let mut settings = apple2_config::Settings::default();
    settings.slots.0[6] = apple2_config::SlotCardSpec::Empty;
    if let Some(m) = video_mode {
        settings.video.mode = m;
    }
    let mut emu = Emulator::new(settings, Box::new(DefaultBackend), 48_000).unwrap();
    for _ in 0..120 {
        emu.run_frame();
    }
    emu
}

#[test]
fn f9_while_paused_repaints_with_the_new_video_type() {
    let mut paused = boot(None); // Color TV (4)
    let before = paused.render().rgba.clone();
    paused.apply(Command::SetPaused(true));
    paused.apply(Command::VideoMode(1)); // -> B&W TV (5)
    let after = paused.render().rgba.clone();
    assert_ne!(before, after, "the screen did not change while paused");

    // The same machine state started directly in B&W TV: only the NTSC phase of the incremental
    // renderer vs the whole-screen redraw may differ, never the picture as a whole.
    let mut direct = boot(Some(apple2_config::settings::VideoType::MonoTv));
    let want = direct.render().rgba.clone();
    let differing = after
        .chunks(4)
        .zip(want.chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        differing * 50 < after.len() / 4,
        "{differing} pixels differ from a B&W TV frame"
    );
}
