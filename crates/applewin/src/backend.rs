//! Plug-in seams between the shell and the other crates. Everything here runs on the emulation
//! thread (it needs `&mut Machine`), so implementors need not be `Send`; only the
//! [`BackendFactory`] that creates them does.

use crate::video::{FrameSource, VideoRenderer};
use apple2_audio::AudioBus;
use apple2_config::Settings;
use apple2_config::SlotCardSpec;
use apple2_core::Machine;
use apple2_storage::disk2::{Disk2Card, DiskStatus};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DriveActivity {
    #[default]
    Off,
    Read,
    Write,
}

/// What the toolbar shows for one drive.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DriveStatus {
    pub present: bool,
    /// File name of the inserted image.
    pub image: Option<String>,
    pub track: u8,
    pub activity: DriveActivity,
    pub write_protected: bool,
}

/// Disk images: implemented on top of `apple2-storage` + the Disk II card later.
pub trait DiskSink {
    fn insert(&mut self, machine: &mut Machine, drive: usize, path: &Path) -> Result<(), String>;
    fn eject(&mut self, machine: &mut Machine, drive: usize);
    fn swap(&mut self, machine: &mut Machine);
    fn status(&self, machine: &Machine) -> [DriveStatus; 2];
    /// AppleWin "enhanced disk speed": run unthrottled while a drive motor spins.
    fn wants_full_speed(&self, _machine: &Machine) -> bool {
        false
    }
    /// Writes dirty tracks back (before saving a state or quitting).
    fn flush(&mut self, _machine: &mut Machine) {}
}

pub struct NullDisks;

impl DiskSink for NullDisks {
    fn insert(&mut self, _: &mut Machine, _: usize, _: &Path) -> Result<(), String> {
        Err("no disk controller available yet (apple2-storage not plugged in)".into())
    }
    fn eject(&mut self, _: &mut Machine, _: usize) {}
    fn swap(&mut self, _: &mut Machine) {}
    fn status(&self, _: &Machine) -> [DriveStatus; 2] {
        Default::default()
    }
}

/// AppleMouse card input. Motion is relative (host pixels).
pub trait MouseSink {
    fn available(&self) -> bool {
        false
    }
    fn motion(&mut self, _machine: &mut Machine, _dx: i32, _dy: i32) {}
    fn button(&mut self, _machine: &mut Machine, _button: u8, _down: bool) {}
}

pub struct NullMouse;
impl MouseSink for NullMouse {}

/// `apple2_cards::MouseCard` in a slot.
pub struct CardMouse {
    pub slot: u8,
}

impl MouseSink for CardMouse {
    fn available(&self) -> bool {
        true
    }
    fn motion(&mut self, m: &mut Machine, dx: i32, dy: i32) {
        apple2_cards::mouse::mouse_move(m, self.slot, dx, dy);
    }
    fn button(&mut self, m: &mut Machine, button: u8, down: bool) {
        apple2_cards::mouse::mouse_button(m, self.slot, usize::from(button), down);
    }
}

/// Everything the emulation thread needs from the rest of the workspace.
pub trait Backend {
    /// Builds the machine from the settings (model, cards, ROMs...). `bus` must be given to
    /// audio cards (Mockingboard, SAM).
    fn build_machine(&mut self, settings: &Settings, bus: &AudioBus) -> Result<Machine, String>;
    fn frame_source(&mut self, settings: &Settings) -> Box<dyn FrameSource>;
    fn disks(&mut self, _settings: &Settings) -> Box<dyn DiskSink> {
        Box::new(NullDisks)
    }
    fn mouse(&mut self, _settings: &Settings) -> Box<dyn MouseSink> {
        Box::new(NullMouse)
    }
    /// Sinks for an already built machine (also after a save-state replaced it); the default
    /// follows the settings.
    fn disks_for(&mut self, _machine: &Machine, settings: &Settings) -> Box<dyn DiskSink> {
        self.disks(settings)
    }
    fn mouse_for(&mut self, _machine: &Machine, settings: &Settings) -> Box<dyn MouseSink> {
        self.mouse(settings)
    }
}

