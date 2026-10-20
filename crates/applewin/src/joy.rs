//! Joystick emulation (AppleWin Joystick.cpp): keyboard numpad / cursors, host gamepads (gilrs),
//! mouse, the second joystick on thumbstick 2, trim, centering modes, button swap, autofire and
//! the "SHIFT key mod". Everything is framework neutral; [`JoySystem`] is fed raw events by the
//! shell and produces the final paddle / button values for the machine.

use crate::keymap::{HostKey, KeyInput};
use apple2_config::settings::{AltControlButtons, Joystick0Kind, Joystick1Kind};
use apple2_config::Settings;

/// Paddle centre as AppleWin's `PDL_CENTRAL`.
pub const CENTRE: u8 = 127;

/// Dead zone applied to the analog sticks of host gamepads.
pub const DEFAULT_DEADZONE: f32 = 0.12;

/// Half period of the autofire square wave. AppleWin toggles the button on every read, which a
/// game doing one read per video frame turns into one toggle per 1/60 s.
pub const AUTOFIRE_HALF_PERIOD_US: u64 = 16_667;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum JoyMode {
    /// No joystick attached (the ][ family then reads PB0/PB1 as pressed).
    #[default]
    Disabled,
    /// Numpad 1-9 directions, 0/Ins = button 0, ./Del = button 1.
    KeyboardNumpad,
    /// Dedicated cursor keys as directions.
    KeyboardCursors,
    /// A host gamepad (first one for joystick 0, second one for joystick 1).
    Gamepad,
    /// Thumbstick 2 of the first gamepad (joystick 1 only).
    Thumbstick2,
    Mouse,
}

