//! P23 (gate G0), linha de comando e opções de disco. Oráculo: AppleWin a8f0d1d (CmdLine.cpp).
//! Resultados em `docs/conformance/results/p23-disco.csv`.

use apple2_config::card::SlotCardSpec;
use apple2_config::cmdline::CmdLine;
use apple2_config::settings::Settings;
use std::path::PathBuf;

fn applied(args: &[&str]) -> Settings {
    let mut s = Settings::default();
    CmdLine::parse(args).unwrap().apply(&mut s);
    s
}

/// GH#663: `-d1`/`-d2` guardam o caminho para o Disk II do slot 6 (CmdLine.cpp:140-151).
#[test]
fn gh663_d1_and_d2_insert_the_images_in_slot_6() {
    let s = applied(&["-d1", "game.dsk", "-d2", "data.po"]);
    match s.slot(6) {
        SlotCardSpec::Disk2 { drives, .. } => {
            assert_eq!(drives[0].image, Some(PathBuf::from("game.dsk")));
            assert_eq!(drives[1].image, Some(PathBuf::from("data.po")));
        }
        other => panic!("slot 6: {other:?}"),
    }
}

/// GH#692: um `.hdv` dado na linha de comando (`-h1`) ocupa o slot 7, `-s7 empty` o esvazia e
/// `-s7-empty-on-exit` o remove na saída (CmdLine.cpp:160-164, :344-347).
#[test]
fn gh692_hard_disk_from_command_line_can_be_dismounted() {
    let s = applied(&["-h1", "a.hdv"]);
    match s.slot(7) {
        SlotCardSpec::GenericHdd { images, .. } => {
            assert_eq!(images[0], Some(PathBuf::from("a.hdv")))
        }
        other => panic!("slot 7: {other:?}"),
    }
    // O HDV lembrado da sessão anterior não prende o slot: `-s7 empty` o esvazia.
    let mut s = Settings::default();
    *s.slot_mut(7) = SlotCardSpec::new(apple2_config::card::CardType::GenericHdd);
    CmdLine::parse(&["-s7", "empty"]).unwrap().apply(&mut s);
    assert!(matches!(s.slot(7), SlotCardSpec::Empty), "{:?}", s.slot(7));
    let s = applied(&["-s7-empty-on-exit"]);
    assert!(s.storage.slot7_empty_on_exit);
}

/// GH#1277: `-hdc-firmware-v1`/`-v2` escolhem o firmware antigo do HDC (CmdLine.cpp:769-776).
#[test]
fn gh1277_hdc_firmware_v1_and_v2_switches() {
    let s = applied(&["-hdc-firmware-v1"]);
    assert!(s.misc.hdc_firmware_v1 && !s.misc.hdc_firmware_v2);
    let s = applied(&["-hdc-firmware-v2"]);
    assert!(s.misc.hdc_firmware_v2 && !s.misc.hdc_firmware_v1);
}
