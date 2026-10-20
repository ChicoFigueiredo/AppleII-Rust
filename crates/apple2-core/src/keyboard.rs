//! Keyboard latch, paste queue and clone key remaps (AppleWin Keyboard.cpp, Pravets.cpp).
//!
//! The core receives *host-independent* input: ANSI/Latin-1 characters as typed (AppleWin's
//! `WM_CHAR` path, [`Keyboard::press`]) and the five navigation keys ([`NavKey`], the
//! `WM_KEYDOWN` path). Left to the frontend: virtual-key/layout translation (Shift, Ctrl-letter
//! to control code, AltGr), Ctrl-Break reset, Shift+Insert paste, the Scroll Lock/Caps Lock
//! LEDs, and key auto-repeat (the core never repeats).

use crate::Model;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Cursor/edit keys that AppleWin maps from virtual keys (Keyboard.cpp:36-41 `asciicode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavKey {
    Left,
    Up,
    Right,
    Down,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyboard {
    /// `keycode`: last Apple key code (bit 7 clear).
    keycode: u8,
    /// `keywaiting`: the strobe.
    strobe: bool,
    /// `g_bCapsLock` (starts on). Latin/Cyrillic lock on the Pravets.
    caps_lock: bool,
    /// `g_bTK3KModeKey`.
    tk3000_mode: bool,
    /// Host reports at least one key held (`IsAKD`).
    any_key_down: bool,
    /// Clipboard characters still to deliver (AppleWin's `lptstr` cursor), CRLF already collapsed.
    paste: VecDeque<u8>,
    /// `Pravets::g_CapsLockAllowed` (Pravets 8A 8-bit keyboard mode, set by writes to $C060).
    pravets_8bit: bool,
    /// `Pravets::P8Shift`.
    pravets_shift: bool,
}

impl Default for Keyboard {
    fn default() -> Self {
        Keyboard {
            keycode: 0,
            strobe: false,
            caps_lock: true,
            tk3000_mode: false,
            any_key_down: false,
            paste: VecDeque::new(),
            pravets_8bit: false,
            pravets_shift: false,
        }
    }
}

fn is_pravets(model: Model) -> bool {
    matches!(model, Model::Pravets8M | Model::Pravets82 | Model::Pravets8A)
}

impl Keyboard {
    /// `KeybReset` + `Pravets::Reset` (Keyboard.cpp:63, Pravets.cpp:46). The paste queue and the
    /// host-driven state (caps lock, held keys) are kept.
    pub fn reset(&mut self) {
        self.keycode = 0;
        self.strobe = false;
        self.pravets_8bit = false;
    }

    pub fn set_caps_lock(&mut self, on: bool) {
        self.caps_lock = on;
    }

    pub fn set_tk3000_mode(&mut self, on: bool) {
        self.tk3000_mode = on;
    }

    pub fn set_any_key_down(&mut self, down: bool) {
        self.any_key_down = down;
    }

    /// `KeybGetKeycode`.
    pub fn keycode(&self) -> u8 {
        self.keycode
    }

    /// `KeybQueueKeypress(key, ASCII)` (Keyboard.cpp:104-200): the model's case/reject/remap rules,
    /// then sets the latch and the strobe. False if the model has no such key (the strobe is not
    /// set, although AppleWin may already have changed `keycode` in the TK3000 mode path).
    pub fn press(&mut self, model: Model, key: u8) -> bool {
        let clone = model.is_clone();
        if model == Model::Tk3000e && key > 0x7F && !self.tk3000_mode {
            return false;
        }
        if !model.is_apple2() {
            if !clone && key > 0x7F {
                return false; // accented chars, eg. AltGr+A
            }
            self.keycode = if self.caps_lock && key.is_ascii_lowercase() { key - 32 } else { key };
        } else {
            if !clone && (key == b'`' || key >= b'{') {
                return false;
            }
            self.keycode = key.to_ascii_uppercase();
        }
        if clone {
            self.keycode &= 0x7F;
            if is_pravets(model) {
                self.keycode = self.pravets_keycode(model, key, self.keycode);
            }
            if model == Model::Tk3000e && self.tk3000_mode && !self.tk3000_remap(key) {
                return false;
            }
        }
        self.strobe = true;
        true
    }

    /// The TK3000 |Mode| key remap (Keyboard.cpp:150-185). False if the key has no mapping.
    fn tk3000_remap(&mut self, mut key: u8) -> bool {
        if (0xC0..=0xDA).contains(&key) {
            key += 0x20; // uppercase to lowercase
        }
        key = match key {
            0xE0 => b'_',
            0xE1 => b'@',
            0xE2 => b'\\',
            0xE3 => b'[',
            0xE7 => b']',
            0xE9 => b'`',
            0xEA => b'&',
            0xED => b'{',
            0xF3 => b'~',
            0xF4 => b'}',
            0xF5 => b'#',
            0xFA => b'|',
            k => k,
        };
        if key > 0x7F {
            return false;
        }
        self.keycode = if key.is_ascii_lowercase() && self.caps_lock { key - 32 } else { key };
        true
    }

    /// `Pravets::ConvertToKeycode` (Pravets.cpp:105-204).
    fn pravets_keycode(&mut self, model: Model, key: u8, mut keycode: u8) -> u8 {
        let caps = self.caps_lock;
        self.pravets_shift = caps && key.is_ascii_lowercase();
        if !caps && key.is_ascii_uppercase() {
            self.pravets_shift = true;
            if model == Model::Pravets8A {
                keycode = key + 32;
            }
        }
        match model {
            Model::Pravets82 => {
                match key {
                    64 => keycode = 96,
                    b'^' => keycode = b'~',
                    _ => {}
                }
                if !caps {
                    match key {
                        b'`' => keycode = b'^',
                        92 => keycode = b'@',
                        124 => keycode = 92,
                        _ => {}
                    }
                } else {
                    match key {
                        91 => keycode = 123,
                        93 => keycode = 125,
                        124 => keycode = 92,
                        _ => {}
                    }
                }
            }
            Model::Pravets8M => {
                if !caps {
                    match key {
                        b'[' => keycode = b'{',
                        b']' => keycode = b'}',
                        b'`' => keycode = b'~',
                        92 => keycode = 96,
                        _ => {}
                    }
                } else if key == b'`' {
                    keycode = b'^';
                }
            }
            Model::Pravets8A => {
                let allowed = self.pravets_8bit;
                if !caps {
                    match key {
                        b'[' => keycode = b'{',
                        b']' => keycode = b'}',
                        b'`' => keycode = b'~',
                        92 => keycode = 96,
                        _ => {}
                    }
                    if allowed {
                        if key == 92 || key == 124 {
                            keycode = 96;
                        }
                        if matches!(key, b'{' | b'}' | b'~' | 124 | b'^' | 95) {
                            self.pravets_shift = true;
                        }
                    }
                } else if !allowed {
                    match key {
                        b'{' => keycode = b'[',
                        b'}' => keycode = b']',
                        124 => keycode = 92,
                        _ => {}
                    }
                } else {
                    match key {
                        b'{' => keycode = 91,
                        b'}' => keycode = 93,
                        124 => keycode = 92,
                        _ => {}
                    }
                    if matches!(key, b'[' | b']' | 92 | b'^' | 95) {
                        self.pravets_shift = true;
                    }
                    if key == 96 {
                        keycode = b'^';
                        self.pravets_shift = true;
                    }
                    if key == 126 {
                        keycode = b'^';
                    }
                }
            }
            _ => {}
        }
        keycode
    }

    /// Cursor/edit key (Keyboard.cpp:36-41 and :215-224). False if the model has no such key.
    pub fn press_nav(&mut self, model: Model, key: NavKey) -> bool {
        let code = match (key, model) {
            (NavKey::Left, _) => 0x08,
            (NavKey::Right, _) => 0x15,
            (NavKey::Up, m) if !m.is_apple2() => 0x0B,
            (NavKey::Down, m) if !m.is_apple2() => 0x0A,
            (NavKey::Delete, m) if !m.is_apple2() || m == Model::Base64A => 0x7F,
            _ => return false,
        };
        self.keycode = code;
        self.strobe = true;
        true
    }

    /// `ClipboardInitiatePaste`: queues `text` (CRLF becomes CR; non-ASCII and everything from a
    /// NUL on is dropped). False, and nothing queued, while a previous paste is still pending.
    pub fn paste(&mut self, text: &str) -> bool {
        if !self.paste.is_empty() {
            return false;
        }
        let mut bytes = text.bytes().take_while(|&b| b != 0).filter(u8::is_ascii).peekable();
        while let Some(b) = bytes.next() {
            if b == b'\r' && bytes.peek() == Some(&b'\n') {
                bytes.next();
            }
            self.paste.push_back(b);
        }
        true
    }

    /// AppleWin's `keycode` (bit 7 clear) and `keywaiting` (the strobe), for save-states.
    pub fn latch(&self) -> (u8, bool) {
        (self.keycode, self.strobe)
    }

    /// Restores [`Keyboard::latch`].
    pub fn set_latch(&mut self, keycode: u8, strobe: bool) {
        self.keycode = keycode & 0x7F;
        self.strobe = strobe;
    }

    /// `KeybReadData` ($C000-$C00F read).
    pub fn read_data(&self) -> u8 {
        match self.paste.front() {
            Some(c) => 0x80 | c,
            None => self.keycode | if self.strobe { 0x80 } else { 0 },
        }
    }

    /// `KeybClearStrobe`: clears the strobe and consumes one pasted character, returned with bit 7
    /// set (0 if nothing was being pasted).
    pub fn clear_strobe(&mut self) -> u8 {
        self.strobe = false;
        self.paste.pop_front().map_or(0, |c| 0x80 | c)
    }

    /// `KeybReadFlag` ($C010 read on the //e): the pasted character, else keycode | any-key-down.
    pub fn read_flag(&mut self) -> u8 {
        match self.clear_strobe() {
            0 => self.keycode | if self.any_key_down { 0x80 } else { 0 },
            c => c,
        }
    }

    /// `Pravets::SetCapsLockAllowed` ($C060 write, Pravets 8A only).
    pub fn set_pravets_8bit(&mut self, value: u8) {
        self.pravets_8bit = value & 1 != 0;
    }

    /// `Pravets::GetKeycode` ($C060 read, Pravets 8A): keyboard mode and case bits over the
    /// floating bus.
    pub fn pravets_c060(&self, floating_bus: u8) -> u8 {
        let mut v = floating_bus;
        if self.pravets_8bit {
            let k = self.keycode;
            if !self.pravets_shift {
                // lowercase: special keys and control combinations read 1
                if k < 65 || (126 < k && k < 193) {
                    v |= 0x80;
                } else {
                    v &= 0x7F;
                }
            } else {
                v |= 0x80;
            }
        } else {
            v = (v & 0xBF) | 0x80;
        }
        v
    }
}