impl JoyMode {
    pub fn is_keyboard(self) -> bool {
        matches!(self, JoyMode::KeyboardNumpad | JoyMode::KeyboardCursors)
    }
    fn is_pad(self) -> bool {
        matches!(self, JoyMode::Gamepad | JoyMode::Thumbstick2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoyOutput {
    pub x: u8,
    pub y: u8,
    pub buttons: [bool; 2],
}

impl Default for JoyOutput {
    fn default() -> Self {
        JoyOutput { x: CENTRE, y: CENTRE, buttons: [false; 2] }
    }
}

// ---------------------------------------------------------------------------------------------
// Keyboard as joystick
// ---------------------------------------------------------------------------------------------

/// What a key meant to a keyboard joystick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyUse {
    /// Not a joystick key.
    Ignored,
    /// A joystick key: the Apple does not see it.
    Joystick,
    /// A joystick key that the Apple keyboard must see too ("allow cursor keys to be read").
    JoystickAndApple,
}

/// Position of a numpad direction in AppleWin's `JOYKEY` order: 1..=9 -> 0..=8.
fn pad_value(index: usize) -> (i32, i32) {
    let x = [0, 127, 255][index % 3];
    let y = [255, 127, 0][index / 3];
    (x, y)
}

/// One keyboard-driven joystick (AppleWin keeps one shared key table; keeping one per stick lets
/// joystick 0 use the cursors and joystick 1 the numpad at the same time).
#[derive(Clone, Debug)]
pub struct KeyJoy {
    pub mode: JoyMode,
    /// Numpad directions 1-9 held (index 0 = key 1).
    pad: [bool; 9],
    cursors: [bool; 4], // left, up, right, down
    centering: bool,
    trim: (i16, i16),
    x: i32,
    y: i32,
}

impl Default for KeyJoy {
    fn default() -> Self {
        KeyJoy::new(JoyMode::Disabled)
    }
}

impl KeyJoy {
    pub fn new(mode: JoyMode) -> Self {
        let mut j = KeyJoy { mode, pad: [false; 9], cursors: [false; 4], centering: false, trim: (0, 0), x: 0, y: 0 };
        j.recentre();
        j
    }

    pub fn with_options(mode: JoyMode, centering: bool, trim: (i16, i16)) -> Self {
        let mut j = KeyJoy::new(mode);
        j.centering = centering;
        j.set_trim(trim);
        j
    }

    /// `JoySetTrim`: re-centres the stick.
    pub fn set_trim(&mut self, trim: (i16, i16)) {
        self.trim = trim;
        self.recentre();
    }

    fn recentre(&mut self) {
        self.x = i32::from(CENTRE) + i32::from(self.trim.0);
        self.y = i32::from(CENTRE) + i32::from(self.trim.1);
    }

    /// Feeds a key transition (`JoyProcessKey`). `cursor_control`: cursor keys also reach the
    /// Apple keyboard.
    pub fn process(&mut self, input: KeyInput, pressed: bool, repeat: bool, cursor_control: bool) -> KeyUse {
        let mut cursor_key = false;
        match self.mode {
            JoyMode::KeyboardNumpad if input.numpad => match numpad_direction(input.key) {
                Some(i) => self.pad[i] = pressed,
                None => return KeyUse::Ignored,
            },
            JoyMode::KeyboardCursors if !input.numpad => {
                let i = match input.key {
                    HostKey::Left => 0,
                    HostKey::Up => 1,
                    HostKey::Right => 2,
                    HostKey::Down => 3,
                    _ => return KeyUse::Ignored,
                };
                self.cursors[i] = pressed;
                cursor_key = true;
            }
            _ => return KeyUse::Ignored,
        }
        // Floating mode only follows fresh key presses; centering mode follows every event.
        if (pressed && !repeat) || self.centering {
            self.recompute();
        }
        if cursor_key && cursor_control {
            KeyUse::JoystickAndApple
        } else {
            KeyUse::Joystick
        }
    }

    fn recompute(&mut self) {
        let (mut xkeys, mut ykeys, mut xtotal, mut ytotal) = (0, 0, 0, 0);
        for (k, held) in self.pad.iter().enumerate() {
            if !held {
                continue;
            }
            let (vx, vy) = pad_value(k);
            if k % 3 != 1 {
                xkeys += 1;
                xtotal += vx;
            }
            if k / 3 != 1 {
                ykeys += 1;
                ytotal += vy;
            }
        }
        if self.cursors[0] {
            xkeys += 1; // left: x = 0
        }
        if self.cursors[2] {
            xkeys += 1;
            xtotal += 255;
        }
        if self.cursors[1] {
            ykeys += 1; // up: y = 0
        }
        if self.cursors[3] {
            ykeys += 1;
            ytotal += 255;
        }
        self.x = if xkeys > 0 { xtotal / xkeys } else { i32::from(CENTRE) + i32::from(self.trim.0) };
        self.y = if ykeys > 0 { ytotal / ykeys } else { i32::from(CENTRE) + i32::from(self.trim.1) };
    }

    pub fn release_all(&mut self) {
        self.pad = [false; 9];
        self.cursors = [false; 4];
        self.recentre();
    }

    pub fn position(&self) -> (u8, u8) {
        (self.x.clamp(0, 255) as u8, self.y.clamp(0, 255) as u8)
    }
}

/// Numpad key (digit with Num Lock on, navigation key with it off) to a direction index 0..=8.
fn numpad_direction(key: HostKey) -> Option<usize> {
    Some(match key {
        HostKey::Char(c @ '1'..='9') => usize::from(c as u8 - b'1'),
        HostKey::End => 0,
        HostKey::Down => 1,
        HostKey::PageDown => 2,
        HostKey::Left => 3,
        HostKey::Clear => 4,
        HostKey::Right => 5,
        HostKey::Home => 6,
        HostKey::Up => 7,
        HostKey::PageUp => 8,
        _ => return None,
    })
}

/// Numpad button keys: `0`/Ins = button 0, `.`/Del = button 1.
fn numpad_button(key: HostKey) -> Option<usize> {
    match key {
        HostKey::Char('0') | HostKey::Insert => Some(0),
        HostKey::Char('.') | HostKey::Char(',') | HostKey::Delete => Some(1),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Analog helpers
// ---------------------------------------------------------------------------------------------

/// Gamepad axis in `-1.0..=1.0` to a paddle value, with a dead zone around the centre.
pub fn axis_to_paddle(v: f32, deadzone: f32) -> u8 {
    let v = v.clamp(-1.0, 1.0);
    if v.abs() < deadzone {
        return CENTRE;
    }
    (((v + 1.0) * 0.5) * 255.0) as u8
}

/// AppleWin adds the trim only when an analog stick reports its centre (127 or 128).
pub fn apply_trim(pos: u8, trim: i16) -> u8 {
    if pos == 127 || pos == 128 {
        (i32::from(pos) + i32::from(trim)).clamp(0, 255) as u8
    } else {
        pos
    }
}

/// Mouse position inside the Apple viewport (`0..size`) to a paddle value (`JoySetPosition`).
pub fn mouse_to_paddle(pos: f32, size: f32) -> u8 {
    if size <= 0.0 {
        return CENTRE;
    }
    ((pos.clamp(0.0, size) * 255.0 / size) as u32).min(255) as u8
}

/// "Square the circle" (`JoyResetPosition`, GH#429) for analog sticks: stretches the circular
/// range of a thumbstick onto the square range of an Apple II paddle pair.
pub fn square_circle(x: u8, y: u8) -> (u8, u8) {
    const SCALAR: f64 = 0.5 * 255.0;
    let fx = f64::from(x) / SCALAR - 1.0;
    let fy = f64::from(y) / SCALAR - 1.0;
    let coeff = if fx * fy != 0.0 {
        let ratio2 = (fy * fy) / (fx * fx);
        let c = ratio2.min(1.0 / ratio2);
        (1.0 + c).sqrt()
    } else {
        1.0
    };
    let conv = |a: f64| ((a * coeff).clamp(-1.0, 1.0) + 1.0) * SCALAR;
    (conv(fx) as u8, conv(fy) as u8)
}

// ---------------------------------------------------------------------------------------------
// Autofire
// ---------------------------------------------------------------------------------------------

/// Autofire: a pressed button is chopped into a square wave (`DoAutofire`).
#[derive(Clone, Debug, Default)]
pub struct Autofire {
    mask: u8,
    since: [Option<u64>; 3],
}

impl Autofire {
    pub fn new(mask: u8) -> Self {
        Autofire { mask, since: [None; 3] }
    }

    /// `pressed` as the host reports it for button `index`; `now_us` is a monotonic clock.
    pub fn apply(&mut self, index: usize, pressed: bool, now_us: u64) -> bool {
        if !pressed {
            self.since[index] = None;
            return false;
        }
        if self.mask & (1 << index) == 0 {
            return true;
        }
        let t0 = *self.since[index].get_or_insert(now_us);
        (now_us.saturating_sub(t0) / AUTOFIRE_HALF_PERIOD_US).is_multiple_of(2)
    }
}

// ---------------------------------------------------------------------------------------------
// The whole game port
// ---------------------------------------------------------------------------------------------

/// Joystick-related settings, resolved from [`Settings`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoyConfig {
    pub kinds: [JoyMode; 2],
    pub trim: (i16, i16),
    pub centering: bool,
    pub cursor_control: bool,
    pub autofire: u8,
    pub swap_buttons: bool,
    pub shift_key_mod: bool,
    pub hook_alt_keys: bool,
    pub alt_control: Option<AltControlButtons>,
    pub disabled_pads: [bool; 2],
}

impl Default for JoyConfig {
    fn default() -> Self {
        JoyConfig::from_settings(&Settings::default())
    }
}

impl JoyConfig {
    pub fn from_settings(s: &Settings) -> Self {
        let j = &s.joystick;
        let j0 = match j.joystick0 {
            Joystick0Kind::Disabled => JoyMode::Disabled,
            Joystick0Kind::HostJoystick1 => JoyMode::Gamepad,
            Joystick0Kind::KeyboardCursors => JoyMode::KeyboardCursors,
            Joystick0Kind::KeyboardNumpad => JoyMode::KeyboardNumpad,
            Joystick0Kind::Mouse => JoyMode::Mouse,
        };
        let j1 = match j.joystick1 {
            Joystick1Kind::Disabled => JoyMode::Disabled,
            Joystick1Kind::HostJoystick2 => JoyMode::Gamepad,
            Joystick1Kind::HostJoystick1Thumbstick2 => JoyMode::Thumbstick2,
            Joystick1Kind::KeyboardCursors => JoyMode::KeyboardCursors,
            Joystick1Kind::KeyboardNumpad => JoyMode::KeyboardNumpad,
            Joystick1Kind::Mouse => JoyMode::Mouse,
        };
        JoyConfig {
            kinds: [j0, j1],
            trim: (j.pdl_x_trim, j.pdl_y_trim),
            centering: j.centering,
            cursor_control: j.cursor_control,
            autofire: j.autofire & 7,
            swap_buttons: j.swap_buttons,
            shift_key_mod: j.shift_key_mod,
            hook_alt_keys: j.hook_alt_keys,
            alt_control: j.alt_control_buttons,
            disabled_pads: j.disabled_host_devices,
        }
    }
}

/// Raw state of one host gamepad (`-1..=1` axes, positive Y = down).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    pub buttons: [bool; 2],
}

/// What the machine must see, from [`JoySystem::output`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoyFinal {
    /// `out[0]`: paddles 0-1 with PB0/PB1; `out[1]`: paddles 2-3 with PB2 in `buttons[0]`.
    pub out: [JoyOutput; 2],
    pub attached: [bool; 2],
}

/// A physical key acting as a game-port button (Open/Closed Apple, or Control with `-*-alt-control-buttons`).
fn button_key(key: HostKey, alt_control: Option<AltControlButtons>) -> Option<usize> {
    match (alt_control, key) {
        (None, HostKey::AltLeft) => Some(0),
        (None, HostKey::AltRight) => Some(1),
        (Some(AltControlButtons::Left), HostKey::ControlLeft) => Some(0),
        (Some(AltControlButtons::Left), HostKey::AltLeft) => Some(1),
        (Some(AltControlButtons::Right), HostKey::AltRight) => Some(0),
        (Some(AltControlButtons::Right), HostKey::ControlRight) => Some(1),
        _ => None,
    }
}

#[derive(Debug)]
pub struct JoySystem {
    cfg: JoyConfig,
    kb: [KeyJoy; 2],
    /// Numpad `0` / `.` held.
    key_buttons: [bool; 2],
    /// Alt (Open/Closed Apple) keys held.
    apple: [bool; 2],
    pub pads: [PadState; 2],
    pads_connected: usize,
    mouse_pos: (u8, u8),
    mouse_buttons: [bool; 3],
    shift: bool,
    autofire: Autofire,
}

impl JoySystem {
    pub fn new(cfg: JoyConfig) -> Self {
        let mut s = JoySystem {
            kb: [KeyJoy::default(), KeyJoy::default()],
            autofire: Autofire::new(cfg.autofire),
            cfg,
            key_buttons: [false; 2],
            apple: [false; 2],
            pads: Default::default(),
            pads_connected: 0,
            mouse_pos: (CENTRE, CENTRE),
            mouse_buttons: [false; 3],
            shift: false,
        };
        s.rebuild_keyboards();
        s
    }

