//! Auditoria wave0 (HU-0601): Z80, MEMPTR e instruções de bloco repetitivas.
//!
//! Oráculo para interrupções: AppleWin a8f0d1d `Z80VICE/z80.cpp:470` e `:5540`
//! ("[AppleWin-TC] Z80 IRQs not supported"): o AppleWin NÃO entrega IRQ/NMI, e o Rust também não
//! (`Z80` não tem entrada de interrupção). Paridade com o oráculo, lacuna contra a HU-0601.
//! O AppleWin também não modela MEMPTR; o Rust modela (além do oráculo). Os efeitos de MEMPTR
//! em INI/OUTI estão certos (testes PASS); o que falta são os efeitos de flags 3/5 (e PF/HF em
//! INIR/OTIR) quando a instrução repete, que o hardware real mostra (Sean Young, "The Undocumented
//! Z80 Documented" + correções de Patrik Rak, 2018+). Esses testes ficam `#[ignore]`.

use apple2_cards::z80::{Z80Bus, Z80};

struct Ram(Vec<u8>);

impl Z80Bus for Ram {
    fn read(&mut self, a: u16) -> u8 {
        self.0[a as usize]
    }
    fn write(&mut self, a: u16, v: u8) {
        self.0[a as usize] = v;
    }
    fn io_in(&mut self, _p: u16) -> u8 {
        0
    }
}

fn ram_with(at: u16, code: &[u8]) -> Ram {
    let mut r = Ram(vec![0; 0x10000]);
    r.0[at as usize..at as usize + code.len()].copy_from_slice(code);
    r
}

/// INI (ED A2): MEMPTR = BC (antes de decrementar B) + 1.
#[test]
fn aud_p_z80_ini_sets_memptr_to_bc_plus_one() {
    let mut ram = ram_with(0, &[0xED, 0xA2]);
    let mut z = Z80 {
        b: 0x12,
        c: 0x34,
        h: 0x40,
        l: 0,
        ..Default::default()
    };
    z.step(&mut ram);
    assert_eq!(z.wz, 0x1235);
}

/// OUTI (ED A3): MEMPTR = BC (depois de decrementar B) + 1.
#[test]
fn aud_p_z80_outi_sets_memptr_to_decremented_bc_plus_one() {
    let mut ram = ram_with(0, &[0xED, 0xA3]);
    let mut z = Z80 {
        b: 0x05,
        c: 0x10,
        h: 0x40,
        l: 0,
        ..Default::default()
    };
    z.step(&mut ram);
    assert_eq!(z.wz, 0x0411);
}

/// Quando repete, LDIR/INIR/OTIR fazem MEMPTR = PC da instrução + 1.
#[test]
fn aud_p_z80_repeating_block_instruction_sets_memptr_to_pc_plus_one() {
    for op in [0xB0u8, 0xB2, 0xB3] {
        let mut ram = ram_with(0x1000, &[0xED, op]);
        let mut z = Z80 {
            pc: 0x1000,
            b: 2,
            c: 1,
            d: 0x50,
            h: 0x40,
            ..Default::default()
        };
        z.step(&mut ram);
        assert_eq!(z.pc, 0x1000, "op ED {op:02X} repete");
        assert_eq!(z.wz, 0x1001, "op ED {op:02X}");
    }
}

/// Opcodes ED indefinidos viram NOP de 8 T-states no hardware; o Rust os executa assim, sem
/// nenhum aviso. Documenta o comportamento (HU-0601 pede relato explícito de opcode não suportado).
#[test]
fn aud_p_z80_undefined_ed_opcode_is_a_silent_8_tstate_nop() {
    let mut ram = ram_with(0, &[0xED, 0x00]);
    let mut z = Z80::default();
    let t = z.step(&mut ram);
    assert_eq!((t, z.pc), (8, 2));
}

/// Repro AUD-P (hardware, não AppleWin): com repetição ativa, os bits 3 e 5 de F de LDIR vêm do
/// byte alto do PC da instrução (0x28 aqui), não de A+(HL).
#[test]
fn aud_p_z80_ldir_repeat_takes_flags_3_5_from_pch() {
    let mut ram = ram_with(0x2800, &[0xED, 0xB0]);
    let mut z = Z80 {
        pc: 0x2800,
        a: 0,
        b: 0,
        c: 2, // BC=2: após uma iteração BC=1, repete
        h: 0x40,
        l: 0,
        d: 0x50,
        e: 0,
        f: 0,
        ..Default::default()
    };
    z.step(&mut ram);
    assert_eq!(z.pc, 0x2800, "repetiu");
    assert_eq!(z.f & 0x28, 0x28, "F3/F5 vêm de PCH=0x28 quando repete");
}

/// Idem para INIR/OTIR (além de PF/HF ajustados; aqui só F3/F5).
#[test]
fn aud_p_z80_inir_otir_repeat_takes_flags_3_5_from_pch() {
    for op in [0xB2u8, 0xB3] {
        let mut ram = ram_with(0x2800, &[0xED, op]);
        let mut z = Z80 {
            pc: 0x2800,
            b: 2,
            c: 1,
            h: 0x40,
            f: 0,
            ..Default::default()
        };
        z.step(&mut ram);
        assert_eq!(z.pc, 0x2800);
        assert_eq!(z.f & 0x28, 0x28, "ED {op:02X}");
    }
}
