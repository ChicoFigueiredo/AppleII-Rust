//! Application actions bound to host keys and toolbar buttons (AppleWin WinFrame.cpp
//! `WM_KEYDOWN` + `ProcessButtonClick`).
//!
//! | Key | AppleWin | Action |
//! |---|---|---|
//! | F1 | BTN_HELP: opens the in-app manual ([`crate::help`]) | [`Action::Help`] |
//! | F2 | BTN_RUN: reboot (`ResetMachineState`); Ctrl+F2 = `CtrlReset` | [`Action::Reboot`] / [`Action::SoftReset`] |
//! | F3 / F4 | BTN_DRIVE1/2: choose image; Ctrl+F3/F4 = drive menu | [`Action::DriveSelect`] / [`Action::DriveMenu`] |
//! | F5 | BTN_DRIVESWAP | [`Action::SwapDrives`] |
//! | F6 | BTN_FULLSCR; Ctrl+F6 toggles 1x/2x; Alt+Enter too | [`Action::ToggleFullscreen`] / [`Action::ToggleScale`] |
//! | F7 | BTN_DEBUG | [`Action::ToggleDebugger`] |
//! | F8 | BTN_SETUP | [`Action::Settings`] |
//! | F9 / Shift+F9 / Ctrl+Shift+F9 | next / previous video mode / half scanlines | |
//! | F10 | //e video-ROM rocker switch (Pravets 8A: caps lock) | [`Action::RockerSwitch`] |
//! | F11 / F12 | save / load state | |
//! | Pause | pause / resume | [`Action::TogglePause`] |
//! | Scroll Lock | full speed (held, or toggled by the setting); TK3000 //e |Mode| key | `app` |
//! | Print Screen | screenshot; Shift = 280x192; Ctrl = copy the text screen | [`Action::Screenshot`] |
//! | Ctrl+0 / Ctrl+1 / Ctrl+3 | custom<->max / 1 MHz / max speed | [`Action::SpeedToggleMax`] |
//!
//! Ctrl+Break (Ctrl+Pause) is the keyboard reset and is handled in [`crate::keymap`].

use crate::keymap::{HostKey, Mods};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Help,
    Reboot,
    SoftReset,
    DriveSelect(u8),
    DriveMenu(u8),
    SwapDrives,
    ToggleFullscreen,
    ToggleScale,
    ToggleDebugger,
    Settings,
    NextVideoMode,
    PrevVideoMode,
    ToggleScanlines,
    RockerSwitch,
    SaveState,
    LoadState,
    TogglePause,
    ToggleFullSpeed,
    Screenshot,
    /// Shift+Print Screen: 280x192 instead of 560x384.
    ScreenshotSmall,
    /// Ctrl+Print Screen: the text screen to the clipboard.
    CopyScreenText,
    /// Ctrl+0: custom speed <-> full speed (GH#678).
    SpeedToggleMax,
    /// Ctrl+1: 1 MHz.
    Speed1Mhz,
    /// Ctrl+3: full speed.
    SpeedMax,
    /// Debugger single-step (F11 in AppleWin's debugger is "step into"; here Ctrl+F11).
    Step,
}