    pub fn config(&self) -> &JoyConfig {
        &self.cfg
    }

    pub fn set_config(&mut self, cfg: JoyConfig) {
        self.autofire = Autofire::new(cfg.autofire);
        self.cfg = cfg;
        self.rebuild_keyboards();
    }

    pub fn set_pads_connected(&mut self, n: usize) {
        self.pads_connected = n;
        self.rebuild_keyboards();
    }

    pub fn set_shift(&mut self, down: bool) {
        self.shift = down;
    }

    /// The device each Apple joystick really uses: a gamepad that is not there makes joystick 0
    /// fall back to the numpad and joystick 1 to "disabled" (`JoyInitialize`).
    pub fn effective_kinds(&self) -> [JoyMode; 2] {
        let pad = |n: usize| self.pads_connected > n && !self.cfg.disabled_pads[n];
        let j0 = match self.cfg.kinds[0] {
            JoyMode::Gamepad if !pad(0) => JoyMode::KeyboardNumpad,
            JoyMode::Thumbstick2 => JoyMode::Disabled,
            k => k,
        };
        let j1 = match self.cfg.kinds[1] {
            JoyMode::Gamepad if !pad(1) => JoyMode::Disabled,
            JoyMode::Thumbstick2 if !pad(0) => JoyMode::Disabled,
            k => k,
        };
        [j0, j1]
    }

