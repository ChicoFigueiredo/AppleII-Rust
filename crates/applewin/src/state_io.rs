//! Save-states: AppleWin's `.aws.yaml` through `apple2-savestate`, plus the file-name and
//! dialog behaviour of `Snapshot_*` (SaveState.cpp) and `SaveStateSelectImage`
//! (PropertySheetHelper.cpp:187).

use crate::factory::{self, Env, SystemClock};
use apple2_audio::AudioBus;
use apple2_cards::DongleKind;
use apple2_config::{CardType, Settings, SlotCardSpec};
use apple2_core::hooks::{GameIoDevice, RomTap};
use apple2_core::{Card, Model};
use apple2_savestate::{ImageRequest, LoadHost, Loaded, SaveOptions};
use apple2_storage::image::{Class, DiskImage};
use std::path::{Path, PathBuf};

/// `DEFAULT_SNAPSHOT_NAME` (SaveState.cpp:50).
pub const DEFAULT_SNAPSHOT_NAME: &str = "SaveState.aws.yaml";
const EXT: &str = ".aws.yaml";

/// The `# Date-stamp:` header line.
pub fn date_stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// A finished save-state document and the things the AppleWin view could not hold.
pub struct SavedState {
    pub text: String,
    pub warnings: Vec<String>,
}

pub fn save_machine(machine: &apple2_core::Machine) -> Result<SavedState, String> {
    save_machine_with_rgb(machine, apple2_savestate::RgbCardState::default())
}

/// Like [`save_machine`], with the RGB card registers of the renderer (`Renderer::rgb_registers`,
/// `RenderConfig::rgb_invert_bit7`) for the aux card's `AppleColor RGB Adaptor` map (AUD-P-005).
pub fn save_machine_with_rgb(
    machine: &apple2_core::Machine,
    rgb: apple2_savestate::RgbCardState,
) -> Result<SavedState, String> {
    let opts = SaveOptions {
        date_stamp: date_stamp(),
        rgb,
        ..Default::default()
    };
    let saved = apple2_savestate::save_aws_yaml(machine, &opts).map_err(|e| e.to_string())?;
    Ok(SavedState {
        text: saved.text,
        warnings: saved.warnings,
    })
}

/// The "append .aws.yaml if it's not there" rules of `SaveStateSelectImage` (:218-247): `file`,
/// `file.aws` and `file.yaml` all become `file.aws.yaml`.
pub fn ensure_aws_yaml_ext(path: &Path) -> PathBuf {
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return path.to_path_buf();
    };
    let lower = name.to_ascii_lowercase();
    let stem = if lower.ends_with(EXT) {
        return path.to_path_buf();
    } else if lower.ends_with(".aws") {
        &name[..name.len() - 4]
    } else if lower.ends_with(".yaml") {
        &name[..name.len() - 5]
    } else {
        &name[..]
    };
    path.with_file_name(format!("{stem}{EXT}"))
}

/// `Snapshot_GetDefaultFilenameAndPath` (:168): the state file implied by the media plugged in,
/// hard disks (slot 7 down to 0) before floppies: the image's file name without extension.
pub fn media_default(settings: &Settings) -> Option<(String, PathBuf)> {
    let split = |p: &Path| -> Option<(String, PathBuf)> {
        let stem = p.file_stem()?.to_string_lossy().into_owned();
        let dir = p
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .map(Path::to_path_buf)?;
        Some((stem, dir))
    };
    for spec in settings.slots.0.iter().rev() {
        if let SlotCardSpec::GenericHdd { images, .. } = spec {
            if let Some(p) = images.iter().flatten().next() {
                return split(p);
            }
        }
    }
    for spec in settings.slots.0.iter() {
        if let SlotCardSpec::Disk2 { drives, .. } = spec {
            if let Some(p) = drives.iter().find_map(|d| d.image.as_ref()) {
                return split(p);
            }
        }
    }
    None
}

/// `Snapshot_SetFilename` at start-up plus `Snapshot_UpdatePath` after the media were inserted:
/// the saved file name wins while it lives in the media's directory; otherwise the media names
/// the file; with neither it is `SaveState.aws.yaml` in the current directory.
pub fn default_state_path(settings: &Settings, current_dir: &Path) -> PathBuf {
    let explicit = settings.storage.savestate_filename.clone();
    if let Some((stem, dir)) = media_default(settings) {
        let same_dir = explicit
            .as_ref()
            .and_then(|e| e.parent())
            .is_some_and(|p| p == dir);
        if !same_dir {
            return dir.join(format!("{stem}{EXT}"));
        }
    }
    explicit.unwrap_or_else(|| current_dir.join(DEFAULT_SNAPSHOT_NAME))
}

