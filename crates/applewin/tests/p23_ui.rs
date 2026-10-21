//! P23 (gate G0): issues upstream de UI verificadas sem janela. Oráculo: AppleWin a8f0d1d
//! (`source/Windows/WinFrame.cpp`, `source/FrameBase.cpp`, `source/Utilities.cpp`).
//! Cada teste cita o arquivo:linha do C++ que o motivou; o resultado por issue fica em
//! `docs/conformance/results/p23-ui.csv`.

use apple2_config::cmdline::CmdLine;
use apple2_config::Settings;
use applewin_shell::actions::{action_for, Action};
use applewin_shell::app::{cursor_idle_hides, fit_scale, reboot_needs_confirmation, window_size};
use applewin_shell::emu::{speed_from_settings, EmuStatus};
use applewin_shell::help;
use applewin_shell::keymap::{HostKey, Mods};
use applewin_shell::pacing::Speed;
use applewin_shell::screenshot::{image_title, ScreenshotNamer};
use applewin_shell::ui::UiState;
use applewin_shell::video::Frame;
use std::time::Duration;

const NONE: Mods = Mods {
    shift: false,
    ctrl: false,
    alt: false,
};
const CTRL: Mods = Mods {
    shift: false,
    ctrl: true,
    alt: false,
};
const ALT: Mods = Mods {
    shift: false,
    ctrl: false,
    alt: true,
};

