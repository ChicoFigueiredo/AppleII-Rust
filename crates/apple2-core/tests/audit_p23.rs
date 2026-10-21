//! P2.3 (G0): regressoes de memoria de issues do upstream que ainda nao tinham teste proprio.
//! O oraculo e o C++ do AppleWin (Memory.cpp, CmdLine.cpp) e os casos de trace do `conformance`.

use apple2_core::{AuxCardKind, Machine, MachineConfig, Model};
use apple2_cpu::Bus;

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

/// #478 (RamWorks ate 16 MB), #1349 (contagem de bancos), #1336 e #146 (RamWorks como memoria
/// auxiliar): `kMaxExMemoryBanks = 256` em Memory.h; o registrador $C073 aceita 0..=255 e cada
/// banco guarda o seu proprio padrao, sem aliasing.
#[test]
fn issue_478_1349_ramworks_with_256_banks_is_addressable() {
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    cfg.aux_card = AuxCardKind::RamWorksIII { banks: 256 };
    let mut m = Machine::new(cfg);
    wr(&mut m, 0xC005, 0); // RAMWRT
    wr(&mut m, 0xC003, 0); // RAMRD
    for b in 0..=255u8 {
        wr(&mut m, 0xC073, b);
        assert_eq!(m.board().memory.aux_bank(), b as u16, "bank register {b}");
        wr(&mut m, 0x0300, b ^ 0xA5);
        wr(&mut m, 0xBFFF, b.wrapping_mul(7));
    }
    for b in 0..=255u8 {
        wr(&mut m, 0xC073, b);
        assert_eq!(rd(&mut m, 0x0300), b ^ 0xA5, "bank {b}");
        assert_eq!(rd(&mut m, 0xBFFF), b.wrapping_mul(7), "bank {b}");
    }
    // More than 256 requested: the card is clamped to 256 banks (CmdLine.cpp:473, Memory.cpp:416).
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    cfg.aux_card = AuxCardKind::RamWorksIII { banks: 300 };
    let mut m = Machine::new(cfg);
    wr(&mut m, 0xC073, 255);
    assert_eq!(m.board().memory.aux_bank(), 255);
}

/// #590: o Apple ][ original nao tem Language Card por padrao (48K); o ][+ tem. A ROM em
/// $D000-$FFFF nao aceita escrita e $C081 nao liga RAM.
#[test]
fn issue_590_original_apple2_defaults_to_no_language_card() {
    let cfg = MachineConfig::for_model(Model::Apple2);
    assert!(cfg.slots[0].is_none(), "Apple ][: slot 0 vazio");
    let cfg = MachineConfig::for_model(Model::Apple2Plus);
    assert!(cfg.slots[0].is_some(), "Apple ][+: Language Card no slot 0");

    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2));
    let before = rd(&mut m, 0xD000);
    rd(&mut m, 0xC081);
    rd(&mut m, 0xC081);
    wr(&mut m, 0xD000, before ^ 0xFF);
    assert_eq!(
        rd(&mut m, 0xD000),
        before,
        "sem LC, a ROM nao e sobrescrita"
    );
}
