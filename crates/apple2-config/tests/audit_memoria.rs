//! HU-0102 (issue #18): cartoes de memoria sem perda de configuracao, pelo caminho completo
//! linha de comando / TOML -> Settings -> MachineConfig -> Machine. Cobre #1486 (Saturn no slot 0
//! pela linha de comando e pela configuracao), #1428 (RAM da //e Enhanced) e o BDD do plano (§17,
//! exemplo Saturn). Oraculo: LanguageCard.cpp:324-345 e :410-434 (total de bancos e aliasing).

use apple2_config::{CmdLine, Settings, SlotCardSpec};
use apple2_core::language_card::LcKind;
use apple2_core::{AuxCardKind, Machine, Model};
use apple2_cpu::Bus;

fn rom_ii() -> Vec<u8> {
    let mut r = vec![0xEAu8; 0x3000];
    r[0x2FFC] = 0x00;
    r[0x2FFD] = 0x03;
    r
}

fn rom_e() -> Vec<u8> {
    let mut r = vec![0xCCu8; 0x1000];
    r.extend(rom_ii());
    r
}

fn machine_from(s: &Settings) -> Machine {
    let mut cfg = s.to_machine_config();
    cfg.custom_rom = Some(if cfg.model.is_apple2() {
        rom_ii()
    } else {
        rom_e()
    });
    Machine::new(cfg)
}

fn saturn_banks(m: &Machine, slot: u8) -> Option<u8> {
    m.board()
        .lc
        .infos()
        .iter()
        .find(|i| i.slot == slot)
        .and_then(|i| match i.kind {
            LcKind::Saturn { banks, .. } => Some(banks),
            _ => None,
        })
}

/// Selector ($C0n4-$C0nF) do banco `b`.
fn select(b: u8) -> u16 {
    u16::from(4 | ((b & 4) << 1) | (b & 3))
}

/// Escreve `0x10+b` em $D000 de cada banco < `n` e devolve o que cada um dos 8 seletores le.
fn probe(m: &mut Machine, slot: u16, n: u8) -> Vec<u8> {
    let base = 0xC080 + slot * 16;
    m.board_mut().read(base + 3);
    m.board_mut().read(base + 3);
    for b in 0..n {
        m.board_mut().read(base + select(b));
        m.board_mut().write(0xD000, 0x10 + b);
    }
    (0..8)
        .map(|b| {
            m.board_mut().read(base + select(b));
            m.board_mut().read(0xD000)
        })
        .collect()
}

/// #1486: `-s0 saturn64` (linha de comando) e o TOML (configuracao) chegam a carta do slot 0.
#[test]
fn issue_1486_saturn_size_from_command_line_and_config() {
    for (args, banks) in [
        (["-model", "apple2p", "-s0", "saturn64"], 4u8),
        (["-model", "apple2p", "-s0", "saturn128"], 8u8),
    ] {
        let c = CmdLine::parse(&args).unwrap();
        let mut s = Settings::default();
        c.apply(&mut s);
        let s = Settings::from_toml(&s.to_toml().unwrap()).unwrap();
        let m = machine_from(&s);
        assert_eq!(saturn_banks(&m, 0), Some(banks), "{args:?}");
    }
    // Configuracao (TOML) sem passar pela linha de comando.
    let text = "[machine]\nmodel = \"Apple2Plus\"\n\n[slots.0]\ncard = \"Saturn128K\"\nbanks = 4\n";
    let s = Settings::from_toml(text).unwrap();
    assert_eq!(saturn_banks(&machine_from(&s), 0), Some(4));
}

/// BDD do plano §17: Scenario Outline "Carregar configuracao respeita quantidade de bancos".
/// Given //e com Saturn de <bancos>; When inicializa e troca pelos bancos validos; Then todos os
/// bancos esperados tem conteudo independente; And os de fora da capacidade nao ficam acessiveis
/// em silencio (aliasing no ultimo banco, como o AppleWin, LanguageCard.cpp:424-430, e nada de
/// armazenamento novo); And coincide com o oraculo.
#[test]
fn bdd_saturn_banks_are_independent_and_capacity_is_enforced() {
    for banks in [4u8, 8] {
        let mut s = Settings::default();
        s.machine.model = Model::Apple2e;
        *s.slot_mut(4) = SlotCardSpec::Saturn128K { banks };
        let s = Settings::from_toml(&s.to_toml().unwrap()).unwrap();
        let mut m = machine_from(&s);
        assert_eq!(saturn_banks(&m, 4), Some(banks));
        let seen = probe(&mut m, 4, banks);
        let last = 0x10 + banks - 1;
        for b in 0..8u8 {
            let want = if b < banks { 0x10 + b } else { last };
            assert_eq!(seen[b as usize], want, "{banks} bancos, seletor {b}");
        }
        // Escrever num seletor fora da capacidade cai no ultimo banco real, sem criar RAM nova.
        if banks < 8 {
            m.board_mut().read(0xC080 + 4 * 16 + select(banks + 1));
            m.board_mut().write(0xD000, 0xEE);
            m.board_mut().read(0xC080 + 4 * 16 + select(banks - 1));
            assert_eq!(m.board_mut().read(0xD000), 0xEE);
        }
    }
}

/// AUD-C-002: contagens fora de 1..=8 nunca expoem mais bancos que a capacidade da carta.
#[test]
fn out_of_range_counts_are_clamped_to_the_card() {
    for (bad, want) in [(0u8, 1u8), (9, 8), (200, 8)] {
        let mut s = Settings::default();
        s.machine.model = Model::Apple2Plus;
        *s.slot_mut(4) = SlotCardSpec::Saturn128K { banks: bad };
        assert_eq!(
            saturn_banks(&machine_from(&s), 4),
            Some(want),
            "banks={bad}"
        );
    }
}

/// #1428 "Enhanced //e RAM issue?": o padrao da //e Enhanced e 128K (Extended 80 Col, 64K aux) e
/// a RAM e ajustavel pelas configuracoes (RamWorks III: 1 a 256 bancos de 64K).
#[test]
fn issue_1428_default_is_128k_and_ram_is_adjustable() {
    let s = Settings::default();
    assert_eq!(s.machine.model, Model::Apple2eEnhanced);
    assert_eq!(s.to_machine_config().aux_card, AuxCardKind::Extended80Col);
    let mut s = Settings::default();
    s.aux.card = SlotCardSpec::RamWorksIII { banks: 16 };
    let s = Settings::from_toml(&s.to_toml().unwrap()).unwrap();
    assert_eq!(
        s.to_machine_config().aux_card,
        AuxCardKind::RamWorksIII { banks: 16 }
    );
    // `-r 127` da linha de comando.
    let c = CmdLine::parse(&["-r", "127"]).unwrap();
    let mut s = Settings::default();
    c.apply(&mut s);
    assert_eq!(
        s.to_machine_config().aux_card,
        AuxCardKind::RamWorksIII { banks: 127 }
    );
}
