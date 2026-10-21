//! Our machine -> AppleWin YAML -> our machine.

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, SaveOptions};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use common::*;

fn booted_machine(frames: u32) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut card = Disk2Card::new();
    card.insert_disk(
        0,
        DiskImage::from_bytes("DOS 3.3 System Master - 680-0210-A.dsk", disk(DOS33)).unwrap(),
    );
    assert!(m.insert_card(6, Box::new(card)));
    m.power_cycle();
    for _ in 0..frames {
        m.run_frame();
    }
    m
}

#[test]
fn dos33_boot_saved_mid_run_resumes_identically() {
    let mut a = booted_machine(120); // the head is moving, the motor is on
    assert!(
        a.card::<Disk2Card>(6).unwrap().is_motor_on(),
        "saved while the drive works"
    );
    let saved = save_aws_yaml(&a, &SaveOptions::default()).unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);

    let mut host = TestHost::default();
    let loaded = load_aws_yaml(&saved.text, &mut host).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let mut b = loaded.machine;
    assert_eq!(host.opened, ["DOS 3.3 System Master - 680-0210-A.dsk"]);
    assert_eq!(
        a.save_state(),
        b.save_state(),
        "identical MachineState after loading"
    );

    for _ in 0..600 {
        a.run_frame();
        b.run_frame();
    }
    assert!(screen(&a).contains("DOS VERSION 3.3"), "{}", screen(&a));
    assert_eq!(screen(&a), screen(&b));
    assert_eq!(
        a.save_state(),
        b.save_state(),
        "identical after running 600 frames"
    );
}

#[test]
fn file_without_the_extension_still_loads_and_boots_dos() {
    let a = booted_machine(120);
    let saved = save_aws_yaml(
        &a,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!saved.text.contains("AppleII-Rust Extension"));
    let mut host = TestHost::default();
    let loaded = load_aws_yaml(&saved.text, &mut host).unwrap();
    let mut b = loaded.machine;
    // Same registers and memory as AppleWin's units carry.
    let (sa, sb) = (a.save_state(), b.save_state());
    assert_eq!(
        (sa.cpu.pc, sa.cpu.a, sa.cpu.x, sa.cpu.y, sa.cpu.sp),
        (sb.cpu.pc, sb.cpu.a, sb.cpu.x, sb.cpu.y, sb.cpu.sp)
    );
    assert_eq!(sa.memory.main, sb.memory.main);
    assert_eq!(sa.memory.aux, sb.memory.aux);
    assert_eq!(sa.memory.memmode, sb.memory.memmode);
    assert_eq!(sa.cycles, sb.cycles);
    for _ in 0..700 {
        b.run_frame();
    }
    assert!(screen(&b).contains("DOS VERSION 3.3"), "{}", screen(&b));
}
