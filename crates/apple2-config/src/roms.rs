//! User ROM overrides: the optional `[roms]` table of `settings.toml`, the `-rom` / `-f8rom` /
//! `-videorom` command line options (all three land in `MachineSettings`), validation of the
//! files, and the provenance report (embedded or file, size, SHA-256) for the conformance
//! manifest and the start-up log.
//!
//! The core never opens files (ADR 0002): the bytes travel in `MachineConfig::custom_rom` and
//! `custom_f8_rom`. Precedence, highest first: the command line / `[machine]` keys (`custom_rom`,
//! `custom_f8_rom`, `video_rom`), then `[roms]`, then the ROM embedded in `apple2-roms`.
//!
//! Size rules, from AppleWin:
//! * `-rom` (any model): 16K (`$C000-$FFFF`) or 12K (`$D000-$FFFF`), CmdLine.cpp:501.
//! * `[roms].<model>`: exactly the size of that model's embedded ROM (Memory.cpp:2054-2063).
//! * `-f8rom`: exactly 2K, CmdLine.cpp:489.
//! * `-videorom`: 2K, 4K, 8K or 16K, Video.cpp:633.
//!
//! AppleWin offers no way to replace the firmware of slot cards, so none is offered here; the
//! report still lists the (embedded) slot firmware in use.

use crate::settings::Settings;
use apple2_core::{MachineConfig, Model};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Optional `[roms]` table: ROM files that replace the embedded ones.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RomSettings {
    /// Apple ][ system ROM, 12K (Memory.cpp:2054).
    pub apple2: Option<PathBuf>,
    /// Apple ][+ system ROM, 12K.
    pub apple2_plus: Option<PathBuf>,
    /// Apple ][ J-Plus system ROM, 12K.
    pub apple2_jplus: Option<PathBuf>,
    /// Apple //e system ROM, 16K (4K internal Cx ROM + 12K).
    pub apple2e: Option<PathBuf>,
    /// Enhanced Apple //e system ROM, 16K.
    pub apple2e_enhanced: Option<PathBuf>,
    /// Pravets 82 system ROM, 12K.
    pub pravets82: Option<PathBuf>,
    /// Pravets 8M system ROM, 12K.
    pub pravets8m: Option<PathBuf>,
    /// Pravets 8A system ROM, 16K.
    pub pravets8a: Option<PathBuf>,
    /// TK3000 //e system ROM, 16K.
    pub tk3000e: Option<PathBuf>,
    /// Base64A system ROM, 48K (4 pages of 12K).
    pub base64a: Option<PathBuf>,
    /// 2K ROM for `$F800-$FFFF` (same as `-f8rom`).
    pub f8: Option<PathBuf>,
    /// Video / character generator ROM, 2K, 4K, 8K or 16K (same as `-videorom`).
    pub video: Option<PathBuf>,
}

impl RomSettings {
    /// The system ROM file set for `model` (the //c and the "bad" Pravets tags have none).
    pub fn system_for(&self, model: Model) -> Option<&PathBuf> {
        match model {
            Model::Apple2 => &self.apple2,
            Model::Apple2Plus => &self.apple2_plus,
            Model::Apple2JPlus => &self.apple2_jplus,
            Model::Apple2e => &self.apple2e,
            Model::Apple2eEnhanced => &self.apple2e_enhanced,
            Model::Pravets82 => &self.pravets82,
            Model::Pravets8M => &self.pravets8m,
            Model::Pravets8A => &self.pravets8a,
            Model::Tk3000e => &self.tk3000e,
            Model::Base64A => &self.base64a,
            Model::Apple2c | Model::BadPravets82 | Model::BadPravets8M => return None,
        }
        .as_ref()
    }

    /// Mutable access to the system ROM entry of `model` (for the settings screen).
    pub fn system_for_mut(&mut self, model: Model) -> Option<&mut Option<PathBuf>> {
        Some(match model {
            Model::Apple2 => &mut self.apple2,
            Model::Apple2Plus => &mut self.apple2_plus,
            Model::Apple2JPlus => &mut self.apple2_jplus,
            Model::Apple2e => &mut self.apple2e,
            Model::Apple2eEnhanced => &mut self.apple2e_enhanced,
            Model::Pravets82 => &mut self.pravets82,
            Model::Pravets8M => &mut self.pravets8m,
            Model::Pravets8A => &mut self.pravets8a,
            Model::Tk3000e => &mut self.tk3000e,
            Model::Base64A => &mut self.base64a,
            Model::Apple2c | Model::BadPravets82 | Model::BadPravets8M => return None,
        })
    }
}

