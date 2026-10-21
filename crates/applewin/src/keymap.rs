//! Host keyboard -> Apple II key events (AppleWin Keyboard.cpp `KeybQueueKeypress` /
//! `KeybAnyKeyDown`, WinFrame.cpp `WM_KEYDOWN`).
//!
//! The winit-specific conversion lives in [`crate::host`]; everything here works on the
//! framework-neutral [`HostKey`] so it can be tested. Keys are mapped by *character* (what
//! `WM_CHAR` carries on the user's layout), never by scancode; the per-model rules (case, which
//! characters exist, Pravets / TK3000 remaps) live in `apple2_core::Keyboard`.

use apple2_core::NavKey;
pub use apple2_debugger::keys::{Key as DbgKey, KeyMods as DbgMods};
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
    ControlLeft,
    ControlRight,
    Other,
}

/// A key event as the shell sees it. `numpad` marks the numeric keypad (non-"extended" in
/// Windows terms), which the keyboard-as-joystick modes care about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyInput {
    pub key: HostKey,
    pub numpad: bool,
    /// Identity of the physical key (0 = unknown). Press and release of one key can carry
    /// different characters (Shift released first), so AKD tracking goes by this when known.
    pub phys: u32,
}

impl KeyInput {
    pub fn new(key: HostKey) -> Self {
        KeyInput {
            key,
            numpad: false,
            phys: 0,
        }
    }
    pub fn numpad(key: HostKey) -> Self {
        KeyInput {
            key,
            numpad: true,
            phys: 0,
        }
    }
    pub fn with_phys(mut self, phys: u32) -> Self {
        self.phys = phys;
        self
    }

    /// What identifies this key between its press and its release.
    fn id(self) -> (u32, bool, HostKey) {
        if self.phys != 0 {
            return (self.phys, self.numpad, HostKey::Other);
        }
        let key = match self.key {
            HostKey::Char(c) => HostKey::Char(c.to_ascii_lowercase()),
            k => k,
        };
        (0, self.numpad, key)
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
    /// Open/Closed Apple are game-port buttons: [`crate::joy::JoySystem`] owns the Alt keys, so
    /// [`KeyboardState`] never produces these two (the machine still understands them).
    OpenApple(bool),
    ClosedApple(bool),
    /// "Any key down" line (AKD, $C010 bit 7 on the //e) changed.
    AnyKeyDown(bool),
    CapsLock(bool),
    /// Ctrl+Break (Ctrl+Pause): soft reset.
    Reset,
    /// Shift+Insert or Ctrl+Shift+V (plain Ctrl+V is ^V for the Apple, as in AppleWin).
    Paste,
    /// Scroll Lock flipped: the TK3000 //e |Mode| key (accented characters) follows it.
    ScrollLock(bool),
}

/// Maps a layout-translated character plus modifiers to the Apple ASCII byte `WM_CHAR` would
/// carry: Ctrl+letter gives the control code, characters above Latin-1 are dropped. Ctrl with
/// Alt is AltGr on Windows (GH#749: "AltGr also fakes Ctrl") and types the plain character.
pub fn char_to_ascii(c: char, mods: Mods) -> Option<u8> {
    if mods.ctrl && !mods.alt {
        let u = c.to_ascii_uppercase();
        return match u {
            '@'..='_' => Some(u as u8 & 0x1F),
            ' ' => Some(b' '), // Ctrl+Space stays a plain space through TranslateMessage
            _ => None,
        };
    }
    let code = c as u32;
    (code != 0 && code <= 0xFF).then_some(code as u8)
}

/// Keys the debugger understands (`DebuggerProcessKey`, Debug.cpp:9653): `Char` is a typed
/// character; `Press` is a key with its modifiers, which navigates or edits depending on whether
/// the command line is empty (the rules live in `apple2_debugger::keys`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugKey {
    Char(char),
    Press(DbgKey, DbgMods),
}

impl DebugKey {
    /// A key without modifiers.
    pub fn plain(k: DbgKey) -> Self {
        DebugKey::Press(k, DbgMods::default())
    }
}