/// Remembers the dialog's choice as the state file (`SaveStateUpdate`: the registry value).
pub fn remember_path(settings: &mut Settings, path: &Path) -> Result<(), String> {
    settings.storage.savestate_filename = Some(path.to_path_buf());
    settings.save_default().map_err(|e| e.to_string())
}

/// F11 (`save = true`) / F12 (`save = false`): the file dialog, starting at the current state
/// file. Saving appends `.aws.yaml`; loading accepts any file.
pub fn pick_file(save: bool, current: &Path) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new()
        .set_title(if save {
            "Select Save State file"
        } else {
            "Select Save State file to load"
        })
        .add_filter("Save State files (*.aws.yaml)", &["yaml", "aws"])
        .add_filter("All Files", &["*"]);
    if let Some(dir) = current.parent().filter(|p| !p.as_os_str().is_empty()) {
        d = d.set_directory(dir);
    }
    if let Some(name) = current.file_name() {
        d = d.set_file_name(name.to_string_lossy());
    }
    if save {
        d.save_file().map(|p| ensure_aws_yaml_ext(&p))
    } else {
        d.pick_file()
    }
}

/// Builds what a loaded state needs from the host: disk images found next to the state file,
/// in the current directory or the disk directory; host-bound cards (serial, printer, network,
/// audio) from the current settings; the host clock.
pub struct StateHost<'a> {
    pub settings: &'a Settings,
    pub bus: &'a AudioBus,
    /// Directory of the state file (images are looked up there first).
    pub state_dir: Option<PathBuf>,
    pub warnings: Vec<String>,
}

fn card_type_of(name: &str) -> Option<CardType> {
    Some(match name {
        "Disk II" => CardType::Disk2,
        "Super Serial Card" => CardType::Ssc,
        "Mockingboard C" => CardType::MockingboardC,
        "Phasor" => CardType::Phasor,
        "MEGA Audio" => CardType::MegaAudio,
        "SD Music" => CardType::SdMusic,
        "Generic Printer" => CardType::GenericPrinter,
        "Hard Disk Controller" => CardType::GenericHdd,
        "Mouse Card" => CardType::MouseInterface,
        "Z80 SoftCard" => CardType::Z80,
        "SAM" => CardType::Sam,
        "Uthernet" => CardType::Uthernet,
        "Uthernet II" => CardType::Uthernet2,
        "4Play" => CardType::FourPlay,
        "SNES MAX" => CardType::SnesMax,
        "VidHD" => CardType::VidHd,
        "Breakpoint" => CardType::BreakpointCard,
        _ => return None,
    })
}

impl LoadHost for StateHost<'_> {
    fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
        let class = if req.hard_disk {
            Class::HardDisk
        } else {
            Class::Floppy
        };
        let base = Path::new(req.filename)
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_default();
        let mut candidates = vec![PathBuf::from(req.filename)];
        if let Some(d) = &self.state_dir {
            candidates.push(d.join(&base));
        }
        if !req.absolute_path.is_empty() {
            candidates.push(PathBuf::from(req.absolute_path));
        }
        let dir = if req.hard_disk {
            &self.settings.storage.hdv_dir
        } else {
            &self.settings.storage.disk_dir
        };
        if let Some(d) = dir {
            candidates.push(d.join(&base));
        }
        let mut last = format!("{}: not found", req.filename);
        for c in candidates.iter().filter(|c| c.is_file()) {
            match DiskImage::open_path(c, class, false) {
                Ok(img) => return Ok(img),
                Err(e) => last = format!("{}: {e:?}", c.display()),
            }
        }
        Err(last)
    }

    fn make_card(&mut self, name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
        let ty = card_type_of(name)?;
        // Media comes from the state, not from the settings: only the host-bound options carry over.
        let spec = match ty {
            CardType::Disk2 | CardType::GenericHdd => SlotCardSpec::new(ty),
            _ => self
                .settings
                .slots
                .0
                .iter()
                .find(|s| s.card_type() == ty)
                .cloned()
                .unwrap_or_else(|| SlotCardSpec::new(ty)),
        };
        let mut settings = self.settings.clone();
        settings.machine.model = model;
        let env = Env {
            settings: &settings,
            bus: self.bus,
        };
        let card = factory::build_card(&spec, slot, &env, &mut self.warnings);
        if card.is_none() {
            self.warnings.push(format!(
                "slot {slot}: '{name}' not available with the current settings"
            ));
        }
        card
    }

    fn make_no_slot_clock(&mut self) -> Option<Box<dyn RomTap>> {
        Some(Box::new(apple2_cards::NoSlotClock::new(Box::new(
            SystemClock,
        ))))
    }

    fn make_dongle(&mut self, kind: DongleKind) -> Option<Box<dyn GameIoDevice>> {
        Some(Box::new(apple2_cards::Dongle::new(kind)))
    }
}

