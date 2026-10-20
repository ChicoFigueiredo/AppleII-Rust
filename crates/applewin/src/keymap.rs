//! Host keyboard -> Apple II key events (AppleWin Keyboard.cpp `KeybQueueKeypress` /
//! `KeybAnyKeyDown`, WinFrame.cpp `WM_KEYDOWN`).
//!
//! The winit-specific conversion lives in [`crate::host`]; everything here works on the
//! framework-neutral [`HostKey`] so it can be tested.

use apple2_core::NavKey;
use std::collections::HashSet;

/// A host key, already layout-translated (`Char` carries the shifted character).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HostKey {
    Char(char),
    Enter,
    Backspace,
    Tab,
    Escape,
    Left,
    Up,
    Right,
    Down,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Clear,
    F(u8),
    Pause,
    PrintScreen,
    ScrollLock,
    CapsLock,
    NumLock,
    /// Left Alt = Open Apple (joystick button 0).
    AltLeft,
    /// Right Alt (AltGr) = Closed Apple (joystick button 1).
    AltRight,
    Shift,
    Control,
    Other,
}

/// A key event as the shell sees it. `numpad` marks the numeric keypad (non-"extended" in
/// Windows terms), which the keyboard-as-joystick modes care about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyInput {
    pub key: HostKey,
    pub numpad: bool,
}

impl KeyInput {
    pub fn new(key: HostKey) -> Self {
        KeyInput { key, numpad: false }
    }
    pub fn numpad(key: HostKey) -> Self {
        KeyInput { key, numpad: true }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// What the machine should do in response to a host key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEvent {
    Ascii(u8),
    Nav(NavKey),
    OpenApple(bool),
    ClosedApple(bool),
    /// "Any key down" line (AKD, $C010 bit 7 on the //e) changed.
    AnyKeyDown(bool),
    CapsLock(bool),
    /// Ctrl+Break (Ctrl+Pause): soft reset.
    Reset,
    /// Shift+Insert or Ctrl+V.
    Paste,
}

/// Maps a layout-translated character plus modifiers to the Apple ASCII byte `WM_CHAR` would
/// carry: Ctrl+letter gives the control code, characters above Latin-1 are dropped.
pub fn char_to_ascii(c: char, mods: Mods) -> Option<u8> {
    if mods.ctrl {
        let u = c.to_ascii_uppercase();
        return match u {
            '@'..='_' => Some(u as u8 & 0x1F),
            ' ' => Some(0),
            _ => None,
        };
    }
    let code = c as u32;
    (code != 0 && code <= 0xFF).then_some(code as u8)
}

/// Stateful translator: tracks held keys (for AKD) and the caps-lock toggle.
#[derive(Debug, Default)]
pub struct KeyboardState {
    held: HashSet<KeyInput>,
    caps: bool,
    any_down: bool,
}

impl KeyboardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn caps_lock(&self) -> bool {
        self.caps
    }

    /// Releases everything (window lost focus): keeps stuck keys and buttons from lingering.
    pub fn release_all(&mut self) -> Vec<KeyEvent> {
        let mut out = vec![KeyEvent::OpenApple(false), KeyEvent::ClosedApple(false)];
        self.held.clear();
        if self.any_down {
            self.any_down = false;
            out.push(KeyEvent::AnyKeyDown(false));
        }
        out
    }

    /// Processes one key transition. `repeat` is the OS auto-repeat flag.
    pub fn handle(&mut self, input: KeyInput, pressed: bool, repeat: bool, mods: Mods) -> Vec<KeyEvent> {
        let mut out = Vec::new();
        match input.key {
            HostKey::AltLeft => out.push(KeyEvent::OpenApple(pressed)),
            HostKey::AltRight => out.push(KeyEvent::ClosedApple(pressed)),
            HostKey::CapsLock => {
                if pressed && !repeat {
                    self.caps = !self.caps;
                    out.push(KeyEvent::CapsLock(self.caps));
                }
            }
            HostKey::Pause if pressed && mods.ctrl => out.push(KeyEvent::Reset),
            HostKey::Insert if pressed && mods.shift => out.push(KeyEvent::Paste),
            HostKey::Char('v' | 'V') if pressed && mods.ctrl && !mods.alt => out.push(KeyEvent::Paste),
            _ if pressed => {
                if let Some(ev) = self.press_event(input.key, mods) {
                    out.push(ev);
                    self.held.insert(input);
                }
            }
            _ => {
                self.held.remove(&input);
            }
        }
        let any = !self.held.is_empty();
        if any != self.any_down {
            self.any_down = any;
            out.push(KeyEvent::AnyKeyDown(any));
        }
        out
    }