/// US-layout shifted digits: Ctrl+Shift+1 reports `!` but AppleWin looks at the key code.
fn unshifted_digit(c: char) -> Option<char> {
    match c {
        '0'..='9' => Some(c),
        ')' => Some('0'),
        '!' => Some('1'),
        '@' => Some('2'),
        '#' => Some('3'),
        '$' => Some('4'),
        '%' => Some('5'),
        '^' => Some('6'),
        '&' => Some('7'),
        '*' => Some('8'),
        '(' => Some('9'),
        _ => None,
    }
}

/// Debugger input for a key press while the debugger is on screen.
pub fn debug_key(key: HostKey, mods: Mods) -> Option<DebugKey> {
    let m = DbgMods {
        shift: mods.shift,
        ctrl: mods.ctrl,
        alt: mods.alt,
    };
    let k = match key {
        HostKey::Char(c) if mods.ctrl => {
            let c = if mods.shift {
                unshifted_digit(c).unwrap_or(c)
            } else {
                c
            };
            if c == ' ' || c.is_ascii_digit() || matches!(c, '~' | '`' | '\u{ac}') {
                DbgKey::Char(c)
            } else {
                return None;
            }
        }
        HostKey::Char(' ') => DbgKey::Char(' '),
        HostKey::Char(c) if !c.is_control() => return Some(DebugKey::Char(c)),
        HostKey::Enter => DbgKey::Enter,
        HostKey::Backspace | HostKey::Delete => DbgKey::Backspace,
        HostKey::Escape => DbgKey::Escape,
        HostKey::Tab => DbgKey::Tab,
        HostKey::Home => DbgKey::Home,
        HostKey::End => DbgKey::End,
        HostKey::Up => DbgKey::Up,
        HostKey::Down => DbgKey::Down,
        HostKey::Left => DbgKey::Left,
        HostKey::Right => DbgKey::Right,
        HostKey::PageUp => DbgKey::PageUp,
        HostKey::PageDown => DbgKey::PageDown,
        _ => return None,
    };
    Some(DebugKey::Press(k, m))
}

/// The wheel over the debugger: up is `VK_UP`, down is `VK_DOWN`, keeping the modifiers.
pub fn wheel_debug_key(dy: f32, mods: Mods) -> Option<DebugKey> {
    let k = match dy {
        d if d > 0.0 => DbgKey::Up,
        d if d < 0.0 => DbgKey::Down,
        _ => return None,
    };
    Some(DebugKey::Press(
        k,
        DbgMods {
            shift: mods.shift,
            ctrl: mods.ctrl,
            alt: mods.alt,
        },
    ))
}

/// Stateful translator: tracks held keys (for AKD), the Caps Lock and Scroll Lock toggles.
#[derive(Debug)]
pub struct KeyboardState {
    held: HashSet<(u32, bool, HostKey)>,
    /// The Apple's Caps Lock (starts on regardless of the host, AppleWin keyboard.html).
    caps: bool,
    /// The host's Caps Lock as far as we can know: assumed off at start-up.
    host_caps: bool,
    scroll: bool,
    any_down: bool,
}

impl Default for KeyboardState {
    fn default() -> Self {
        KeyboardState::with_caps(true)
    }
}

impl KeyboardState {
    pub fn new() -> Self {
        Self::default()
    }

    /// `caps`: the Apple's initial Caps Lock (`-capslock=off` starts it off).
    pub fn with_caps(caps: bool) -> Self {
        KeyboardState {
            held: HashSet::new(),
            caps,
            host_caps: false,
            scroll: false,
            any_down: false,
        }
    }

    pub fn caps_lock(&self) -> bool {
        self.caps
    }

    /// Releases everything (window lost focus): keeps stuck keys from lingering.
    pub fn release_all(&mut self) -> Vec<KeyEvent> {
        self.held.clear();
        if self.any_down {
            self.any_down = false;
            return vec![KeyEvent::AnyKeyDown(false)];
        }
        Vec::new()
    }

