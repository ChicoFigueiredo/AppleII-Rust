//! Importers for existing AppleWin configurations: a Windows registry export (`.reg`) and the
//! INI file used by `-conf` / the Linux port (Registry.cpp:34-55). Both reduce to a
//! [`RegDump`] and are interpreted like `LoadConfiguration` (Utilities.cpp:79-345).

use crate::card::*;
use crate::settings::*;
use crate::Settings;
use apple2_core::Model;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// `section (lower-case, relative to CurrentVersion) -> value name (lower-case) -> text`.
pub type RegDump = BTreeMap<String, BTreeMap<String, String>>;

fn insert(d: &mut RegDump, section: &str, key: &str, value: String) {
    d.entry(section.to_ascii_lowercase()).or_default().insert(key.to_ascii_lowercase(), value);
}

/// Decodes a `.reg` file: UTF-16LE with BOM (what regedit writes) or plain UTF-8/ANSI.
pub fn decode_reg_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..].as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)).into_owned()
    }
}

/// Parses `"Name"="text"` / `"Name"=dword:0000000a` lines under
/// `[HKEY_...\Software\AppleWin\CurrentVersion\<section>]` headers; other keys are skipped.
pub fn parse_reg_text(text: &str) -> RegDump {
    const MARK: &str = "\\software\\applewin\\currentversion\\";
    let mut d = RegDump::new();
    let mut section: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(h) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let lower = h.to_ascii_lowercase();
            section = lower.find(MARK).map(|i| h[i + MARK.len()..].to_string());
        } else if let (Some(sec), Some(rest)) = (&section, line.strip_prefix('"')) {
            let Some((name, val)) = split_quoted(rest) else { continue };
            let Some(val) = val.strip_prefix('=') else { continue };
            let val = if let Some(q) = val.strip_prefix('"') {
                match split_quoted(q) {
                    Some((s, _)) => s,
                    None => continue,
                }
            } else if let Some(h) = val.strip_prefix("dword:") {
                match u32::from_str_radix(h.trim(), 16) {
                    Ok(v) => v.to_string(),
                    Err(_) => continue,
                }
            } else {
                continue; // hex:/binary values are not used by AppleWin
            };
            insert(&mut d, sec, &name, val);
        }
    }
    d
}

/// Reads up to the closing unescaped quote; returns (unescaped, remainder after the quote).
fn split_quoted(s: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => out.push(chars.next()?.1),
            '"' => return Some((out, &s[i + 1..])),
            c => out.push(c),
        }
    }
    None
}

/// Parses AppleWin's INI configuration (`[Configuration\Slot 6]` / `key=value`).
pub fn parse_ini_text(text: &str) -> RegDump {
    let mut d = RegDump::new();
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(h) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = h.to_string();
        } else if let Some((k, v)) = line.split_once('=') {
            insert(&mut d, &section, k.trim(), v.trim().to_string());
        }
    }
    d
}

struct View<'a>(&'a RegDump);

impl View<'_> {
    fn s(&self, section: &str, key: &str) -> Option<&str> {
        self.0.get(&section.to_ascii_lowercase())?.get(&key.to_ascii_lowercase()).map(String::as_str)
    }
    fn n(&self, section: &str, key: &str) -> Option<u32> {
        let v = self.s(section, key)?.trim();
        match v.strip_prefix("0x") {
            Some(h) => u32::from_str_radix(h, 16).ok(),
            None => v.parse::<i64>().ok().map(|x| x as u32),
        }
    }
    fn b(&self, section: &str, key: &str) -> Option<bool> {
        self.n(section, key).map(|v| v != 0)
    }
    fn path(&self, section: &str, key: &str) -> Option<PathBuf> {
        self.s(section, key).filter(|s| !s.is_empty()).map(PathBuf::from)
    }
}

fn set<T>(dst: &mut T, v: Option<T>) {
    if let Some(v) = v {
        *dst = v;
    }
}

