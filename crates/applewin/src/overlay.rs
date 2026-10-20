//! On-screen status like AppleWin's frame: the "PAUSED" state, the Caps Lock indicator (a small
//! "A" / "a"), the drive LEDs with their track in the full-screen layout, a full-speed badge and
//! the mouse-capture hint. [`Overlay::new`] is a pure model; [`paint`] draws it with egui.
//!
//! AppleWin has no "LOADING" message and no on-screen text inside the emulated video, so none is
//! drawn here; transient messages (screenshot saved, state saved...) go through the status bar.

use crate::backend::DriveStatus;
use crate::emu::EmuStatus;
use crate::toolbar::{led, Led};
use egui::{Align2, Color32, Context, FontId, Rect};

/// Models without a Caps Lock (Apple ][, ][+, J-Plus: always upper case).
pub fn model_has_caps_lock(model: &str) -> bool {
    !matches!(model, "Apple2" | "Apple2Plus" | "Apple2JPlus")
}

/// AppleWin's Caps Lock indicator: 'A' when the lock is on, 'a' when lower case is allowed.
pub fn caps_indicator(s: &EmuStatus) -> Option<char> {
    model_has_caps_lock(&s.model).then_some(if s.caps_lock { 'A' } else { 'a' })
}

/// What changed the mouse hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MouseState {
    /// The mouse drives a joystick or an AppleMouse card, so clicking the screen captures it.
    pub usable: bool,
    pub captured: bool,
    pub over_screen: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriveLine {
    pub led: Led,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overlay {
    /// Large centred text: "PAUSED".
    pub banner: Option<&'static str>,
    /// Corner badge: "MAX SPEED" or the speed multiplier when it is not 1.0x.
    pub speed_badge: Option<String>,
    pub caps: Option<char>,
    /// Drive LEDs with the track (only when the toolbar column is not on screen).
    pub drives: Vec<DriveLine>,
    pub mouse_hint: Option<&'static str>,
}

/// "T17" like AppleWin's track read-out (decimal; the sector needs the DOS/ProDOS probe).
pub fn track_text(d: &DriveStatus) -> String {
    format!("T{:2}", d.track)
}

impl Overlay {
    /// `toolbar_visible`: the AppleWin button column (with its LEDs and status bar) is on screen.
    pub fn new(s: &EmuStatus, toolbar_visible: bool, mouse: MouseState) -> Overlay {
        let banner = if s.paused {
            Some("PAUSED")
        } else if s.debugger_active {
            Some("DEBUGGER")
        } else {
            None
        };
        let speed_badge = if s.full_speed {
            Some("MAX SPEED".to_string())
        } else if !s.speed_label.is_empty() && s.speed_label != "1.0x" {
            Some(s.speed_label.clone())
        } else {
            None
        };
        let drives = if toolbar_visible {
            Vec::new()
        } else {
            s.drives
                .iter()
                .enumerate()
                .filter(|(_, d)| d.present)
                .map(|(i, d)| DriveLine { led: led(d), text: format!("{} {}", i + 1, track_text(d)) })
                .collect()
        };
        let mouse_hint = match (mouse.usable, mouse.captured, mouse.over_screen) {
            (true, true, _) => Some("Ctrl+click to release the mouse"),
            (true, false, true) => Some("Click to capture the mouse"),
            _ => None,
        };
        Overlay { banner, speed_badge, caps: caps_indicator(s), drives, mouse_hint }
    }
}

fn led_color(l: Led) -> Color32 {
    match l {
        Led::Off => Color32::from_gray(90),
        Led::Green => Color32::from_rgb(40, 220, 60),
        Led::Red => Color32::from_rgb(230, 40, 40),
    }
}

/// Draws `o` over the Apple screen `rect`.
pub fn paint(ctx: &Context, rect: Rect, o: &Overlay) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("apple-overlay")));
    let small = FontId::monospace(13.0);
    let shade = Color32::from_black_alpha(160);
    let text = |anchor: Align2, pos: egui::Pos2, s: &str, color: Color32, font: FontId| {
        let galley = painter.layout_no_wrap(s.to_string(), font.clone(), color);
        let r = anchor.anchor_size(pos, galley.size()).expand(3.0);
        painter.rect_filled(r, 3.0, shade);
        painter.text(pos, anchor, s, font, color);
    };
    if let Some(b) = o.banner {
        text(Align2::CENTER_CENTER, rect.center(), b, Color32::WHITE, FontId::proportional(28.0));
    }
    if let Some(s) = &o.speed_badge {
        text(Align2::RIGHT_TOP, rect.right_top() + egui::vec2(-8.0, 8.0), s, Color32::from_rgb(255, 210, 80), small.clone());
    }
    let mut y = 8.0;
    for d in &o.drives {
        text(Align2::LEFT_TOP, rect.left_top() + egui::vec2(8.0, y), &d.text, led_color(d.led), small.clone());
        y += 20.0;
    }
    if let Some(c) = o.caps {
        if !o.drives.is_empty() {
            text(Align2::LEFT_TOP, rect.left_top() + egui::vec2(8.0, y), &c.to_string(), Color32::from_gray(200), small.clone());
        }
    }
    if let Some(h) = o.mouse_hint {
        text(Align2::CENTER_BOTTOM, rect.center_bottom() + egui::vec2(0.0, -8.0), h, Color32::WHITE, small);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::DriveActivity;

    fn status() -> EmuStatus {
        EmuStatus { model: "Apple2eEnhanced".into(), speed_label: "1.0x".into(), caps_lock: true, ..Default::default() }
    }

    #[test]
    fn nothing_to_show_while_running_normally() {
        let mut s = status();
        s.caps_lock = false;
        let o = Overlay::new(&s, true, MouseState::default());
        assert_eq!(o.banner, None);
        assert_eq!(o.speed_badge, None);
        assert_eq!(o.mouse_hint, None);
        assert!(o.drives.is_empty());
    }

    #[test]
    fn paused_and_debugger_banners() {
        let mut s = status();
        s.paused = true;
        assert_eq!(Overlay::new(&s, true, MouseState::default()).banner, Some("PAUSED"));
        s.paused = false;
        s.debugger_active = true;
        assert_eq!(Overlay::new(&s, true, MouseState::default()).banner, Some("DEBUGGER"));
    }

    #[test]
    fn full_speed_and_speed_multiplier_badge() {
        let mut s = status();
        s.full_speed = true;
        s.speed_label = "max".into();
        assert_eq!(Overlay::new(&s, true, MouseState::default()).speed_badge.as_deref(), Some("MAX SPEED"));
        s.full_speed = false;
        s.speed_label = "2.5x".into();
        assert_eq!(Overlay::new(&s, true, MouseState::default()).speed_badge.as_deref(), Some("2.5x"));
    }

    #[test]
    fn caps_indicator_is_a_or_lowercase_a_and_absent_on_the_ii() {
        let mut s = status();
        assert_eq!(caps_indicator(&s), Some('A'));
        s.caps_lock = false;
        assert_eq!(caps_indicator(&s), Some('a'));
        for m in ["Apple2", "Apple2Plus", "Apple2JPlus"] {
            s.model = m.into();
            assert_eq!(caps_indicator(&s), None, "{m}");
        }
        s.model = "Pravets82".into();
        assert_eq!(caps_indicator(&s), Some('a'));
    }

    #[test]
    fn drive_leds_with_track_appear_without_the_toolbar() {
        let mut s = status();
        s.drives[0] = DriveStatus { present: true, image: Some("a.dsk".into()), track: 17, activity: DriveActivity::Read, write_protected: false };
        s.drives[1] = DriveStatus { present: true, image: None, track: 3, activity: DriveActivity::Write, write_protected: false };
        assert!(Overlay::new(&s, true, MouseState::default()).drives.is_empty());
        let o = Overlay::new(&s, false, MouseState::default());
        assert_eq!(o.drives, vec![DriveLine { led: Led::Green, text: "1 T17".into() }, DriveLine { led: Led::Red, text: "2 T 3".into() }]);
        s.drives[1].present = false;
        assert_eq!(Overlay::new(&s, false, MouseState::default()).drives.len(), 1);
    }

    #[test]
    fn mouse_hint_follows_capture_state() {
        let s = status();
        let m = |usable, captured, over_screen| Overlay::new(&s, true, MouseState { usable, captured, over_screen }).mouse_hint;
        assert_eq!(m(false, false, true), None, "nothing uses the mouse");
        assert_eq!(m(true, false, false), None, "not over the screen");
        assert_eq!(m(true, false, true), Some("Click to capture the mouse"));
        assert_eq!(m(true, true, false), Some("Ctrl+click to release the mouse"));
    }
}
