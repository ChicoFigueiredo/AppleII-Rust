//! Toolbar model (AppleWin's button column: Help, Run, Drive 1, Drive 2, Swap, Full screen,
//! Debug, Settings) and the status text. egui drawing lives in [`crate::ui`]; this part is pure.

use crate::actions::Action;
use crate::backend::{DriveActivity, DriveStatus};
use crate::emu::EmuStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Help,
    Run,
    Drive1,
    Drive2,
    Swap,
    Fullscreen,
    Debug,
    Settings,
}

impl Button {
    /// In AppleWin's order: F1..F8.
    pub const ALL: [Button; 8] =
        [Button::Help, Button::Run, Button::Drive1, Button::Drive2, Button::Swap, Button::Fullscreen, Button::Debug, Button::Settings];

    pub fn fkey(self) -> u8 {
        Button::ALL.iter().position(|b| *b == self).unwrap_or(0) as u8 + 1
    }

    pub fn label(self) -> &'static str {
        match self {
            Button::Help => "Help",
            Button::Run => "Run",
            Button::Drive1 => "Drive 1",
            Button::Drive2 => "Drive 2",
            Button::Swap => "Swap",
            Button::Fullscreen => "Full",
            Button::Debug => "Debug",
            Button::Settings => "Config",
        }
    }

    /// The action a click triggers (the same as the F-key).
    pub fn action(self) -> Action {
        match self {
            Button::Help => Action::Help,
            Button::Run => Action::Reboot,
            Button::Drive1 => Action::DriveSelect(0),
            Button::Drive2 => Action::DriveSelect(1),
            Button::Swap => Action::SwapDrives,
            Button::Fullscreen => Action::ToggleFullscreen,
            Button::Debug => Action::ToggleDebugger,
            Button::Settings => Action::Settings,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Led {
    Off,
    /// Green: motor on, reading.
    Green,
    /// Red: writing.
    Red,
}

pub fn led(d: &DriveStatus) -> Led {
    match d.activity {
        DriveActivity::Off => Led::Off,
        DriveActivity::Read => Led::Green,
        DriveActivity::Write => Led::Red,
    }
}

/// "Drive 1: T$11 DOS33.dsk".
pub fn drive_text(n: usize, d: &DriveStatus) -> String {
    if !d.present {
        return format!("Drive {}: -", n + 1);
    }
    let image = d.image.as_deref().unwrap_or("(empty)");
    format!("Drive {}: T${:02X} {}{}", n + 1, d.track, image, if d.write_protected { " [WP]" } else { "" })
}

/// Window title, like AppleWin's `GetAppModeTitle` text: model, mode, speed.
pub fn title(s: &EmuStatus) -> String {
    let mut t = format!("AppleII-Rust - {}", s.model);
    if s.paused {
        t.push_str(" - PAUSED");
    } else if s.debugger_active {
        t.push_str(" - DEBUGGER");
    } else if s.full_speed {
        t.push_str(" - MAX SPEED");
    } else if s.speed_label != "1.0x" && !s.speed_label.is_empty() {
        t.push_str(&format!(" - {}", s.speed_label));
    }
    t
}

/// One-line status bar text.
pub fn status_line(s: &EmuStatus) -> String {
    let caps = if s.caps_lock { "CAPS  " } else { "" };
    let audio = if s.underruns > 0 { format!("  audio underruns: {}", s.underruns) } else { String::new() };
    format!("{caps}{:.2} MHz  {}{audio}", s.mhz, s.video)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_follow_function_keys() {
        assert_eq!(Button::Help.fkey(), 1);
        assert_eq!(Button::Drive2.fkey(), 4);
        assert_eq!(Button::Settings.fkey(), 8);
        // Every F1-F8 binding in `actions` matches the toolbar button of the same number.
        use crate::keymap::{HostKey, Mods};
        for b in Button::ALL {
            let key = HostKey::F(b.fkey());
            assert_eq!(crate::actions::action_for(key, Mods::default()), Some(b.action()), "{b:?}");
        }
    }

    #[test]
    fn led_and_drive_text() {
        let d = DriveStatus { present: true, image: Some("DOS33.dsk".into()), track: 17, activity: DriveActivity::Write, write_protected: true };
        assert_eq!(led(&d), Led::Red);
        assert_eq!(drive_text(0, &d), "Drive 1: T$11 DOS33.dsk [WP]");
        assert_eq!(drive_text(1, &DriveStatus::default()), "Drive 2: -");
    }

    #[test]
    fn title_reflects_mode() {
        let mut s = EmuStatus { model: "Apple2eEnhanced".into(), speed_label: "1.0x".into(), ..Default::default() };
        assert_eq!(title(&s), "AppleII-Rust - Apple2eEnhanced");
        s.paused = true;
        assert!(title(&s).ends_with("PAUSED"));
        s.paused = false;
        s.full_speed = true;
        assert!(title(&s).ends_with("MAX SPEED"));
    }
}
