//! Map from AppleWin registry values (Common.h:63-139) to [`Settings`](crate::Settings) fields.
//!
//! Sections are relative to `HKCU\Software\AppleWin\CurrentVersion` (or `[Section]` in the INI
//! used by `-conf` and the Linux port). `Slot N` sections are `Configuration\Slot N`.

/// One registry value and where it lives in our settings.
#[derive(Clone, Copy, Debug)]
pub struct RegKey {
    pub section: &'static str,
    pub value: &'static str,
    /// Dotted path in `Settings` (TOML key path).
    pub field: &'static str,
    pub note: &'static str,
}

const fn k(
    section: &'static str,
    value: &'static str,
    field: &'static str,
    note: &'static str,
) -> RegKey {
    RegKey {
        section,
        value,
        field,
        note,
    }
}

const C: &str = "Configuration";
const P: &str = "Preferences";
const S: &str = "Configuration\\Slot N";

/// Every registry value AppleWin reads or writes, plus deprecated aliases. Entries with an empty
/// `field` are deprecated/ignored (documented so importers do not warn about them).
pub const KEYS: &[RegKey] = &[
    k(
        C,
        "Apple2 Type",
        "machine.model",
        "eApple2Type; bad Pravets/clone values remapped (Utilities.cpp:97)",
    ),
    k(
        C,
        "Computer Emulation",
        "machine.model",
        "deprecated: 0=][ 1=][+ else //e enhanced",
    ),
    k(C, "CPU Type", "machine.cpu", "1=6502 2=65C02"),
    k(C, "Confirm Reboot", "misc.confirm_reboot", ""),
    k(
        C,
        "Full-screen show subunit status",
        "video.fullscreen_show_subunit_status",
        "",
    ),
    k(C, "Show Disk II Status", "video.show_disk_leds", ""),
    k(C, "Master volume", "sound.master_volume", "0=loudest"),
    k(
        C,
        "Speaker Volume",
        "sound.master_volume",
        "deprecated alias",
    ),
    k(C, "Save State Filename", "storage.savestate_filename", ""),
    k(C, "Save State On Exit", "storage.save_state_on_exit", ""),
    k(C, "Harddisk Enable", "", "deprecated (1 => slot 7 HDC)"),
    k(
        C,
        "Joystick0 Emu Type v3",
        "joystick.joystick0",
        "JOY0CHOICE",
    ),
    k(
        C,
        "Joystick1 Emu Type v3",
        "joystick.joystick1",
        "JOY1CHOICE",
    ),
    k(
        C,
        "Joystick0 Emu Type",
        "joystick.joystick0",
        "deprecated GH#434",
    ),
    k(
        C,
        "Joystick1 Emu Type",
        "joystick.joystick1",
        "deprecated GH#434",
    ),
    k(
        C,
        "Joystick 0 Emulation",
        "joystick.joystick0",
        "deprecated <1.24",
    ),
    k(
        C,
        "Joystick 1 Emulation",
        "joystick.joystick1",
        "deprecated <1.24",
    ),
    k(C, "PDL X-Trim", "joystick.pdl_x_trim", ""),
    k(C, "PDL Y-Trim", "joystick.pdl_y_trim", ""),
    k(C, "ScrollLock Toggle", "keyboard.scroll_lock_toggle", ""),
    k(C, "Joystick Cursor Control", "joystick.cursor_control", ""),
    k(
        C,
        "Joystick Centering Control",
        "joystick.centering",
        "1=centering",
    ),
    k(C, "Autofire", "joystick.autofire", "bitmap of buttons"),
    k(C, "Swap buttons 0 and 1", "joystick.swap_buttons", ""),
    k(C, "Shift Key Mod", "joystick.shift_key_mod", ""),
    k(C, "Mouse crosshair", "keyboard.mouse_show_crosshair", ""),
    k(
        C,
        "Mouse restrict to window",
        "keyboard.mouse_restrict_to_window",
        "",
    ),
    k(C, "No-Slot clock", "machine.no_slot_clock", ""),
    k(C, "The Freeze's F8 Rom", "machine.freeze_f8_rom", ""),
    k(C, "CiderPress Location", "storage.ciderpress_path", ""),
    k(C, "Video Emulation", "video.mode", "VideoType_e"),
    k(
        C,
        "Video Style",
        "video.half_scanlines/vertical_blend",
        "bit0 half scanlines, bit1 vertical blend",
    ),
    k(
        C,
        "Half Scan Lines",
        "video.half_scanlines",
        "deprecated <1.28.2",
    ),
    k(
        C,
        "Monochrome Color",
        "video.monochrome_rgb",
        "COLORREF 0x00BBGGRR",
    ),
    k(
        C,
        "Video Refresh Rate",
        "machine.refresh_rate",
        "1=50Hz 2=60Hz",
    ),
    k(
        C,
        "Serial Port Name",
        "slots.N (Ssc).serial_port",
        "per-slot section",
    ),
    k(C, "Enhance Disk Speed", "storage.enhance_disk_speed", ""),
    k(C, "Emulation Speed", "machine.speed", "10=normal"),
    k(C, "Window Scale", "video.window_scale", ""),
    k(C, "Uthernet Active", "", "deprecated: slot 3 Uthernet"),
    k(
        C,
        "Uthernet Interface",
        "slots.N (Uthernet).interface",
        "legacy location; now per-slot",
    ),
    k(C, "Slot 4", "", "deprecated card type for slot 4"),
    k(C, "Slot 5", "", "deprecated card type for slot 5"),
    k(C, "Version", "misc.imported_applewin_version", ""),
    k(S, "Card type", "slots.N", "SS_CARDTYPE"),
    k(
        S,
        "Last Disk Image 1",
        "slots.N (Disk2).drives[0].image",
        "",
    ),
    k(
        S,
        "Last Disk Image 2",
        "slots.N (Disk2).drives[1].image",
        "",
    ),
    k(
        S,
        "13-sector Firmware",
        "slots.N (Disk2).firmware_13_sector",
        "",
    ),
    k(
        S,
        "Last Harddisk Image 1..8",
        "slots.N (GenericHdd).images[0..8]",
        "",
    ),
    k(S, "Firmware", "slots.N (GenericHdd).firmware", "HdcMode"),
    k(
        S,
        "Number of Banks",
        "slots.N (Saturn128K).banks",
        "16K banks",
    ),
    k(
        S,
        "Uthernet Interface",
        "slots.N (Uthernet/Uthernet2).interface",
        "",
    ),
    k(
        S,
        "Uthernet Virtual DNS",
        "slots.N (Uthernet2).virtual_dns",
        "",
    ),
    k(
        S,
        "AY Socket 0..3",
        "slots.N (MockingboardC/Phasor).ay[0..4]",
        "AY891xType",
    ),
    k(
        S,
        "SSI263 Socket 0..1",
        "slots.N (MockingboardC/Phasor).ssi263[0..2]",
        "SSI263Type",
    ),
    k(S, "SC01", "slots.N (MockingboardC/Phasor).sc01", ""),
    k(S, "Serial Port Name", "slots.N (Ssc).serial_port", ""),
    k(
        S,
        "Dump to printer",
        "slots.N (GenericPrinter).dump_to_printer",
        "",
    ),
    k(
        S,
        "Convert printer encoding for clones",
        "slots.N (GenericPrinter).convert_encoding",
        "",
    ),
    k(
        S,
        "Filter unprintable characters",
        "slots.N (GenericPrinter).filter_unprintable",
        "",
    ),
    k(
        S,
        "Printer Filename",
        "slots.N (GenericPrinter).filename",
        "",
    ),
    k(
        S,
        "Append to printer file",
        "slots.N (GenericPrinter).append",
        "",
    ),
    k(
        S,
        "Printer idle limit",
        "slots.N (GenericPrinter).idle_limit",
        "",
    ),
    k(
        "Configuration\\Slot Auxiliary",
        "Card type",
        "aux.card",
        "SS_CARDTYPE (80Col/Extended80Col/RamWorksIII/Empty)",
    ),
    k(
        "Configuration\\Slot Auxiliary",
        "Number of Banks",
        "aux.card (RamWorksIII).banks",
        "",
    ),
    k(
        "Configuration\\Game I/O Connector",
        "Game I/O type",
        "joystick.dongle",
        "DONGLETYPE",
    ),
    k(P, "Starting Directory", "storage.disk_dir", ""),
    k(P, "HDV Starting Directory", "storage.hdv_dir", ""),
    k(P, "Window X-Position", "video.window_pos.0", ""),
    k(P, "Window Y-Position", "video.window_pos.1", ""),
    k(
        P,
        "NewDiskCopyBitsyBoot",
        "storage.new_disk_copy_bitsy_boot",
        "",
    ),
    k(
        P,
        "NewDiskCopyBitsyBye",
        "storage.new_disk_copy_bitsy_bye",
        "",
    ),
    k(P, "NewDiskCopyBASIC", "storage.new_disk_copy_basic", ""),
    k(P, "NewDiskCopyProDOS", "storage.new_disk_copy_prodos", ""),
];

/// Looks up the first entry for `section`/`value` (case-insensitive).
pub fn lookup(section: &str, value: &str) -> Option<&'static RegKey> {
    KEYS.iter()
        .find(|k| k.section.eq_ignore_ascii_case(section) && k.value.eq_ignore_ascii_case(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(
            lookup("configuration", "apple2 type").unwrap().field,
            "machine.model"
        );
        assert!(lookup("Configuration", "nope").is_none());
    }
}
