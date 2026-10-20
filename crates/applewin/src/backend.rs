//! Plug-in seams between the shell and the other crates. Everything here runs on the emulation
//! thread (it needs `&mut Machine`), so implementors need not be `Send`; only the
//! [`BackendFactory`] that creates them does.

use crate::video::{FrameSource, TextPlaceholder};
use apple2_audio::{AudioBus, MockingboardCard, Variant};
use apple2_config::Settings;
use apple2_config::SlotCardSpec;
use apple2_core::Machine;
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

/// AppleMouse card input. Coordinates are normalised to the Apple viewport (`0.0..=1.0`).
pub trait MouseSink {
    fn available(&self) -> bool {
        false
    }
    fn motion(&mut self, _machine: &mut Machine, _x: f32, _y: f32) {}
    fn button(&mut self, _machine: &mut Machine, _button: u8, _down: bool) {}
}

pub struct NullMouse;
impl MouseSink for NullMouse {}

/// UI-less debugger (`apple2-debugger`). F7 toggles it, Ctrl+F11 single-steps.
pub trait DebuggerHook {
    fn available(&self) -> bool {
        false
    }
    fn active(&self) -> bool {
        false
    }
    fn toggle(&mut self, _machine: &mut Machine) {}
    /// Called after every executed instruction while the debugger is active; true = stay halted.
    fn halt_after_step(&mut self, _machine: &mut Machine) -> bool {
        true
    }
}

pub struct NullDebugger;
impl DebuggerHook for NullDebugger {}

/// Everything the emulation thread needs from the rest of the workspace.
pub trait Backend {
    /// Builds the machine from the settings (model, cards, ROMs...). `bus` must be given to
    /// audio cards (Mockingboard, SAM).
    fn build_machine(&mut self, settings: &Settings, bus: &AudioBus) -> Result<Machine, String>;
    fn frame_source(&mut self, settings: &Settings) -> Box<dyn FrameSource>;
    fn disks(&mut self) -> Box<dyn DiskSink> {
        Box::new(NullDisks)
    }
    fn mouse(&mut self) -> Box<dyn MouseSink> {
        Box::new(NullMouse)
    }
    fn debugger(&mut self) -> Box<dyn DebuggerHook> {
        Box::new(NullDebugger)
    }
}

/// Creates the [`Backend`] inside the emulation thread (cards are not `Send`).
pub type BackendFactory = Box<dyn FnOnce() -> Box<dyn Backend> + Send>;

/// Core-only backend: the machine from `Settings::to_machine_config`, a Mockingboard in slot 4
/// when enabled, the text placeholder for video.
#[derive(Default)]
pub struct DefaultBackend;

impl Backend for DefaultBackend {
    fn build_machine(&mut self, settings: &Settings, bus: &AudioBus) -> Result<Machine, String> {
        let mut config = settings.to_machine_config();
        if let Some(path) = &settings.machine.custom_rom {
            config.custom_rom = Some(std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        let mut m = Machine::try_new(config).map_err(|e| format!("{e:?}"))?;
        if !settings.sound.disable_sound {
            for (slot, spec) in settings.slots.0.iter().enumerate().skip(1) {
                let card: Option<Box<dyn apple2_core::Card>> = match spec {
                    SlotCardSpec::MockingboardC(_) if !settings.sound.disable_mockingboard => {
                        Some(Box::new(MockingboardCard::new(Variant::MockingboardC, bus)))
                    }
                    SlotCardSpec::Phasor(_) if !settings.sound.disable_mockingboard => {
                        Some(Box::new(MockingboardCard::new(Variant::Phasor, bus)))
                    }
                    SlotCardSpec::MegaAudio(_) if !settings.sound.disable_mockingboard => {
                        Some(Box::new(MockingboardCard::new(Variant::MegaAudio, bus)))
                    }
                    SlotCardSpec::SdMusic(_) if !settings.sound.disable_mockingboard => {
                        Some(Box::new(MockingboardCard::new(Variant::SdMusic, bus)))
                    }
                    SlotCardSpec::Sam => Some(Box::new(apple2_audio::SamCard::new(bus))),
                    _ => None,
                };
                if let Some(c) = card {
                    m.insert_card(slot as u8, c);
                }
            }
        }
        Ok(m)
    }

    fn frame_source(&mut self, settings: &Settings) -> Box<dyn FrameSource> {
        Box::new(TextPlaceholder::new(settings.machine.model))
    }
}
