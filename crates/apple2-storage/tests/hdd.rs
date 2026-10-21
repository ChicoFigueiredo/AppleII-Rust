//! The hard disk controller: ProDOS boot through the slot firmware and the register protocol.

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_storage::disk2::DiskStatus;
use apple2_storage::hdd::{HardDiskCard, HdcMode};
use apple2_storage::image::DiskImage;
use apple2_storage::store::MemStore;
use common::*;

fn hdd_machine(model: Model, mode: HdcMode, image: DiskImage) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(model));
    let mut card = HardDiskCard::new(7, model, mode);
    card.insert(0, image);
    assert!(m.insert_card(7, Box::new(card)));
    m.power_cycle();
    m
}

fn prodos_hdv() -> DiskImage {
    DiskImage::hard_disk_from_bytes("ProDOS.hdv", disk("ProDOS_2_4_3.po")).unwrap()
}

#[test]
fn prodos_boots_from_the_hard_disk_card_in_smartport_mode() {
    let mut m = hdd_machine(Model::Apple2eEnhanced, HdcMode::Default, prodos_hdv());
    let frames = run_until(&mut m, 900, "ProDOS on S7,D1", |m| {
        screen_has(m, "S7,D1:/PRODOS.2.4.3")
    });
    println!("HDD boot: {frames} frames");
    assert!(screen_has(&m, "BITSY"));
}

#[test]
fn prodos_boots_from_the_hard_disk_card_in_block_mode_on_a_plain_iie() {
    let mut m = hdd_machine(Model::Apple2e, HdcMode::Default, prodos_hdv());
    run_until(&mut m, 900, "ProDOS on S7,D1 (block mode)", |m| {
        screen_has(m, "S7,D1:/PRODOS.2.4.3")
    });
}

fn setup_machine() -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut card = HardDiskCard::new(7, Model::Apple2eEnhanced, HdcMode::SmartPort);
    card.insert(0, prodos_hdv());
    assert!(m.insert_card(7, Box::new(card)));
    // Park the CPU so only the test touches the card.
    for (i, b) in [0x4C, 0x00, 0x03].iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    m.run_cycles(10); // the busy test is `cycle <= not_busy_cycle`, true at cycle 0
    m
}

fn reg(lo: u16) -> u16 {
    0xC0F0 + lo
}

/// Programs the BLK protocol and executes: returns the status byte.
fn blk(m: &mut Machine, cmd: u8, unit: u8, mem: u16, block: u16) -> u8 {
    let b = m.board_mut();
    b.write(reg(2), cmd);
    b.write(reg(3), unit);
    b.write(reg(4), mem as u8);
    b.write(reg(5), (mem >> 8) as u8);
    b.write(reg(6), block as u8);
    b.write(reg(7), (block >> 8) as u8);
    b.read(reg(0))
}

#[test]
fn block_read_dmas_the_block_and_reports_busy_while_it_runs() {
    let mut m = setup_machine();
    let img = disk("ProDOS_2_4_3.po");
    let st = blk(&mut m, 1, 0x70, 0x4000, 2);
    assert_eq!(st & 0x01, 0, "no error");
    assert_eq!(st & 0x80, 0x80, "busy during the DMA");
    assert_eq!(m.board_mut().read(reg(1)) & 0x80, 0x80);
    assert_eq!(
        m.card::<HardDiskCard>(7).unwrap().light_status(0),
        DiskStatus::Read
    );
    m.run_cycles(600);
    assert_eq!(m.board_mut().read(reg(1)), 0, "idle again, no error");
    let got: Vec<u8> = (0..512).map(|i| m.peek(0x4000 + i)).collect();
    assert_eq!(got, &img[1024..1536]);
    // Size in blocks: 280.
    assert_eq!(
        (m.board_mut().read(reg(9)), m.board_mut().read(reg(10))),
        (0x18, 0x01)
    );
    assert_eq!(
        m.card::<HardDiskCard>(7).unwrap().light_status(0),
        DiskStatus::Off
    );
}

