use apple2_core::{Machine, MachineConfig, Model};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
fn main() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    let mut card = Disk2Card::new();
    let p = "assets/disks/DOS 3.3 System Master - 680-0210-A.dsk";
    card.insert_disk(
        0,
        DiskImage::from_bytes(p, std::fs::read(p).unwrap()).unwrap(),
    );
    m.insert_card(6, Box::new(card));
    m.power_cycle();
    while m.cycles() < 132308 {
        m.step();
    }
    println!(
        "cycle {} pc {:04X} cif {} fb {:02X}",
        m.cycles(),
        m.cpu().pc,
        m.cycles_in_frame(),
        m.board().floating_bus()
    );
    for row in 0..32u16 {
        let a = 0x400 + (row % 8) * 0x80 + (row / 8) * 0x28;
        let v: Vec<u8> = (0..48).map(|i| m.peek_main(a + i)).collect();
        println!("row {row:2} {a:04X}: {:02X?}", &v[..]);
    }
}