    fn rebuild_keyboards(&mut self) {
        let kinds = self.effective_kinds();
        for (kb, kind) in self.kb.iter_mut().zip(kinds) {
            if kb.mode != kind {
                *kb = KeyJoy::with_options(kind, self.cfg.centering, self.cfg.trim);
            } else {
                kb.centering = self.cfg.centering;
                if kb.trim != self.cfg.trim {
                    kb.set_trim(self.cfg.trim);
                }
            }
        }
    }

    /// Feeds a key transition. True if the key belongs to the game port and must not reach the
    /// Apple keyboard (cursor keys may still be shown to it, see [`KeyUse`]).
    pub fn process_key(&mut self, input: KeyInput, pressed: bool, repeat: bool) -> KeyUse {
        // Alt keys can be switched off (`-no-hook-alt`, GH#583).
        if !self.cfg.hook_alt_keys && matches!(input.key, HostKey::AltLeft | HostKey::AltRight) {
            return KeyUse::Ignored;
        }
        if let Some(b) = button_key(input.key, self.cfg.alt_control) {
            self.apple[b] = pressed;
            return if matches!(input.key, HostKey::ControlLeft | HostKey::ControlRight) { KeyUse::JoystickAndApple } else { KeyUse::Joystick };
        }
        let kinds = self.effective_kinds();
        if (kinds[0] == JoyMode::KeyboardNumpad || kinds[1] == JoyMode::KeyboardNumpad) && input.numpad {
            if let Some(b) = numpad_button(input.key) {
                self.key_buttons[b] = pressed;
                return KeyUse::Joystick;
            }
        }
        for kb in &mut self.kb {
            let r = kb.process(input, pressed, repeat, self.cfg.cursor_control);
            if r != KeyUse::Ignored {
                return r;
            }
        }
        KeyUse::Ignored
    }

