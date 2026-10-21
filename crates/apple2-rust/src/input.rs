//! Input routing without a window: host key / gamepad events in, [`Command`]s for the emulation
//! thread out. [`crate::app`] only translates winit events and forwards them here, so the whole
//! path (including the "stuck key" cases of AUD-P-028) can be tested headless.

use crate::emu::Command;
use crate::joy::{HostPads, JoyConfig, JoyFinal, JoySystem, KeyUse};
use crate::keymap::{HostKey, KeyEvent, KeyInput, KeyboardState, Mods};
use apple2_config::Settings;

/// Where the host keyboard is right now.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyCtx {
    pub mods: Mods,
    /// The debugger console owns the keyboard.
    pub debugger_active: bool,
    /// egui (a text field, a menu) consumes key presses.
    pub ui_wants_keys: bool,
}

/// What a key transition turned into.
#[derive(Debug)]
pub enum Routed {
    Cmd(Command),
    /// Shift+Insert / Ctrl+Shift+V: paste the host clipboard.
    Paste,
    /// Scroll Lock (full speed): needs the user's "toggle" preference, so the app decides.
    ScrollLock {
        pressed: bool,
        repeat: bool,
    },
}

/// Scroll Lock and full speed (AppleWin: held = full speed while down; with "ScrollLock Toggle"
/// each press flips it): true when this key event must flip the full-speed state. GH#7, GH#56.
pub fn scroll_lock_flips_full_speed(pressed: bool, repeat: bool, toggle_setting: bool) -> bool {
    !repeat && (pressed || !toggle_setting)
}

/// Actions after which the keyboard is somewhere else (the debugger, another screen mode): the
/// held keys are let go so that no release gets lost (AUD-P-028).
pub fn releases_keys(a: crate::actions::Action) -> bool {
    use crate::actions::Action;
    matches!(a, Action::ToggleDebugger | Action::ToggleFullscreen)
}

pub struct InputRouter {
    pub kbd: KeyboardState,
    pub joy: JoySystem,
    /// What the machine was last told about the joysticks and the pad cards.
    last_joy: Option<JoyFinal>,
    last_pads: Option<HostPads>,
}

impl InputRouter {
    pub fn new(settings: &Settings) -> Self {
        let mut kbd = KeyboardState::with_caps(settings.keyboard.caps_lock);
        kbd.set_altgr_sends_char(settings.keyboard.altgr_sends_char);
        kbd.set_hook_altgr_control(settings.keyboard.hook_altgr_control);
        InputRouter {
            kbd,
            joy: JoySystem::new(JoyConfig::from_settings(settings)),
            last_joy: None,
            last_pads: None,
        }
    }

    /// The machine was rebuilt: it knows nothing, so the next [`InputRouter::update`] sends all.
    pub fn resend(&mut self) {
        self.last_joy = None;
        self.last_pads = None;
    }

    /// Routes one key transition.
    ///
    /// Releases always reach [`KeyboardState`] and [`JoySystem`], whoever owns the keyboard now:
    /// the press that opened the debugger or a UI widget must not leave its key down (AUD-P-028).
    pub fn key(
        &mut self,
        input: KeyInput,
        pressed: bool,
        repeat: bool,
        ctx: KeyCtx,
        now_us: u64,
    ) -> Vec<Routed> {
        let mut out = Vec::new();
        if pressed && ctx.debugger_active {
            if let Some(k) = crate::keymap::debug_key(input.key, ctx.mods) {
                out.push(Routed::Cmd(Command::Debug(k)));
            }
            return out;
        }
        if pressed && ctx.ui_wants_keys {
            return out;
        }
        if input.key == HostKey::ScrollLock {
            out.push(Routed::ScrollLock { pressed, repeat });
        }
        if input.key == HostKey::AltRight {
            self.kbd.set_altgr_down(pressed);
        }
        // the game port takes its keys first (AppleWin JoyProcessKey)
        match self.joy.process_key(input, pressed, repeat) {
            KeyUse::Joystick => {
                out.extend(self.update(now_us).into_iter().map(Routed::Cmd));
                return out;
            }
            KeyUse::JoystickAndApple => {
                out.extend(self.update(now_us).into_iter().map(Routed::Cmd));
            }
            KeyUse::Ignored => {}
        }
        for e in self.kbd.handle(input, pressed, repeat, ctx.mods) {
            out.push(match e {
                KeyEvent::Paste => Routed::Paste,
                other => Routed::Cmd(Command::Key(other)),
            });
        }
        // the 4Play keys (E/S/D/F...) are not consumed, but they change the card's ports
        out.extend(self.update(now_us).into_iter().map(Routed::Cmd));
        out
    }

    /// Lets go of every key and key-driven joystick (the debugger or full screen took over, the
    /// window lost focus). Mouse buttons are released too.
    pub fn release_all(&mut self, now_us: u64) -> Vec<Command> {
        let mut out: Vec<Command> = self
            .kbd
            .release_all()
            .into_iter()
            .map(Command::Key)
            .collect();
        self.joy.release_all();
        out.extend(self.update(now_us));
        out
    }

    /// Sends whatever changed in the joystick and pad-card state (also advances autofire).
    pub fn update(&mut self, now_us: u64) -> Vec<Command> {
        let mut cmds = Vec::new();
        let f = self.joy.output(now_us);
        let prev = self.last_joy;
        if prev != Some(f) {
            if prev.is_none_or(|p| p.pb2_shift != f.pb2_shift) {
                cmds.push(Command::JoyPb2Shift(f.pb2_shift));
            }
            for i in 0..2 {
                if prev.is_none_or(|p| p.attached[i] != f.attached[i]) {
                    cmds.push(Command::JoyAttached {
                        index: i,
                        attached: f.attached[i],
                    });
                }
                if prev.is_none_or(|p| p.out[i] != f.out[i]) {
                    cmds.push(Command::Joy {
                        index: i,
                        out: f.out[i],
                    });
                }
            }
            self.last_joy = Some(f);
        }
        let hp = self.joy.host_pads();
        if self.last_pads != Some(hp) {
            self.last_pads = Some(hp);
            cmds.push(Command::HostPads(hp));
        }
        cmds
    }
}

#[cfg(test)]
mod scroll_lock_tests {
    use super::scroll_lock_flips_full_speed as flips;

    /// GH#7 (temporary speed-up): hold = full speed while down, back to normal on release.
    #[test]
    fn held_scroll_lock_is_full_speed_only_while_down() {
        assert!(flips(true, false, false), "press turns it on");
        assert!(flips(false, false, false), "release turns it off");
        assert!(!flips(true, true, false), "auto-repeat is ignored");
    }

    /// GH#56 (lock/toggle for turbo mode): with the toggle setting a press flips, a release not.
    #[test]
    fn toggle_setting_flips_on_press_only() {
        assert!(flips(true, false, true));
        assert!(!flips(false, false, true));
        assert!(!flips(true, true, true));
    }
}