    fn press_event(&self, key: HostKey, mods: Mods) -> Option<KeyEvent> {
        Some(match key {
            HostKey::Char(c) => KeyEvent::Ascii(char_to_ascii(c, mods)?),
            HostKey::Enter => KeyEvent::Ascii(0x0D),
            HostKey::Backspace => KeyEvent::Ascii(0x08),
            HostKey::Tab => KeyEvent::Ascii(0x09),
            HostKey::Escape => KeyEvent::Ascii(0x1B),
            HostKey::Left => KeyEvent::Nav(NavKey::Left),
            HostKey::Up => KeyEvent::Nav(NavKey::Up),
            HostKey::Right => KeyEvent::Nav(NavKey::Right),
            HostKey::Down => KeyEvent::Nav(NavKey::Down),
            HostKey::Delete => KeyEvent::Nav(NavKey::Delete),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(k: &mut KeyboardState, key: HostKey, mods: Mods) -> Vec<KeyEvent> {
        k.handle(KeyInput::new(key), true, false, mods)
    }

    #[test]
    fn plain_and_shifted_characters_pass_through() {
        let mut k = KeyboardState::new();
        let ev = press(&mut k, HostKey::Char('A'), Mods { shift: true, ..Mods::default() });
        assert_eq!(ev, vec![KeyEvent::Ascii(b'A'), KeyEvent::AnyKeyDown(true)]);
    }

    #[test]
    fn ctrl_letters_become_control_codes() {
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        assert_eq!(char_to_ascii('c', ctrl), Some(3));
        assert_eq!(char_to_ascii('G', ctrl), Some(7));
        assert_eq!(char_to_ascii('[', ctrl), Some(0x1B));
        assert_eq!(char_to_ascii('é', Mods::default()), Some(0xE9));
        assert_eq!(char_to_ascii('€', Mods::default()), None);
    }

    #[test]
    fn special_keys() {
        let mut k = KeyboardState::new();
        let m = Mods::default();
        assert!(press(&mut k, HostKey::Enter, m).contains(&KeyEvent::Ascii(0x0D)));
        assert!(press(&mut k, HostKey::Escape, m).contains(&KeyEvent::Ascii(0x1B)));
        assert!(press(&mut k, HostKey::Left, m).contains(&KeyEvent::Nav(NavKey::Left)));
        assert!(press(&mut k, HostKey::Delete, m).contains(&KeyEvent::Nav(NavKey::Delete)));
    }

    #[test]
    fn alt_keys_are_open_and_closed_apple() {
        let mut k = KeyboardState::new();
        let m = Mods { alt: true, ..Mods::default() };
        assert_eq!(press(&mut k, HostKey::AltLeft, m), vec![KeyEvent::OpenApple(true)]);
        assert_eq!(k.handle(KeyInput::new(HostKey::AltRight), true, false, m), vec![KeyEvent::ClosedApple(true)]);
        assert_eq!(k.handle(KeyInput::new(HostKey::AltLeft), false, false, m), vec![KeyEvent::OpenApple(false)]);
    }

    #[test]
    fn ctrl_break_resets_and_paste_shortcuts() {
        let mut k = KeyboardState::new();
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        assert_eq!(press(&mut k, HostKey::Pause, ctrl), vec![KeyEvent::Reset]);
        assert_eq!(press(&mut k, HostKey::Char('v'), ctrl), vec![KeyEvent::Paste]);
        let shift = Mods { shift: true, ..Mods::default() };
        assert_eq!(press(&mut k, HostKey::Insert, shift), vec![KeyEvent::Paste]);
    }

    #[test]
    fn akd_follows_the_held_keys() {
        let mut k = KeyboardState::new();
        let m = Mods::default();
        let a = KeyInput::new(HostKey::Char('a'));
        let b = KeyInput::new(HostKey::Char('b'));
        assert!(k.handle(a, true, false, m).contains(&KeyEvent::AnyKeyDown(true)));
        assert_eq!(k.handle(b, true, false, m), vec![KeyEvent::Ascii(b'b')]);
        assert_eq!(k.handle(a, false, false, m), vec![]);
        assert_eq!(k.handle(b, false, false, m), vec![KeyEvent::AnyKeyDown(false)]);
    }

    #[test]
    fn caps_lock_toggles() {
        let mut k = KeyboardState::new();
        let m = Mods::default();
        assert_eq!(press(&mut k, HostKey::CapsLock, m), vec![KeyEvent::CapsLock(true)]);
        k.handle(KeyInput::new(HostKey::CapsLock), false, false, m);
        assert_eq!(press(&mut k, HostKey::CapsLock, m), vec![KeyEvent::CapsLock(false)]);
    }

    #[test]
    fn focus_loss_releases_everything() {
        let mut k = KeyboardState::new();
        press(&mut k, HostKey::Char('a'), Mods::default());
        let ev = k.release_all();
        assert!(ev.contains(&KeyEvent::AnyKeyDown(false)) && ev.contains(&KeyEvent::OpenApple(false)));
    }
}