    /// Releases everything held (focus lost).
    pub fn release_all(&mut self) {
        for kb in &mut self.kb {
            kb.release_all();
        }
        self.key_buttons = [false; 2];
        self.apple = [false; 2];
        self.mouse_buttons = [false; 3];
        self.shift = false;
    }

    pub fn set_mouse_position(&mut self, x: u8, y: u8) {
        self.mouse_pos = (x, y);
    }

    /// `JoySetButton`: a mouse button acting as a game-port button.
    pub fn set_mouse_button(&mut self, number: usize, down: bool) {
        let kinds = self.effective_kinds();
        if number > 1 || (kinds[1] != JoyMode::Disabled && number != 0) {
            return;
        }
        let number = if kinds[1] == JoyMode::Mouse { 1 } else { number };
        self.mouse_buttons[number] = down;
    }

    pub fn using_mouse(&self) -> bool {
        self.effective_kinds().contains(&JoyMode::Mouse)
    }

    fn stick(&self, i: usize, kind: JoyMode) -> (u8, u8) {
        let (trim_x, trim_y) = self.cfg.trim;
        match kind {
            JoyMode::Disabled => (CENTRE, CENTRE),
            JoyMode::KeyboardNumpad | JoyMode::KeyboardCursors => self.kb[i].position(),
            JoyMode::Mouse => self.mouse_pos,
            JoyMode::Gamepad | JoyMode::Thumbstick2 => {
                let p = if kind == JoyMode::Gamepad { &self.pads[i] } else { &self.pads[0] };
                let (ax, ay) = if kind == JoyMode::Thumbstick2 { (p.rx, p.ry) } else { (p.lx, p.ly) };
                let x = apply_trim(axis_to_paddle(ax, DEFAULT_DEADZONE), trim_x);
                let y = apply_trim(axis_to_paddle(ay, DEFAULT_DEADZONE), trim_y);
                square_circle(x, y)
            }
        }
    }

    /// The values to give the machine now. `now_us`: monotonic microseconds (autofire).
    pub fn output(&mut self, now_us: u64) -> JoyFinal {
        let kinds = self.effective_kinds();
        let joy1_present = kinds[1] != JoyMode::Disabled;

        // Pushbuttons from gamepads (CheckJoystick0/1).
        let mut pad_b = [false; 3];
        if kinds[0] == JoyMode::Gamepad {
            pad_b[0] = self.pads[0].buttons[0];
            if !joy1_present {
                pad_b[1] = self.pads[0].buttons[1];
            }
        }
        match kinds[1] {
            JoyMode::Gamepad => {
                pad_b[2] = self.pads[1].buttons[0];
                pad_b[1] = self.pads[1].buttons[0];
            }
            JoyMode::Thumbstick2 => {
                pad_b[2] = self.pads[0].buttons[1];
                pad_b[1] = self.pads[0].buttons[1];
            }
            _ => {}
        }
        // Numpad 0 / '.' only when joystick 1 is not itself a keyboard stick.
        let keys_ok = !kinds[1].is_keyboard();
        let raw = [
            pad_b[0] || self.mouse_buttons[0] || self.apple[0] || (keys_ok && self.key_buttons[0]),
            pad_b[1] || self.mouse_buttons[1] || self.apple[1] || (keys_ok && self.key_buttons[1]),
            pad_b[2] || self.mouse_buttons[2],
        ];
        let af = [self.autofire.apply(0, raw[0], now_us), self.autofire.apply(1, raw[1], now_us), self.autofire.apply(2, raw[2], now_us)];
        let (pb0, pb1) = if self.cfg.swap_buttons { (af[1], af[0]) } else { (af[0], af[1]) };

        let mut attached = [kinds[0] != JoyMode::Disabled, joy1_present];
        // "SHIFT key mod": PB2 floats high unless Shift pulls it low. Only without joystick 1.
        let mut pb2 = af[2];
        if !joy1_present && self.cfg.shift_key_mod {
            attached[1] = true;
            pb2 = !self.shift;
        }

        let (x0, y0) = self.stick(0, kinds[0]);
        let (x1, y1) = self.stick(1, kinds[1]);
        JoyFinal {
            out: [JoyOutput { x: x0, y: y0, buttons: [pb0, pb1] }, JoyOutput { x: x1, y: y1, buttons: [pb2, false] }],
            attached,
        }
    }

