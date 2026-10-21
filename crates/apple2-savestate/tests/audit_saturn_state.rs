//! Auditoria wave0-core, areas 1 e 6 (HU-0102 x HU-0401): Saturn 64K no save-state.

use apple2_core::language_card::LcKind;
use apple2_core::{CardKind, Machine, MachineConfig, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::image::DiskImage;

struct NoImages;

impl LoadHost for NoImages {
    fn open_image(&mut self, _req: &ImageRequest) -> Result<DiskImage, String> {
        Err("sem imagens".into())
    }
}

fn saturn_banks(m: &Machine) -> Option<u8> {
    m.board().lc.infos().iter().find_map(|i| match i.kind {
        LcKind::Saturn { banks, .. } => Some(banks),
        _ => None,
    })
}

fn plus_with_saturn() -> Machine {
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.slots[4] = Some(CardKind::Saturn128K);
    Machine::new(cfg)
}

#[test]
fn eight_bank_saturn_round_trips() {
    let m = plus_with_saturn();
    let text = save_aws_yaml(&m, &SaveOptions::default()).unwrap().text;
    let loaded = load_aws_yaml(&text, &mut NoImages).unwrap();
    assert_eq!(saturn_banks(&loaded.machine), Some(8));
}

#[test]
fn saturn_with_four_banks_keeps_its_size_through_load() {
    // C++ LanguageCard.cpp:499/:518 grava e le o total real de bancos (4 = 64K). Aqui o nucleo
    // fixa 8 (language_card.rs:116): ao ler um arquivo do AppleWin com 4 bancos, os bancos 4-7
    // passam a existir (zerados e acessiveis) e o proximo save grava 8.
    let m = plus_with_saturn();
    let text = save_aws_yaml(
        &m,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .unwrap()
    .text;
    let four = text.replace("Num Saturn Banks: 0x08", "Num Saturn Banks: 0x04");
    assert_ne!(four, text, "campo nao encontrado");
    let loaded = load_aws_yaml(&four, &mut NoImages).unwrap();
    assert_eq!(saturn_banks(&loaded.machine), Some(4));
}

#[test]
fn four_bank_saturn_is_saved_and_reloaded_with_four_banks() {
    // #1486/#1495, AUD-C-003: o save grava o total configurado (LanguageCard.cpp:499) e o
    // conteudo de cada banco; reler e salvar de novo nao infla para 8 bancos.
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.slots[4] = Some(CardKind::Saturn128K);
    cfg.saturn_banks[4] = 4;
    let mut m = Machine::new(cfg);
    {
        use apple2_cpu::Bus;
        m.board_mut().read(0xC0C3);
        m.board_mut().read(0xC0C3);
        m.board_mut().read(0xC0C7); // bank 3 (selector 4+3)
        m.board_mut().write(0xD000, 0x33);
    }
    let opts = SaveOptions {
        extension: false,
        ..Default::default()
    };
    let text = save_aws_yaml(&m, &opts).unwrap().text;
    assert!(text.contains("Num Saturn Banks: 0x04"), "{text}");
    assert!(!text.contains("Memory Bank04"));
    let loaded = load_aws_yaml(&text, &mut NoImages).unwrap();
    assert_eq!(saturn_banks(&loaded.machine), Some(4));
    let again = save_aws_yaml(&loaded.machine, &opts).unwrap().text;
    assert!(again.contains("Num Saturn Banks: 0x04"));
    assert!(again.contains("Active Saturn Bank: 0x03"));
}
