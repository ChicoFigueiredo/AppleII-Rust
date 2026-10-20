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
    RamWorksIII { banks: u16 },
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
    /// Replaces the model's system ROM when set.
    pub custom_rom: Option<Vec<u8>>,
    /// Use "Freeze's F8" non-autostart ROM instead of the ][ F8.
    pub use_freeze_f8: bool,
}

impl MachineConfig {
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
            custom_rom: None,
            use_freeze_f8: false,
        }
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
