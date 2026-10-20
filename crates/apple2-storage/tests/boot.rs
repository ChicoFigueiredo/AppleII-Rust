//! Booting real disks through the Disk II card on an Enhanced //e.

mod common;
use apple2_storage::disk2::{Disk2Card, DiskStatus};
use apple2_storage::image::DiskImage;
use apple2_storage::store::MemStore;
use common::*;

#[test]
fn dos33_system_master_boots_to_the_prompt() {
    let mut m = machine_with_disk(DiskImage::from_bytes("master.dsk", disk(DOS33)).unwrap());
    let frames = run_until(&mut m, 600, "DOS 3.3 prompt", |m| screen_has(m, "DOS VERSION 3.3") && screen(m).lines().any(|l| l.starts_with(']')));
    println!("DOS 3.3 boot: {frames} frames");
    let c = m.card::<Disk2Card>(6).unwrap();
    assert_eq!(c.current_firmware(), 16);
    assert!(c.track(0) > 0, "the head moved while booting");
}

#[test]
fn dos33_boots_in_authentic_speed_mode_too() {
    let mut m = machine_with_disk(DiskImage::from_bytes("master.dsk", disk(DOS33)).unwrap());
    m.card_mut::<Disk2Card>(6).unwrap().set_enhance_disk(false);
    m.power_cycle();
    let frames = run_until(&mut m, 1500, "DOS 3.3 prompt (authentic)", |m| screen_has(m, "DOS VERSION 3.3") && screen(m).lines().any(|l| l.starts_with(']')));
    println!("DOS 3.3 authentic boot: {frames} frames");
}

#[test]
fn prodos_boots_to_its_screen() {
    let mut m = machine_with_disk(DiskImage::from_bytes("ProDOS_2_4_3.po", disk("ProDOS_2_4_3.po")).unwrap());
    for _ in 0..900 {
        m.run_frame();
    }
    println!("{}", screen(&m));
    assert!(screen_has(&m, "PRODOS") || screen_has(&m, "ProDOS"), "{}", screen(&m));
}

#[test]
fn motor_and_leds_follow_the_boot() {
    let mut m = machine_with_disk(DiskImage::from_bytes("master.dsk", disk(DOS33)).unwrap());
    let mut saw_read = false;
    let mut events = Vec::new();
    for _ in 0..700 {
        m.run_frame();
        let c = m.card_mut::<Disk2Card>(6).unwrap();
        saw_read |= c.light_status(0) == DiskStatus::Read && c.is_motor_on();
        events.extend(c.drain_events());
    }
    assert!(saw_read);
    assert!(events.iter().any(|e| matches!(e, apple2_storage::disk2::DriveEvent::MotorOn { drive: 0 })));
    assert!(events.iter().any(|e| matches!(e, apple2_storage::disk2::DriveEvent::HeadStep { .. })));
    let c = m.card::<Disk2Card>(6).unwrap();
    assert!(!c.is_motor_on(), "the boot ends with the motor off");
    assert_eq!(c.light_status(0), DiskStatus::Off, "and the drive has spun down");
}

fn at_prompt(m: &apple2_core::Machine) -> bool {
    let s = screen(m);
    s.lines().last().is_some_and(|_| s.lines().any(|l| l.starts_with(']')))
}

/// Types `cmd`, waits for the motor to stop and the prompt to come back.
fn command(m: &mut apple2_core::Machine, cmd: &str) {
    type_text(m, cmd);
    type_text(m, "\n");
    for _ in 0..30 {
        m.run_frame();
    }
    run_until(m, 900, cmd, |m| !m.card::<Disk2Card>(6).unwrap().is_motor_on());
    for _ in 0..90 {
        m.run_frame(); // the drive spins down; dirty tracks are written back then
    }
}

#[test]
fn saving_a_basic_program_changes_the_image_and_it_loads_back() {
    let store = MemStore::new(disk(DOS33));
    let image = DiskImage::from_bytes("master.dsk", disk(DOS33)).unwrap().with_store(Box::new(store.clone()));
    let mut m = machine_with_disk(image);
    run_until(&mut m, 600, "DOS 3.3 prompt", |m| screen_has(m, "DOS VERSION 3.3") && at_prompt(m));
    let before = store.bytes();
    assert_eq!(before, disk(DOS33));

    // The cursor is on the first column of the line below the greeting after a RETURN.
    command(&mut m, "NEW");
    command(&mut m, "10 PRINT \"STORED ON DISK\"");
    command(&mut m, "SAVE ZZTEST");
    let after = store.bytes();
    assert_ne!(after, before, "SAVE wrote sectors");
    assert_eq!(after.len(), before.len());
    let changed = after.iter().zip(&before).filter(|(a, b)| a != b).count();
    assert!(changed > 0 && changed < 4096, "only a few sectors changed (catalog, VTOC, file): {changed}");

    // A fresh machine booting the saved bytes finds the file in the catalog and can run it.
    let mut m2 = machine_with_disk(DiskImage::from_bytes("saved.dsk", after).unwrap());
    run_until(&mut m2, 600, "DOS 3.3 prompt", |m| screen_has(m, "DOS VERSION 3.3") && at_prompt(m));
    command(&mut m2, "RUN ZZTEST");
    assert!(screen_has(&m2, "STORED ON DISK"), "{}", screen(&m2));
}

