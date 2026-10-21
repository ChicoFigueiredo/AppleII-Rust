//! Auditoria wave0-core, area 7: a importacao do registro/INI do AppleWin preserva tudo?
//!
//! Oraculo: Common.h:66-148 (REGVALUE_*) e Utilities.cpp:79-345 (`LoadConfiguration`), SHA a8f0d1d.

use apple2_config::import::{import_ini, import_reg};
use apple2_config::settings::{CpuType, RefreshRate};
use apple2_config::SlotCardSpec;

#[test]
fn saturn_banks_survive_the_import_into_settings() {
    let s = import_ini("[Configuration\\Slot 4]\nCard type=18\nNumber of Banks=4\n");
    assert_eq!(s.slot(4), &SlotCardSpec::Saturn128K { banks: 4 });
}

#[test]
fn explicit_65c02_cpu_type_is_imported() {
    // Utilities.cpp:145-148: valor 2 (CPU_65C02) e o padrao e e respeitado em qualquer modelo.
    // import.rs: `Some(1) => Cpu6502, _ => Auto` descarta o 2 e o "ausente" do AppleWin (= 65C02).
    let s = import_ini("[Configuration]\nApple2 Type=1\nCPU Type=2\n");
    assert_eq!(s.machine.cpu, CpuType::Cpu65C02);
}

#[test]
fn six502_cpu_type_is_imported() {
    let s = import_ini("[Configuration]\nCPU Type=1\n");
    assert_eq!(s.machine.cpu, CpuType::Cpu6502);
}

#[test]
fn common_values_are_imported() {
    let ini = "\
[Configuration]
Apple2 Type=2
Emulation Speed=20
Video Refresh Rate=1
Video Emulation=3
Master volume=7
Enhance Disk Speed=0
Save State On Exit=1
Save State Filename=C:\\Games\\Zoe #1\\s.aws.yaml
Window Scale=3
Joystick0 Emu Type v3=2
PDL X-Trim=-5
Autofire=2
Show Disk II Status=0
No-Slot clock=1
[Preferences]
Window X-Position=-20
Window Y-Position=30
[Configuration\\Slot 6]
Card type=1
13-sector Firmware=1
Last Disk Image 1=/jogos/a.dsk
Last Disk Image 2=/jogos/b.dsk
[Configuration\\Slot 7]
Card type=5
Last Harddisk Image 1=/hd/um.hdv
Last Harddisk Image 2=/hd/dois.hdv
[Configuration\\Slot 4]
Card type=3
AY Socket 1=2
SSI263 Socket 0=1
SC01=1
";
    let s = import_ini(ini);
    assert_eq!(s.machine.speed, 20);
    assert_eq!(s.machine.refresh_rate, RefreshRate::Hz50);
    assert_eq!(s.sound.master_volume, 7);
    assert!(!s.storage.enhance_disk_speed);
    assert!(s.storage.save_state_on_exit);
    assert_eq!(
        s.storage.savestate_filename.as_deref(),
        Some(std::path::Path::new("C:\\Games\\Zoe #1\\s.aws.yaml"))
    );
    assert_eq!(s.video.window_scale, 3);
    assert_eq!(s.video.window_pos, Some((-20, 30)));
    assert_eq!(s.joystick.pdl_x_trim, -5);
    assert_eq!(s.joystick.autofire, 2);
    assert!(!s.video.show_disk_leds);
    assert!(s.machine.no_slot_clock);
    let SlotCardSpec::Disk2 {
        firmware_13_sector,
        drives,
    } = s.slot(6)
    else {
        panic!("slot 6")
    };
    assert!(*firmware_13_sector);
    assert_eq!(
        drives[0].image.as_deref(),
        Some(std::path::Path::new("/jogos/a.dsk"))
    );
    assert_eq!(
        drives[1].image.as_deref(),
        Some(std::path::Path::new("/jogos/b.dsk"))
    );
    let SlotCardSpec::GenericHdd { images, .. } = s.slot(7) else {
        panic!("slot 7")
    };
    assert_eq!(
        images[0].as_deref(),
        Some(std::path::Path::new("/hd/um.hdv"))
    );
    assert_eq!(
        images[1].as_deref(),
        Some(std::path::Path::new("/hd/dois.hdv"))
    );
    let SlotCardSpec::MockingboardC(k) = s.slot(4) else {
        panic!("slot 4")
    };
    assert!(k.sc01);
    assert_eq!(k.ssi263[0], apple2_config::card::Ssi263Chip::Ssi263P);
    assert_eq!(k.ay[1], apple2_config::card::AyChip::Ay8912);
}

#[test]
fn utf16_reg_export_with_backslash_and_quotes_is_decoded() {
    let text = "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\AppleWin\\CurrentVersion\\Configuration]\r\n\"Save State Filename\"=\"C:\\\\Users\\\\Zo\u{eb}\\\\s.aws.yaml\"\r\n\"Emulation Speed\"=dword:00000014\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        bytes.extend(u.to_le_bytes());
    }
    let s = import_reg(&bytes);
    assert_eq!(s.machine.speed, 20);
    assert_eq!(
        s.storage.savestate_filename.as_deref(),
        Some(std::path::Path::new("C:\\Users\\Zo\u{eb}\\s.aws.yaml"))
    );
}

#[test]
fn malformed_dumps_never_panic() {
    for t in [
        "",
        "[",
        "[]\n=",
        "[Configuration]\nCPU Type=\n",
        "[Configuration]\nEmulation Speed=99999999999999999999\n",
        "[Configuration\\Slot 99]\nCard type=18\n",
        "[Configuration\\Slot 4]\nCard type=-1\nNumber of Banks=abc\n",
        "[Configuration\\Slot 7]\nCard type=18\nNumber of Banks=0\n",
    ] {
        let _ = import_ini(t);
    }
    for b in [
        &b""[..],
        &[0xFF, 0xFE][..],
        &[0xFF, 0xFE, 0x41][..],
        &[0xFF; 9][..],
    ] {
        let _ = import_reg(b);
    }
}