#[test]
fn block_write_updates_the_image_and_the_store() {
    let store = MemStore::new(disk("ProDOS_2_4_3.po"));
    let image = DiskImage::hard_disk_from_bytes("ProDOS.hdv", disk("ProDOS_2_4_3.po"))
        .unwrap()
        .with_store(Box::new(store.clone()));
    let mut m = setup_machine();
    m.card_mut::<HardDiskCard>(7).unwrap().insert(0, image);
    for i in 0..512u16 {
        m.poke(0x6000 + i, (i * 3) as u8);
    }
    let st = blk(&mut m, 2, 0x70, 0x6000, 5);
    assert_eq!(st & 1, 0);
    m.run_cycles(600);
    let want: Vec<u8> = (0..512u16).map(|i| (i * 3) as u8).collect();
    assert_eq!(&store.bytes()[5 * 512..6 * 512], &want[..]);
    // Reading it back through the card gives the same.
    blk(&mut m, 1, 0x70, 0x7000, 5);
    m.run_cycles(600);
    assert_eq!(
        (0..512u16).map(|i| m.peek(0x7000 + i)).collect::<Vec<_>>(),
        want
    );
}

#[test]
fn errors_unknown_unit_write_protect_and_bad_destinations() {
    let mut m = setup_machine();
    // BLK unit 2 (slot 7, drive 2) has no image: device not connected.
    let st = blk(&mut m, 1, 0xF0, 0x4000, 0);
    assert_eq!(st, (0x28 << 1) | 1);
    // Reading into I/O space fails with an I/O error (the Language Card is write-enabled at power-on, so $D000 would work).
    let st = blk(&mut m, 1, 0x70, 0xC100, 0);
    assert_eq!(st >> 1 & 0x3F, 0x27);
    // Block past the end of the image.
    let st = blk(&mut m, 1, 0x70, 0x4000, 400);
    assert_eq!(st >> 1 & 0x3F, 0x27);
    m.card_mut::<HardDiskCard>(7)
        .unwrap()
        .image_mut(0)
        .unwrap()
        .set_write_protected(true);
    let st = blk(&mut m, 2, 0x70, 0x4000, 1);
    assert_eq!(st >> 1 & 0x3F, 0x2B, "write-protected");
}

#[test]
fn smartport_status_reports_the_devices() {
    let mut m = setup_machine();
    let b = |m: &mut Machine, lo: u16, v: u8| m.board_mut().write(reg(lo), v);
    // Status command ($80), unit 0 (the controller), status code 0, result buffer $5000.
    b(&mut m, 2, 0x80);
    b(&mut m, 3, 0);
    b(&mut m, 4, 0x00);
    b(&mut m, 5, 0x50);
    b(&mut m, 6, 0);
    let r = m.board_mut().read(reg(0));
    assert_eq!(r & 1, 0);
    assert_eq!(m.peek(0x5000), 1, "one device connected");
    // Unit 1: device status, 280 blocks, write allowed, not write-protected.
    b(&mut m, 2, 0x80);
    b(&mut m, 3, 1);
    b(&mut m, 4, 0x00);
    b(&mut m, 5, 0x51);
    b(&mut m, 6, 0);
    m.board_mut().read(reg(0));
    assert_eq!(
        [
            m.peek(0x5100),
            m.peek(0x5101),
            m.peek(0x5102),
            m.peek(0x5103)
        ],
        [0xF8, 0x18, 0x01, 0x00]
    );
    // Device information block carries the ID string.
    b(&mut m, 2, 0x80);
    b(&mut m, 3, 1);
    b(&mut m, 6, 3);
    m.board_mut().read(reg(0));
    let id: String = (0..14).map(|i| m.peek(0x5105 + i) as char).collect();
    assert_eq!(id, "AppleWin SP D#");
    // Type, subtype and firmware version (AppleWin 1.32.1.0 of a8f0d1d, version.h => 132.1):
    // the SmartPort-firmware save-state prints these bytes.
    assert_eq!(
        [
            m.peek(0x5100 + 21),
            m.peek(0x5100 + 22),
            m.peek(0x5100 + 23),
            m.peek(0x5100 + 24)
        ],
        [0x02, 0x20, 132, 1]
    );
}

#[test]
fn fifo_ports_load_the_registers_in_order() {
    let mut m = setup_machine();
    // BLK FIFO at $C089: command, unit, mem lo/hi, block lo/hi.
    for v in [1u8, 0x70, 0x00, 0x40, 3, 0] {
        m.board_mut().write(reg(9), v);
    }
    assert_eq!(m.board_mut().read(reg(0)) & 1, 0);
    m.run_cycles(600);
    assert_eq!(m.peek(0x4000), disk("ProDOS_2_4_3.po")[3 * 512]);
}
