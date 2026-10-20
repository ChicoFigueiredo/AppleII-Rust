//! Joystick sources (AppleWin Joystick.cpp): keyboard numpad / cursors, host gamepad (gilrs),
//! mouse. All of them reduce to a [`JoyOutput`]: two paddle axes and two buttons per stick.

use crate::keymap::{HostKey, KeyInput};

/// Paddle centre as AppleWin's `PDL_CENTRAL`.
pub const CENTRE: u8 = 127;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum JoyMode {
    /// No joystick attached (the ][ family then reads PB0/PB1 as pressed).
    #[default]
    Disabled,
    /// Numpad 1-9 directions, 0/Ins = button 0, ./Del = button 1.
    KeyboardNumpad,
    /// Dedicated cursor keys as directions (Alt keys are the buttons).
    KeyboardCursors,
    Gamepad,
    Mouse,
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

/// Keyboard-as-joystick state.
#[derive(Debug, Default)]
pub struct KeyJoy {
    pub mode: JoyMode,
    /// Numpad directions 1-9 held (index 0 unused).
    pad: [bool; 10],
    cursors: [bool; 4], // left, up, right, down
    buttons: [bool; 2],
}

impl KeyJoy {
    pub fn new(mode: JoyMode) -> Self {
        KeyJoy { mode, ..Default::default() }
    }

    /// Feeds a key transition; true if the key belongs to the joystick (not for the Apple).
    pub fn process(&mut self, input: KeyInput, pressed: bool) -> bool {
        match self.mode {
            JoyMode::KeyboardNumpad if input.numpad => self.numpad(input.key, pressed),
            JoyMode::KeyboardCursors if !input.numpad => match input.key {
                HostKey::Left => self.set_cursor(0, pressed),
                HostKey::Up => self.set_cursor(1, pressed),
                HostKey::Right => self.set_cursor(2, pressed),
                HostKey::Down => self.set_cursor(3, pressed),
                _ => false,
            },
            _ => false,
        }
    }

    fn set_cursor(&mut self, i: usize, v: bool) -> bool {
        self.cursors[i] = v;
        true
    }

    fn numpad(&mut self, key: HostKey, pressed: bool) -> bool {
        let digit = |d: u8, s: &mut Self| {
            s.pad[usize::from(d)] = pressed;
            true
        };
        match key {
            HostKey::Char(c @ '1'..='9') => digit(c as u8 - b'0', self),
            HostKey::End => digit(1, self),
            HostKey::Down => digit(2, self),
            HostKey::PageDown => digit(3, self),
            HostKey::Left => digit(4, self),
            HostKey::Clear => digit(5, self),
            HostKey::Right => digit(6, self),
            HostKey::Home => digit(7, self),
            HostKey::Up => digit(8, self),
            HostKey::PageUp => digit(9, self),
            HostKey::Char('0') | HostKey::Insert => {
                self.buttons[0] = pressed;
                true
            }
            HostKey::Char('.') | HostKey::Char(',') | HostKey::Delete => {
                self.buttons[1] = pressed;
                true
            }
            _ => false,
        }
    }

    pub fn release_all(&mut self) {
        self.pad = [false; 10];
        self.cursors = [false; 4];
        self.buttons = [false; 2];
    }

    pub fn output(&self) -> JoyOutput {
        let axis = |neg: bool, pos: bool| match (neg, pos) {
            (true, false) => 0,
            (false, true) => 255,
            _ => CENTRE,
        };
        let p = &self.pad;
        let left = p[1] || p[4] || p[7] || self.cursors[0];
        let right = p[3] || p[6] || p[9] || self.cursors[2];
        let up = p[7] || p[8] || p[9] || self.cursors[1];
        let down = p[1] || p[2] || p[3] || self.cursors[3];
        JoyOutput { x: axis(left, right), y: axis(up, down), buttons: self.buttons }
    }
}

/// Gamepad axis in `-1.0..=1.0` to a paddle value, with a dead zone around the centre.
pub fn axis_to_paddle(v: f32, deadzone: f32) -> u8 {
    let v = v.clamp(-1.0, 1.0);
    if v.abs() < deadzone {
        return CENTRE;
    }
    (((v + 1.0) * 0.5) * 255.0).round() as u8
}

/// Mouse position inside the Apple viewport (`0..size`) to a paddle value.
pub fn mouse_to_paddle(pos: f32, size: f32) -> u8 {
    if size <= 0.0 {
        return CENTRE;
    }
    ((pos / size).clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn np(c: char) -> KeyInput {
        KeyInput::numpad(HostKey::Char(c))
    }

    #[test]
    fn numpad_directions_and_buttons() {
        let mut j = KeyJoy::new(JoyMode::KeyboardNumpad);
        assert!(j.process(np('7'), true));
        assert_eq!((j.output().x, j.output().y), (0, 0));
        j.process(np('7'), false);
        j.process(np('3'), true);
        assert_eq!((j.output().x, j.output().y), (255, 255));
        j.process(np('0'), true);
        assert_eq!(j.output().buttons, [true, false]);
        assert_eq!(j.output().buttons, [true, false]);
    }

    #[test]
    fn numlock_off_keys_work_too() {
        let mut j = KeyJoy::new(JoyMode::KeyboardNumpad);
        assert!(j.process(KeyInput::numpad(HostKey::Left), true));
        assert_eq!(j.output().x, 0);
    }

    #[test]
    fn cursor_mode_only_takes_dedicated_cursors() {
        let mut j = KeyJoy::new(JoyMode::KeyboardCursors);
        assert!(!j.process(np('4'), true));
        assert!(j.process(KeyInput::new(HostKey::Right), true));
        assert_eq!(j.output().x, 255);
        assert!(!j.process(KeyInput::new(HostKey::Char('a')), true));
    }

    #[test]
    fn disabled_mode_takes_nothing() {
        let mut j = KeyJoy::new(JoyMode::Disabled);
        assert!(!j.process(np('4'), true));
    }

    #[test]
    fn axes_map_to_paddles() {
        assert_eq!(axis_to_paddle(0.0, 0.1), CENTRE);
        assert_eq!(axis_to_paddle(0.05, 0.1), CENTRE);
        assert_eq!(axis_to_paddle(-1.0, 0.1), 0);
        assert_eq!(axis_to_paddle(1.0, 0.1), 255);
        assert_eq!(mouse_to_paddle(100.0, 200.0), 128);
        assert_eq!(mouse_to_paddle(-5.0, 200.0), 0);
    }
}