/// Remaps registry model values like `LoadConfiguration` (Utilities.cpp:97-125).
fn model_from_registry(v: u32) -> Model {
    match v {
        0x110 | 0x102 | 0x100 => Model::Pravets82,
        0x111 => Model::Pravets8M,
        v => u16::try_from(v).ok().and_then(|v| Model::try_from(v).ok()).unwrap_or(Model::Apple2eEnhanced),
    }
}

/// Interprets a dump with AppleWin's loading rules; absent values keep our defaults (= AppleWin's).
pub fn settings_from_dump(dump: &RegDump) -> Settings {
    let r = View(dump);
    let c = "Configuration";
    let mut s = Settings::default();

    // Model and CPU
    if let Some(v) = r.n(c, "Apple2 Type") {
        s.machine.model = model_from_registry(v);
    } else if let Some(v) = r.n(c, "Computer Emulation") {
        s.machine.model = match v {
            0 => Model::Apple2,
            1 => Model::Apple2Plus,
            _ => Model::Apple2eEnhanced,
        };
    }
    s.machine.cpu = match r.n(c, "CPU Type") {
        Some(1) => CpuType::Cpu6502,
        _ => CpuType::Auto,
    };

    // Joysticks (with the pre-1.24 fallback)
    for j in 0..2 {
        let key = |sfx: &str| format!("Joystick{j}{sfx}");
        let v = r.n(c, &key(" Emu Type v3")).or_else(|| r.n(c, &key(" Emu Type")));
        if j == 0 {
            if let Some(k) = v.and_then(Joystick0Kind::from_value) {
                s.joystick.joystick0 = k;
            }
        } else if let Some(k) = v.and_then(Joystick1Kind::from_value) {
            s.joystick.joystick1 = k;
        }
        if v.is_none() {
            if let Some(old) = r.n(c, &format!("Joystick {j} Emulation")) {
                let kind = match old {
                    1 => 1,
                    2 | 3 => 3,
                    4 => 4,
                    _ => 0,
                };
                if old == 2 || old == 3 {
                    s.joystick.centering = old == 3;
                }
                if j == 0 {
                    s.joystick.joystick0 = Joystick0Kind::from_value(kind).unwrap();
                } else {
                    s.joystick.joystick1 = Joystick1Kind::from_value(kind).unwrap();
                }
            }
        }
    }
    set(&mut s.joystick.pdl_x_trim, r.n(c, "PDL X-Trim").map(|v| v as i16));
    set(&mut s.joystick.pdl_y_trim, r.n(c, "PDL Y-Trim").map(|v| v as i16));
    set(&mut s.joystick.cursor_control, r.b(c, "Joystick Cursor Control"));
    set(&mut s.joystick.centering, r.b(c, "Joystick Centering Control"));
    set(&mut s.joystick.autofire, r.n(c, "Autofire").map(|v| v as u8));
    set(&mut s.joystick.swap_buttons, r.b(c, "Swap buttons 0 and 1"));
    set(&mut s.joystick.shift_key_mod, r.b(c, "Shift Key Mod"));
    if let Some(d) = r.n("Configuration\\Game I/O Connector", "Game I/O type").and_then(DongleType::from_value) {
        s.joystick.dongle = d;
    }

    // Keyboard / mouse
    set(&mut s.keyboard.scroll_lock_toggle, r.b(c, "ScrollLock Toggle"));
    set(&mut s.keyboard.mouse_show_crosshair, r.b(c, "Mouse crosshair"));
    set(&mut s.keyboard.mouse_restrict_to_window, r.b(c, "Mouse restrict to window"));

    // Machine
    set(&mut s.machine.speed, r.n(c, "Emulation Speed"));
    set(&mut s.machine.no_slot_clock, r.b(c, "No-Slot clock"));
    set(&mut s.machine.freeze_f8_rom, r.b(c, "The Freeze's F8 Rom"));
    match r.n(c, "Video Refresh Rate") {
        Some(1) => s.machine.refresh_rate = RefreshRate::Hz50,
        Some(2) => s.machine.refresh_rate = RefreshRate::Hz60,
        _ => {}
    }

    // Video
    if let Some(t) = r.n(c, "Video Emulation").and_then(VideoType::from_value) {
        s.video.mode = t;
    }
    if let Some(style) = r.n(c, "Video Style") {
        s.video.half_scanlines = style & 1 != 0;
        s.video.vertical_blend = style & 2 != 0;
    } else {
        set(&mut s.video.half_scanlines, r.b(c, "Half Scan Lines"));
    }
    if let Some(bgr) = r.n(c, "Monochrome Color") {
        s.video.monochrome_rgb = ((bgr & 0xFF) << 16) | (bgr & 0xFF00) | ((bgr >> 16) & 0xFF);
    }
    set(&mut s.video.window_scale, r.n(c, "Window Scale"));
    set(&mut s.video.show_disk_leds, r.b(c, "Show Disk II Status"));
    set(&mut s.video.fullscreen_show_subunit_status, r.b(c, "Full-screen show subunit status"));
    if let (Some(x), Some(y)) = (r.n("Preferences", "Window X-Position"), r.n("Preferences", "Window Y-Position")) {
        s.video.window_pos = Some((x as i32, y as i32));
    }

    // Sound, misc, storage
    set(&mut s.sound.master_volume, r.n(c, "Master volume").or_else(|| r.n(c, "Speaker Volume")));
    set(&mut s.misc.confirm_reboot, r.b(c, "Confirm Reboot"));
    s.misc.imported_applewin_version = r.s(c, "Version").filter(|v| !v.is_empty()).map(str::to_string);
    set(&mut s.storage.enhance_disk_speed, r.b(c, "Enhance Disk Speed"));
    set(&mut s.storage.save_state_on_exit, r.b(c, "Save State On Exit"));
    s.storage.savestate_filename = r.path(c, "Save State Filename");
    s.storage.ciderpress_path = r.path(c, "CiderPress Location");
    s.storage.disk_dir = r.path("Preferences", "Starting Directory");
    s.storage.hdv_dir = r.path("Preferences", "HDV Starting Directory");
    set(&mut s.storage.new_disk_copy_bitsy_boot, r.b("Preferences", "NewDiskCopyBitsyBoot"));
    set(&mut s.storage.new_disk_copy_bitsy_bye, r.b("Preferences", "NewDiskCopyBitsyBye"));
    set(&mut s.storage.new_disk_copy_basic, r.b("Preferences", "NewDiskCopyBASIC"));
    set(&mut s.storage.new_disk_copy_prodos, r.b("Preferences", "NewDiskCopyProDOS"));

    // Slots
    s.reset_slot0_for_model();
    for slot in 0..8 {
        let sec = format!("Configuration\\Slot {slot}");
        if let Some(ty) = r.n(&sec, "Card type").and_then(CardType::from_value) {
            s.slots.0[slot] = load_slot_card(&r, &sec, ty);
        } else {
            // Legacy (AppleWin <= 1.30.3), Utilities.cpp:240-257
            s.slots.0[slot] = match slot {
                3 => {
                    let mut c = SlotCardSpec::new(if r.b(c, "Uthernet Active") == Some(true) { CardType::Uthernet } else { CardType::Empty });
                    if let SlotCardSpec::Uthernet { interface } = &mut c {
                        *interface = r.s(c_str(), "Uthernet Interface").unwrap_or("").to_string();
                    }
                    c
                }
                4 | 5 => match r.n(c, &format!("Slot {slot}")).and_then(CardType::from_value) {
                    Some(t) => SlotCardSpec::new(t),
                    None => SlotCardSpec::default_for_slot(slot, s.machine.model),
                },
                7 if r.n(c, "Harddisk Enable") == Some(1) => SlotCardSpec::new(CardType::GenericHdd),
                _ => SlotCardSpec::default_for_slot(slot, s.machine.model),
            };
        }
    }
    // Slot-0 Language Card on the //e is built in
    if s.machine.model.is_apple2e_family() || s.machine.model.is_apple2c() {
        if let SlotCardSpec::LanguageCard = s.slots.0[0] {
            s.slots.0[0] = SlotCardSpec::LanguageCardIIe;
        }
    }

    // Aux slot
    let aux = "Configuration\\Slot Auxiliary";
    if let Some(ty) = r.n(aux, "Card type").and_then(CardType::from_value) {
        s.aux.card = match ty {
            CardType::Empty => SlotCardSpec::Empty,
            CardType::Col80 => SlotCardSpec::Col80,
            CardType::RamWorksIII => SlotCardSpec::RamWorksIII { banks: r.n(aux, "Number of Banks").unwrap_or(16).clamp(1, 256) as u16 },
            _ => SlotCardSpec::Extended80Col,
        };
    }
    s
}