/// Returns the action bound to a key press, if any. Auto-repeat must be filtered by the caller.
pub fn action_for(key: HostKey, mods: Mods) -> Option<Action> {
    Some(match key {
        HostKey::F(1) => Action::Help,
        HostKey::F(2) if mods.ctrl => Action::SoftReset,
        HostKey::F(2) => Action::Reboot,
        HostKey::F(n @ (3 | 4)) if mods.ctrl => Action::DriveMenu(n - 3),
        HostKey::F(n @ (3 | 4)) => Action::DriveSelect(n - 3),
        HostKey::F(5) => Action::SwapDrives,
        HostKey::F(6) if mods.ctrl => Action::ToggleScale,
        HostKey::F(6) => Action::ToggleFullscreen,
        HostKey::Enter if mods.alt => Action::ToggleFullscreen,
        HostKey::F(7) => Action::ToggleDebugger,
        HostKey::F(8) => Action::Settings,
        HostKey::F(9) if mods.ctrl && mods.shift => Action::ToggleScanlines,
        HostKey::F(9) if mods.shift => Action::PrevVideoMode,
        HostKey::F(9) if !mods.ctrl => Action::NextVideoMode,
        HostKey::F(10) => Action::RockerSwitch,
        HostKey::F(11) if mods.ctrl => Action::Step,
        HostKey::F(11) => Action::SaveState,
        HostKey::F(12) => Action::LoadState,
        HostKey::Pause if !mods.ctrl => Action::TogglePause,
        HostKey::PrintScreen if mods.ctrl => Action::CopyScreenText,
        HostKey::PrintScreen if mods.shift => Action::ScreenshotSmall,
        HostKey::PrintScreen => Action::Screenshot,
        HostKey::Char('0') if mods.ctrl && !mods.alt => Action::SpeedToggleMax,
        HostKey::Char('1') if mods.ctrl && !mods.alt => Action::Speed1Mhz,
        HostKey::Char('3') if mods.ctrl && !mods.alt => Action::SpeedMax,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    const SHIFT: Mods = Mods {
        shift: true,
        ctrl: false,
        alt: false,
    };

    #[test]
    fn function_keys_follow_applewin() {
        assert_eq!(action_for(HostKey::F(1), NONE), Some(Action::Help));
        assert_eq!(action_for(HostKey::F(2), NONE), Some(Action::Reboot));
        assert_eq!(action_for(HostKey::F(2), CTRL), Some(Action::SoftReset));
        assert_eq!(
            action_for(HostKey::F(4), NONE),
            Some(Action::DriveSelect(1))
        );
        assert_eq!(action_for(HostKey::F(3), CTRL), Some(Action::DriveMenu(0)));
        assert_eq!(action_for(HostKey::F(6), CTRL), Some(Action::ToggleScale));
        assert_eq!(
            action_for(HostKey::F(9), SHIFT),
            Some(Action::PrevVideoMode)
        );
        assert_eq!(action_for(HostKey::F(11), NONE), Some(Action::SaveState));
        assert_eq!(action_for(HostKey::F(12), NONE), Some(Action::LoadState));
        assert_eq!(
            action_for(HostKey::PrintScreen, NONE),
            Some(Action::Screenshot)
        );
        assert_eq!(
            action_for(HostKey::PrintScreen, SHIFT),
            Some(Action::ScreenshotSmall)
        );
        assert_eq!(
            action_for(HostKey::PrintScreen, CTRL),
            Some(Action::CopyScreenText)
        );
    }

    #[test]
    fn speed_hotkeys_need_ctrl_without_alt() {
        assert_eq!(
            action_for(HostKey::Char('0'), CTRL),
            Some(Action::SpeedToggleMax)
        );
        assert_eq!(
            action_for(HostKey::Char('1'), CTRL),
            Some(Action::Speed1Mhz)
        );
        assert_eq!(action_for(HostKey::Char('3'), CTRL), Some(Action::SpeedMax));
        assert_eq!(action_for(HostKey::Char('2'), CTRL), None);
        assert_eq!(action_for(HostKey::Char('0'), NONE), None);
        // AltGr also fakes Ctrl (GH#749)
        assert_eq!(
            action_for(HostKey::Char('0'), Mods { alt: true, ..CTRL }),
            None
        );
    }

    #[test]
    fn scroll_lock_is_handled_by_the_shell() {
        assert_eq!(action_for(HostKey::ScrollLock, NONE), None);
    }

    #[test]
    fn ctrl_pause_is_reset_not_pause() {
        assert_eq!(action_for(HostKey::Pause, NONE), Some(Action::TogglePause));
        assert_eq!(action_for(HostKey::Pause, CTRL), None);
    }

    #[test]
    fn plain_keys_are_not_actions() {
        assert_eq!(action_for(HostKey::Char('a'), NONE), None);
        assert_eq!(action_for(HostKey::Enter, NONE), None);
        assert_eq!(
            action_for(HostKey::Enter, Mods { alt: true, ..NONE }),
            Some(Action::ToggleFullscreen)
        );
    }
}