/// What a ROM is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RomRole {
    System,
    F8,
    Video,
    Slot(u8),
}

impl std::fmt::Display for RomRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RomRole::System => f.write_str("system ROM"),
            RomRole::F8 => f.write_str("F8 ROM"),
            RomRole::Video => f.write_str("video ROM"),
            RomRole::Slot(n) => write!(f, "slot {n} ROM"),
        }
    }
}

/// Where the bytes in use came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RomSource {
    /// Embedded in the binary (`apple2-roms`), by original file name.
    Embedded(&'static str),
    /// A file given by the user.
    File(PathBuf),
}

/// One ROM in use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RomInfo {
    pub role: RomRole,
    pub source: RomSource,
    pub size: usize,
    /// Lowercase hex SHA-256 of the bytes.
    pub sha256: String,
}

impl RomInfo {
    fn embedded(role: RomRole, name: &'static str, bytes: &[u8]) -> Self {
        RomInfo {
            role,
            source: RomSource::Embedded(name),
            size: bytes.len(),
            sha256: apple2_roms::sha256_hex(bytes),
        }
    }

    fn file(role: RomRole, path: &Path, bytes: &[u8]) -> Self {
        RomInfo {
            role,
            source: RomSource::File(path.to_path_buf()),
            size: bytes.len(),
            sha256: apple2_roms::sha256_hex(bytes),
        }
    }
}

impl std::fmt::Display for RomInfo {
    /// `system ROM: file /x/y.rom, 16384 bytes, sha256=...` (one log line).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.source {
            RomSource::Embedded(n) => write!(f, "{}: embedded {n}", self.role)?,
            RomSource::File(p) => write!(f, "{}: file {}", self.role, p.display())?,
        }
        write!(f, ", {} bytes, sha256={}", self.size, self.sha256)
    }
}

/// A user ROM that cannot be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RomError {
    /// Missing, unreadable or not a regular file.
    Unreadable {
        role: RomRole,
        path: PathBuf,
        reason: String,
    },
    BadSize {
        role: RomRole,
        path: PathBuf,
        size: u64,
        expected: String,
    },
}