fn c_str() -> &'static str {
    "Configuration"
}

fn load_slot_card(r: &View, sec: &str, ty: CardType) -> SlotCardSpec {
    let mut card = SlotCardSpec::new(ty);
    match &mut card {
        SlotCardSpec::Disk2 { firmware_13_sector, drives } => {
            set(firmware_13_sector, r.b(sec, "13-sector Firmware"));
            drives[0].image = r.path(sec, "Last Disk Image 1");
            drives[1].image = r.path(sec, "Last Disk Image 2");
        }
        SlotCardSpec::GenericHdd { firmware, images } => {
            if let Some(f) = r.n(sec, "Firmware").and_then(HdcFirmware::from_value) {
                *firmware = f;
            }
            for (i, img) in images.iter_mut().enumerate() {
                *img = r.path(sec, &format!("Last Harddisk Image {}", i + 1));
            }
        }
        SlotCardSpec::Ssc { serial_port, .. } => {
            *serial_port = r.s(sec, "Serial Port Name").unwrap_or("").to_string();
        }
        SlotCardSpec::GenericPrinter(p) => {
            set(&mut p.dump_to_printer, r.b(sec, "Dump to printer"));
            set(&mut p.convert_encoding, r.b(sec, "Convert printer encoding for clones"));
            set(&mut p.filter_unprintable, r.b(sec, "Filter unprintable characters"));
            set(&mut p.append, r.b(sec, "Append to printer file"));
            p.filename = r.path(sec, "Printer Filename");
            set(&mut p.idle_limit, r.n(sec, "Printer idle limit"));
        }
        SlotCardSpec::MockingboardC(k) | SlotCardSpec::Phasor(k) | SlotCardSpec::MegaAudio(k) | SlotCardSpec::SdMusic(k) => {
            for i in 0..4 {
                if let Some(t) = r.n(sec, &format!("AY Socket {i}")).and_then(AyChip::from_value) {
                    k.ay[i] = t;
                }
            }
            for i in 0..2 {
                if let Some(t) = r.n(sec, &format!("SSI263 Socket {i}")).and_then(Ssi263Chip::from_value) {
                    k.ssi263[i] = t;
                }
            }
            set(&mut k.sc01, r.b(sec, "SC01"));
        }
        SlotCardSpec::Uthernet { interface } => *interface = r.s(sec, "Uthernet Interface").unwrap_or("").to_string(),
        SlotCardSpec::Uthernet2 { interface, virtual_dns } => {
            *interface = r.s(sec, "Uthernet Interface").unwrap_or("").to_string();
            set(virtual_dns, r.b(sec, "Uthernet Virtual DNS"));
        }
        SlotCardSpec::Saturn128K { banks } => set(banks, r.n(sec, "Number of Banks").map(|v| v.clamp(1, 8) as u8)),
        _ => {}
    }
    card
}

