//! GH#1321 (e #418): o Uthernet I e o Uthernet II nao ligam IOSEL, entao o espaco $C3xx de um
//! cartao deles no slot 3 de um Apple ][+ e o barramento flutuante e nao "uma ROM de zeros" que o
//! ProDOS tomaria por um cartao de 80 colunas ($BF98 = $62 em vez de $60). A correcao do
//! AppleWin (1.30.20.0) devolve o barramento flutuante nos dois cartoes.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::network::NullNetwork;
use apple2_hostio::{Uthernet1, Uthernet2};

fn check(mut m: Machine, name: &str) {
    for a in 0x400..0x800u16 {
        m.poke_main(a, a as u8);
    }
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..2000 {
        m.step();
        let fb = m.board().floating_bus();
        let v = m.board_mut().read(0xC315);
        assert_eq!(v, fb, "{name}: $C315 e o barramento flutuante");
        seen.insert(v);
    }
    assert!(seen.len() > 1, "{name}: o valor segue o scanner de video");
    // A deteccao de terminal do ProDOS (DEVSRCH): $C305=$38, $C307=$18, $C30B=$01.
    let sig = [0xC305u16, 0xC307, 0xC30B].map(|a| m.board_mut().read(a));
    assert_ne!(
        sig,
        [0x38, 0x18, 0x01],
        "{name}: nao parece um cartao de 80 colunas"
    );
}

#[test]
fn uthernet_i_in_slot_3_of_an_apple_2_plus_floats_the_rom_space() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert!(m.insert_card(3, Box::new(Uthernet1::new(Box::new(NullNetwork)))));
    check(m, "Uthernet I");
}

#[test]
fn uthernet_ii_in_slot_3_of_an_apple_2_plus_floats_the_rom_space() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert!(m.insert_card(3, Box::new(Uthernet2::new(Box::new(NullNetwork), true))));
    check(m, "Uthernet II");
}
