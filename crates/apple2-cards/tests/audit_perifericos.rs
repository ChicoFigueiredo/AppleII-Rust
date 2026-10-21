//! Regressões de relógio (P23, gate G0): #179/#969 (convenção do dia da semana do No-Slot
//! Clock), #827 (NSC não responde em `$C3xx` no ][/][+) e #1196 (utilitário GPIO e o `$C305` do
//! slot 3). Oráculo: AppleWin a8f0d1d. Resultados em `docs/conformance/results/p23-perifericos.csv`.

use apple2_cards::{FixedClock, NoSlotClock, WallTime};
use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;

/// Padrão de desbloqueio (`CNoSlotClock::m_ComparisonRegister`, NoSlotClock.cpp).
const PATTERN: u64 = 0x5CA3_3AC5_5CA3_3AC5;

fn nsc(t: WallTime) -> Box<NoSlotClock> {
    Box::new(NoSlotClock::new(Box::new(FixedClock(t))))
}

/// Desbloqueia o relógio pelas leituras de `base` e lê os 64 bits do registrador de `base | 4`.
fn read_register(m: &mut Machine, base: u16) -> [u8; 8] {
    m.board_mut().read(base | 4);
    for i in 0..64 {
        m.board_mut().read(base | ((PATTERN >> i) & 1) as u16);
    }
    let mut bits = 0u64;
    for i in 0..64 {
        bits |= u64::from(m.board_mut().read(base | 4) & 1) << i;
    }
    bits.to_le_bytes()
}

/// #179/#969: o dia da semana do NSC é `wDayOfWeek + 1` (1 = domingo, NoSlotClock.cpp:166). O
/// ProLine BBS segue a convenção oposta e por isso mostra o dia errado em AppleWin, como a
/// issue concluiu; o port repete a convenção do AppleWin para os sete dias.
#[test]
fn gh179_gh969_day_of_week_is_wday_plus_one_for_all_seven_days() {
    for weekday in 0..7u8 {
        let t = WallTime {
            weekday,
            ..WallTime::SAMPLE
        };
        let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
        m.set_rom_tap(Some(nsc(t)));
        m.board_mut().write(0xC007, 0); // INTCXROM: o NSC vigia $C3xx e $C8xx
        let reg = read_register(&mut m, 0xC300);
        // centésimos, s, min, h, DOW, dia, mês, ano (BCD, nibble baixo primeiro)
        assert_eq!(reg[4], weekday + 1, "weekday {weekday}");
        assert_eq!(reg[5], 0x19, "o dia do mês não muda");
        // as mesmas páginas $C8xx respondem com o mesmo registrador (#179: antes só havia $C8)
        let mut m2 = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
        m2.set_rom_tap(Some(nsc(t)));
        m2.board_mut().write(0xC007, 0);
        assert_eq!(read_register(&mut m2, 0xC800), reg);
    }
}

/// #827: no ][ e no ][+ a ROM de `$C3xx` não existe (nem o NSC, de 28 pinos, cabe nas ROMs de 24
/// pinos): `IsPotentialNoSlotClockAccess` só vale com `!IS_APPLE2` (Memory.cpp:1035). A máquina
/// com o NSC instalado lê `$C3xx` exatamente como a sem NSC, depois do padrão de desbloqueio.
#[test]
fn gh827_nsc_does_not_answer_at_c3xx_on_the_apple_ii_and_ii_plus() {
    for model in [Model::Apple2, Model::Apple2Plus] {
        let mut with = Machine::new(MachineConfig::for_model(model));
        with.set_rom_tap(Some(nsc(WallTime::SAMPLE)));
        let mut without = Machine::new(MachineConfig::for_model(model));
        let a = read_register(&mut with, 0xC300);
        let b = read_register(&mut without, 0xC300);
        assert_eq!(a, b, "{model:?}: o NSC não pode mudar $C3xx");
    }
}

/// #1196: o utilitário GPIO lê `$C305` duas vezes em laço e só segue se os valores forem iguais
/// (`LDA $C305 / CMP $C305`). Na //e Enhanced o `$C3xx` é a ROM interna (valor fixo); no ][+ com o
/// slot 3 vazio é o barramento flutuante (varia com o vídeo), então o laço não termina, como o
/// issue explica; com uma carta de ROM no slot 3 (o mouse) o valor é fixo. O port repete o
/// comportamento do AppleWin nos três casos.
#[test]
fn gh1196_c305_is_constant_with_a_rom_and_floating_without_a_card_on_the_ii_plus() {
    fn distinct_reads(m: &mut Machine) -> usize {
        m.run_cycles(500_000); // fim da partida: as chaves da ROM de $C3xx já estão estáveis
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..20_000 {
            m.step();
            seen.insert(m.board_mut().read(0xC305));
        }
        seen.len()
    }
    let mut iie = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert_eq!(distinct_reads(&mut iie), 1, "//e: ROM interna");

    let mut plus = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert!(
        distinct_reads(&mut plus) > 1,
        "][+ com slot 3 vazio: barramento flutuante"
    );

    let mut plus_card = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert!(plus_card.insert_card(3, Box::new(apple2_cards::MouseCard::new())));
    assert_eq!(distinct_reads(&mut plus_card), 1, "][+ com ROM no slot 3");
}