impl std::fmt::Display for RomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RomError::Unreadable { role, path, reason } => {
                write!(f, "custom {role} {}: {reason}", path.display())
            }
            RomError::BadSize {
                role,
                path,
                size,
                expected,
            } => write!(
                f,
                "custom {role} {}: {size} bytes, expected {expected}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for RomError {}

/// "12288 (12K) or 16384 (16K)".
fn describe_sizes(sizes: &[usize]) -> String {
    sizes
        .iter()
        .map(|n| format!("{n} ({}K)", n / 1024))
        .collect::<Vec<_>>()
        .join(" or ")
}

/// Reads `path`, accepting only the sizes in `allowed`. The size is checked on the metadata
/// first, so a huge file is never read. Never panics.
pub fn load_rom_file(role: RomRole, path: &Path, allowed: &[usize]) -> Result<Vec<u8>, RomError> {
    let unreadable = |e: std::io::Error| RomError::Unreadable {
        role,
        path: path.to_path_buf(),
        reason: match e.kind() {
            std::io::ErrorKind::NotFound => "file not found".to_string(),
            _ => e.to_string(),
        },
    };
    let bad_size = |size: u64| RomError::BadSize {
        role,
        path: path.to_path_buf(),
        size,
        expected: describe_sizes(allowed),
    };
    let meta = std::fs::metadata(path).map_err(unreadable)?;
    if !meta.is_file() {
        return Err(RomError::Unreadable {
            role,
            path: path.to_path_buf(),
            reason: "not a regular file".into(),
        });
    }
    if !allowed.iter().any(|&n| n as u64 == meta.len()) {
        return Err(bad_size(meta.len()));
    }
    let bytes = std::fs::read(path).map_err(unreadable)?;
    // The file may have changed between the metadata and the read.
    if !allowed.contains(&bytes.len()) {
        return Err(bad_size(bytes.len() as u64));
    }
    Ok(bytes)
}

/// Sizes accepted by `-rom` (CmdLine.cpp:501).
pub const CUSTOM_ROM_SIZES: [usize; 2] = [0x3000, 0x4000];
/// Size accepted by `-f8rom` (CmdLine.cpp:489).
pub const F8_ROM_SIZES: [usize; 1] = [0x800];
/// Sizes accepted by `-videorom` (Video.cpp:633).
pub const VIDEO_ROM_SIZES: [usize; 4] = [0x800, 0x1000, 0x2000, 0x4000];

/// Settings turned into what the machine and the renderer need, with the ROM provenance.
#[derive(Clone, Debug)]
pub struct ResolvedRoms {
    pub machine: MachineConfig,
    /// User video ROM bytes for the renderer.
    pub video_rom: Option<Vec<u8>>,
    pub report: Vec<RomInfo>,
}

/// File name of the embedded system ROM of `model`.
fn embedded_system_name(model: Model) -> &'static str {
    match model {
        Model::Apple2 => "Apple2.rom",
        Model::Apple2Plus => "Apple2_Plus.rom",
        Model::Apple2JPlus => "Apple2_JPlus.rom",
        Model::Apple2e => "Apple2e.rom",
        Model::Apple2eEnhanced => "Apple2e_Enhanced.rom",
        Model::Pravets82 => "PRAVETS82.ROM",
        Model::Pravets8M => "PRAVETS8M.ROM",
        Model::Pravets8A => "PRAVETS8C.ROM",
        Model::Tk3000e => "TK3000e.rom",
        Model::Base64A => "Base64A.rom",
        Model::Apple2c | Model::BadPravets82 | Model::BadPravets8M => "(none)",
    }
}

/// The embedded character generator used for `model` (as the renderer picks it).
fn embedded_video(model: Model) -> (&'static str, &'static [u8]) {
    match model {
        Model::Apple2 | Model::Apple2Plus => ("Apple2_Video.rom", apple2_roms::APPLE2_VIDEO),
        Model::Apple2JPlus => ("Apple2_JPlus_Video.rom", apple2_roms::APPLE2_JPLUS_VIDEO),
        Model::Base64A => (
            "Base64A_German_Video.rom",
            apple2_roms::BASE64A_GERMAN_VIDEO,
        ),
        _ => (
            "Apple2e_Enhanced_Video.rom",
            apple2_roms::APPLE2E_ENHANCED_VIDEO,
        ),
    }
}

/// Embedded firmware of the card in a slot, if it has one.
fn embedded_slot_rom(spec: &crate::card::SlotCardSpec) -> Option<(&'static str, &'static [u8])> {
    use crate::card::SlotCardSpec as S;
    Some(match spec {
        S::Disk2 {
            firmware_13_sector: true,
            ..
        } => ("DISK2-13sector.rom", apple2_roms::DISK2_13SECTOR),
        S::Disk2 { .. } => ("DISK2.rom", apple2_roms::DISK2),
        S::Ssc { .. } => ("SSC.rom", apple2_roms::SSC),
        S::GenericPrinter(_) => ("Parallel.rom", apple2_roms::PARALLEL),
        S::GenericHdd { .. } => ("HDC-SmartPort.bin", apple2_roms::HDC_SMARTPORT_BIN),
        S::MouseInterface { .. } => ("MouseInterface.rom", apple2_roms::MOUSEINTERFACE),
        _ => return None,
    })
}