/// Imports a `.reg` export (raw bytes as regedit wrote them).
pub fn import_reg(bytes: &[u8]) -> Settings {
    settings_from_dump(&parse_reg_text(&decode_reg_bytes(bytes)))
}

/// Imports an AppleWin INI configuration (`-conf <file>` / Linux port).
pub fn import_ini(text: &str) -> Settings {
    settings_from_dump(&parse_ini_text(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REG: &str = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\AppleWin\CurrentVersion\Configuration]
"Apple2 Type"=dword:00000001
"Emulation Speed"="20"
"Video Emulation"="3"
"Video Style"="2"
"Monochrome Color"=dword:00ff8040
"Video Refresh Rate"="1"
"PDL X-Trim"="-3"
"Save State Filename"="C:\\saves\\my.aws"
"Master volume"="5"

[HKEY_CURRENT_USER\Software\AppleWin\CurrentVersion\Configuration\Slot 6]
"Card type"="1"
"Last Disk Image 1"="C:\\disks\\dos33.dsk"
"13-sector Firmware"="1"

[HKEY_CURRENT_USER\Software\AppleWin\CurrentVersion\Configuration\Slot 7]
"Card type"="5"
"Firmware"="1"
"Last Harddisk Image 2"="C:\\hd\\a.hdv"

[HKEY_CURRENT_USER\Software\AppleWin\CurrentVersion\Configuration\Slot Auxiliary]
"Card type"="14"
"Number of Banks"="64"

[HKEY_CURRENT_USER\Software\Other]
"Apple2 Type"="0"
"#;

    #[test]
    fn imports_reg_text() {
        let s = import_reg(REG.as_bytes());
        assert_eq!(s.machine.model, Model::Apple2Plus);
        assert_eq!(s.machine.speed, 20);
        assert_eq!(s.video.mode, VideoType::ColorMonitorNtsc);
        assert!(s.video.vertical_blend && !s.video.half_scanlines);
        assert_eq!(s.video.monochrome_rgb, 0x4080ff);
        assert_eq!(s.machine.refresh_rate, RefreshRate::Hz50);
        assert_eq!(s.joystick.pdl_x_trim, -3);
        assert_eq!(s.sound.master_volume, 5);
        assert_eq!(s.storage.savestate_filename, Some(PathBuf::from("C:\\saves\\my.aws")));
        assert_eq!(s.slot(0).card_type(), CardType::LanguageCard);
        match s.slot(6) {
            SlotCardSpec::Disk2 { firmware_13_sector, drives } => {
                assert!(*firmware_13_sector);
                assert_eq!(drives[0].image, Some(PathBuf::from("C:\\disks\\dos33.dsk")));
            }
            c => panic!("{c:?}"),
        }
        match s.slot(7) {
            SlotCardSpec::GenericHdd { firmware, images } => {
                assert_eq!(*firmware, HdcFirmware::SmartPort);
                assert_eq!(images[1], Some(PathBuf::from("C:\\hd\\a.hdv")));
            }
            c => panic!("{c:?}"),
        }
        assert_eq!(s.slot(1).card_type(), CardType::GenericPrinter); // not in file: new-install default
        assert_eq!(s.aux.card, SlotCardSpec::RamWorksIII { banks: 64 });
    }

    #[test]
    fn decodes_utf16_and_remaps_old_models() {
        let text = "[HKEY_CURRENT_USER\\Software\\AppleWin\\CurrentVersion\\Configuration]\r\n\"Apple2 Type\"=dword:00000110\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            bytes.extend(u.to_le_bytes());
        }
        assert_eq!(import_reg(&bytes).machine.model, Model::Pravets82);
        let bad = "[HKEY_CURRENT_USER\\Software\\AppleWin\\CurrentVersion\\Configuration]\n\"Apple2 Type\"=\"9999\"\n";
        assert_eq!(import_reg(bad.as_bytes()).machine.model, Model::Apple2eEnhanced);
    }

    #[test]
    fn imports_ini() {
        let s = import_ini("[Configuration]\nApple2 Type=0\n; c\n[Configuration\\Slot 4]\nCard type=9\nAY Socket 2=2\n");
        assert_eq!(s.machine.model, Model::Apple2);
        assert_eq!(s.slot(0).card_type(), CardType::Empty);
        match s.slot(4) {
            SlotCardSpec::Phasor(k) => assert_eq!(k.ay[2], AyChip::Ay8912),
            c => panic!("{c:?}"),
        }
    }

    #[test]
    fn legacy_hdd_enable_and_uthernet() {
        let s = import_ini("[Configuration]\nHarddisk Enable=1\nUthernet Active=1\nUthernet Interface=eth9\n");
        assert_eq!(s.slot(7).card_type(), CardType::GenericHdd);
        assert_eq!(s.slot(3), &SlotCardSpec::Uthernet { interface: "eth9".into() });
    }
}