/// Parses `text` and builds the machine (`Snapshot_LoadState_v2`). Warnings from the loader and
/// from the host are merged.
pub fn load_machine(
    text: &str,
    settings: &Settings,
    bus: &AudioBus,
    state_dir: Option<&Path>,
) -> Result<Loaded, String> {
    let mut host = StateHost {
        settings,
        bus,
        state_dir: state_dir.map(Path::to_path_buf),
        warnings: Vec::new(),
    };
    let mut loaded = apple2_savestate::load_aws_yaml(text, &mut host).map_err(|e| e.to_string())?;
    loaded.warnings.append(&mut host.warnings);
    Ok(loaded)
}

/// AppleWin refuses the v1 `.aws` binary format (SaveState.cpp:485).
pub fn check_loadable(path: &Path) -> Result<(), String> {
    let name = path.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".aws") {
        return Err("Save-state v1 no longer supported. Please load using AppleWin 1.27, and re-save as a v2 state file.".into());
    }
    Ok(())
}

pub fn is_state_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    name.ends_with(EXT) || name.ends_with(".aws") || name.ends_with(".yaml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_config::card::DriveSpec;

    #[test]
    fn extension_rules() {
        // Compare as paths: on Windows the joined separator is `\`.
        let e = |s: &str| ensure_aws_yaml_ext(Path::new(s));
        assert_eq!(e("/x/file"), Path::new("/x/file.aws.yaml"));
        assert_eq!(e("/x/file.aws"), Path::new("/x/file.aws.yaml"));
        assert_eq!(e("/x/file.yaml"), Path::new("/x/file.aws.yaml"));
        assert_eq!(e("/x/file.aws.yaml"), Path::new("/x/file.aws.yaml"));
        assert_eq!(e("/x/file.dsk"), Path::new("/x/file.dsk.aws.yaml"));
    }

    #[test]
    fn default_path_follows_media_like_snapshot_update_path() {
        let cwd = Path::new("/cwd");
        let mut s = Settings::default();
        assert_eq!(
            default_state_path(&s, cwd),
            Path::new("/cwd/SaveState.aws.yaml")
        );
        s.storage.savestate_filename = Some("/saves/mine.aws.yaml".into());
        assert_eq!(
            default_state_path(&s, cwd),
            Path::new("/saves/mine.aws.yaml")
        );
        // a floppy in another directory renames the file; in the same directory it keeps the name
        s.slots.0[6] = SlotCardSpec::Disk2 {
            firmware_13_sector: false,
            drives: [
                DriveSpec {
                    image: Some("/games/Choplifter.dsk".into()),
                    ..Default::default()
                },
                DriveSpec::default(),
            ],
        };
        assert_eq!(
            default_state_path(&s, cwd),
            Path::new("/games/Choplifter.aws.yaml")
        );
        s.storage.savestate_filename = Some("/games/mine.aws.yaml".into());
        assert_eq!(
            default_state_path(&s, cwd),
            Path::new("/games/mine.aws.yaml")
        );
        // hard disks win over floppies
        s.slots.0[7] = SlotCardSpec::GenericHdd {
            firmware: Default::default(),
            images: [
                Some("/hd/Work.hdv".into()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
        };
        assert_eq!(default_state_path(&s, cwd), Path::new("/hd/Work.aws.yaml"));
    }

    #[test]
    fn v1_states_are_refused() {
        assert!(check_loadable(Path::new("old.aws")).is_err());
        assert!(check_loadable(Path::new("new.aws.yaml")).is_ok());
    }
}