impl Settings {
    /// Reads and validates every ROM override and reports all the ROMs in use. The first
    /// invalid file is returned as an error: the machine must not start with it.
    pub fn resolve_roms(&self) -> Result<ResolvedRoms, RomError> {
        let mut c = self.to_machine_config();
        let mut report = Vec::new();
        let model = self.machine.model;

        // System ROM: `-rom` / [machine].custom_rom, else [roms].<model>, else embedded.
        if let Some(p) = &self.machine.custom_rom {
            let b = load_rom_file(RomRole::System, p, &CUSTOM_ROM_SIZES)?;
            report.push(RomInfo::file(RomRole::System, p, &b));
            c.custom_rom = Some(b);
        } else if let (Some(p), Some(embedded)) = (self.roms.system_for(model), model.system_rom())
        {
            let b = load_rom_file(RomRole::System, p, &[embedded.len()])?;
            report.push(RomInfo::file(RomRole::System, p, &b));
            c.custom_rom = Some(b);
        } else if let Some(embedded) = model.system_rom() {
            report.push(RomInfo::embedded(
                RomRole::System,
                embedded_system_name(model),
                embedded,
            ));
        }

        // F8 ROM.
        if let Some(p) = self
            .machine
            .custom_f8_rom
            .as_ref()
            .or(self.roms.f8.as_ref())
        {
            let b = load_rom_file(RomRole::F8, p, &F8_ROM_SIZES)?;
            report.push(RomInfo::file(RomRole::F8, p, &b));
            c.custom_f8_rom = Some(b);
        }
        // Embedded overlay applied by the core after the above (Memory.cpp:2163).
        if self.machine.freeze_f8_rom && model.is_apple2() {
            report.push(RomInfo::embedded(
                RomRole::F8,
                "Freezes_Non-autostart_F8_Rom.rom",
                apple2_roms::FREEZES_NON_AUTOSTART_F8_ROM,
            ));
        }

        // Video ROM.
        let user_video = self.machine.video_rom.as_ref().or(self.roms.video.as_ref());
        let video_rom = match user_video {
            Some(p) => {
                let b = load_rom_file(RomRole::Video, p, &VIDEO_ROM_SIZES)?;
                report.push(RomInfo::file(RomRole::Video, p, &b));
                Some(b)
            }
            None => {
                let (n, b) = embedded_video(model);
                report.push(RomInfo::embedded(RomRole::Video, n, b));
                None
            }
        };

        // Firmware of the slot cards (always embedded).
        for (i, spec) in self.slots.0.iter().enumerate() {
            if let Some((n, b)) = embedded_slot_rom(spec) {
                report.push(RomInfo::embedded(RomRole::Slot(i as u8), n, b));
            }
        }

        Ok(ResolvedRoms {
            machine: c,
            video_rom,
            report,
        })
    }

