//! TOML persistence in the platform config directory.

use crate::{Settings, SlotCardSpec};
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

/// Platform config directory for the emulator (`~/.config/apple2-rust`, `%APPDATA%\...`,
/// `~/Library/Application Support/...`), `None` when no home directory exists.
pub fn config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "AppleII-Rust", "apple2-rust")
        .map(|d| d.config_dir().to_path_buf())
}

/// Config directory used before the product was renamed to `apple2-rust` (ADR 0007).
pub fn legacy_config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "AppleII-Rust", "applewin-rs")
        .map(|d| d.config_dir().to_path_buf())
}

/// Files the emulator keeps in the config directory (copied by the migration).
pub const MIGRATED_FILES: [&str; 2] = [FILE_NAME, "recent_disks.txt"];

/// Result of [`migrate_legacy_dir`].
#[derive(Debug, PartialEq, Eq)]
pub enum Migration {
    /// Nothing to do: the new directory already has settings, or the old one has none.
    NotNeeded,
    /// Files copied from the old directory (which is never touched).
    Migrated { copied: Vec<&'static str> },
    /// A copy failed; the message says which.
    Failed(String),
}

/// First-run migration: when `new` has no `settings.toml` and `old` has one, copies the
/// emulator's files from `old` to `new` (never overwriting, never deleting anything in `old`).
pub fn migrate_legacy_dir(old: &Path, new: &Path) -> Migration {
    if new.join(FILE_NAME).exists() || !old.join(FILE_NAME).is_file() {
        return Migration::NotNeeded;
    }
    if let Err(e) = std::fs::create_dir_all(new) {
        return Migration::Failed(format!("cannot create {}: {e}", new.display()));
    }
    let mut copied = Vec::new();
    for name in MIGRATED_FILES {
        let (from, to) = (old.join(name), new.join(name));
        if from.is_file() && !to.exists() {
            if let Err(e) = std::fs::copy(&from, &to) {
                return Migration::Failed(format!("cannot copy {}: {e}", from.display()));
            }
            copied.push(name);
        }
    }
    Migration::Migrated { copied }
}

/// Runs the migration between the platform directories and returns the warning to show the
/// user (`None` when nothing happened). Call once at startup, before loading the settings.
pub fn migrate_legacy_default() -> Option<String> {
    let (old, new) = (legacy_config_dir()?, config_dir()?);
    migration_notice(&old, &new, &migrate_legacy_dir(&old, &new))
}

/// The user-facing text for a migration result.
pub fn migration_notice(old: &Path, new: &Path, result: &Migration) -> Option<String> {
    match result {
        Migration::NotNeeded => None,
        Migration::Migrated { .. } => Some(format!(
            "Settings copied from {} to {} (apple2-rust); the old directory was kept",
            old.display(),
            new.display()
        )),
        Migration::Failed(e) => Some(format!("Settings migration failed: {e}")),
    }
}

static PATH_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// `-conf <file>`: makes [`default_path`] (and so `load_default` / `save_default`) use `path`.
/// Only the first call has an effect; returns false if a path was already set.
pub fn set_path_override(path: PathBuf) -> bool {
    PATH_OVERRIDE.set(path).is_ok()
}

/// Default settings file path.
pub fn default_path() -> Option<PathBuf> {
    PATH_OVERRIDE
        .get()
        .cloned()
        .or_else(|| config_dir().map(|d| d.join(FILE_NAME)))
}

impl Settings {
    /// Parses TOML. Unknown keys are ignored and missing ones take AppleWin's defaults.
    pub fn from_toml(text: &str) -> Result<Settings, StoreError> {
        toml::from_str(text).map_err(StoreError::Parse)
    }

    pub fn to_toml(&self) -> Result<String, StoreError> {
        toml::to_string_pretty(self).map_err(StoreError::Serialize)
    }

    /// What reaches the disk. `-s7-empty-on-exit` (AppleWin.cpp:1042, GH#790) leaves slot 7 empty
    /// in the saved configuration; the switch itself is per run and is never saved.
    fn persisted(&self) -> Settings {
        let mut s = self.clone();
        if s.storage.slot7_empty_on_exit {
            s.slots.0[7] = SlotCardSpec::Empty;
            s.storage.slot7_empty_on_exit = false;
        }
        s
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
        let text = self.persisted().to_toml()?;
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
        *s.slot_mut(7) = SlotCardSpec::GenericHdd {
            firmware: HdcFirmware::SmartPort,
            images: std::array::from_fn(|i| (i == 2).then(|| "h.hdv".into())),
        };
        *s.slot_mut(5) = SlotCardSpec::Uthernet2 {
            interface: "eth0".into(),
            virtual_dns: true,
        };
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

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("apple2-mig-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn migration_copies_old_settings_when_new_has_none() {
        let root = tmp("copy");
        let (old, new) = (root.join("applewin-rs"), root.join("apple2-rust"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(FILE_NAME), "[video]\nwindow_scale = 3\n").unwrap();
        std::fs::write(old.join("recent_disks.txt"), "a.dsk\n").unwrap();
        let r = migrate_legacy_dir(&old, &new);
        assert_eq!(
            r,
            Migration::Migrated {
                copied: vec![FILE_NAME, "recent_disks.txt"]
            }
        );
        assert_eq!(
            Settings::load(&new.join(FILE_NAME))
                .unwrap()
                .video
                .window_scale,
            3
        );
        assert_eq!(
            std::fs::read_to_string(new.join("recent_disks.txt")).unwrap(),
            "a.dsk\n"
        );
        // the old directory is never removed or altered
        assert!(old.join(FILE_NAME).is_file() && old.join("recent_disks.txt").is_file());
        assert!(migration_notice(&old, &new, &r)
            .unwrap()
            .contains("old directory was kept"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn migration_leaves_everything_alone_when_both_exist() {
        let root = tmp("both");
        let (old, new) = (root.join("applewin-rs"), root.join("apple2-rust"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join(FILE_NAME), "old").unwrap();
        std::fs::write(old.join("recent_disks.txt"), "old.dsk\n").unwrap();
        std::fs::write(new.join(FILE_NAME), "new").unwrap();
        let r = migrate_legacy_dir(&old, &new);
        assert_eq!(r, Migration::NotNeeded);
        assert_eq!(std::fs::read_to_string(new.join(FILE_NAME)).unwrap(), "new");
        assert!(!new.join("recent_disks.txt").exists());
        assert_eq!(migration_notice(&old, &new, &r), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn migration_with_neither_directory_does_nothing() {
        let root = tmp("none");
        let (old, new) = (root.join("applewin-rs"), root.join("apple2-rust"));
        assert_eq!(migrate_legacy_dir(&old, &new), Migration::NotNeeded);
        assert!(!new.exists() && !old.exists());
        assert_eq!(
            Settings::load(&new.join(FILE_NAME)).unwrap(),
            Settings::default()
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