// ---- screenshots: FrameBase.cpp:122-127, Disk.cpp:438,888,895 ----

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("p23-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// GH#1009 / GH#379: the name follows the LAST floppy inserted (either drive), with a 9-digit
/// counter; with no floppy (a hard disk only) it is `AppleWin_ScreenShot_`; the counter restarts on
/// every insert or eject. Same as the oracle, which is why disk 2 can "win".
#[test]
fn p23_screenshot_name_follows_the_last_floppy_like_applewin() {
    let dir = temp_dir("shots");
    let mut n = ScreenshotNamer::default();
    assert_eq!(
        n.next_path(&dir).file_name().unwrap(),
        "AppleWin_ScreenShot_000000000.png"
    );
    n.reset(Some("GAME.DSK")); // drive 1
    n.reset(Some("utility.po")); // drive 2 inserted later: its name wins (GH#1009)
    let p = n.next_path(&dir);
    assert_eq!(p.file_name().unwrap(), "utility_000000000.png");
    std::fs::write(&p, b"x").unwrap();
    assert_eq!(
        n.next_path(&dir).file_name().unwrap(),
        "utility_000000001.png"
    );
    n.reset(None); // eject
    assert_eq!(
        n.next_path(&dir).file_name().unwrap(),
        "AppleWin_ScreenShot_000000000.png"
    );
    // GetImageTitle (DiskImage.cpp:274-310)
    assert_eq!(image_title("/a/b/GAME.DSK"), "Game");
    assert_eq!(image_title("Mixed Case.dsk"), "Mixed Case");
    assert_eq!(image_title("A.DSK"), "A");
    let _ = std::fs::remove_dir_all(&dir);
}

/// GH#1512 point 1: the screenshot is a PNG (FrameBase.cpp:127 `%s_%09d.png`).
#[test]
fn p23_screenshot_is_a_png() {
    let dir = temp_dir("png");
    let mut f = Frame::new(4, 2);
    f.rgba.iter_mut().for_each(|b| *b = 200);
    let p = ScreenshotNamer::default().next_path(&dir);
    applewin_shell::screenshot::save_png(&p, &f).unwrap();
    let bytes = std::fs::read(&p).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// GH#189: `-no-printscreen-dlg` (CmdLine.cpp:528) turns the warning dialog off.
#[test]
fn p23_no_printscreen_dialog_command_line_option() {
    let mut s = Settings::default();
    assert!(s.video.show_printscreen_warning_dialog);
    CmdLine::parse(&["-no-printscreen-dlg"])
        .unwrap()
        .apply(&mut s);
    assert!(!s.video.show_printscreen_warning_dialog);
}

// ---- keys: WinFrame.cpp:1274, 1976-1993, 1854 ----

/// GH#221 / GH#1188 / GH#198: F6 and Alt+Enter toggle full screen (so there is always a way back),
/// Ctrl+F6 toggles 1x/2x.
#[test]
fn p23_fullscreen_keys() {
    assert_eq!(
        action_for(HostKey::F(6), NONE),
        Some(Action::ToggleFullscreen)
    );
    assert_eq!(
        action_for(HostKey::Enter, ALT),
        Some(Action::ToggleFullscreen)
    );
    assert_eq!(action_for(HostKey::F(6), CTRL), Some(Action::ToggleScale));
}

/// GH#597: Ctrl+F10 has no special meaning (the oracle's F10 handler never looks at Ctrl); it is
/// the same rocker-switch key and does not touch the mouse.
#[test]
fn p23_ctrl_f10_has_no_mouse_release_function() {
    assert_eq!(action_for(HostKey::F(10), CTRL), Some(Action::RockerSwitch));
    assert_eq!(action_for(HostKey::F(10), NONE), Some(Action::RockerSwitch));
}

/// GH#199: ConfirmReboot(bFromButtonUI) (WinFrame.cpp:1997): only the toolbar button asks, and
/// only while "Confirm reboot" is on.
#[test]
fn p23_reboot_confirmation_follows_the_option() {
    assert!(reboot_needs_confirmation(Action::Reboot, true, true));
    assert!(!reboot_needs_confirmation(Action::Reboot, true, false));
    assert!(!reboot_needs_confirmation(Action::Reboot, false, true)); // internal restart, not F2
    assert!(!reboot_needs_confirmation(Action::SoftReset, true, true));
}

// ---- window and full screen ----

/// GH#132 / GH#161 / GH#932 / GH#1190: 2x is the default, 1x/2x differ by the screen size, and a
/// scale that does not fit the screen of the window falls back (WinFrame.cpp:3058).
#[test]
fn p23_window_scale_and_small_screens() {
    assert_eq!(Settings::default().video.window_scale, 2);
    assert_eq!(window_size(2, false).width, 1120.0);
    assert_eq!(window_size(1, false).height, 384.0);
    let with_bar = window_size(2, true);
    assert!(with_bar.width > 1120.0 && with_bar.height > 768.0);
    assert_eq!(fit_scale(2, (1366.0, 768.0), true), 1); // small screen of GH#932
    assert_eq!(fit_scale(2, (1024.0, 600.0), true), 1); // netbook of GH#161
    assert_eq!(fit_scale(2, (1920.0, 1080.0), true), 2); // the secondary display of GH#932
    assert_eq!(fit_scale(2, (100.0, 100.0), true), 1); // never below 1x
}

/// GH#504: the pointer hides after 2 s idle in full screen, only without a mouse card or a mouse
/// joystick (WinFrame.cpp:1627-1640), and shows again at the next movement.
#[test]
fn p23_idle_fullscreen_pointer_hides() {
    let idle = |ms| Duration::from_millis(ms);
    assert!(!cursor_idle_hides(idle(1999), true));
    assert!(cursor_idle_hides(idle(2001), true));
    assert!(!cursor_idle_hides(idle(60_000), false));
    assert!(!cursor_idle_hides(idle(0), true)); // moved: visible again
}

fn layout(fullscreen: bool, integer: bool, w: f32, h: f32) -> egui::Rect {
    let mut s = Settings::default();
    s.video.window_scale = 2;
    let mut ui = UiState::new(&s);
    ui.integer_scale = integer;
    let ctx = egui::Context::default();
    let mut frame = Frame::new(560, 384);
    frame.rgba.iter_mut().for_each(|b| *b = 255);
    let status = EmuStatus::default();
    let mut rect = None;
    for _ in 0..3 {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w, h),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            ui.upload(ctx, &frame);
            let _ = ui.draw(ctx, &status, fullscreen);
        });
        rect = ui.screen_rect;
    }
    rect.expect("the Apple screen was drawn")
}

