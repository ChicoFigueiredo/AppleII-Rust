//! winit -> framework-neutral key types.
//!
//! Characters come from the layout-translated text of the event (what `WM_CHAR` carries on
//! Windows), so non-US layouts, dead keys, AltGr and compose sequences type the right character;
//! scancodes are only used to identify a physical key between press and release.

use crate::keymap::{HostKey, KeyInput, Mods};
use winit::event::KeyEvent;
use winit::keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NamedKey, PhysicalKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

pub fn mods(m: ModifiersState) -> Mods {
    Mods {
        shift: m.shift_key(),
        ctrl: m.control_key(),
        alt: m.alt_key(),
    }
}

/// Stable id of the physical key (0 for unidentified keys).
pub fn phys_id(key: &PhysicalKey) -> u32 {
    match key {
        PhysicalKey::Code(c) => *c as u32 + 1,
        PhysicalKey::Unidentified(_) => 0,
    }
}

/// Converts a winit key event.
pub fn key_input(ev: &KeyEvent, m: ModifiersState) -> KeyInput {
    let numpad = ev.location == KeyLocation::Numpad;
    let phys = phys_id(&ev.physical_key);
    // Left Alt = Open Apple, right Alt (AltGr) = Closed Apple.
    if let PhysicalKey::Code(code) = ev.physical_key {
        let modifier = match code {
            KeyCode::AltLeft => Some(HostKey::AltLeft),
            KeyCode::AltRight => Some(HostKey::AltRight),
            KeyCode::ShiftLeft | KeyCode::ShiftRight => Some(HostKey::Shift),
            KeyCode::ControlLeft => Some(HostKey::ControlLeft),
            KeyCode::ControlRight => Some(HostKey::ControlRight),
            _ => None,
        };
        if let Some(key) = modifier {
            return KeyInput {
                key,
                numpad: false,
                phys,
            };
        }
    }
    // With Ctrl held some platforms report the control character; ask for the plain key.
    let logical = if m.control_key() {
        ev.key_without_modifiers()
    } else {
        ev.logical_key.clone()
    };
    let mut key = host_key(&logical);
    // Text wins when there is any: it already includes layout, Shift, Caps Lock, dead keys and
    // AltGr. Control characters (Enter, Tab...) keep their named keys.
    if !m.control_key() && !matches!(ev.logical_key, Key::Named(_)) {
        if let Some(c) = ev
            .text
            .as_ref()
            .and_then(|t| t.chars().next())
            .filter(|c| !c.is_control())
        {
            key = HostKey::Char(c);
        }
    }
    KeyInput { key, numpad, phys }
}

pub fn host_key(key: &Key) -> HostKey {
    match key {
        Key::Character(s) => s.chars().next().map_or(HostKey::Other, HostKey::Char),
        Key::Named(n) => match n {
            NamedKey::Space => HostKey::Char(' '),
            NamedKey::Enter => HostKey::Enter,
            NamedKey::Backspace => HostKey::Backspace,
            NamedKey::Tab => HostKey::Tab,
            NamedKey::Escape => HostKey::Escape,
            NamedKey::ArrowLeft => HostKey::Left,
            NamedKey::ArrowUp => HostKey::Up,
            NamedKey::ArrowRight => HostKey::Right,
            NamedKey::ArrowDown => HostKey::Down,
            NamedKey::Delete => HostKey::Delete,
            NamedKey::Insert => HostKey::Insert,
            NamedKey::Home => HostKey::Home,
            NamedKey::End => HostKey::End,
            NamedKey::PageUp => HostKey::PageUp,
            NamedKey::PageDown => HostKey::PageDown,
            NamedKey::Clear => HostKey::Clear,
            // Ctrl+Break is VK_CANCEL on Windows.
            NamedKey::Pause | NamedKey::Cancel => HostKey::Pause,
            NamedKey::PrintScreen => HostKey::PrintScreen,
            NamedKey::ScrollLock => HostKey::ScrollLock,
            NamedKey::CapsLock => HostKey::CapsLock,
            NamedKey::NumLock => HostKey::NumLock,
            NamedKey::F1 => HostKey::F(1),
            NamedKey::F2 => HostKey::F(2),
            NamedKey::F3 => HostKey::F(3),
            NamedKey::F4 => HostKey::F(4),
            NamedKey::F5 => HostKey::F(5),
            NamedKey::F6 => HostKey::F(6),
            NamedKey::F7 => HostKey::F(7),
            NamedKey::F8 => HostKey::F(8),
            NamedKey::F9 => HostKey::F(9),
            NamedKey::F10 => HostKey::F(10),
            NamedKey::F11 => HostKey::F(11),
            NamedKey::F12 => HostKey::F(12),
            _ => HostKey::Other,
        },
        _ => HostKey::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_and_character_keys() {
        assert_eq!(host_key(&Key::Character("a".into())), HostKey::Char('a'));
        assert_eq!(host_key(&Key::Named(NamedKey::Space)), HostKey::Char(' '));
        assert_eq!(host_key(&Key::Named(NamedKey::F7)), HostKey::F(7));
        assert_eq!(host_key(&Key::Named(NamedKey::ArrowLeft)), HostKey::Left);
        assert_eq!(host_key(&Key::Named(NamedKey::Shift)), HostKey::Other);
    }

    #[test]
    fn break_and_pause_are_the_same_key() {
        assert_eq!(host_key(&Key::Named(NamedKey::Cancel)), HostKey::Pause);
        assert_eq!(host_key(&Key::Named(NamedKey::Pause)), HostKey::Pause);
    }

    #[test]
    fn physical_ids_are_distinct_and_non_zero() {
        let a = phys_id(&PhysicalKey::Code(KeyCode::KeyA));
        let b = phys_id(&PhysicalKey::Code(KeyCode::KeyB));
        assert!(a != 0 && b != 0 && a != b);
    }

    #[test]
    fn non_us_layout_characters_map_by_character() {
        // AZERTY 'a' and the German '@' on AltGr+Q are just characters to the machine.
        assert_eq!(host_key(&Key::Character("@".into())), HostKey::Char('@'));
        assert_eq!(host_key(&Key::Character("é".into())), HostKey::Char('é'));
    }
}
