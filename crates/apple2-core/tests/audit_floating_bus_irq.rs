//! Auditoria wave0-core, areas 3 e 4 (HU-0101 / HU-0103): bordas de IRQ e barramento flutuante
//! com RAMRD. Oraculo: CPU.cpp (IRQ, g_irqDefer1Opcode), Memory.cpp:2456-2471 (ReadFloatingBus).

use apple2_core::irq::IrqSource;
use apple2_core::{AuxCardKind, Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use std::collections::BTreeSet;

fn iie() -> Machine {
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    cfg.aux_card = AuxCardKind::Extended80Col;
    Machine::new(cfg)
}

fn nop_sled(m: &mut Machine) {
    for a in 0x6000..0xC000u16 {
        m.poke_main(a, 0xEA);
        m.poke_aux(a, 0xEA);
    }
    m.cpu_mut().pc = 0x6000;
}

/// Valores que o barramento flutuante assume durante um quadro inteiro, instrucao a instrucao.
fn floating_values(m: &mut Machine) -> BTreeSet<u8> {
    let mut seen = BTreeSet::new();
    for _ in 0..8600 {
        m.step();
        seen.insert(m.board().floating_bus());
    }
    seen
}

#[test]
fn floating_bus_reads_main_text_page_with_default_switches() {
    let mut m = iie();
    nop_sled(&mut m);
    for a in 0x400..0x800u16 {
        m.poke_main(a, 0xAA);
        m.poke_aux(a, 0x55);
    }
    let seen = floating_values(&mut m);
    assert!(seen.contains(&0xAA), "{seen:02X?}");
    assert!(!seen.contains(&0x55), "{seen:02X?}");
}

#[test]
#[ignore = "reproduz achado AUD-C-050; remover ignore quando corrigido"]
fn floating_bus_with_ramrd_follows_the_cpu_read_image_like_applewin() {
    // Memory.cpp:2460/2470: `MemGetMainPtr(0)[scanner]` indexa `mem`, a imagem que a CPU le. Com
    // RAMRD=1 (e ALTZP=0) o AppleWin devolve o byte da RAM AUXILIAR; memory.rs/io.rs:22 le sempre
    // a principal (`peek_main`). Divergencia do AppleWin; o hardware real le a principal. Se a
    // equipe optar por manter o hardware, registrar como "Divergencia intencional" com evidencia
    // e trocar este teste por um que afirme o comportamento atual.
    let mut m = iie();
    nop_sled(&mut m);
    for a in 0x400..0x800u16 {
        m.poke_main(a, 0xAA);
        m.poke_aux(a, 0x55);
    }
    m.board_mut().write(0xC003, 0); // RAMRD
    let seen = floating_values(&mut m);
    assert!(seen.contains(&0x55), "{seen:02X?}");
    assert!(!seen.contains(&0xAA), "{seen:02X?}");
}

#[test]
fn irq_is_taken_right_after_cli_without_the_one_instruction_delay() {
    // CPU.cpp:IRQ(): g_bmIRQ && !(ps & AF_INTERRUPT) e testado antes de cada opcode; nao ha o
    // atraso de uma instrucao do 6502 real depois de CLI. O PC empilhado e o do proximo opcode.
    let mut m = iie();
    nop_sled(&mut m);
    // SEI; CLI; NOP
    for (i, b) in [0x78u8, 0x58, 0xEA].iter().enumerate() {
        m.poke_main(0x6000 + i as u16, *b);
    }
    m.board_mut().interrupts.set_irq(IrqSource::Mouse, true);
    m.step(); // SEI: mascarado, nada acontece
    assert_eq!(m.cpu().pc, 0x6001);
    m.step(); // CLI
    assert_eq!(m.cpu().pc, 0x6002);
    let c = m.step(); // aqui o IRQ e atendido (7 ciclos), sem executar o NOP
    assert_eq!(c, 7, "IRQ entry");
    assert_ne!(m.cpu().pc, 0x6003);
    // pilha: PC empilhado = $6002
    let sp = m.cpu().sp as u16;
    assert_eq!(m.peek(0x100 + ((sp + 3) & 0xFF)), 0x60);
    assert_eq!(m.peek(0x100 + ((sp + 2) & 0xFF)), 0x02);
}

#[test]
fn irq_while_masked_by_sei_is_held_until_cli() {
    let mut m = iie();
    nop_sled(&mut m);
    m.poke_main(0x6000, 0x78); // SEI
    m.board_mut().interrupts.set_irq(IrqSource::Mouse, true);
    for _ in 0..50 {
        m.step();
    }
    assert!(
        m.cpu().pc >= 0x6001 && m.cpu().pc < 0x6100,
        "{:04X}",
        m.cpu().pc
    );
}
