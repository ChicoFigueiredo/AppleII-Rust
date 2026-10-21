//! Regressões de cartas e periféricos (HU-0702): #115 (DDial, TAPEIN), #751 (No-Slot Clock e
//! power-cycle), #232 (mouse), #495 (mouse + NSC + `STA $CFFF`) e #498 (Z80 + mouse, `$C800`).
//! Oráculo: AppleWin a8f0d1d. Resultados em `docs/conformance/results/entrada-cartas.csv`.

use apple2_cards::{FixedClock, MouseCard, NoSlotClock, SoftCard, WallTime};
use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

fn nsc() -> Box<NoSlotClock> {
    Box::new(NoSlotClock::new(Box::new(FixedClock(WallTime::SAMPLE))))
}

// ------------------------------------------------------------------ #115

/// #115 (Diversi-Dial: o relógio corria rápido): `TapeRead` devolve o barramento flutuante com o
/// bit 7 sempre em 1 ("TAPEIN has high bit 1 when input is low or not connected", Tape.cpp:45).
/// Antes da correção o bit 7 vinha do barramento flutuante e piscava com o vídeo, e o programa
/// contava transições que não existiam.
#[test]
fn gh115_tapein_bit7_is_always_high() {
    for model in [
        Model::Apple2,
        Model::Apple2Plus,
        Model::Apple2e,
        Model::Apple2eEnhanced,
    ] {
        let mut m = Machine::new(MachineConfig::for_model(model));
        // varre um quadro inteiro (o flutuante muda de byte a cada ciclo)
        for _ in 0..17_030 {
            m.step();
            assert_eq!(rd(&mut m, 0xC060) & 0x80, 0x80, "{model:?}");
        }
    }
}

// ------------------------------------------------------------------ #751

/// Pattern de desbloqueio do NSC (`0x5CA33AC55CA33AC5`), como em `clocks.rs`.
const PATTERN: u64 = 0x5CA3_3AC5_5CA3_3AC5;

/// #751: o `m_bClockRegisterEnabled` ficava ligado depois do power-cycle e `$C8xx` com A2 em 1
/// (p.ex. `$C855`) devolvia o relógio (`00`, um BRK) em vez da ROM. Depois do power-cycle os
/// bytes da ROM interna voltam a ser os da ROM.
#[test]
fn gh751_power_cycle_clears_the_clock_register_enabled() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    m.set_rom_tap(Some(nsc()));
    wr(&mut m, 0xC007, 0); // INTCXROM: $C8xx é a ROM interna, vigiada pelo NSC
                           // bytes da ROM nos endereços com A2 em 1, lidos antes de qualquer desbloqueio
    let addrs: Vec<u16> = (0xC840u16..0xC900).filter(|a| a & 4 != 0).collect();
    let rom: Vec<u8> = addrs.iter().map(|&a| m.peek(a)).collect();
    assert!(
        rom.iter().any(|&b| b > 1),
        "a ROM interna tem bytes que o relógio não imita"
    );
    // desbloqueia o relógio (reset do registrador de comparação + 64 bits do padrão)
    rd(&mut m, 0xC804);
    for i in 0..64 {
        rd(&mut m, 0xC800 + ((PATTERN >> i) & 1) as u16);
    }
    assert_ne!(
        addrs.iter().map(|&a| rd(&mut m, a)).collect::<Vec<_>>(),
        rom,
        "com o relógio habilitado a leitura vem dele"
    );
    m.power_cycle();
    wr(&mut m, 0xC007, 0);
    let after: Vec<u8> = addrs.iter().map(|&a| rd(&mut m, a)).collect();
    assert_eq!(after, rom, "depois do power-cycle a ROM é visível de novo");
}

// ------------------------------------------------------------------ #232

