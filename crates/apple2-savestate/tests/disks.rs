//! Disk II and hard disk controller units.

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::hdd::{HardDiskCard, HdcMode};
use apple2_storage::image::DiskImage;
use common::*;

fn iie() -> Machine {
    Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced))
}

/// A //e Apple2 unit followed by `slots` (one `Unit: Slots`), as AppleWin writes it for a
/// machine with only the Disk II / HDC; memory is all zero.
fn document(slot_entries: &str) -> String {
    let mut t = String::from(
        "---\nFile_hdr:\n  Tag: AppleWin Save State\n  Version: 2\n\nUnit:\n  Type: Apple2\n  Version: 9\n  State:\n    Model: Enhanced Apple//e\n    CPU:\n      Type: 65C02\n      A: 0x00\n      X: 0x00\n      Y: 0x00\n      P: 0x34\n      S: 0xFF\n      PC: 0xC600\n      Cumulative Cycles: 0x0000000000100000\n      Defer IRQ By 1 Opcode: false\n    Joystick:\n      Joystick0 TrimX: 0\n      Joystick0 TrimY: 0\n      Joystick1 TrimX: 0 # not implemented yet\n      Joystick1 TrimY: 0 # not implemented yet\n      Paddle0 Inactive Cycle: 0x0000000000000000\n      Paddle1 Inactive Cycle: 0x0000000000000000\n      Paddle2 Inactive Cycle: 0x0000000000000000\n      Paddle3 Inactive Cycle: 0x0000000000000000\n    Keyboard:\n      Last Key: 0x00\n      Key Waiting: false\n    Speaker:\n      Last Cycle: 0x0000000000000000\n    Video:\n      Alt Char Set: false\n      Video Mode: 0x00000040\n      Cycles This Frame: 100\n      Video Refresh Rate: 2\n    Memory:\n      Memory Mode: 0x00000000\n      MMU LC Mode: 0x00000410\n      Last RAM Write: 0\n      IO_SELECT: 0x00\n      IO_SELECT_InternalROM: 0x00\n      Expansion ROM Type: 0\n      Peripheral ROM Slot: 0\n      Last Slot to Set Main Mem LC: 0\n      Annunciator0: false\n      Annunciator1: false\n      Annunciator2: false\n      Annunciator3: false\n    Main Memory:\n",
    );
    for line in 0..1024 {
        t.push_str(&format!("      {:04X}: {}\n", line * 64, "00".repeat(64)));
    }
    t.push_str("\nUnit:\n  Type: Auxiliary Slot\n  Version: 2\n  State:\n    Card: Empty\n    Version: 4\n");
    t.push_str("\nUnit:\n  Type: Slots\n  Version: 1\n  State:\n");
    t.push_str(slot_entries);
    t.push_str("\nUnit:\n  Type: Game I/O Connector\n  Version: 4\n  State:\n    Shift Key Mod: false\n");
    t
}

fn disk2_v9(filename: &str) -> String {
    let track = "FF".repeat(64);
    let mut s = String::new();
    s.push_str("    6:\n      Card: Disk II\n      Version: 9\n      State:\n");
    s.push_str("        Current Drive: 1\n        Phases: 0x4\n        Enhance Disk: false\n        Floppy Latch: 0x96\n        Floppy Motor On: true\n");
    s.push_str("        Last Cycle: 0x00000000000FFF00\n        Last Read Latch Cycle: 0x00000000000FFF10\n        LSS Shift Reg: 0x5A\n        LSS Latch Delay: -2\n        LSS Sequencer Function: 2\n");
    s.push_str("        Deferred Stepper Event: false\n        Deferred Stepper Address: 0x00\n        Deferred Stepper Cycle: 0x0000000000000000\n");
    s.push_str("        Format Track state:\n          Written Sector Address Fields: 0x0000\n          Write Track Start Index: 0xFFFF\n          Write Track Wrapped: false\n          Write Data Field Prologue Count: 0x00\n          Track State: 0x00\n          Last 3 bytes: 0x000000\n          Vol,Trk,Sec,Chk (4 and 4): 0x0000000000000000\n          Index (4 and 4): 0x0\n");
    for unit in 0..2 {
        s.push_str(&format!("        Unit{unit}:\n          Drive Connected: true\n          Phase: {}\n          Phase (precise): {:.6}\n          Head Window: 0x3\n          Last Stepper Cycle: 0x00000000000FFE00\n          Motor On Cycle: 0x00000000000FF000\n          Spinning: 77\n          Write Light: 0\n", 8 * (unit + 1), 8.0 * (unit as f32 + 1.0)));
        s.push_str("          Floppy:\n");
        if unit == 0 {
            s.push_str(&format!("            Filename: {filename}\n            Absolute Path: C:\\\\disks\\\\{filename}\n"));
            s.push_str("            Byte: 0x0123\n            Nibbles: 0x1A00\n            Bit Offset: 0x00000000\n            Bit Count: 0x00000000\n            Extra Cycles: 0.250000\n            Write Protected: false\n            Track Image Data: 1\n            Track Image Dirty: 0\n            Track Image:\n");
            for line in 0..0x1A00 / 64 {
                s.push_str(&format!("              {:04X}: {track}\n", line * 64));
            }
            // 0x1A00 is not a multiple of 64: AppleWin writes a short last line.
        } else {
            s.push_str("            Filename: \"\"\n            Absolute Path: \"\"\n            Byte: 0x0000\n            Nibbles: 0x0000\n            Bit Offset: 0x00000000\n            Bit Count: 0x00000000\n            Extra Cycles: 0.000000\n            Write Protected: false\n            Track Image Data: 0\n            Track Image Dirty: 0\n");
        }
    }
    s
}