/// Disk II in `slot` (apple2-storage).
pub struct Disk2Sink {
    pub slot: u8,
}

impl DiskSink for Disk2Sink {
    fn insert(&mut self, m: &mut Machine, drive: usize, path: &Path) -> Result<(), String> {
        let card = m.card_mut::<Disk2Card>(self.slot).ok_or("no Disk II card in this slot")?;
        card.insert_path(drive, path, false).map_err(|e| format!("{}: {e:?}", path.display()))
    }
    fn eject(&mut self, m: &mut Machine, drive: usize) {
        if let Some(c) = m.card_mut::<Disk2Card>(self.slot) {
            c.eject_disk(drive);
        }
    }
    fn swap(&mut self, m: &mut Machine) {
        if let Some(c) = m.card_mut::<Disk2Card>(self.slot) {
            c.swap_disks();
        }
    }
    fn status(&self, m: &Machine) -> [DriveStatus; 2] {
        let Some(c) = m.card::<Disk2Card>(self.slot) else { return Default::default() };
        std::array::from_fn(|d| DriveStatus {
            present: c.is_drive_connected(d),
            image: (!c.is_drive_empty(d)).then(|| c.base_name(d).to_string()),
            track: c.track(d) as u8,
            activity: match c.light_status(d) {
                DiskStatus::Read => DriveActivity::Read,
                DiskStatus::Write => DriveActivity::Write,
                _ => DriveActivity::Off,
            },
            write_protected: c.is_write_protected(d),
        })
    }
    fn wants_full_speed(&self, m: &Machine) -> bool {
        apple2_storage::disk2::manager::is_condition_for_full_speed(m)
    }
    fn flush(&mut self, m: &mut Machine) {
        apple2_storage::disk2::manager::flush_all(m);
    }
}

/// Creates the [`Backend`] inside the emulation thread (cards are not `Send`).
pub type BackendFactory = Box<dyn FnOnce() -> Box<dyn Backend> + Send>;

/// The real backend: the machine and every card from [`crate::factory`], the `apple2-video`
/// renderer, Disk II and mouse sinks found by looking at the machine's slots.
#[derive(Default)]
pub struct DefaultBackend;

impl Backend for DefaultBackend {
    fn build_machine(&mut self, settings: &Settings, bus: &AudioBus) -> Result<Machine, String> {
        crate::factory::build_machine(settings, bus)
    }

    fn frame_source(&mut self, settings: &Settings) -> Box<dyn FrameSource> {
        Box::new(VideoRenderer::from_settings(settings))
    }

    fn disks(&mut self, settings: &Settings) -> Box<dyn DiskSink> {
        match settings.slots.0.iter().position(|s| matches!(s, SlotCardSpec::Disk2 { .. })) {
            Some(slot) => Box::new(Disk2Sink { slot: slot as u8 }),
            None => Box::new(NullDisks),
        }
    }

    fn mouse(&mut self, settings: &Settings) -> Box<dyn MouseSink> {
        match settings.slots.0.iter().position(|s| matches!(s, SlotCardSpec::MouseInterface { .. })) {
            Some(slot) => Box::new(CardMouse { slot: slot as u8 }),
            None => Box::new(NullMouse),
        }
    }

    fn disks_for(&mut self, machine: &Machine, _settings: &Settings) -> Box<dyn DiskSink> {
        match crate::factory::slot_of::<Disk2Card>(machine) {
            Some(slot) => Box::new(Disk2Sink { slot }),
            None => Box::new(NullDisks),
        }
    }

    fn mouse_for(&mut self, machine: &Machine, _settings: &Settings) -> Box<dyn MouseSink> {
        match crate::factory::slot_of::<apple2_cards::MouseCard>(machine) {
            Some(slot) => Box::new(CardMouse { slot }),
            None => Box::new(NullMouse),
        }
    }
}
