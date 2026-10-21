//! Hot swap contract (AUD-P-048): which configuration changes take effect at once and which
//! need a restart. AppleWin (`Configuration/PageSlots.cpp` `ApplyConfigAfterClose`,
//! `PropertySheetHelper.cpp` `HardwareConfigChanged` / `IsOkToRestart`) decides it like this, and
//! so does this program:
//!
//! | Change | Takes effect | AppleWin |
//! |---|---|---|
//! | Disk II image (F3/F4, drive menu, drop, settings) | at once, the machine keeps running | `Disk2InterfaceCard::InsertDisk` |
//! | Disk II eject / write protect | at once | `EjectDisk` / `SetProtect` |
//! | Hard disk image of a controller already in the slot | at once | `HarddiskInterfaceCard::Insert` / `Unplug` |
//! | Video, input, sound, speed, preferences, mouse and printer card options | at once | `ApplyConfigAfterClose` |
//! | Computer model, CPU, refresh rate, card in a slot, card options that build the card (Mockingboard sockets, SSC port, Uthernet, RamWorks size, firmware...) | restart, only after a prompt that warns about the lost RAM | `WM_USER_RESTART`, `IsOkToRestart` |
//!
//! The restart is never silent: the settings dialog shows "Restarting the emulator will reset
//! the state of the emulated machine, causing you to lose any unsaved work" and `Cancel` leaves
//! the machine untouched (see `settings::Confirm::Restart`). F2 / Ctrl+F2 are the only other
//! ways to reset, and they are the user's explicit act.

use crate::emu::Command;
use apple2_config::{Settings, SlotCardSpec};

/// The commands that bring a running machine from `old` to `new` without a restart: image
/// changes of Disk II drives and hard disks in cards present in both. Everything else that is
/// "live" is applied by swapping the settings (`Command::UpdateSettings`).
pub fn commands(old: &Settings, new: &Settings) -> Vec<Command> {
    let mut v = Vec::new();
    for slot in 0..8 {
        match (&old.slots.0[slot], &new.slots.0[slot]) {
            (SlotCardSpec::Disk2 { drives: a, .. }, SlotCardSpec::Disk2 { drives: b, .. }) => {
                for d in 0..2 {
                    if a[d].image != b[d].image {
                        v.push(match &b[d].image {
                            Some(p) => Command::InsertDisk {
                                drive: d,
                                path: p.clone(),
                                read_only: b[d].write_protect,
                            },
                            None => Command::EjectDisk { drive: d },
                        });
                    } else if a[d].write_protect != b[d].write_protect {
                        v.push(Command::SetWriteProtect {
                            drive: d,
                            on: b[d].write_protect,
                        });
                    }
                }
            }
            (
                SlotCardSpec::GenericHdd { images: a, .. },
                SlotCardSpec::GenericHdd { images: b, .. },
            ) => {
                for unit in 0..a.len() {
                    if a[unit] != b[unit] {
                        v.push(Command::SetHdd {
                            slot: slot as u8,
                            unit,
                            image: b[unit].clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    v
}

/// AppleWin writes a floppy's image to the registry as soon as it changes
/// (`Disk2InterfaceCard::SaveLastDiskImage`, Disk.cpp:227-258), so that a restart from the
/// configuration dialog, and the next run, find the same disks (GH#98, GH#511, GH#960). Brings
/// `settings` up to date with an insert / drop / eject / swap command; `true` = it changed.
pub fn remember_media(settings: &mut Settings, c: &Command) -> bool {
    match c {
        Command::InsertDisk { drive, path, .. } | Command::DropDisk { drive, path, .. } => {
            settings.remember_floppy(*drive, Some(path.clone()))
        }
        Command::EjectDisk { drive } => settings.remember_floppy(*drive, None),
        Command::SwapDrives => settings.swap_floppies(),
        _ => return false,
    }
    true
}

/// `spec` with the parts that are hot-swappable (images, write protect) cleared, so that two
/// specs compare equal when only those differ.
pub fn without_media(spec: &SlotCardSpec) -> SlotCardSpec {
    let mut s = spec.clone();
    match &mut s {
        SlotCardSpec::Disk2 { drives, .. } => {
            for d in drives.iter_mut() {
                d.image = None;
                d.write_protect = false;
            }
        }
        SlotCardSpec::GenericHdd { images, .. } => {
            for i in images.iter_mut() {
                *i = None;
            }
        }
        _ => {}
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_config::{CardType, SlotCardSpec};

    fn with_cards() -> Settings {
        let mut s = Settings::default();
        s.slots.0[6] = SlotCardSpec::new(CardType::Disk2);
        s.slots.0[7] = SlotCardSpec::new(CardType::GenericHdd);
        s
    }

    #[test]
    fn disk_and_hdd_images_are_hot() {
        let old = with_cards();
        let mut new = old.clone();
        if let SlotCardSpec::Disk2 { drives, .. } = &mut new.slots.0[6] {
            drives[1].image = Some("a.dsk".into());
        }
        if let SlotCardSpec::GenericHdd { images, .. } = &mut new.slots.0[7] {
            images[0] = Some("b.hdv".into());
        }
        let cmds = commands(&old, &new);
        assert!(matches!(
            cmds.as_slice(),
            [
                Command::InsertDisk { drive: 1, .. },
                Command::SetHdd {
                    slot: 7,
                    unit: 0,
                    image: Some(_)
                }
            ]
        ));
        assert!(crate::settings::model::restart_reasons(&old, &new).is_empty());
        // and back: eject / unplug
        let back = commands(&new, &old);
        assert!(matches!(
            back.as_slice(),
            [
                Command::EjectDisk { drive: 1 },
                Command::SetHdd {
                    slot: 7,
                    image: None,
                    ..
                }
            ]
        ));
    }

    #[test]
    fn cards_and_machine_need_a_restart_and_no_hot_command() {
        let old = with_cards();
        let mut new = old.clone();
        new.slots.0[7] = SlotCardSpec::Empty;
        assert!(commands(&old, &new).is_empty());
        assert!(!crate::settings::model::restart_reasons(&old, &new).is_empty());
        let mut new = old.clone();
        new.machine.model = apple2_core::Model::Apple2Plus;
        assert!(!crate::settings::model::restart_reasons(&old, &new).is_empty());
        // a firmware change of the Disk II card is a hardware change
        let mut new = old.clone();
        if let SlotCardSpec::Disk2 {
            firmware_13_sector, ..
        } = &mut new.slots.0[6]
        {
            *firmware_13_sector = true;
        }
        assert!(!crate::settings::model::restart_reasons(&old, &new).is_empty());
    }
}