    /// Processes one key transition. `repeat` is the OS auto-repeat flag.
    pub fn handle(
        &mut self,
        input: KeyInput,
        pressed: bool,
        repeat: bool,
        mods: Mods,
    ) -> Vec<KeyEvent> {
        let mut out = Vec::new();
        match input.key {
            // Game-port buttons and modifiers produce no Apple key (see `joy`).
            HostKey::AltLeft
            | HostKey::AltRight
            | HostKey::Shift
            | HostKey::ControlLeft
            | HostKey::ControlRight => {}
            HostKey::CapsLock => {
                // AppleWin copies the host's toggle (`GetKeyState(VK_CAPITAL) & 1`): the first
                // press with the host's lock off turns the host's on and leaves the Apple's on.
                if pressed && !repeat {
                    self.host_caps = !self.host_caps;
                    self.caps = self.host_caps;
                    out.push(KeyEvent::CapsLock(self.caps));
                }
            }
            HostKey::ScrollLock => {
                if pressed && !repeat {
                    self.scroll = !self.scroll;
                    out.push(KeyEvent::ScrollLock(self.scroll));
                }
            }
            HostKey::Pause if pressed && mods.ctrl => out.push(KeyEvent::Reset),
            HostKey::Insert if pressed && mods.shift => out.push(KeyEvent::Paste),
            HostKey::Char('v' | 'V') if pressed && mods.ctrl && mods.shift && !mods.alt => {
                out.push(KeyEvent::Paste)
            }
            _ if pressed => {
                if let Some(ev) = self.press_event(input.key, mods) {
                    out.push(ev);
                    self.held.insert(input.id());
                }
            }
            _ => {
                self.held.remove(&input.id());
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
        let ctrl = mods.ctrl && !mods.alt;
        Some(match key {
            HostKey::Char(c) => KeyEvent::Ascii(char_to_ascii(c, mods)?),
            HostKey::Enter if ctrl => KeyEvent::Ascii(0x0A), // Ctrl+Enter is LF
            HostKey::Enter => KeyEvent::Ascii(0x0D),
            HostKey::Backspace if ctrl => KeyEvent::Ascii(0x7F), // Ctrl+Backspace is DEL
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
    use apple2_core::keyboard::Keyboard;
    use apple2_core::Model;

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

    fn press(k: &mut KeyboardState, key: HostKey, mods: Mods) -> Vec<KeyEvent> {
        k.handle(KeyInput::new(key), true, false, mods)
    }

    #[test]
    fn plain_and_shifted_characters_pass_through() {
        let mut k = KeyboardState::new();
        let ev = press(&mut k, HostKey::Char('A'), SHIFT);
        assert_eq!(ev, vec![KeyEvent::Ascii(b'A'), KeyEvent::AnyKeyDown(true)]);
    }

    #[test]
    fn ctrl_letters_become_control_codes() {
        assert_eq!(char_to_ascii('c', CTRL), Some(3));
        assert_eq!(char_to_ascii('G', CTRL), Some(7));
        assert_eq!(char_to_ascii('[', CTRL), Some(0x1B));
        assert_eq!(char_to_ascii('\\', CTRL), Some(0x1C));
        assert_eq!(char_to_ascii(']', CTRL), Some(0x1D));
        assert_eq!(char_to_ascii('^', CTRL), Some(0x1E));
        assert_eq!(char_to_ascii('_', CTRL), Some(0x1F));
        assert_eq!(char_to_ascii('@', CTRL), Some(0));
        assert_eq!(char_to_ascii('5', CTRL), None);
        assert_eq!(char_to_ascii('é', NONE), Some(0xE9));
        assert_eq!(char_to_ascii('€', NONE), None);
        assert_eq!(char_to_ascii(' ', CTRL), Some(b' '));
    }

    #[test]
    fn every_ctrl_letter_maps_to_1_through_26() {
        for (i, c) in ('a'..='z').enumerate() {
            assert_eq!(char_to_ascii(c, CTRL), Some(i as u8 + 1), "Ctrl+{c}");
            assert_eq!(
                char_to_ascii(c.to_ascii_uppercase(), CTRL),
                Some(i as u8 + 1)
            );
        }
    }

    #[test]
    fn altgr_is_not_ctrl() {
        // Windows reports AltGr as Ctrl+Alt: a layout character such as '@' must stay '@'.
        let altgr = Mods {
            shift: false,
            ctrl: true,
            alt: true,
        };
        assert_eq!(char_to_ascii('@', altgr), Some(b'@'));
        assert_eq!(char_to_ascii('q', altgr), Some(b'q'));
    }

    #[test]
    fn every_printable_ascii_reaches_the_machine_unchanged() {
        for b in 0x20u8..=0x7E {
            let mut k = KeyboardState::new();
            let m = if b.is_ascii_uppercase() || b"~!@#$%^&*()_+{}|:\"<>?".contains(&b) {
                SHIFT
            } else {
                NONE
            };
            let ev = press(&mut k, HostKey::Char(b as char), m);
            assert_eq!(ev[0], KeyEvent::Ascii(b), "{:?}", b as char);
        }
    }

    #[test]
    fn special_keys() {
        let mut k = KeyboardState::new();
        assert!(press(&mut k, HostKey::Enter, NONE).contains(&KeyEvent::Ascii(0x0D)));
        assert!(press(&mut k, HostKey::Escape, NONE).contains(&KeyEvent::Ascii(0x1B)));
        assert!(press(&mut k, HostKey::Tab, NONE).contains(&KeyEvent::Ascii(0x09)));
        assert!(press(&mut k, HostKey::Backspace, NONE).contains(&KeyEvent::Ascii(0x08)));
        assert!(press(&mut k, HostKey::Left, NONE).contains(&KeyEvent::Nav(NavKey::Left)));
        assert!(press(&mut k, HostKey::Delete, NONE).contains(&KeyEvent::Nav(NavKey::Delete)));
        assert!(press(&mut k, HostKey::Enter, CTRL).contains(&KeyEvent::Ascii(0x0A)));
        assert!(press(&mut k, HostKey::Backspace, CTRL).contains(&KeyEvent::Ascii(0x7F)));
    }

    #[test]
    fn keys_the_apple_does_not_have_are_dropped() {
        let mut k = KeyboardState::new();
        for key in [
            HostKey::Home,
            HostKey::End,
            HostKey::PageUp,
            HostKey::PageDown,
            HostKey::Insert,
            HostKey::F(5),
            HostKey::Clear,
        ] {
            assert_eq!(press(&mut k, key, NONE), vec![], "{key:?}");
        }
    }

    #[test]
    fn alt_and_modifier_keys_are_not_apple_keys() {
        let mut k = KeyboardState::new();
        let alt = Mods { alt: true, ..NONE };
        for key in [
            HostKey::AltLeft,
            HostKey::AltRight,
            HostKey::Shift,
            HostKey::ControlLeft,
            HostKey::ControlRight,
        ] {
            assert_eq!(press(&mut k, key, alt), vec![], "{key:?}");
        }
    }

    #[test]
    fn ctrl_break_resets_and_paste_shortcuts() {
        let mut k = KeyboardState::new();
        assert_eq!(press(&mut k, HostKey::Pause, CTRL), vec![KeyEvent::Reset]);
        assert_eq!(press(&mut k, HostKey::Insert, SHIFT), vec![KeyEvent::Paste]);
        let both = Mods {
            shift: true,
            ctrl: true,
            alt: false,
        };
        assert_eq!(
            press(&mut k, HostKey::Char('V'), both),
            vec![KeyEvent::Paste]
        );
        // plain Ctrl+V is ^V for the Apple
        assert_eq!(
            press(&mut k, HostKey::Char('v'), CTRL)[0],
            KeyEvent::Ascii(0x16)
        );
    }

    #[test]
    fn akd_follows_the_held_keys() {
        let mut k = KeyboardState::new();
        let a = KeyInput::new(HostKey::Char('a'));
        let b = KeyInput::new(HostKey::Char('b'));
        assert!(k
            .handle(a, true, false, NONE)
            .contains(&KeyEvent::AnyKeyDown(true)));
        assert_eq!(k.handle(b, true, false, NONE), vec![KeyEvent::Ascii(b'b')]);
        assert_eq!(k.handle(a, false, false, NONE), vec![]);
        assert_eq!(
            k.handle(b, false, false, NONE),
            vec![KeyEvent::AnyKeyDown(false)]
        );
    }

    #[test]
    fn akd_survives_shift_released_before_the_key() {
        // Press Shift+A, release Shift, release A: the release carries 'a', not 'A'.
        let mut k = KeyboardState::new();
        let down = KeyInput::new(HostKey::Char('A')).with_phys(30);
        let up = KeyInput::new(HostKey::Char('a')).with_phys(30);
        assert!(k
            .handle(down, true, false, SHIFT)
            .contains(&KeyEvent::AnyKeyDown(true)));
        assert_eq!(
            k.handle(up, false, false, NONE),
            vec![KeyEvent::AnyKeyDown(false)]
        );
        // Without a physical id the letter case is folded.
        let down = KeyInput::new(HostKey::Char('B'));
        k.handle(down, true, false, SHIFT);
        assert_eq!(
            k.handle(KeyInput::new(HostKey::Char('b')), false, false, NONE),
            vec![KeyEvent::AnyKeyDown(false)]
        );
    }

    #[test]
    fn autorepeat_keeps_typing_without_changing_akd() {
        let mut k = KeyboardState::new();
        let a = KeyInput::new(HostKey::Char('a'));
        k.handle(a, true, false, NONE);
        assert_eq!(k.handle(a, true, true, NONE), vec![KeyEvent::Ascii(b'a')]);
    }

    #[test]
    fn caps_lock_follows_the_host_toggle() {
        // AppleWin starts with the Apple's caps on; the first host press (host lock off -> on)
        // keeps it on, the second turns it off.
        let mut k = KeyboardState::new();
        assert!(k.caps_lock());
        assert_eq!(
            press(&mut k, HostKey::CapsLock, NONE),
            vec![KeyEvent::CapsLock(true)]
        );
        k.handle(KeyInput::new(HostKey::CapsLock), false, false, NONE);
        assert_eq!(
            press(&mut k, HostKey::CapsLock, NONE),
            vec![KeyEvent::CapsLock(false)]
        );
        // held key does not toggle again
        assert_eq!(
            k.handle(KeyInput::new(HostKey::CapsLock), true, true, NONE),
            vec![]
        );
        let off = KeyboardState::with_caps(false);
        assert!(!off.caps_lock());
    }

    #[test]
    fn scroll_lock_is_the_tk3000_mode_key() {
        let mut k = KeyboardState::new();
        assert_eq!(
            press(&mut k, HostKey::ScrollLock, NONE),
            vec![KeyEvent::ScrollLock(true)]
        );
        k.handle(KeyInput::new(HostKey::ScrollLock), false, false, NONE);
        assert_eq!(
            press(&mut k, HostKey::ScrollLock, NONE),
            vec![KeyEvent::ScrollLock(false)]
        );
    }

    #[test]
    fn debugger_console_keys() {
        assert_eq!(
            debug_key(HostKey::Char('g'), NONE),
            Some(DebugKey::Char('g'))
        );
        assert_eq!(
            debug_key(HostKey::Enter, NONE),
            Some(DebugKey::plain(DbgKey::Enter))
        );
        // Up/Down move the disassembly cursor in AppleWin (Debug.cpp:9989), not the history
        assert_eq!(
            debug_key(HostKey::Up, NONE),
            Some(DebugKey::plain(DbgKey::Up))
        );
        assert_eq!(
            debug_key(HostKey::Char(' '), NONE),
            Some(DebugKey::plain(DbgKey::Char(' ')))
        );
        assert_eq!(debug_key(HostKey::F(3), NONE), None);
        assert_eq!(debug_key(HostKey::Char('c'), CTRL), None);
    }

    #[test]
    fn focus_loss_releases_everything() {
        let mut k = KeyboardState::new();
        press(&mut k, HostKey::Char('a'), NONE);
        assert_eq!(k.release_all(), vec![KeyEvent::AnyKeyDown(false)]);
        assert_eq!(k.release_all(), vec![]);
    }

    // ---- per-model table: shell translation + the core's model rules ----

    /// Types `key` through the shell and the core and returns the Apple keycode, if any.
    fn typed(model: Model, caps: bool, ev: KeyEvent) -> Option<u8> {
        let mut kb = Keyboard::default();
        kb.set_caps_lock(caps);
        let ok = match ev {
            KeyEvent::Ascii(c) => kb.press(model, c),
            KeyEvent::Nav(n) => kb.press_nav(model, n),
            _ => return None,
        };
        ok.then(|| kb.keycode())
    }

    /// Independent statement of AppleWin's rules for the unmodified models (Keyboard.cpp:104-200).
    fn expected_plain(model: Model, caps: bool, key: u8) -> Option<u8> {
        match model {
            Model::Apple2 | Model::Apple2Plus | Model::Apple2JPlus => {
                if key == b'`' || key >= b'{' {
                    None
                } else {
                    Some(key.to_ascii_uppercase())
                }
            }
            Model::Apple2e | Model::Apple2eEnhanced => {
                if key > 0x7F {
                    None
                } else if caps && key.is_ascii_lowercase() {
                    Some(key - 32)
                } else {
                    Some(key)
                }
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn printable_ascii_per_model_matches_applewin() {
        let models = [
            Model::Apple2,
            Model::Apple2Plus,
            Model::Apple2JPlus,
            Model::Apple2e,
            Model::Apple2eEnhanced,
        ];
        for model in models {
            for caps in [true, false] {
                for key in 0x20u8..=0x7E {
                    let mods = if key.is_ascii_uppercase() {
                        SHIFT
                    } else {
                        NONE
                    };
                    let mut k = KeyboardState::new();
                    let ev = press(&mut k, HostKey::Char(key as char), mods)[0];
                    assert_eq!(
                        typed(model, caps, ev),
                        expected_plain(model, caps, key),
                        "{model:?} caps={caps} {:?}",
                        key as char
                    );
                }
            }
        }
    }

    #[test]
    fn navigation_keys_per_model() {
        // (key, Apple ][ family, //e family) from AppleWin's `asciicode` table.
        let table = [
            (NavKey::Left, Some(0x08), Some(0x08)),
            (NavKey::Right, Some(0x15), Some(0x15)),
            (NavKey::Up, None, Some(0x0B)),
            (NavKey::Down, None, Some(0x0A)),
            (NavKey::Delete, None, Some(0x7F)),
        ];
        for (nav, ii, iie) in table {
            assert_eq!(
                typed(Model::Apple2Plus, true, KeyEvent::Nav(nav)),
                ii,
                "{nav:?} on ][+"
            );
            assert_eq!(
                typed(Model::Apple2e, true, KeyEvent::Nav(nav)),
                iie,
                "{nav:?} on //e"
            );
            assert_eq!(
                typed(Model::Apple2eEnhanced, true, KeyEvent::Nav(nav)),
                iie,
                "{nav:?} on enhanced //e"
            );
        }
    }

    #[test]
    fn accented_characters_only_exist_on_clones() {
        // 'é' (0xE9): rejected on the //e, accepted (masked to 7 bits) on a Pravets clone.
        let ev = KeyEvent::Ascii(char_to_ascii('é', NONE).unwrap());
        assert_eq!(typed(Model::Apple2e, true, ev), None);
        assert!(typed(Model::Pravets82, true, ev).is_some());
    }

    #[test]
    fn control_codes_reach_every_model() {
        for model in [
            Model::Apple2Plus,
            Model::Apple2e,
            Model::Apple2eEnhanced,
            Model::Pravets82,
        ] {
            let ev = press(&mut KeyboardState::new(), HostKey::Char('c'), CTRL)[0];
            assert_eq!(typed(model, true, ev), Some(3), "{model:?}");
        }
    }
}
