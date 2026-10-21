//! Typed description of every peripheral AppleWin can put in a slot (Card.h:5-33, `SS_CARDTYPE`).
//!
//! This crate only *describes* cards; the frontend's card factory builds them.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// `SS_CARDTYPE` with AppleWin's numeric values (Card.h:7-33; saved to the registry, never reorder).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CardType {
    Empty = 0,
    Disk2,
    Ssc,
    MockingboardC,
    GenericPrinter,
    GenericHdd,
    GenericClock,
    MouseInterface,
    Z80,
    Phasor,
    Echo,
    Sam,
    Col80,
    Extended80Col,
    RamWorksIII,
    Uthernet,
    LanguageCard,
    LanguageCardIIe,
    Saturn128K,
    FourPlay,
    SnesMax,
    VidHd,
    Uthernet2,
    MegaAudio,
    SdMusic,
    BreakpointCard,
}

impl CardType {
    const ALL: [CardType; 26] = {
        use CardType::*;
        [
            Empty,
            Disk2,
            Ssc,
            MockingboardC,
            GenericPrinter,
            GenericHdd,
            GenericClock,
            MouseInterface,
            Z80,
            Phasor,
            Echo,
            Sam,
            Col80,
            Extended80Col,
            RamWorksIII,
            Uthernet,
            LanguageCard,
            LanguageCardIIe,
            Saturn128K,
            FourPlay,
            SnesMax,
            VidHd,
            Uthernet2,
            MegaAudio,
            SdMusic,
            BreakpointCard,
        ]
    };

    /// Registry value -> type (`None` for >= `CT_NUM_CARDS`).
    pub fn from_value(v: u32) -> Option<CardType> {
        Self::ALL.get(v as usize).copied()
    }

    pub fn value(self) -> u32 {
        self as u32
    }

    /// Canonical short name (the spelling used by `-sN <card>` where AppleWin has one).
    pub fn name(self) -> &'static str {
        use CardType::*;
        match self {
            Empty => "empty",
            Disk2 => "diskii",
            Ssc => "ssc",
            MockingboardC => "mockingboard",
            GenericPrinter => "parallel",
            GenericHdd => "hdc",
            GenericClock => "clock",
            MouseInterface => "mouse",
            Z80 => "z80",
            Phasor => "phasor",
            Echo => "echo",
            Sam => "sam",
            Col80 => "std80",
            Extended80Col => "ext80",
            RamWorksIII => "rw3",
            Uthernet => "uthernet",
            LanguageCard => "languagecard",
            LanguageCardIIe => "languagecard-iie",
            Saturn128K => "saturn",
            FourPlay => "fourplay",
            SnesMax => "snesmax",
            VidHd => "vidhd",
            Uthernet2 => "uthernet2",
            MegaAudio => "megaaudio",
            SdMusic => "sdmusic",
            BreakpointCard => "breakpointcard",
        }
    }
}

/// AY-3-8910 family chip in a Mockingboard/Phasor socket (`AY891xType`, CmdLine.cpp:277-297).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AyChip {
    Empty,
    #[default]
    Ay8910,
    Ay8912,
    Ay8913,
    Ym2149,
}

/// SSI263 speech chip type (`SSI263Type`, MockingboardDefs.h:10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ssi263Chip {
    #[default]
    Empty,
    Ssi263P,
    Ssi263AP,
}

/// Hard disk controller firmware (`HdcMode`, Harddisk.h:41).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdcFirmware {
    #[default]
    Default,
    SmartPort,
    BlockMode2Devices,
    BlockMode4Devices,
}

/// One Disk II drive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DriveSpec {
    pub image: Option<PathBuf>,
    /// Force write-protect regardless of the file (AppleWin otherwise follows the file's attribute).
    pub write_protect: bool,
    /// `-d1-disconnected` (CmdLine.cpp:152).
    pub connected: bool,
}

impl Default for DriveSpec {
    fn default() -> Self {
        DriveSpec {
            image: None,
            write_protect: false,
            connected: true,
        }
    }
}

/// Parallel printer options (ParallelPrinter.cpp:186-213).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrinterSpec {
    pub dump_to_printer: bool,
    pub convert_encoding: bool,
    pub filter_unprintable: bool,
    pub append: bool,
    pub filename: Option<PathBuf>,
    /// Seconds of idle before the file is closed (AppleWin default 10).
    pub idle_limit: u32,
}

impl Default for PrinterSpec {
    fn default() -> Self {
        PrinterSpec {
            dump_to_printer: false,
            convert_encoding: true,
            filter_unprintable: true,
            append: false,
            filename: None,
            idle_limit: 10,
        }
    }
}

/// Sound card sockets shared by Mockingboard C, Phasor, MegaAudio and SD Music (Mockingboard.cpp:100-170).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundSockets {
    /// AY sockets 0..3 (2 and 3 only on Phasor).
    pub ay: [AyChip; 4],
    /// SSI263 sockets 0 ($Cs20) and 1 ($Cs40). Defaults: kSSI263A_Default/kSSI263B_Default.
    pub ssi263: [Ssi263Chip; 2],
    /// SC-01 present (kSC01_Default = yes).
    pub sc01: bool,
    /// `-sN 6522a-bad` / `6522b-bad`.
    pub bad_6522: [bool; 2],
}

