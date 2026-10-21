//! Importing an AppleWin `.aws.yaml` into a [`Machine`] for conformance runs (HU-0401).
//!
//! The host used here never touches the user's files: disk images are read into memory from the
//! directories given, printers print to memory, serial and network back ends are loopbacks. An
//! image the save-state names but no directory has is recorded in [`ImportHost::missing`], which
//! turns the case into BLOCKED instead of a silent pass.

use apple2_audio::{AudioBus, MockingboardCard, SamCard, Variant};
use apple2_core::{Card, Machine, Model, VideoStandard};
use apple2_hostio::network::MockNetwork;
use apple2_hostio::printer::{MemorySink, ParallelPrinterCard, PrinterOptions};
use apple2_hostio::ssc::SuperSerialCard;
use apple2_hostio::uthernet1::Uthernet1;
use apple2_hostio::uthernet2::Uthernet2;
use apple2_hostio::LoopbackSerial;
use apple2_savestate::{default_card, load_aws_yaml, ImageRequest, LoadHost};
use apple2_storage::image::DiskImage;
use std::path::{Path, PathBuf};

/// [`LoadHost`] that resolves images by base name in `dirs`.
pub struct ImportHost {
    pub dirs: Vec<PathBuf>,
    pub bus: AudioBus,
    /// Base names of the images the state needs and no directory has.
    pub missing: Vec<String>,
}

impl ImportHost {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        ImportHost {
            dirs,
            bus: AudioBus::new(44_100, VideoStandard::Ntsc),
            missing: Vec::new(),
        }
    }
}

impl LoadHost for ImportHost {
    fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
        // The name may be a Windows path (`F:\Apple][\x.dsk`): only the base name is used.
        let base = req
            .filename
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(req.filename);
        if base.is_empty() {
            return Err("empty image name".into());
        }
        for d in &self.dirs {
            if let Ok(bytes) = std::fs::read(d.join(base)) {
                let img = if req.hard_disk {
                    DiskImage::hard_disk_from_bytes(base, bytes)
                } else {
                    DiskImage::from_bytes(base, bytes)
                };
                return img.map_err(|e| e.to_string());
            }
        }
        self.missing.push(base.to_string());
        Err(format!("image '{base}' not found"))
    }

    fn make_card(&mut self, name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
        let mb = |v| Some(Box::new(MockingboardCard::new(v, &self.bus)) as Box<dyn Card>);
        match name {
            "Super Serial Card" => Some(Box::new(SuperSerialCard::new(Box::new(
                LoopbackSerial::new().0,
            )))),
            "Generic Printer" => Some(Box::new(ParallelPrinterCard::new(
                Box::new(MemorySink::default()),
                PrinterOptions::default(),
            ))),
            "Uthernet" => Some(Box::new(Uthernet1::new(Box::new(MockNetwork::new().0)))),
            "Uthernet II" => Some(Box::new(Uthernet2::new(
                Box::new(MockNetwork::new().0),
                true,
            ))),
            "Mockingboard C" => mb(Variant::MockingboardC),
            "Phasor" => mb(Variant::Phasor),
            "MEGA Audio" => mb(Variant::MegaAudio),
            "SD Music" => mb(Variant::SdMusic),
            "SAM" => Some(Box::new(SamCard::new(&self.bus))),
            _ => default_card(name, slot, model),
        }
    }
}

/// A save-state loaded into a machine.
pub struct Imported {
    pub machine: Machine,
    pub warnings: Vec<String>,
    /// Images the file refers to that could not be found (BLOCKED when not empty).
    pub missing: Vec<String>,
}

/// Directories searched for images: the state's own, then `extra`.
pub fn image_dirs(state: &Path, extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = state.parent() {
        dirs.push(if d.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            d.to_path_buf()
        });
    }
    dirs.extend(extra.iter().cloned());
    dirs
}

/// Imports the text of a save-state.
pub fn import_text(text: &str, dirs: Vec<PathBuf>) -> Result<Imported, String> {
    let mut host = ImportHost::new(dirs);
    let loaded = load_aws_yaml(text, &mut host).map_err(|e| e.to_string())?;
    Ok(Imported {
        machine: loaded.machine,
        warnings: loaded.warnings,
        missing: host.missing,
    })
}

/// Imports the save-state at `path`.
pub fn import_file(path: &Path, extra_dirs: &[PathBuf]) -> Result<Imported, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    import_text(&text, image_dirs(path, extra_dirs))
}
