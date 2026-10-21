//! Machine configuration (serializable).

use crate::{Model, VideoStandard};
use serde::{Deserialize, Serialize};

/// Card in the //e auxiliary slot (spec item 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuxCardKind {
    Empty,
    /// 80-column card, 1K.
    Col80_1K,
    /// Extended 80-column card, 64K.
    Extended80Col,
    /// RamWorks III, up to 256 banks of 64K.
    RamWorksIII {
        banks: u16,
    },
}

/// Peripheral kinds that can be placed in a slot. Only the RAM cards exist so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardKind {
    LanguageCard16K,
    Saturn128K,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineConfig {
    pub model: Model,
    pub video_standard: VideoStandard,
    pub aux_card: AuxCardKind,
    /// Slots 0-7.
    pub slots: [Option<CardKind>; 8],
    /// Banks (1-8, 16K each) of the Saturn in each slot; unused for other slots. Anything
    /// outside 1-8 is clamped (`kMaxSaturnBanks`, LanguageCard.cpp:324-345).
    #[serde(default = "default_saturn_banks")]
    pub saturn_banks: [u8; 8],
    /// Replaces the model's system ROM when set.
    pub custom_rom: Option<Vec<u8>>,
    /// Use "Freeze's F8" non-autostart ROM instead of the ][ F8.
    pub use_freeze_f8: bool,
    /// `-f8rom`: replaces the 2K at `$F800` (applied before Freeze's F8, as in Memory.cpp:2139).
    #[serde(default)]
    pub custom_f8_rom: Option<Vec<u8>>,
    /// "CPU Type" override: `Some(true)` forces a 65C02, `Some(false)` an NMOS 6502, `None`
    /// follows the model (Utilities.cpp:145, `SetMainCpu`).
    #[serde(default)]
    pub cpu_cmos: Option<bool>,
}

/// A full Saturn 128K (`kMaxSaturnBanks`).
fn default_saturn_banks() -> [u8; 8] {
    [8; 8]
}

/// Invalid [`MachineConfig`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// `custom_rom` length (bytes) is not 16K (//e: 4K internal Cx ROM + 12K) or 1-4 whole pages
    /// of 12K (Memory.cpp:2105-2118; `MaxRomPages` is 4).
    BadRomSize(usize),
    /// `custom_f8_rom` is not exactly 2K.
    BadF8RomSize(usize),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::BadRomSize(n) => {
                write!(f, "custom ROM of {n} bytes: expected 16K or 12K x 1-4")
            }
            ConfigError::BadF8RomSize(n) => write!(f, "custom F8 ROM of {n} bytes: expected 2K"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl MachineConfig {
    /// Checks what `Machine::new` cannot handle (currently the custom ROM size).
    pub fn validate(&self) -> Result<(), ConfigError> {
        match &self.custom_rom {
            _ if matches!(&self.custom_f8_rom, Some(r) if r.len() != 0x800) => Err(
                ConfigError::BadF8RomSize(self.custom_f8_rom.as_ref().map_or(0, Vec::len)),
            ),
            Some(r)
                if r.len() != 0x4000
                    && !(r.len().is_multiple_of(0x3000)
                        && (1..=4).contains(&(r.len() / 0x3000))) =>
            {
                Err(ConfigError::BadRomSize(r.len()))
            }
            _ => Ok(()),
        }
    }

    /// Defaults per model: no Language Card on the ][ but one in slot 0 on the other
    /// ][-family models (the //e has it built in); Extended 80col on the //e family.
    pub fn for_model(model: Model) -> Self {
        let mut slots = [None; 8];
        if model.is_apple2() && model != Model::Apple2 {
            slots[0] = Some(CardKind::LanguageCard16K);
        }
        let aux_card = if model.is_apple2e_family() || model.is_apple2c() {
            AuxCardKind::Extended80Col
        } else {
            AuxCardKind::Empty
        };
        MachineConfig {
            model,
            video_standard: VideoStandard::Ntsc,
            aux_card,
            slots,
            saturn_banks: default_saturn_banks(),
            custom_rom: None,
            use_freeze_f8: false,
            custom_f8_rom: None,
            cpu_cmos: None,
        }
    }

    /// Whether the main CPU is a 65C02 (the override, else the model's).
    pub fn is_cmos(&self) -> bool {
        self.cpu_cmos
            .unwrap_or(self.model.cpu_variant() == apple2_cpu::Variant::Cmos65C02)
    }
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self::for_model(Model::Apple2eEnhanced)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_per_model() {
        let c = MachineConfig::for_model(Model::Apple2);
        assert_eq!(c.slots[0], None);
        assert_eq!(c.aux_card, AuxCardKind::Empty);
        let c = MachineConfig::for_model(Model::Apple2Plus);
        assert_eq!(c.slots[0], Some(CardKind::LanguageCard16K));
        assert!(c.slots[1..].iter().all(Option::is_none));
        let c = MachineConfig::for_model(Model::Apple2e);
        assert_eq!(c.slots[0], None);
        assert_eq!(c.aux_card, AuxCardKind::Extended80Col);
        assert_eq!(c.video_standard, VideoStandard::Ntsc);
        assert!(!c.use_freeze_f8 && c.custom_rom.is_none());
    }

    #[test]
    fn is_serde() {
        fn assert_serde<T: Serialize + for<'a> Deserialize<'a>>() {}
        assert_serde::<MachineConfig>();
    }
}
