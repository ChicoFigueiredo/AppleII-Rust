//! TOML persistence in the platform config directory.

use crate::Settings;
use std::path::{Path, PathBuf};

/// Config file name inside the config directory.
pub const FILE_NAME: &str = "settings.toml";

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    Serialize(toml::ser::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "I/O error: {e}"),
            StoreError::Parse(e) => write!(f, "invalid settings file: {e}"),
            StoreError::Serialize(e) => write!(f, "cannot serialize settings: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

/// Platform config directory for the emulator (`~/.config/applewin-rs`, `%APPDATA%\...`,
/// `~/Library/Application Support/...`), `None` when no home directory exists.
pub fn config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "AppleII-Rust", "applewin-rs").map(|d| d.config_dir().to_path_buf())
}

static PATH_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// `-conf <file>`: makes [`default_path`] (and so `load_default` / `save_default`) use `path`.
/// Only the first call has an effect; returns false if a path was already set.
pub fn set_path_override(path: PathBuf) -> bool {
    PATH_OVERRIDE.set(path).is_ok()
}

/// Default settings file path.
pub fn default_path() -> Option<PathBuf> {
    PATH_OVERRIDE.get().cloned().or_else(|| config_dir().map(|d| d.join(FILE_NAME)))
}

impl Settings {
    /// Parses TOML. Unknown keys are ignored and missing ones take AppleWin's defaults.
    pub fn from_toml(text: &str) -> Result<Settings, StoreError> {
        toml::from_str(text).map_err(StoreError::Parse)
    }

    pub fn to_toml(&self) -> Result<String, StoreError> {
        toml::to_string_pretty(self).map_err(StoreError::Serialize)
    }

    /// Loads `path`; a missing file yields the defaults.
    pub fn load(path: &Path) -> Result<Settings, StoreError> {
        match std::fs::read_to_string(path) {
            Ok(t) => Settings::from_toml(&t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Saves atomically (temp file + rename), creating parent directories.
    pub fn save(&self, path: &Path) -> Result<(), StoreError> {
        let text = self.to_toml()?;
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Loads from the platform default location (defaults when there is none).
    pub fn load_default() -> Result<Settings, StoreError> {
        match default_path() {
            Some(p) => Settings::load(&p),
            None => Ok(Settings::default()),
        }
    }

    pub fn save_default(&self) -> Result<(), StoreError> {
        match default_path() {
            Some(p) => self.save(&p),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::*;
    use apple2_core::Model;

    #[test]
    fn defaults_match_applewin() {
        let s = Settings::default();
        assert_eq!(s.machine.model, Model::Apple2eEnhanced);
        assert_eq!(s.machine.speed, 10);
        assert!(s.storage.enhance_disk_speed && s.misc.confirm_reboot && s.machine.no_slot_clock);
        assert_eq!(s.video.window_scale, 2);
        assert_eq!(s.video.monochrome_rgb, 0xC0C0C0);
        assert_eq!(s.slot(1).card_type(), CardType::GenericPrinter);
        assert_eq!(s.slot(2).card_type(), CardType::Ssc);
        assert_eq!(s.slot(4).card_type(), CardType::MockingboardC);
        assert_eq!(s.slot(6).card_type(), CardType::Disk2);
        assert_eq!(s.slot(0).card_type(), CardType::LanguageCardIIe);
        assert_eq!(s.aux.card, SlotCardSpec::Extended80Col);
    }

    #[test]
    fn roundtrip_with_rich_cards() {
        let mut s = Settings::default();
        s.machine.model = Model::Apple2Plus;
        s.reset_slot0_for_model();
        s.disk_drives_mut(6)[0].image = Some("games/a.dsk".into());
        *s.slot_mut(7) = SlotCardSpec::GenericHdd { firmware: HdcFirmware::SmartPort, images: std::array::from_fn(|i| (i == 2).then(|| "h.hdv".into())) };
        *s.slot_mut(5) = SlotCardSpec::Uthernet2 { interface: "eth0".into(), virtual_dns: true };
        *s.slot_mut(3) = SlotCardSpec::Saturn128K { banks: 4 };
        s.video.window_pos = Some((10, -20));
        let text = s.to_toml().unwrap();
        assert_eq!(Settings::from_toml(&text).unwrap(), s);
    }

    #[test]
    fn missing_and_unknown_fields_are_tolerated() {
        let s = Settings::from_toml("future_key = 1\n[video]\nwindow_scale = 3\nnew_thing = true\n[slots.4]\ncard = \"Teleporter\"\n").unwrap();
        assert_eq!(s.video.window_scale, 3);
        assert_eq!(s.machine, Default::default());
        assert_eq!(s.slot(4).card_type(), CardType::MockingboardC); // unknown card -> default
        assert_eq!(Settings::from_toml("").unwrap(), Settings::default());
    }

    #[test]
    fn save_then_load_and_missing_file() {
        let dir = std::env::temp_dir().join(format!("apple2-config-test-{}", std::process::id()));
        let path = dir.join("nested").join(FILE_NAME);
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());
        let mut s = Settings::default();
        s.storage.save_state_on_exit = true;
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), s);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