#[test]
fn woz2_image_boots_dos33_through_the_bit_stream_sequencer() {
    let woz = woz2_from_dsk(disk(DOS33), 0);
    let image = DiskImage::from_bytes("synth.woz", woz).unwrap();
    assert!(image.is_woz());
    let mut m = machine_with_disk(image);
    let frames = run_until(&mut m, 900, "DOS 3.3 prompt from WOZ", |m| screen_has(m, "DOS VERSION 3.3") && at_prompt(m));
    println!("WOZ boot: {frames} frames");
}

#[test]
fn woz2_writes_survive_a_reboot() {
    let woz = woz2_from_dsk(disk(DOS33), 0);
    let store = MemStore::new(woz.clone());
    let image = DiskImage::from_bytes("synth.woz", woz.clone()).unwrap().with_store(Box::new(store.clone()));
    let mut m = machine_with_disk(image);
    run_until(&mut m, 900, "DOS 3.3 prompt from WOZ", |m| screen_has(m, "DOS VERSION 3.3") && at_prompt(m));
    command(&mut m, "NEW");
    command(&mut m, "10 PRINT \"WOZ WRITE OK\"");
    command(&mut m, "SAVE WOZTEST");
    let after = store.bytes();
    assert_ne!(after, woz);
    assert_eq!(after.len(), woz.len(), "all tracks already existed: the file does not grow");
    let mut m2 = machine_with_disk(DiskImage::from_bytes("again.woz", after).unwrap());
    run_until(&mut m2, 900, "DOS 3.3 prompt after reboot", |m| screen_has(m, "DOS VERSION 3.3") && at_prompt(m));
    command(&mut m2, "RUN WOZTEST");
    assert!(screen_has(&m2, "WOZ WRITE OK"), "{}", screen(&m2));
}

/// Machine code run from RAM to poke the controller: `code` at $0300, executes for `cycles`.
fn run_code(m: &mut apple2_core::Machine, code: &[u8], cycles: u64) {
    for (i, b) in code.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    m.run_cycles(cycles);
}

#[test]
fn woz_bit_stream_gives_the_first_address_field_of_track_zero() {
    let image = DiskImage::from_bytes("synth.woz", woz2_from_dsk(disk(DOS33), 0)).unwrap();
    let mut m = machine_with_disk(image);
    // Motor on, then the ROM-style search for D5 AA 96 and a copy of the next 8 nibbles to $0500.
    let code: Vec<u8> = vec![
        0xA2, 0x60, 0xBD, 0x89, 0xC0, 0xBD, 0x8E, 0xC0, 0xBD, 0x8C, 0xC0, 0x10, 0xFB, 0xC9, 0xD5, 0xD0, 0xF7, 0xBD, 0x8C, 0xC0, 0x10, 0xFB, 0xC9, 0xAA, 0xD0, 0xEE, 0xBD, 0x8C, 0xC0,
        0x10, 0xFB, 0xC9, 0x96, 0xD0, 0xE5, 0xA0, 0x00, 0xBD, 0x8C, 0xC0, 0x10, 0xFB, 0x99, 0x00, 0x05, 0xC8, 0xC0, 0x08, 0xD0, 0xF3, 0x4C, 0x32, 0x03,
    ];
    run_code(&mut m, &code, 300_000);
    let v: Vec<u8> = (0..8).map(|i| m.peek(0x500 + i)).collect();
    // Volume 254, track 0, sector 0 in 4-and-4, then the checksum.
    assert_eq!(v, [0xFF, 0xFE, 0xAA, 0xAA, 0xAA, 0xAA, 0xFF, 0xFE]);
}

#[test]
fn woz_boot_sector_format_selects_the_13_sector_firmware_at_reset() {
    let woz = woz2_from_dsk(disk(DOS33), 2);
    let mut m = machine_with_disk(DiskImage::from_bytes("d13.woz", woz).unwrap());
    assert_eq!(m.card::<Disk2Card>(6).unwrap().current_firmware(), 13);
    m.card_mut::<Disk2Card>(6).unwrap().eject_disk(0);
    m.reset();
    assert_eq!(m.card::<Disk2Card>(6).unwrap().current_firmware(), 16);
    m.card_mut::<Disk2Card>(6).unwrap().set_force_13_sector_firmware(true);
    m.reset();
    assert_eq!(m.card::<Disk2Card>(6).unwrap().current_firmware(), 13);
}
