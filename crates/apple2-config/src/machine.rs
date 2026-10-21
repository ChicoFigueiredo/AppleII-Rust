//! `Settings` -> `apple2_core::MachineConfig`.

use crate::card::SlotCardSpec;
use crate::settings::{CpuType, RefreshRate, Settings};
use apple2_core::{AuxCardKind, CardKind, MachineConfig, VideoStandard};

impl Settings {
    /// [`Settings::to_machine_config`] plus the ROM files named by `-rom`, `-f8rom` and `[roms]`,
    /// validated (see [`crate::roms`]).
    /// An invalid ROM file falls back to the embedded ROM (AppleWin's behaviour); the reasons are
    /// in [`Settings::resolve_roms`]'s `warnings`.
    pub fn to_machine_config_with_roms(&self) -> MachineConfig {
        self.resolve_roms().machine
    }

    /// The `-videorom` / `[roms].video` bytes (for the renderer), if given and valid.
    pub fn read_video_rom(&self) -> Option<Vec<u8>> {
        self.resolve_roms().video_rom
    }

    /// The part of the settings the core models: model, video standard, aux card and the RAM cards
    /// (Language Card / Saturn). Other cards are the frontend's business. ROM files are read by
    /// [`Settings::resolve_roms`] (`machine.custom_rom` is a path here, bytes in the core).
    pub fn to_machine_config(&self) -> MachineConfig {
        let mut c = MachineConfig::for_model(self.machine.model);
        c.video_standard = match self.machine.refresh_rate {
            RefreshRate::Hz50 => VideoStandard::Pal,
            RefreshRate::Hz60 => VideoStandard::Ntsc,
        };
        c.use_freeze_f8 = self.machine.freeze_f8_rom;
        c.ram_init = apple2_core::RamInit {
            pattern: self.machine.memory_clear_type.min(7) as i32,
            random: self.machine.memory_seed.map_or(
                apple2_core::RandomSource::Zero,
                apple2_core::RandomSource::Seed,
            ),
        };
        c.cpu_cmos = match self.machine.cpu {
            CpuType::Auto => None,
            CpuType::Cpu6502 => Some(false),
            CpuType::Cpu65C02 => Some(true),
        };
        let is_e = self.machine.model.is_apple2e_family() || self.machine.model.is_apple2c();
        c.aux_card = if is_e {
            match &self.aux.card {
                SlotCardSpec::Empty => AuxCardKind::Empty,
                SlotCardSpec::Col80 => AuxCardKind::Col80_1K,
                SlotCardSpec::RamWorksIII { banks } => AuxCardKind::RamWorksIII {
                    banks: (*banks).clamp(1, 256),
                },
                _ => AuxCardKind::Extended80Col,
            }
        } else {
            AuxCardKind::Empty
        };
        c.slots = [None; 8];
        c.saturn_banks = [8; 8];
        for (i, spec) in self.slots.0.iter().enumerate() {
            c.slots[i] = match spec {
                // The //e's Language Card is built in; it is not a slot card.
                SlotCardSpec::LanguageCard if !is_e => Some(CardKind::LanguageCard16K),
                SlotCardSpec::Saturn128K { banks } => {
                    c.saturn_banks[i] = (*banks).clamp(1, 8); // AUD-C-002: never beyond the card
                    Some(CardKind::Saturn128K)
                }
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
        let expected = MachineConfig {
            ram_init: apple2_core::RamInit::APPLEWIN_DETERMINISTIC, // `-memclear` default 2
            ..MachineConfig::default()
        };
        assert_eq!(Settings::default().to_machine_config(), expected);
    }

    #[test]
    fn memclear_and_seed_reach_the_core_config() {
        use apple2_core::{RamInit, RandomSource};
        let mut s = Settings::default();
        assert_eq!(
            s.to_machine_config().ram_init,
            RamInit::APPLEWIN_DETERMINISTIC
        );
        s.machine.memory_clear_type = 7;
        s.machine.memory_seed = Some(42);
        let r = s.to_machine_config().ram_init;
        assert_eq!((r.pattern, r.random), (7, RandomSource::Seed(42)));
        // Without a seed the host adds one from the clock; the machine itself stays deterministic.
        s.machine.memory_seed = None;
        let r = s.to_machine_config_with_roms().ram_init;
        assert!(matches!(r.random, RandomSource::Seed(_)));
    }

    #[test]
    fn cpu_override_and_roms() {
        use crate::settings::CpuType;
        let mut s = Settings::default();
        assert_eq!(s.to_machine_config().cpu_cmos, None);
        s.machine.cpu = CpuType::Cpu6502;
        assert_eq!(s.to_machine_config().cpu_cmos, Some(false));
        s.machine.cpu = CpuType::Cpu65C02;
        assert_eq!(s.to_machine_config().cpu_cmos, Some(true));
        let dir = std::env::temp_dir().join(format!("a2cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f8 = dir.join("f8.rom");
        std::fs::write(&f8, vec![1u8; 0x800]).unwrap();
        s.machine.custom_f8_rom = Some(f8);
        let c = s.to_machine_config_with_roms();
        assert_eq!(c.custom_f8_rom.as_deref().map(<[u8]>::len), Some(0x800));
        s.machine.video_rom = Some(dir.join("missing.rom"));
        assert!(s.read_video_rom().is_none());
        assert_eq!(s.rom_problems().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn plus_with_lc_pal_rw3() {
        let mut s = Settings::default();
        s.machine.model = Model::Apple2Plus;
        s.reset_slot0_for_model();
        s.machine.refresh_rate = RefreshRate::Hz50;
        let c = s.to_machine_config();
        assert_eq!(c, {
            let mut x = MachineConfig::for_model(Model::Apple2Plus);
            x.video_standard = VideoStandard::Pal;
            x.ram_init = apple2_core::RamInit::APPLEWIN_DETERMINISTIC;
            x
        });
        let mut e = Settings::default();
        e.aux.card = SlotCardSpec::RamWorksIII { banks: 300 };
        *e.slot_mut(3) = SlotCardSpec::Saturn128K { banks: 8 };
        let c = e.to_machine_config();
        assert_eq!(c.aux_card, AuxCardKind::RamWorksIII { banks: 256 });
        assert_eq!(c.slots[3], Some(CardKind::Saturn128K));
    }
}