    /// For tests and the status area: whether any stick is driven by a pad.
    pub fn uses_pad(&self) -> bool {
        self.effective_kinds().iter().any(|k| k.is_pad())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn np(c: char) -> KeyInput {
        KeyInput::numpad(HostKey::Char(c))
    }

    fn press(j: &mut KeyJoy, k: KeyInput) -> KeyUse {
        j.process(k, true, false, false)
    }

    fn release(j: &mut KeyJoy, k: KeyInput) -> KeyUse {
        j.process(k, false, false, false)
    }

    fn sys(f: impl FnOnce(&mut Settings)) -> JoySystem {
        let mut s = Settings::default();
        f(&mut s);
        JoySystem::new(JoyConfig::from_settings(&s))
    }

    fn numpad_sys(centering: bool) -> JoySystem {
        sys(|s| {
            s.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
            s.joystick.centering = centering;
        })
    }

    #[test]
    fn numpad_directions_average_like_applewin() {
        let mut j = KeyJoy::new(JoyMode::KeyboardNumpad);
        assert_eq!(press(&mut j, np('7')), KeyUse::Joystick);
        assert_eq!(j.position(), (0, 0));
        press(&mut j, np('9'));
        assert_eq!(j.position(), (127, 0)); // (0 + 255) / 2 horizontally
        let mut j = KeyJoy::new(JoyMode::KeyboardNumpad);
        press(&mut j, np('3'));
        assert_eq!(j.position(), (255, 255));
        let mut j = KeyJoy::new(JoyMode::KeyboardNumpad);
        press(&mut j, np('6'));
        assert_eq!(j.position(), (255, 127));
        press(&mut j, np('8'));
        assert_eq!(j.position(), (255, 0)); // 6 only sets x, 8 only y
    }

    #[test]
    fn numlock_off_navigation_keys_are_the_same_directions() {
        let mut j = KeyJoy::new(JoyMode::KeyboardNumpad);
        assert_eq!(press(&mut j, KeyInput::numpad(HostKey::Left)), KeyUse::Joystick);
        assert_eq!(j.position().0, 0);
        // the dedicated (non numpad) cursors are not joystick keys in numpad mode
        assert_eq!(press(&mut j, KeyInput::new(HostKey::Right)), KeyUse::Ignored);
    }

    #[test]
    fn floating_mode_keeps_position_after_release() {
        let mut j = KeyJoy::with_options(JoyMode::KeyboardNumpad, false, (0, 0));
        press(&mut j, np('4'));
        release(&mut j, np('4'));
        assert_eq!(j.position(), (0, 127), "no-centering: stick stays");
        press(&mut j, np('5')); // the centre key re-centres explicitly
        assert_eq!(j.position(), (127, 127));
    }

    #[test]
    fn centering_mode_returns_to_centre_with_trim() {
        let mut j = KeyJoy::with_options(JoyMode::KeyboardNumpad, true, (3, -4));
        assert_eq!(j.position(), (130, 123), "starts on the trimmed centre");
        press(&mut j, np('4'));
        release(&mut j, np('4'));
        assert_eq!(j.position(), (130, 123));
    }

    #[test]
    fn floating_mode_ignores_autorepeat() {
        let mut j = KeyJoy::with_options(JoyMode::KeyboardNumpad, false, (0, 0));
        j.process(np('4'), true, false, false);
        j.process(np('6'), true, false, false); // x = (0+255)/2
        assert_eq!(j.position().0, 127);
        j.process(np('6'), false, false, false);
        j.process(np('4'), true, true, false); // repeat: ignored in floating mode
        assert_eq!(j.position().0, 127);
    }

    #[test]
    fn cursor_mode_only_takes_dedicated_cursors() {
        let mut j = KeyJoy::new(JoyMode::KeyboardCursors);
        assert_eq!(press(&mut j, np('4')), KeyUse::Ignored);
        assert_eq!(press(&mut j, KeyInput::new(HostKey::Right)), KeyUse::Joystick);
        assert_eq!(j.position().0, 255);
        assert_eq!(press(&mut j, KeyInput::new(HostKey::Char('a'))), KeyUse::Ignored);
        press(&mut j, KeyInput::new(HostKey::Down));
        assert_eq!(j.position(), (255, 255));
    }

    #[test]
    fn cursor_control_lets_the_apple_see_the_cursor_keys() {
        let mut j = KeyJoy::new(JoyMode::KeyboardCursors);
        assert_eq!(j.process(KeyInput::new(HostKey::Left), true, false, true), KeyUse::JoystickAndApple);
    }

    #[test]
    fn disabled_mode_takes_nothing() {
        let mut j = KeyJoy::new(JoyMode::Disabled);
        assert_eq!(press(&mut j, np('4')), KeyUse::Ignored);
    }

    #[test]
    fn analog_helpers() {
        assert_eq!(axis_to_paddle(0.0, 0.1), CENTRE);
        assert_eq!(axis_to_paddle(0.05, 0.1), CENTRE);
        assert_eq!(axis_to_paddle(-1.0, 0.1), 0);
        assert_eq!(axis_to_paddle(1.0, 0.1), 255);
        assert_eq!(mouse_to_paddle(100.0, 200.0), 127);
        assert_eq!(mouse_to_paddle(-5.0, 200.0), 0);
        assert_eq!(mouse_to_paddle(200.0, 200.0), 255);
        assert_eq!(apply_trim(127, 5), 132);
        assert_eq!(apply_trim(128, -5), 123);
        assert_eq!(apply_trim(50, 5), 50);
        assert_eq!(apply_trim(127, 300), 255);
    }

    #[test]
    fn square_the_circle_stretches_diagonals() {
        // Hard right stays full right, a pure axis is untouched.
        assert_eq!(square_circle(255, 127).0, 255);
        assert_eq!(square_circle(0, 127).0, 0);
        // A 45 degree diagonal on the unit circle (0.707, 0.707) reaches the square corner.
        let v = (127.5 * (1.0 + std::f64::consts::FRAC_1_SQRT_2)) as u8; // 217
        let (x, y) = square_circle(v, 255 - v);
        assert!(x >= 253, "x = {x}");
        assert!(y <= 2, "y = {y}");
    }

    #[test]
    fn autofire_toggles_only_chosen_buttons() {
        let mut a = Autofire::new(0b001);
        let h = AUTOFIRE_HALF_PERIOD_US;
        assert!(a.apply(0, true, 1_000));
        assert!(a.apply(0, true, 1_000 + h / 2));
        assert!(!a.apply(0, true, 1_000 + h + 1));
        assert!(a.apply(0, true, 1_000 + 2 * h + 1));
        assert!(!a.apply(0, false, 1_000 + 2 * h + 2));
        // re-press starts on again
        assert!(a.apply(0, true, 9_000_000));
        // button 1 has no autofire
        assert!(a.apply(1, true, 0) && a.apply(1, true, h + 1));
    }

    #[test]
    fn gamepad_missing_falls_back_to_numpad_and_disables_joystick_one() {
        let s = sys(|s| {
            s.joystick.joystick0 = Joystick0Kind::HostJoystick1;
            s.joystick.joystick1 = Joystick1Kind::HostJoystick2;
        });
        assert_eq!(s.effective_kinds(), [JoyMode::KeyboardNumpad, JoyMode::Disabled]);
        let mut s = s;
        s.set_pads_connected(2);
        assert_eq!(s.effective_kinds(), [JoyMode::Gamepad, JoyMode::Gamepad]);
    }

    #[test]
    fn buttons_from_numpad_alt_and_swap() {
        let mut s = numpad_sys(false);
        s.process_key(np('0'), true, false);
        assert_eq!(s.output(0).out[0].buttons, [true, false]);
        s.process_key(np('0'), false, false);
        s.process_key(KeyInput::new(HostKey::AltRight), true, false);
        assert_eq!(s.output(0).out[0].buttons, [false, true], "right Alt is button 1 (Closed Apple)");
        let mut sw = sys(|c| {
            c.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
            c.joystick.swap_buttons = true;
        });
        sw.process_key(KeyInput::new(HostKey::AltLeft), true, false);
        assert_eq!(sw.output(0).out[0].buttons, [false, true], "swap affects the Apple keys too");
    }

    #[test]
    fn alt_control_buttons_variants() {
        let mut s = sys(|c| c.joystick.alt_control_buttons = Some(AltControlButtons::Left));
        assert_eq!(s.process_key(KeyInput::new(HostKey::ControlLeft), true, false), KeyUse::JoystickAndApple);
        assert_eq!(s.output(0).out[0].buttons, [true, false]);
        s.process_key(KeyInput::new(HostKey::AltLeft), true, false);
        assert_eq!(s.output(0).out[0].buttons, [true, true]);
        // right Alt does nothing in this mode
        assert_eq!(s.process_key(KeyInput::new(HostKey::AltRight), true, false), KeyUse::Ignored);
        let mut r = sys(|c| c.joystick.alt_control_buttons = Some(AltControlButtons::Right));
        r.process_key(KeyInput::new(HostKey::AltRight), true, false);
        assert_eq!(r.output(0).out[0].buttons, [true, false]);
    }

    #[test]
    fn no_hook_alt_ignores_alt() {
        let mut s = sys(|c| c.joystick.hook_alt_keys = false);
        assert_eq!(s.process_key(KeyInput::new(HostKey::AltLeft), true, false), KeyUse::Ignored);
        assert_eq!(s.output(0).out[0].buttons, [false, false]);
    }

    #[test]
    fn second_joystick_drives_pb1_and_pb2() {
        let mut s = sys(|c| {
            c.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
            c.joystick.joystick1 = Joystick1Kind::HostJoystick2;
        });
        s.set_pads_connected(2);
        s.pads[1].buttons[0] = true;
        s.pads[0].buttons[1] = true; // joystick 0's second button is ignored with two sticks
        let f = s.output(0);
        assert_eq!(f.out[0].buttons, [false, true]);
        assert!(f.out[1].buttons[0]);
        assert_eq!(f.attached, [true, true]);
    }

    #[test]
    fn thumbstick2_uses_the_first_pad() {
        let mut s = sys(|c| {
            c.joystick.joystick0 = Joystick0Kind::HostJoystick1;
            c.joystick.joystick1 = Joystick1Kind::HostJoystick1Thumbstick2;
        });
        s.set_pads_connected(1);
        s.pads[0] = PadState { lx: -1.0, ly: 0.0, rx: 1.0, ry: 0.0, buttons: [false, true] };
        let f = s.output(0);
        assert_eq!(f.out[0].x, 0);
        assert_eq!(f.out[1].x, 255);
        assert!(f.out[1].buttons[0]);
    }

    #[test]
    fn gamepad_trim_only_on_the_centre() {
        let mut s = sys(|c| {
            c.joystick.joystick0 = Joystick0Kind::HostJoystick1;
            c.joystick.pdl_x_trim = 10;
        });
        s.set_pads_connected(1);
        let f = s.output(0);
        assert_eq!(f.out[0].x, 137, "centre 127 + trim 10");
        s.pads[0].lx = 1.0;
        assert_eq!(s.output(0).out[0].x, 255);
    }

    #[test]
    fn mouse_buttons_follow_joysticks_rule() {
        let mut s = sys(|c| c.joystick.joystick0 = Joystick0Kind::Mouse);
        s.set_mouse_button(0, true);
        s.set_mouse_button(1, true);
        assert_eq!(s.output(0).out[0].buttons, [true, true]);
        // With a second joystick present only the left button counts.
        let mut t = sys(|c| {
            c.joystick.joystick0 = Joystick0Kind::Mouse;
            c.joystick.joystick1 = Joystick1Kind::KeyboardNumpad;
        });
        t.set_mouse_button(1, true);
        assert_eq!(t.output(0).out[0].buttons, [false, false]);
        // Mouse as joystick 1 drives Apple button 1.
        let mut u = sys(|c| {
            c.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
            c.joystick.joystick1 = Joystick1Kind::Mouse;
        });
        u.set_mouse_button(0, true);
        assert_eq!(u.output(0).out[0].buttons, [false, true]);
    }

    #[test]
    fn shift_key_mod_drives_pb2_when_joystick_one_is_absent() {
        let mut s = sys(|c| c.joystick.shift_key_mod = true);
        let f = s.output(0);
        assert!(f.attached[1] && f.out[1].buttons[0], "PB2 floats high");
        s.set_shift(true);
        assert!(!s.output(0).out[1].buttons[0], "Shift pulls it low");
        // Without the mod joystick 1 stays unattached (the machine floats PB2 itself).
        let mut n = sys(|_| {});
        assert!(!n.output(0).attached[1]);
    }

    #[test]
    fn autofire_in_the_system() {
        let mut s = sys(|c| {
            c.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
            c.joystick.autofire = 1;
        });
        s.process_key(np('0'), true, false);
        let h = AUTOFIRE_HALF_PERIOD_US;
        assert!(s.output(100).out[0].buttons[0]);
        assert!(!s.output(100 + h + 1).out[0].buttons[0]);
        assert!(s.output(100 + 2 * h + 1).out[0].buttons[0]);
    }

    #[test]
    fn numpad_buttons_ignored_when_joystick_one_is_a_keyboard() {
        let mut s = sys(|c| {
            c.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
            c.joystick.joystick1 = Joystick1Kind::KeyboardCursors;
        });
        s.process_key(np('0'), true, false);
        assert_eq!(s.output(0).out[0].buttons, [false, false]);
    }

    #[test]
    fn release_all_centres_everything() {
        let mut s = numpad_sys(false);
        s.process_key(np('4'), true, false);
        s.process_key(KeyInput::new(HostKey::AltLeft), true, false);
        s.release_all();
        let f = s.output(0);
        assert_eq!((f.out[0].x, f.out[0].buttons), (127, [false, false]));
    }
}
