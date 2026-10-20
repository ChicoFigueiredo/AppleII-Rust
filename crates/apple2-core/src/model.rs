//! Apple II models, with the numeric values of AppleWin's `eApple2Type`.

use apple2_cpu::Variant;
use serde::{Deserialize, Serialize};

const APPLE2E_MASK: u16 = 0x10;
const APPLE2C_MASK: u16 = 0x20;
const CLONE_MASK: u16 = 0x100;

/// Computer model. Values match AppleWin `eApple2Type` (Common.h:177-211); they are persisted
/// in save-states, so they must not change.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Model {
    Apple2 = 0,
    Apple2Plus = 1,
    Apple2JPlus = 2,
    Apple2e = 0x10,
    Apple2eEnhanced = 0x11,
    /// Skeleton only, as in AppleWin: no ROM.
    Apple2c = 0x20,
    Pravets8M = 0x101,
    Pravets82 = 0x102,
    Base64A = 0x103,
    /// Pravets 82 wrongly tagged as //e clone (< AppleWin 1.26).
    BadPravets82 = 0x110,
    /// Pravets 8M wrongly tagged as //e clone (< AppleWin 1.26).
    BadPravets8M = 0x111,
    Pravets8A = 0x112,
    Tk3000e = 0x113,
}

/// Error for a `u16` that is not an `eApple2Type` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownModel(pub u16);

impl TryFrom<u16> for Model {
    type Error = UnknownModel;
    fn try_from(v: u16) -> Result<Self, UnknownModel> {
        use Model::*;
        Ok(match v {
            0 => Apple2,
            1 => Apple2Plus,
            2 => Apple2JPlus,
            0x10 => Apple2e,
            0x11 => Apple2eEnhanced,
            0x20 => Apple2c,
            0x101 => Pravets8M,
            0x102 => Pravets82,
            0x103 => Base64A,
            0x110 => BadPravets82,
            0x111 => BadPravets8M,
            0x112 => Pravets8A,
            0x113 => Tk3000e,
            _ => return Err(UnknownModel(v)),
        })
    }
}

impl Model {
    pub fn value(self) -> u16 {
        self as u16
    }

    /// `APPLE2E_MASK` set (Common.h:177 area).
    pub fn is_apple2e_family(self) -> bool {
        self.value() & APPLE2E_MASK != 0
    }

    pub fn is_apple2c(self) -> bool {
        self.value() & APPLE2C_MASK != 0
    }

    pub fn is_clone(self) -> bool {
        self.value() & CLONE_MASK != 0
    }

    /// `IS_APPLE2`: neither //e family nor //c (Common.h:190).
    pub fn is_apple2(self) -> bool {
        self.value() & (APPLE2E_MASK | APPLE2C_MASK) == 0
    }

    /// 65C02 only on the Enhanced //e, TK3000 //e and //c; all others are NMOS.
    pub fn cpu_variant(self) -> Variant {
        match self {
            Model::Apple2eEnhanced | Model::Tk3000e | Model::Apple2c => Variant::Cmos65C02,
            _ => Variant::Nmos6502,
        }
    }

    /// Firmware image for the model (AppleWin Memory.cpp:2054-2086); `None` for the //c
    /// and the two "bad" Pravets tags, which AppleWin has no ROM for.
    pub fn system_rom(self) -> Option<&'static [u8]> {
        use apple2_roms as r;
        Some(match self {
            Model::Apple2 => r::APPLE2,
            Model::Apple2Plus => r::APPLE2_PLUS,
            Model::Apple2JPlus => r::APPLE2_JPLUS,
            Model::Apple2e => r::APPLE2E,
            Model::Apple2eEnhanced => r::APPLE2E_ENHANCED,
            Model::Pravets82 => r::PRAVETS82,
            Model::Pravets8M => r::PRAVETS8M,
            Model::Pravets8A => r::PRAVETS8C,
            Model::Tk3000e => r::TK3000E,
            Model::Base64A => r::BASE64A,
            Model::Apple2c | Model::BadPravets82 | Model::BadPravets8M => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Model; 13] = [
        Model::Apple2, Model::Apple2Plus, Model::Apple2JPlus, Model::Apple2e,
        Model::Apple2eEnhanced, Model::Apple2c, Model::Pravets8M, Model::Pravets82,
        Model::Base64A, Model::BadPravets82, Model::BadPravets8M, Model::Pravets8A, Model::Tk3000e,
    ];

    #[test]
    fn numeric_values_match_applewin() {
        let v: Vec<u16> = ALL.iter().map(|m| m.value()).collect();
        assert_eq!(v, [0, 1, 2, 0x10, 0x11, 0x20, 0x101, 0x102, 0x103, 0x110, 0x111, 0x112, 0x113]);
    }

    #[test]
    fn try_from_roundtrips_and_rejects_unknown() {
        for m in ALL {
            assert_eq!(Model::try_from(m.value()), Ok(m));
        }
        assert_eq!(Model::try_from(0x12), Err(UnknownModel(0x12)));
        assert_eq!(Model::try_from(0x104), Err(UnknownModel(0x104)));
    }

    #[test]
    fn family_predicates() {
        assert!(Model::Apple2.is_apple2() && !Model::Apple2.is_clone());
        assert!(Model::Pravets82.is_apple2() && Model::Pravets82.is_clone());
        assert!(Model::Apple2e.is_apple2e_family() && !Model::Apple2e.is_apple2());
        assert!(Model::Tk3000e.is_apple2e_family() && Model::Tk3000e.is_clone());
        assert!(Model::Apple2c.is_apple2c() && !Model::Apple2c.is_apple2());
    }

    #[test]
    fn cmos_only_on_enhanced_tk3000_and_iic() {
        let cmos: Vec<Model> = ALL.iter().copied().filter(|m| m.cpu_variant() == Variant::Cmos65C02).collect();
        assert_eq!(cmos, [Model::Apple2eEnhanced, Model::Apple2c, Model::Tk3000e]);
    }

    #[test]
    fn system_rom_sizes() {
        assert_eq!(Model::Apple2.system_rom().unwrap().len(), 12 * 1024);
        assert_eq!(Model::Pravets82.system_rom().unwrap().len(), 12 * 1024);
        assert_eq!(Model::Apple2e.system_rom().unwrap().len(), 16 * 1024);
        assert_eq!(Model::Pravets8A.system_rom().unwrap().len(), 16 * 1024);
        assert_eq!(Model::Base64A.system_rom().unwrap().len(), 48 * 1024);
        assert!(Model::Apple2c.system_rom().is_none());
    }

    #[test]
    fn serde_roundtrip_json_free() {
        // serde derive compiles and Model is Serialize/Deserialize.
        fn assert_serde<T: Serialize + for<'a> Deserialize<'a>>() {}
        assert_serde::<Model>();
    }
}
