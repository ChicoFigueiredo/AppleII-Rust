//! GH#640: with drive 1 empty (the machine sitting at the boot failure), inserting a disk and
//! rebooting boots it. The oracle cannot insert a disk while running, so this test is Rust-only
//! (state BLOCKED in docs/conformance/results/disco.csv, the test itself passes).

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use common::*;

#[test]
fn inserting_a_disk_into_an_empty_drive_and_resetting_boots_it_gh640() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(6, Box::new(Disk2Card::new())));
    m.power_cycle();
    for _ in 0..180 {
        m.run_frame(); // no disk: the autostart ROM keeps looking
    }
    assert!(!screen_has(&m, "DOS VERSION 3.3"));
    let image = DiskImage::from_bytes("master.dsk", disk(DOS33)).unwrap();
    assert!(m
        .card_mut::<Disk2Card>(6)
        .unwrap()
        .insert_disk(0, image)
        .is_none());
    m.power_cycle(); // cold start (Ctrl+Reset on a running //e only warm-starts Applesoft)
    run_until(
        &mut m,
        700,
        "DOS 3.3 after inserting into the empty drive",
        |m| screen_has(m, "DOS VERSION 3.3"),
    );
}