    /// The ROMs in use, one log line each; the error text when an override is invalid.
    pub fn rom_report_lines(&self) -> Vec<String> {
        match self.resolve_roms() {
            Ok(r) => r.report.iter().map(ToString::to_string).collect(),
            Err(e) => vec![e.to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("a2roms-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Synthetic firmware (never Apple code): a recognisable pattern of `len` bytes.
    fn synthetic(len: usize, seed: u8) -> Vec<u8> {
        (0..len)
            .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
            .collect()
    }

    #[test]
    fn valid_override_replaces_the_bytes() {
        let d = tmpdir("ok");
        let sys = d.join("sys.rom");
        let rom = synthetic(0x4000, 7);
        std::fs::write(&sys, &rom).unwrap();
        let f8 = d.join("f8.rom");
        std::fs::write(&f8, synthetic(0x800, 9)).unwrap();
        let mut s = Settings::default();
        s.roms.apple2e_enhanced = Some(sys.clone());
        s.roms.f8 = Some(f8);
        let r = s.resolve_roms().unwrap();
        assert_eq!(r.machine.custom_rom.as_deref(), Some(&rom[..]));
        assert_eq!(
            r.machine.custom_f8_rom.as_deref().map(<[u8]>::len),
            Some(0x800)
        );
        let sys_info = &r.report[0];
        assert_eq!(sys_info.source, RomSource::File(sys));
        assert_eq!(sys_info.sha256, apple2_roms::sha256_hex(&rom));
        assert!(sys_info.to_string().contains(&sys_info.sha256));
        // the core accepts the config
        assert!(apple2_core::Machine::try_new(r.machine).is_ok());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn command_line_rom_beats_the_roms_table() {
        let d = tmpdir("prec");
        let a = d.join("a.rom");
        let b = d.join("b.rom");
        std::fs::write(&a, synthetic(0x3000, 1)).unwrap();
        std::fs::write(&b, synthetic(0x4000, 2)).unwrap();
        let mut s = Settings::default();
        s.roms.apple2e_enhanced = Some(b);
        s.machine.custom_rom = Some(a.clone());
        let r = s.resolve_roms().unwrap();
        assert_eq!(r.report[0].source, RomSource::File(a));
        assert_eq!(r.report[0].size, 0x3000);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn bad_size_is_an_error() {
        let d = tmpdir("size");
        let f = d.join("x.rom");
        std::fs::write(&f, vec![0u8; 1000]).unwrap();
        // -rom: 12K or 16K only
        let mut s = Settings::default();
        s.machine.custom_rom = Some(f.clone());
        let e = s.resolve_roms().unwrap_err();
        assert!(matches!(e, RomError::BadSize { size: 1000, .. }), "{e}");
        assert!(e.to_string().contains("12288 (12K) or 16384 (16K)"));
        // [roms].<model> must match the model's size: 12K on an //e is wrong
        std::fs::write(&f, vec![0u8; 0x3000]).unwrap();
        let mut s = Settings::default();
        s.roms.apple2e_enhanced = Some(f.clone());
        assert!(matches!(
            s.resolve_roms(),
            Err(RomError::BadSize { size: 0x3000, .. })
        ));
        // -f8rom: exactly 2K
        let mut s = Settings::default();
        s.machine.custom_f8_rom = Some(f.clone());
        assert!(matches!(s.resolve_roms(), Err(RomError::BadSize { .. })));
        // -videorom: 2/4/8/16K
        std::fs::write(&f, vec![0u8; 3000]).unwrap();
        let mut s = Settings::default();
        s.machine.video_rom = Some(f.clone());
        assert!(matches!(s.resolve_roms(), Err(RomError::BadSize { .. })));
        for n in VIDEO_ROM_SIZES {
            std::fs::write(&f, vec![0u8; n]).unwrap();
            assert!(s.resolve_roms().unwrap().video_rom.is_some());
        }
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn missing_file_is_an_error_not_a_panic() {
        let mut s = Settings::default();
        s.roms.video = Some(PathBuf::from("/nonexistent/dir/video.rom"));
        let e = s.resolve_roms().unwrap_err();
        assert!(matches!(e, RomError::Unreadable { .. }));
        assert!(e.to_string().contains("file not found"), "{e}");
        // a directory is not a ROM either
        let d = tmpdir("dir");
        let mut s = Settings::default();
        s.machine.custom_rom = Some(d.clone());
        assert!(matches!(s.resolve_roms(), Err(RomError::Unreadable { .. })));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn without_override_the_hashes_are_the_embedded_ones() {
        let models = [
            Model::Apple2,
            Model::Apple2Plus,
            Model::Apple2JPlus,
            Model::Apple2e,
            Model::Apple2eEnhanced,
            Model::Pravets82,
            Model::Pravets8M,
            Model::Pravets8A,
            Model::Tk3000e,
            Model::Base64A,
        ];
        for m in models {
            let mut s = Settings::default();
            s.machine.model = m;
            let r = s.resolve_roms().unwrap();
            assert!(r.machine.custom_rom.is_none() && r.video_rom.is_none());
            let sys = &r.report[0];
            assert_eq!(sys.role, RomRole::System);
            assert!(matches!(sys.source, RomSource::Embedded(_)));
            assert_eq!(sys.sha256, apple2_roms::sha256_hex(m.system_rom().unwrap()));
            assert_eq!(sys.size, m.system_rom().unwrap().len());
        }
    }

    #[test]
    fn roms_table_round_trips_through_toml() {
        let mut s = Settings::default();
        s.roms.apple2_plus = Some("/x/plus.rom".into());
        s.roms.video = Some("/x/v.rom".into());
        let t = s.to_toml().unwrap();
        assert!(t.contains("[roms]") && t.contains("apple2_plus"), "{t}");
        assert_eq!(Settings::from_toml(&t).unwrap().roms, s.roms);
        // optional: an old file without the table loads
        assert_eq!(
            Settings::from_toml("").unwrap().roms,
            RomSettings::default()
        );
    }
}