#[test]
fn golden_disk2_v9_entry_restores_the_drive() {
    let doc = document(&disk2_v9(DOS33));
    let mut host = TestHost::default();
    let loaded = load_aws_yaml(&doc, &mut host).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(host.opened, [DOS33]);
    let m = loaded.machine;
    let c = m.card::<Disk2Card>(6).expect("Disk II in slot 6");
    let s = c.snapshot();
    assert_eq!((s.current_drive, s.magnet_states, s.enhance_disk, s.floppy_latch, s.motor_on), (1, 4, false, 0x96, true));
    assert_eq!((s.last_cycle, s.last_read_latch_cycle), (0xFFF00, 0xFFF10));
    assert_eq!((s.shift_reg, s.latch_delay, s.seq_func), (0x5A, -2, 2));
    let d0 = &s.drives[0];
    assert!(d0.connected);
    assert_eq!((d0.phase, d0.phase_precise, d0.head_window), (8, 8.0, 3));
    assert_eq!((d0.last_stepper_cycle, d0.motor_on_cycle, d0.spinning), (0xFFE00, 0xFF000, 77));
    assert_eq!((d0.byte, d0.nibbles, d0.extra_cycles, d0.track_data, d0.track_dirty), (0x123, 0x1A00, 0.25, true, false));
    assert_eq!(d0.filename, DOS33);
    assert_eq!(d0.track.as_ref().unwrap()[..64], [0xFF; 64]);
    assert!(c.is_drive_empty(1));
    assert_eq!(m.save_state().cycles, 0x100000);
}

#[test]
fn old_v3_unit_layout_loads() {
    // Version 3: the floppy keys sit directly in UnitN, with Track, Phase, Spinning, Write Light.
    let mut s = String::new();
    s.push_str("    6:\n      Card: Disk II\n      Version: 3\n      State:\n        Current Drive: 0\n        Phases: 0x0\n        Enhance Disk: true\n        Floppy Latch: 0x00\n        Floppy Motor On: false\n        Floppy Write Mode: false\n        Disk Accessed: false\n        Last Cycle: 0x0000000000000010\n        Last Read Latch Cycle: 0x0000000000000020\n");
    s.push_str("        Format Track state:\n          Written Sector Address Fields: 0x0000\n          Write Track Start Index: 0xFFFF\n          Write Track Wrapped: false\n          Write Data Field Prologue Count: 0x00\n          Track State: 0x00\n          Last 3 bytes: 0x000000\n          Vol,Trk,Sec,Chk (4 and 4): 0x0000000000000000\n          Index (4 and 4): 0x0\n");
    for unit in 0..2 {
        s.push_str(&format!("        Unit{unit}:\n            Filename: \"\"\n            Byte: 0x0000\n            Nibbles: 0x0000\n            Write Protected: false\n            Track Image Data: 0\n            Track Image Dirty: 0\n            Track: 0\n            Phase: 6\n            Spinning: 0\n            Write Light: 0\n"));
    }
    let loaded = load_aws_yaml(&document(&s), &mut TestHost::default()).unwrap();
    let c = loaded.machine.card::<Disk2Card>(6).unwrap();
    let snap = c.snapshot();
    assert_eq!((snap.last_cycle, snap.last_read_latch_cycle), (0x10, 0x20));
    assert_eq!((snap.drives[0].phase, snap.drives[0].phase_precise), (6, 6.0));
}

