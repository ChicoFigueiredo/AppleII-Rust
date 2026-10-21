//! Firmware images embedded from `roms/` (ADR 0002). Sizes follow AppleWin Memory.cpp.

#![forbid(unsafe_code)]

pub const APPLE2: &[u8] = include_bytes!("../../../roms/Apple2.rom");
pub const APPLE2_JPLUS: &[u8] = include_bytes!("../../../roms/Apple2_JPlus.rom");
pub const APPLE2_JPLUS_VIDEO: &[u8] = include_bytes!("../../../roms/Apple2_JPlus_Video.rom");
pub const APPLE2_PLUS: &[u8] = include_bytes!("../../../roms/Apple2_Plus.rom");
pub const APPLE2_VIDEO: &[u8] = include_bytes!("../../../roms/Apple2_Video.rom");
pub const APPLE2E: &[u8] = include_bytes!("../../../roms/Apple2e.rom");
pub const APPLE2E_ENHANCED: &[u8] = include_bytes!("../../../roms/Apple2e_Enhanced.rom");
pub const APPLE2E_ENHANCED_VIDEO: &[u8] =
    include_bytes!("../../../roms/Apple2e_Enhanced_Video.rom");
pub const BASE64A: &[u8] = include_bytes!("../../../roms/Base64A.rom");
pub const BASE64A_GERMAN_VIDEO: &[u8] = include_bytes!("../../../roms/Base64A_German_Video.rom");
pub const DISK2_13SECTOR: &[u8] = include_bytes!("../../../roms/DISK2-13sector.rom");
pub const DISK2: &[u8] = include_bytes!("../../../roms/DISK2.rom");
pub const FREEZES_NON_AUTOSTART_F8_ROM: &[u8] =
    include_bytes!("../../../roms/Freezes_Non-autostart_F8_Rom.rom");
pub const HDC_SMARTPORT_BIN: &[u8] = include_bytes!("../../../roms/HDC-SmartPort.bin");
pub const HDDRVR_V2_BIN: &[u8] = include_bytes!("../../../roms/Hddrvr-v2.bin");
pub const HDDRVR_BIN: &[u8] = include_bytes!("../../../roms/Hddrvr.bin");
pub const MOCKINGBOARD_D: &[u8] = include_bytes!("../../../roms/Mockingboard-D.rom");
pub const MOUSEINTERFACE: &[u8] = include_bytes!("../../../roms/MouseInterface.rom");
pub const PRAVETS82: &[u8] = include_bytes!("../../../roms/PRAVETS82.ROM");
pub const PRAVETS8C: &[u8] = include_bytes!("../../../roms/PRAVETS8C.ROM");
pub const PRAVETS8M: &[u8] = include_bytes!("../../../roms/PRAVETS8M.ROM");
pub const PARALLEL: &[u8] = include_bytes!("../../../roms/Parallel.rom");
pub const SSC: &[u8] = include_bytes!("../../../roms/SSC.rom");
pub const TK3000E: &[u8] = include_bytes!("../../../roms/TK3000e.rom");
pub const TKCLOCK: &[u8] = include_bytes!("../../../roms/TKClock.rom");
pub const THUNDERCLOCKPLUS: &[u8] = include_bytes!("../../../roms/ThunderClockPlus.rom");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_roms_have_expected_sizes() {
        assert_eq!(APPLE2.len(), 12288);
        assert_eq!(APPLE2_JPLUS.len(), 12288);
        assert_eq!(APPLE2_JPLUS_VIDEO.len(), 2048);
        assert_eq!(APPLE2_PLUS.len(), 12288);
        assert_eq!(APPLE2_VIDEO.len(), 2048);
        assert_eq!(APPLE2E.len(), 16384);
        assert_eq!(APPLE2E_ENHANCED.len(), 16384);
        assert_eq!(APPLE2E_ENHANCED_VIDEO.len(), 4096);
        assert_eq!(BASE64A.len(), 49152);
        assert_eq!(BASE64A_GERMAN_VIDEO.len(), 4096);
        assert_eq!(DISK2_13SECTOR.len(), 256);
        assert_eq!(DISK2.len(), 256);
        assert_eq!(FREEZES_NON_AUTOSTART_F8_ROM.len(), 2048);
        assert_eq!(HDC_SMARTPORT_BIN.len(), 256);
        assert_eq!(HDDRVR_V2_BIN.len(), 256);
        assert_eq!(HDDRVR_BIN.len(), 256);
        assert_eq!(MOCKINGBOARD_D.len(), 2048);
        assert_eq!(MOUSEINTERFACE.len(), 2048);
        assert_eq!(PRAVETS82.len(), 12288);
        assert_eq!(PRAVETS8C.len(), 16384);
        assert_eq!(PRAVETS8M.len(), 12288);
        assert_eq!(PARALLEL.len(), 256);
        assert_eq!(SSC.len(), 2048);
        assert_eq!(TK3000E.len(), 16384);
        assert_eq!(TKCLOCK.len(), 2304);
        assert_eq!(THUNDERCLOCKPLUS.len(), 2048);
    }
}
