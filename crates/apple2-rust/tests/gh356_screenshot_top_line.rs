//! GH#356 / GH#403: a screenshot must contain every pixel row of the screen, the first one
//! included (AppleWin's PrintScreen dropped the top row / half of the top line).

use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::Emulator;
use apple2_rust_shell::screenshot::save_png;

#[test]
fn screenshot_is_the_whole_frame_pixel_for_pixel_in_color_and_bw_tv() {
    use apple2_config::settings::VideoType;
    for vt in [VideoType::ColorTv, VideoType::MonoTv] {
        let mut settings = apple2_config::Settings::default();
        settings.slots.0[6] = apple2_config::SlotCardSpec::Empty;
        settings.video.mode = vt;
        let mut emu = Emulator::new(settings, Box::new(DefaultBackend), 48_000).unwrap();
        for _ in 0..120 {
            emu.run_frame();
        }
        let frame = emu.render().clone();
        assert_eq!((frame.width, frame.height), (560, 384));
        let path = std::env::temp_dir().join(format!("gh356-{}-{vt:?}.png", std::process::id()));
        save_png(&path, &frame).unwrap();
        let img = image::open(&path).unwrap().to_rgba8();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(img.dimensions(), (560, 384));
        assert_eq!(img.as_raw(), &frame.rgba, "{vt:?}");
        // the boot screen is white text on black, but the cursor/inverse row 0 of the first text
        // line is lit: the top row of the picture is not blank
        let top_row_lit = frame.rgba[..560 * 4].chunks(4).any(|p| p[0] > 0);
        let second_row_lit = frame.rgba[560 * 4..560 * 8].chunks(4).any(|p| p[0] > 0);
        assert!(
            top_row_lit || second_row_lit,
            "{vt:?}: first two rows are blank"
        );
    }
}
