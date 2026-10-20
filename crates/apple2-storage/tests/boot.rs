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
