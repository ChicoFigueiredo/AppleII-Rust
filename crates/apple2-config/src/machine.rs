//! `Settings` -> `apple2_core::MachineConfig`.

use crate::card::SlotCardSpec;
use crate::settings::{RefreshRate, Settings};
use apple2_core::{AuxCardKind, CardKind, MachineConfig, VideoStandard};

impl Settings {
    /// The part of the settings the core models: model, video standard, aux card and the RAM cards
    /// (Language Card / Saturn). Other cards are the frontend's business. The custom ROM is read
    /// by the frontend (`machine.custom_rom` is a path here, bytes in the core).
    pub fn to_machine_config(&self) -> MachineConfig {
        let mut c = MachineConfig::for_model(self.machine.model);
        c.video_standard = match self.machine.refresh_rate {
            RefreshRate::Hz50 => VideoStandard::Pal,
            RefreshRate::Hz60 => VideoStandard::Ntsc,
        };
        c.use_freeze_f8 = self.machine.freeze_f8_rom;
        let is_e = self.machine.model.is_apple2e_family() || self.machine.model.is_apple2c();
        c.aux_card = if is_e {
            match &self.aux.card {
                SlotCardSpec::Empty => AuxCardKind::Empty,
                SlotCardSpec::Col80 => AuxCardKind::Col80_1K,
                SlotCardSpec::RamWorksIII { banks } => AuxCardKind::RamWorksIII { banks: (*banks).clamp(1, 256) },
                _ => AuxCardKind::Extended80Col,
            }
        } else {
            AuxCardKind::Empty
        };
        c.slots = [None; 8];
        for (i, spec) in self.slots.0.iter().enumerate() {
            c.slots[i] = match spec {
                // The //e's Language Card is built in; it is not a slot card.
                SlotCardSpec::LanguageCard if !is_e => Some(CardKind::LanguageCard16K),
                SlotCardSpec::Saturn128K { .. } => Some(CardKind::Saturn128K),
                _ => None,
            };
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_core::Model;

    #[test]
    fn default_equals_core_default() {
        assert_eq!(Settings::default().to_machine_config(), MachineConfig::default());
    }

    #[test]
    fn plus_with_lc_pal_rw3() {
        let mut s = Settings::default();
        s.machine.model = Model::Apple2Plus;
        s.reset_slot0_for_model();
        s.machine.refresh_rate = RefreshRate::Hz50;
        let c = s.to_machine_config();
        assert_eq!(c, { let mut x = MachineConfig::for_model(Model::Apple2Plus); x.video_standard = VideoStandard::Pal; x });
        let mut e = Settings::default();
        e.aux.card = SlotCardSpec::RamWorksIII { banks: 300 };
        *e.slot_mut(3) = SlotCardSpec::Saturn128K { banks: 8 };
        let c = e.to_machine_config();
        assert_eq!(c.aux_card, AuxCardKind::RamWorksIII { banks: 256 });
        assert_eq!(c.slots[3], Some(CardKind::Saturn128K));
    }
}
