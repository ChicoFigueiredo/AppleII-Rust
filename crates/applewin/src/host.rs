//! winit -> framework-neutral key types.

use crate::keymap::{HostKey, KeyInput, Mods};
use winit::event::KeyEvent;
use winit::keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NamedKey, PhysicalKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

pub fn mods(m: ModifiersState) -> Mods {
    Mods { shift: m.shift_key(), ctrl: m.control_key(), alt: m.alt_key() }
}

/// Converts a winit key event. `None` for keys the emulator has no use for.
pub fn key_input(ev: &KeyEvent, m: ModifiersState) -> KeyInput {
    let numpad = ev.location == KeyLocation::Numpad;
    // Left Alt = Open Apple, right Alt (AltGr) = Closed Apple.
    if let PhysicalKey::Code(code) = ev.physical_key {
        match code {
            KeyCode::AltLeft => return KeyInput::new(HostKey::AltLeft),
            KeyCode::AltRight => return KeyInput::new(HostKey::AltRight),
            KeyCode::ShiftLeft | KeyCode::ShiftRight => return KeyInput::new(HostKey::Shift),
            KeyCode::ControlLeft | KeyCode::ControlRight => return KeyInput::new(HostKey::Control),
            _ => {}
        }
    }
    // With Ctrl held some platforms report the control character; ask for the plain key.
    let logical = if m.control_key() { ev.key_without_modifiers() } else { ev.logical_key.clone() };
    KeyInput { key: host_key(&logical), numpad }
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
            NamedKey::Pause => HostKey::Pause,
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
}