/// GH#280 / GH#358 / GH#1374: in full screen the Apple screen fills the display in height, keeps
/// 560:384 and is centred (no small window inside the screen). The oracle's integer scale
/// (`min(w/560, h/384)`, WinFrame.cpp:2784-2795) is the "Integer scaling" option.
#[test]
fn p23_fullscreen_fills_the_display() {
    let r = layout(true, false, 1920.0, 1080.0);
    assert!((r.height() - 1080.0).abs() < 1.0, "{r:?}");
    assert!((r.width() / r.height() - 560.0 / 384.0).abs() < 0.01);
    assert!((r.center().x - 960.0).abs() < 1.0);
    let int = layout(true, true, 1920.0, 1080.0);
    let oracle = (1920.0f32 / 560.0).floor().min((1080.0f32 / 384.0).floor());
    assert_eq!(int.width(), 560.0 * oracle);
    assert_eq!(int.height(), 384.0 * oracle);
    // windowed: the toolbar column takes room on the side, the screen is not centred on it
    let win = layout(false, false, 1229.0, 802.0);
    assert!(win.width() < 1229.0);
}

/// GH#224 / GH#168: the full-screen drive status and Caps Lock marks follow the option
/// (`overlay::tests::fullscreen_subunit_status_option_hides_leds_and_caps`); here its default and
/// its wiring through the settings.
#[test]
fn p23_fullscreen_subunit_status_option_default_is_on() {
    let s = Settings::default();
    assert!(s.video.fullscreen_show_subunit_status); // Win32Frame.h:85
    let mut s = Settings::default();
    s.video.fullscreen_show_subunit_status = false;
    let ov = applewin_shell::overlay::Overlay::new(
        &EmuStatus {
            model: "Apple2eEnhanced".into(),
            ..Default::default()
        },
        false,
        Default::default(),
    )
    .with_subunit_status(s.video.fullscreen_show_subunit_status);
    assert!(ov.drives.is_empty() && ov.caps.is_none());
}

// ---- speed ----

/// GH#1448: "Fastest" is full speed in every model (the oracle bug was model dependent).
#[test]
fn p23_fastest_speed_on_every_model() {
    use apple2_core::Model;
    for m in [
        Model::Apple2,
        Model::Apple2Plus,
        Model::Apple2JPlus,
        Model::Apple2e,
        Model::Apple2eEnhanced,
    ] {
        let mut s = Settings::default();
        s.machine.model = m;
        s.machine.speed = 40;
        assert_eq!(speed_from_settings(&s), Speed::Max, "{m:?}");
    }
}

// ---- help (the in-app manual replaces the .chm) ----

fn manual() -> Vec<help::Doc> {
    help::MANUAL.iter().map(|md| help::parse(md)).collect()
}

/// GH#314 / GH#437: F1 opens the in-app help and no page of the manual is empty.
#[test]
fn p23_help_opens_and_has_content() {
    assert_eq!(action_for(HostKey::F(1), NONE), Some(Action::Help));
    for d in manual() {
        assert!(!d.title.is_empty() && d.blocks.len() >= 2, "{}", d.title);
    }
}

/// GH#258 / GH#535 / GH#471 / GH#122: the manual documents ejecting from the drive menu, the Caps
/// Lock start-up behaviour, window/full-screen terms, and the licence with the AppleWin address.
#[test]
fn p23_manual_covers_the_help_requests() {
    let docs = manual();
    let hit = |q: &str| !help::search(&docs, q, 5).is_empty();
    assert!(hit("ejetar"), "eject from the drive menu");
    assert!(hit("Caps Lock ligado"), "Caps Lock starts on");
    assert!(hit("janela"));
    assert!(hit("tela cheia"));
    assert!(hit("Ctrl+F6"));
    assert!(hit("GNU General Public License"));
    assert!(hit("github.com/AppleWin/AppleWin"));
}
