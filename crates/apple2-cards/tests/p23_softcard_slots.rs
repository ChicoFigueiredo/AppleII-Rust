//! GH#80 (variable CP/M card slot; CP/M Plus needs slot 4) and GH#79: the Z80 SoftCard works in
//! any slot 1-7, not only in slot 5. The Z80 hands the CPU back by writing to the card's own
//! `$Cn00` (Z80 address `$E000 + n*$100`).

use apple2_cards::softcard::SoftCard;
use apple2_core::{Machine, MachineConfig, Model};

fn load(m: &mut Machine, at: u16, code: &[u8]) {
    for (i, b) in code.iter().enumerate() {
        m.poke(at + i as u16, *b);
    }
}

#[test]
fn softcard_works_in_any_slot() {
    for slot in 1..=7u8 {
        let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
        assert!(
            m.insert_card(slot, Box::new(SoftCard::new())),
            "slot {slot}"
        );
        let port = 0xC000u16 + u16::from(slot) * 0x100;
        let z80_port = port + 0x2000;
        // Z80 (starts at Apple $1000): LD A,$42 / LD ($F010),A / LD (port),A / JR $
        load(
            &mut m,
            0x1000,
            &[
                0x3E,
                0x42,
                0x32,
                0x10,
                0xF0,
                0x32,
                z80_port as u8,
                (z80_port >> 8) as u8,
                0x18,
                0xFE,
            ],
        );
        // 6502: LDA #1 / STA port (-> Z80) / STA $11 / JMP *
        load(
            &mut m,
            0x300,
            &[
                0xA9,
                0x01,
                0x8D,
                port as u8,
                (port >> 8) as u8,
                0x85,
                0x11,
                0x4C,
                0x07,
                0x03,
            ],
        );
        m.cpu_mut().pc = 0x300;
        m.run_cycles(200);
        assert_eq!(m.peek(0x10), 0x42, "slot {slot}: Z80 wrote Apple RAM");
        assert_eq!(m.peek(0x11), 1, "slot {slot}: 6502 resumed");
        assert_eq!(m.cpu_owner(), None, "slot {slot}");
    }
}