impl Default for SoundSockets {
    fn default() -> Self {
        SoundSockets {
            ay: [AyChip::Ay8910, AyChip::Ay8910, AyChip::Empty, AyChip::Empty],
            ssi263: [Ssi263Chip::Empty, Ssi263Chip::Ssi263AP],
            sc01: true,
            bad_6522: [false; 2],
        }
    }
}

/// What is plugged into one slot, with its per-card options.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "card")]
pub enum SlotCardSpec {
    Empty,
    Disk2 {
        /// 13-sector (P5A) boot firmware (`-sN diskii13`, Disk.cpp:82).
        #[serde(default)]
        firmware_13_sector: bool,
        #[serde(default)]
        drives: [DriveSpec; 2],
    },
    Ssc {
        /// Host serial port name (SerialComms.cpp:98), empty = none.
        #[serde(default)]
        serial_port: String,
        /// `-ssc-tcp-port`: serve the card over TCP instead of a COM port.
        #[serde(default)]
        tcp_port: Option<u16>,
        /// `-dcd` / `-modem`.
        #[serde(default)]
        support_dcd: bool,
    },
    MockingboardC(#[serde(default)] SoundSockets),
    Phasor(#[serde(default)] SoundSockets),
    MegaAudio(#[serde(default)] SoundSockets),
    SdMusic(#[serde(default)] SoundSockets),
    GenericPrinter(#[serde(default)] PrinterSpec),
    GenericHdd {
        #[serde(default)]
        firmware: HdcFirmware,
        /// Up to 8 units (`-sNh1..h8`).
        #[serde(default, with = "opt_paths")]
        images: [Option<PathBuf>; 8],
    },
    GenericClock,
    MouseInterface {
        #[serde(default = "yes")]
        show_crosshair: bool,
        #[serde(default)]
        restrict_to_window: bool,
    },
    Z80,
    Echo,
    Sam,
    Col80,
    Extended80Col,
    RamWorksIII {
        /// 64K banks, 1..=256 (Memory.h:54; default `kDefaultExMemoryBanksRealRW3`).
        #[serde(default = "rw3_default")]
        banks: u16,
    },
    Uthernet {
        /// Host network interface name (PCapBackend.cpp:114).
        #[serde(default)]
        interface: String,
    },
    Uthernet2 {
        #[serde(default)]
        interface: String,
        /// Uthernet2.cpp:1662.
        #[serde(default)]
        virtual_dns: bool,
    },
    LanguageCard,
    LanguageCardIIe,
    Saturn128K {
        /// 16K banks, 1..=8 (LanguageCard.h:106; `saturn64` = 4).
        #[serde(default = "saturn_default")]
        banks: u8,
    },
    FourPlay,
    SnesMax {
        /// `-snes-max-alt-joy1/2`.
        #[serde(default)]
        alt_controller: [bool; 2],
        /// `-snes-max-user-joy1/2` mapping files.
        #[serde(default, with = "opt_paths")]
        user_mapping: [Option<PathBuf>; 2],
    },
    VidHd,
    BreakpointCard,
}

/// TOML has no null: `[Option<PathBuf>; N]` is stored as strings, empty = none.
mod opt_paths {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::path::PathBuf;

    pub fn serialize<S: Serializer, const N: usize>(
        v: &[Option<PathBuf>; N],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let strings: Vec<String> = v
            .iter()
            .map(|p| {
                p.as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
            .collect();
        strings.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        d: D,
    ) -> Result<[Option<PathBuf>; N], D::Error> {
        let strings = Vec::<String>::deserialize(d)?;
        Ok(std::array::from_fn(|i| {
            strings.get(i).filter(|s| !s.is_empty()).map(PathBuf::from)
        }))
    }
}

fn yes() -> bool {
    true
}
fn rw3_default() -> u16 {
    16
}
fn saturn_default() -> u8 {
    8
}

impl SlotCardSpec {
    /// A card of `ty` with default options.
    pub fn new(ty: CardType) -> SlotCardSpec {
        use CardType as C;
        use SlotCardSpec as S;
        match ty {
            C::Empty => S::Empty,
            C::Disk2 => S::Disk2 {
                firmware_13_sector: false,
                drives: Default::default(),
            },
            C::Ssc => S::Ssc {
                serial_port: String::new(),
                tcp_port: None,
                support_dcd: false,
            },
            C::MockingboardC => S::MockingboardC(Default::default()),
            C::Phasor => S::Phasor(SoundSockets {
                ay: [AyChip::Ay8910; 4],
                ..Default::default()
            }),
            C::MegaAudio => S::MegaAudio(Default::default()),
            C::SdMusic => S::SdMusic(Default::default()),
            C::GenericPrinter => S::GenericPrinter(Default::default()),
            C::GenericHdd => S::GenericHdd {
                firmware: HdcFirmware::Default,
                images: Default::default(),
            },
            C::GenericClock => S::GenericClock,
            C::MouseInterface => S::MouseInterface {
                show_crosshair: true,
                restrict_to_window: false,
            },
            C::Z80 => S::Z80,
            C::Echo => S::Echo,
            C::Sam => S::Sam,
            C::Col80 => S::Col80,
            C::Extended80Col => S::Extended80Col,
            C::RamWorksIII => S::RamWorksIII { banks: 16 },
            C::Uthernet => S::Uthernet {
                interface: String::new(),
            },
            C::Uthernet2 => S::Uthernet2 {
                interface: String::new(),
                virtual_dns: false,
            },
            C::LanguageCard => S::LanguageCard,
            C::LanguageCardIIe => S::LanguageCardIIe,
            C::Saturn128K => S::Saturn128K { banks: 8 },
            C::FourPlay => S::FourPlay,
            C::SnesMax => S::SnesMax {
                alt_controller: [false; 2],
                user_mapping: Default::default(),
            },
            C::VidHd => S::VidHd,
            C::BreakpointCard => S::BreakpointCard,
        }
    }

    pub fn card_type(&self) -> CardType {
        use CardType as C;
        use SlotCardSpec as S;
        match self {
            S::Empty => C::Empty,
            S::Disk2 { .. } => C::Disk2,
            S::Ssc { .. } => C::Ssc,
            S::MockingboardC(_) => C::MockingboardC,
            S::Phasor(_) => C::Phasor,
            S::MegaAudio(_) => C::MegaAudio,
            S::SdMusic(_) => C::SdMusic,
            S::GenericPrinter(_) => C::GenericPrinter,
            S::GenericHdd { .. } => C::GenericHdd,
            S::GenericClock => C::GenericClock,
            S::MouseInterface { .. } => C::MouseInterface,
            S::Z80 => C::Z80,
            S::Echo => C::Echo,
            S::Sam => C::Sam,
            S::Col80 => C::Col80,
            S::Extended80Col => C::Extended80Col,
            S::RamWorksIII { .. } => C::RamWorksIII,
            S::Uthernet { .. } => C::Uthernet,
            S::Uthernet2 { .. } => C::Uthernet2,
            S::LanguageCard => C::LanguageCard,
            S::LanguageCardIIe => C::LanguageCardIIe,
            S::Saturn128K { .. } => C::Saturn128K,
            S::FourPlay => C::FourPlay,
            S::SnesMax { .. } => C::SnesMax,
            S::VidHd => C::VidHd,
            S::BreakpointCard => C::BreakpointCard,
        }
    }

    /// AppleWin's new-install card for `slot` (CardManager.cpp:336-365; slot 0 depends on the model).
    pub fn default_for_slot(slot: usize, model: apple2_core::Model) -> SlotCardSpec {
        let ty = match slot {
            0 if model == apple2_core::Model::Apple2 => CardType::Empty,
            0 if model.is_apple2() => CardType::LanguageCard,
            0 => CardType::LanguageCardIIe,
            1 => CardType::GenericPrinter,
            2 => CardType::Ssc,
            4 => CardType::MockingboardC,
            6 => CardType::Disk2,
            _ => CardType::Empty,
        };
        SlotCardSpec::new(ty)
    }
}

impl AyChip {
    /// `AY891xType` registry value (AY8910.h:3).
    pub fn from_value(v: u32) -> Option<Self> {
        [
            AyChip::Empty,
            AyChip::Ay8910,
            AyChip::Ay8912,
            AyChip::Ay8913,
            AyChip::Ym2149,
        ]
        .get(v as usize)
        .copied()
    }
}

impl Ssi263Chip {
    /// `SSI263Type` registry value (MockingboardDefs.h:10); 3 (SC01) is not a socket chip.
    pub fn from_value(v: u32) -> Option<Self> {
        [Ssi263Chip::Empty, Ssi263Chip::Ssi263P, Ssi263Chip::Ssi263AP]
            .get(v as usize)
            .copied()
    }
}

impl HdcFirmware {
    /// `HdcMode` registry value (Harddisk.h:41).
    pub fn from_value(v: u32) -> Option<Self> {
        [
            HdcFirmware::Default,
            HdcFirmware::SmartPort,
            HdcFirmware::BlockMode2Devices,
            HdcFirmware::BlockMode4Devices,
        ]
        .get(v as usize)
        .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_type_values_match_card_h() {
        assert_eq!(CardType::Disk2.value(), 1);
        assert_eq!(CardType::Phasor.value(), 9);
        assert_eq!(CardType::Saturn128K.value(), 18);
        assert_eq!(CardType::BreakpointCard.value(), 25);
        assert_eq!(CardType::from_value(15), Some(CardType::Uthernet));
        assert_eq!(CardType::from_value(26), None);
        for v in 0..26 {
            assert_eq!(CardType::from_value(v).unwrap().value(), v);
            assert_eq!(
                SlotCardSpec::new(CardType::from_value(v).unwrap())
                    .card_type()
                    .value(),
                v
            );
        }
    }
}