#[test]
fn missing_image_is_a_warning_not_an_error() {
    struct NoDisks;
    impl LoadHost for NoDisks {
        fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
            Err(format!("{} not found", req.filename))
        }
    }
    let loaded = load_aws_yaml(&document(&disk2_v9("gone.dsk")), &mut NoDisks).unwrap();
    assert_eq!(loaded.warnings.len(), 1, "{:?}", loaded.warnings);
    assert!(loaded.warnings[0].contains("gone.dsk"));
    assert!(loaded.machine.card::<Disk2Card>(6).unwrap().is_drive_empty(0));
}

#[test]
fn path_with_backslashes_round_trips() {
    let mut m = iie();
    let mut card = Disk2Card::new();
    let mut img = DiskImage::from_bytes("x.dsk", disk(DOS33)).unwrap();
    img.set_write_protected(true);
    card.insert_disk(0, img);
    m.insert_card(6, Box::new(card));
    let saved = save_aws_yaml(&m, &SaveOptions { extension: false, ..Default::default() }).unwrap();
    assert!(saved.text.contains("Filename: x.dsk\n"), "{}", saved.text.lines().filter(|l| l.contains("Filename")).collect::<String>());
}

#[test]
fn woz_disk_saved_mid_boot_continues_identically() {
    let woz = woz2_from_dsk(disk(DOS33), 0);
    let mut a = iie();
    let mut card = Disk2Card::new();
    card.insert_disk(0, DiskImage::from_bytes("synth.woz", woz.clone()).unwrap());
    a.insert_card(6, Box::new(card));
    a.power_cycle();
    for _ in 0..150 {
        a.run_frame();
    }
    struct Woz(Vec<u8>);
    impl LoadHost for Woz {
        fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
            DiskImage::from_bytes(req.filename, self.0.clone()).map_err(|e| e.to_string())
        }
    }
    let text = save_aws_yaml(&a, &SaveOptions::default()).unwrap().text;
    let mut b = load_aws_yaml(&text, &mut Woz(woz.clone())).unwrap().machine;
    assert_eq!(a.save_state(), b.save_state());
    // Without the extension the WOZ bit position, shift register and track buffer still come
    // from the AppleWin keys.
    let text2 = save_aws_yaml(&a, &SaveOptions { extension: false, ..Default::default() }).unwrap().text;
    let c = load_aws_yaml(&text2, &mut Woz(woz)).unwrap().machine;
    let (sa, sc) = (a.card::<Disk2Card>(6).unwrap().snapshot(), c.card::<Disk2Card>(6).unwrap().snapshot());
    assert_eq!(sa, sc);
    for _ in 0..500 {
        a.run_frame();
        b.run_frame();
    }
    assert_eq!(a.save_state(), b.save_state());
    assert!(screen(&a).contains("DOS VERSION 3.3"), "{}", screen(&a));
}

#[test]
fn hard_disk_controller_round_trips() {
    let mut m = iie();
    let mut hdc = HardDiskCard::new(7, Model::Apple2eEnhanced, HdcMode::Default);
    let img = DiskImage::hard_disk_from_bytes("ProDOS.hdv", disk("ProDOS_2_4_3.po")).unwrap();
    hdc.insert(0, img);
    assert!(m.insert_card(7, Box::new(hdc)));
    m.power_cycle();
    for _ in 0..200 {
        m.run_frame();
    }
    struct Hd;
    impl LoadHost for Hd {
        fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
            assert!(req.hard_disk);
            DiskImage::hard_disk_from_bytes(req.filename, disk("ProDOS_2_4_3.po")).map_err(|e| e.to_string())
        }
    }
    let saved = save_aws_yaml(&m, &SaveOptions::default()).unwrap();
    assert!(saved.text.contains("Card: Hard Disk Controller\n      Version: 6\n"), "{}", saved.text.lines().filter(|l| l.contains("Card")).collect::<String>());
    assert!(saved.text.contains("        Unit1:\n          Filename: ProDOS.hdv\n"));
    let mut b = load_aws_yaml(&saved.text, &mut Hd).unwrap().machine;
    assert_eq!(m.save_state(), b.save_state());
    for _ in 0..300 {
        m.run_frame();
        b.run_frame();
    }
    assert_eq!(m.save_state(), b.save_state());
    let no_ext = save_aws_yaml(&m, &SaveOptions { extension: false, ..Default::default() }).unwrap();
    let c = load_aws_yaml(&no_ext.text, &mut Hd).unwrap().machine;
    assert_eq!(m.card::<HardDiskCard>(7).unwrap().snapshot().units, c.card::<HardDiskCard>(7).unwrap().snapshot().units);
}
