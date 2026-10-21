//! Auditoria wave0-core, area 1 (HU-0102): caminho completo de `Saturn128K.banks`
//! TOML -> Settings -> MachineConfig -> Machine -> carta.
//!
//! C++: LanguageCard.cpp:324-345 (`m_uSaturnTotalBanks = banks`), :410-434 (selecao de banco
//! >= total e fixada em total-1), :499 (save grava o total real).

use apple2_config::{Settings, SlotCardSpec};
use apple2_core::language_card::LcKind;
use apple2_core::{Machine, Model};
use apple2_cpu::Bus;

fn rom_ii() -> Vec<u8> {
    let mut r = vec![0xEAu8; 0x3000];
    r[0x2FFC] = 0x00;
    r[0x2FFD] = 0x03;
    r
}

fn settings_with_saturn(banks: u8) -> Settings {
    let mut s = Settings::default();
    s.machine.model = Model::Apple2Plus;
    s.reset_slot0_for_model();
    *s.slot_mut(4) = SlotCardSpec::Saturn128K { banks };
    // Passa pelo TOML, como o programa faz ao reabrir as configuracoes.
    Settings::from_toml(&s.to_toml().unwrap()).unwrap()
}

fn machine_from(s: &Settings) -> Machine {
    let mut cfg = s.to_machine_config();
    cfg.custom_rom = Some(rom_ii());
    Machine::new(cfg)
}

fn machine_from_settings(banks: u8) -> Machine {
    machine_from(&settings_with_saturn(banks))
}

fn saturn_total_banks(m: &Machine) -> u8 {
    m.board()
        .lc
        .infos()
        .iter()
        .find_map(|i| match i.kind {
            LcKind::Saturn { banks, .. } => Some(banks),
            _ => None,
        })
        .expect("Saturn in slot 4")
}

/// Escreve `v` em $D000 do banco selecionado por `sel` (offset em $C0C0) e devolve o que se le
/// de $D000 depois de selecionar `probe`.
fn write_in_then_read_from(m: &mut Machine, sel: u16, v: u8, probe: u16) -> u8 {
    m.board_mut().read(0xC0C3);
    m.board_mut().read(0xC0C3); // habilita escrita
    m.board_mut().read(0xC0C0 + sel);
    m.board_mut().write(0xD000, v);
    m.board_mut().read(0xC0C0 + probe);
    m.board_mut().read(0xD000)
}

#[test]
fn settings_toml_keeps_the_banks() {
    // Primeiro elo da cadeia: PASS.
    assert_eq!(
        settings_with_saturn(4).slot(4),
        &SlotCardSpec::Saturn128K { banks: 4 }
    );
}

#[test]
fn machine_config_carries_saturn_bank_count() {
    // machine.rs:64 descarta `banks` (`Saturn128K { .. }`); language_card.rs:116 fixa 8.
    let m4 = machine_from_settings(4);
    let m8 = machine_from_settings(8);
    assert_eq!(saturn_total_banks(&m4), 4, "4 bancos configurados");
    assert_eq!(saturn_total_banks(&m8), 8);
}

#[test]
fn four_and_eight_banks_behave_differently() {
    // Banco 4 ($C0CC) num Saturn 64K: o AppleWin fixa em total-1 (banco 3), logo ve o que foi
    // escrito no banco 3 ($C0C7). Com 8 bancos reais, o banco 4 e outro e le 0.
    let mut m4 = machine_from_settings(4);
    let mut m8 = machine_from_settings(8);
    let via4 = write_in_then_read_from(&mut m4, 7, 0x77, 0xC);
    let via8 = write_in_then_read_from(&mut m8, 7, 0x77, 0xC);
    assert_eq!(
        via4, 0x77,
        "Saturn 64K: banco 4 aliasa o banco 3 (C++ :424-430)"
    );
    assert_eq!(via8, 0x00, "Saturn 128K: banco 4 e distinto");
}

#[test]
fn eight_banks_work_through_settings() {
    let mut m8 = machine_from_settings(8);
    assert_eq!(saturn_total_banks(&m8), 8);
    assert_eq!(write_in_then_read_from(&mut m8, 7, 0x77, 0xC), 0x00);
    assert_eq!(write_in_then_read_from(&mut m8, 7, 0x77, 0x7), 0x77);
}

#[test]
fn out_of_range_bank_count_never_exceeds_the_card_capacity() {
    // AUD-C-002 (guarda): o TOML aceita banks = 0 e 200 sem erro (card.rs:284 nao valida; so o
    // editor da UI, model.rs:641, valida). Hoje o valor e ignorado e a carta sempre tem 8 bancos,
    // entao nada alem da capacidade fica acessivel. Ao corrigir AUD-C-001, esta guarda tem de
    // continuar valendo: 1..=8 bancos, nunca mais que 8, e banks > 8 vira erro ou clamp a 8.
    for bad in [0u8, 9, 200] {
        let text = format!("[slots.4]\ncard = \"Saturn128K\"\nbanks = {bad}\n");
        if let Ok(s) = Settings::from_toml(&text) {
            let m = machine_from(&s);
            let n = saturn_total_banks(&m);
            assert!((1..=8).contains(&n), "banks={bad} virou {n} bancos");
        }
    }
}