/// #232: o mouse não pode continuar gerando VBL depois de desligado. O AppleWin zerou o modo no
/// `Reset()` (970dd00: `m_byMode = 0`); o port zera o modo no reset e não dá IRQ por VBL sem o
/// modo ligado.
#[test]
fn gh232_mouse_stops_raising_vbl_after_a_reset() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert!(m.insert_card(4, Box::new(MouseCard::new())));
    // SETMOUSE com modo 0x09 (ligado + interrupção de VBL) pelo firmware, como em mouse.rs
    m.poke(0x300, 0xA2);
    m.poke(0x301, 0xC4);
    m.poke(0x302, 0xA0);
    m.poke(0x303, 0x40);
    m.poke(0x304, 0xA9);
    m.poke(0x305, 0x09);
    for (i, b) in [0x20u8, 0x40, 0x03, 0x4C, 0x09, 0x03].iter().enumerate() {
        m.poke(0x306 + i as u16, *b);
    }
    for (i, b) in [
        0x48u8, 0xAD, 0x12, 0xC4, 0x8D, 0x49, 0x03, 0x68, 0x4C, 0x00, 0xC4,
    ]
    .iter()
    .enumerate()
    {
        m.poke(0x340 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    for _ in 0..200_000 {
        if m.cpu().pc == 0x309 {
            break;
        }
        m.step();
    }
    assert_eq!(m.cpu().pc, 0x309);
    m.run_frame();
    m.run_frame();
    assert!(m.board().interrupts.irq_line(), "VBL levantou o IRQ");
    m.reset();
    // o Ctrl-Reset não mexe na linha de IRQ (no AppleWin também não); quem a solta é o firmware
    m.board_mut().interrupts.clear();
    m.cpu_mut().pc = 0x0300;
    for a in 0x300..0x400u16 {
        m.poke(a, 0xEA); // sem o programa de SETMOUSE: o modo não volta
    }
    for _ in 0..3 {
        m.run_frame();
    }
    assert_eq!(m.card::<MouseCard>(4).unwrap().mode(), 0);
    assert!(
        !m.board().interrupts.irq_line(),
        "o mouse parou de gerar VBL"
    );
}

// ------------------------------------------------------------------ #495

/// #495: `STA $CFFF` (solta o espaço de expansão) marcava a página suja e o loop final de
/// `UpdatePaging()` copiava uma página de zeros sobre a ROM interna `$CF00-$CFFF`; depois disso
/// o programa caía no Monitor em `$CF38`. As escritas em `$C800-$CFFF` e em `$Cn00` nunca
/// alteram a ROM interna, com mouse e NSC.
#[test]
fn gh495_writes_to_cfff_do_not_corrupt_the_internal_rom() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(4, Box::new(MouseCard::new())));
    m.set_rom_tap(Some(nsc()));
    wr(&mut m, 0xC007, 0); // INTCXROM
    let snapshot = |m: &mut Machine| -> Vec<u8> { (0xC100u16..0xD000).map(|a| rd(m, a)).collect() };
    let before = snapshot(&mut m);
    assert!(
        before[0xCF00 - 0xC100..].iter().any(|&b| b != 0),
        "$CF00-$CFFF da ROM interna não é uma página de zeros"
    );
    for a in [0xCFFFu16, 0xC800, 0xCF00, 0xCF38, 0xC3FF, 0xC400] {
        wr(&mut m, a, 0x00);
        wr(&mut m, a, 0xFF);
    }
    // com INTCXROM desligado, o mesmo
    wr(&mut m, 0xC006, 0);
    wr(&mut m, 0xCFFF, 0x00);
    rd(&mut m, 0xCFFF);
    wr(&mut m, 0xC007, 0);
    assert_eq!(snapshot(&mut m), before, "a ROM interna ficou intacta");
    // e por código: STA $CFFF seguido de JSR no meio da ROM
    m.poke(0x300, 0x8D);
    m.poke(0x301, 0xFF);
    m.poke(0x302, 0xCF);
    m.poke(0x303, 0x4C);
    m.poke(0x304, 0x03);
    m.poke(0x305, 0x03);
    m.cpu_mut().pc = 0x300;
    m.step();
    assert_eq!(snapshot(&mut m), before);
}

// ------------------------------------------------------------------ #498

/// #498: o CP/M com mouse no slot 4 e Z80 no slot 5 executava `JSR $C800` no barramento
/// flutuante. Não era defeito do AppleWin: o CP/M Microsoft antigo usa o protocolo Pascal 1.0
/// (entrada em `$C800`), que o mouse e o Z80 não têm. O AppleWin só tem que seguir o hardware
/// (sequência da issue no //e real): `$C800` é flutuante, `$C300` liga a ROM interna em `$C800`,
/// `$CFFF` solta, `INTCXROM` mantém a interna, e ler `$C400` (mouse, sem ROM de expansão) não
/// mexe em `INTC8ROM`.
#[test]
fn gh498_c800_follows_the_real_hardware_with_mouse_and_z80() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(4, Box::new(MouseCard::new())));
    assert!(m.insert_card(5, Box::new(SoftCard::new())));
    let internal = |m: &mut Machine| -> Vec<u8> { (0xC800u16..0xC810).map(|a| rd(m, a)).collect() };
    let floating = |m: &mut Machine| -> Vec<u8> {
        let fb = m.board().floating_bus();
        vec![fb; 16]
    };
    // valor da ROM interna: INTCXROM ligado
    wr(&mut m, 0xC007, 0);
    let rom = internal(&mut m);
    assert_ne!(
        rom,
        floating(&mut m),
        "a ROM interna difere do barramento flutuante"
    );
    wr(&mut m, 0xC006, 0);
    rd(&mut m, 0xCFFF);

    // C800L: só o barramento flutuante
    assert_eq!(internal(&mut m), floating(&mut m));
    // ler $C400 (o mouse) e $C500 (o Z80) não liga a ROM interna (corrige a regressão 1.26.2.0)
    rd(&mut m, 0xC400);
    assert_eq!(
        internal(&mut m),
        floating(&mut m),
        "slot 4 seleciona o IO SELECT', não a interna"
    );
    rd(&mut m, 0xC500);
    assert_eq!(internal(&mut m), floating(&mut m));
    // C300 liga a ROM interna em $C800
    rd(&mut m, 0xC300);
    assert_eq!(internal(&mut m), rom, "$C300 liga INTC8ROM");
    // $CFFF solta
    rd(&mut m, 0xCFFF);
    assert_eq!(internal(&mut m), floating(&mut m));
    // INTCXROM: $C800 interna mesmo depois de $CFFF; ao desligar, volta ao flutuante
    wr(&mut m, 0xC007, 0);
    rd(&mut m, 0xCFFF);
    assert_eq!(internal(&mut m), rom);
    assert_eq!(rd(&mut m, 0xC015) & 0x80, 0x80, "INTCXROM aparece em $C015");
    wr(&mut m, 0xC006, 0);
    assert_eq!(rd(&mut m, 0xC015) & 0x80, 0);
    rd(&mut m, 0xCFFF);
    assert_eq!(internal(&mut m), floating(&mut m));
}
