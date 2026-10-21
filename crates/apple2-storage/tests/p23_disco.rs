//! P23 (gate G0): issues upstream de disco e hdd. Oráculo: AppleWin a8f0d1d (C++ transcrito, com
//! arquivo:linha em cada teste). Resultados em `docs/conformance/results/p23-disco.csv`.

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_storage::disk2::Disk2Card;
use apple2_storage::hdd::{HardDiskCard, HdcMode};
use apple2_storage::image::{Class, DiskImage, ImageError, ImageKind};
use apple2_storage::new_disk::{new_prodos_disk, ProDosFiles};
use common::*;
use std::io::Write;

const DOS33: &str = "DOS 3.3 System Master - 680-0210-A.dsk";

fn two_img_v(version: u16, format: u32, body: &[u8]) -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[0..4].copy_from_slice(b"2IMG");
    h[4..8].copy_from_slice(b"TEST");
    h[8..10].copy_from_slice(&64u16.to_le_bytes());
    h[10..12].copy_from_slice(&version.to_le_bytes());
    h[12..16].copy_from_slice(&format.to_le_bytes());
    h[20..24].copy_from_slice(&((body.len() / 512) as u32).to_le_bytes());
    h[24..28].copy_from_slice(&64u32.to_le_bytes());
    h[28..32].copy_from_slice(&(body.len() as u32).to_le_bytes());
    [h, body.to_vec()].concat()
}

fn zip_of(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, d) in entries {
        w.start_file(*n, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(d).unwrap();
    }
    w.finish().unwrap().into_inner()
}

// ---- Imagens de disquete ----

/// GH#87, #387, #568: imagens de 36 a 40 trilhas. DiskImageHelper.cpp:590 (`> TRACKS_MAX*4096`
/// recusa), :597-600 (140K + n*4K vale), :793 (NIB de 35 a 40 trilhas), DiskImage.h:42-44.
#[test]
fn gh87_gh387_gh568_images_of_up_to_forty_tracks_are_accepted() {
    for tracks in 36..=40usize {
        let d = DiskImage::from_bytes("x.dsk", vec![0; tracks * 4096]).unwrap();
        assert_eq!(d.num_tracks() as usize, tracks, "dsk {tracks}");
        let n = DiskImage::from_bytes("x.nib", vec![0x96; tracks * 0x1A00]).unwrap();
        assert_eq!(
            (n.kind(), n.num_tracks() as usize),
            (ImageKind::Nib1, tracks)
        );
    }
    // A 40a trilha (fase 78) é lida sem pânico.
    let mut d = DiskImage::from_bytes("x.dsk", vec![0; 40 * 4096]).unwrap();
    let mut buf = vec![0u8; 0x1A18];
    assert!(d.read_track(78.0, &mut buf, true).0 > 0);
    // Tamanhos que não são 140K + n*4K ou passam de 160K: recusados (`IsValidImageSize`).
    assert!(DiskImage::from_bytes("x.dsk", vec![0; 35 * 4096 + 100]).is_err());
    assert!(DiskImage::from_bytes("x.dsk", vec![0; 41 * 4096]).is_err());
    assert!(DiskImage::from_bytes("x.nib", vec![0; 41 * 0x1A00]).is_err());
    assert!(DiskImage::from_bytes("x.nib", vec![0; 35 * 0x1A00 + 7]).is_err());
}

/// GH#1370: o AppleWin a8f0d1d também para em 40 trilhas (DiskImage.h:44), então 41 é recusado
/// nos dois (mesmo teste acima); aqui só o limite de WOZ (flux) fica maior, como em
/// DiskImageHelper.cpp:2186-2188.
#[test]
fn gh1370_floppy_limit_is_forty_tracks_like_applewin() {
    assert!(matches!(
        DiskImage::from_bytes("x.dsk", vec![0; 45 * 4096]),
        Err(ImageError::Unsupported | ImageError::BadSize)
    ));
}

/// GH#317: `.2mg` com versão 0 monta (DiskImageHelper.cpp:1449 só recusa `Version > 1`), em
/// disquete e em disco rígido.
#[test]
fn gh317_two_img_version_zero_mounts() {
    let body = disk(DOS33);
    let d = DiskImage::from_bytes("v0.2mg", two_img_v(0, 0, &body)).unwrap();
    assert_eq!((d.kind(), d.image_size()), (ImageKind::Do, 143360));
    let d = DiskImage::from_bytes("v1.2mg", two_img_v(1, 0, &body)).unwrap();
    assert_eq!(d.kind(), ImageKind::Do);
    assert!(DiskImage::from_bytes("v2.2mg", two_img_v(2, 0, &body)).is_err());
    let hd = DiskImage::hard_disk_from_bytes("v0.2mg", two_img_v(0, 1, &vec![0; 800 * 1024]));
    assert_eq!(hd.unwrap().image_size(), 800 * 1024);
}

/// GH#628: o sufixo `.gz`/`.zip` não diferencia maiúsculas (DiskImageHelper.cpp:1990 e 1994,
/// `_stricmp`).
#[test]
fn gh628_gz_and_zip_suffix_is_case_insensitive() {
    let z = zip_of(&[("a.dsk", disk(DOS33))]);
    for name in ["Bad.Apple.Zip", "X.ZIP", "x.zip"] {
        let d = DiskImage::from_bytes(name, z.clone()).unwrap();
        assert_eq!(d.kind(), ImageKind::Do, "{name}");
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&disk(DOS33)).unwrap();
    let gz = gz.finish().unwrap();
    for name in ["a.dsk.GZ", "A.Dsk.Gz"] {
        assert_eq!(
            DiskImage::from_bytes(name, gz.clone()).unwrap().kind(),
            ImageKind::Do,
            "{name}"
        );
    }
}

/// GH#824: o `__MACOSX/._x.dsk` (metadados do Finder, tamanho inválido) não é imagem válida:
/// o primeiro item válido do zip é escolhido (DiskImageHelper.cpp:1703-1779).
#[test]
fn gh824_zip_skips_macosx_metadata_entries() {
    let z = zip_of(&[
        (
            "__MACOSX/._mtrkdemo.dsk",
            vec![0x00, 0x05, 0x16, 0x07, 0, 2, 0, 0],
        ),
        ("mtrkdemo.dsk", disk(DOS33)),
    ]);
    let d = DiskImage::from_bytes("mtrkdemo.zip", z).unwrap();
    assert_eq!(d.kind(), ImageKind::Do);
    assert_eq!(d.name_in_zip(), Some("mtrkdemo.dsk"));
    assert_eq!(d.num_valid_images_in_zip(), 1);
    // Mais de uma entrada no zip: somente leitura (DiskImageHelper.cpp:1771, `uNumEntriesInZip`).
    assert!(d.is_write_protected());
}

/// GH#1240: um WOZ de ~905 KB (a2woz) continua sendo um disquete de 5,25" (o tamanho do arquivo
/// não o torna um disco de 800K). DiskImageHelper.cpp:2186-2188.
#[test]
fn gh1240_big_woz_file_is_still_a_525_floppy() {
    let mut woz = woz2_from_dsk(disk(DOS33), 1);
    // Chunk extra "PAD " até ~905 KB, como o a2woz faz com o espaço não usado.
    let pad = 905 * 1024 - woz.len();
    woz.extend(b"PAD ");
    woz.extend((pad as u32 - 8).to_le_bytes());
    woz.extend(vec![0u8; pad - 8]);
    apple2_storage::woz::update_crc(&mut woz);
    let d = DiskImage::from_bytes("a2woz.woz", woz).unwrap();
    assert!(d.is_woz());
    assert_eq!(d.num_tracks(), 40); // CWOZHelper::MAX_TRACKS_5_25, DiskImageHelper.cpp:1176
}

/// GH#1482: um arquivo binário/Applesoft cru montado como disco roda no boot (imagens `.apl`/`.prg`,
/// DiskImageHelper.cpp CAplImage/CPrgImage); o teste existente `simsystem_iie_images_apl_and_prg_programs`
/// já cobre o carregamento; aqui o binário `.apl` gera um programa de boot.
#[test]
fn gh1482_raw_apl_binary_gives_a_boot_program() {
    let mut apl = vec![0x00, 0x60, 0x03, 0x00]; // endereço $6000, comprimento 3
    apl.extend([0xA9, 0x01, 0x60]);
    let mut d = DiskImage::from_bytes("prog.apl", apl).unwrap();
    assert_eq!(d.kind(), ImageKind::Apl);
    let (addr, code) = d.boot_program().expect("programa de boot");
    assert_eq!(addr, 0x6000);
    assert_eq!(code, vec![0xA9, 0x01, 0x60]);
}

/// Quantos descritores abertos do processo apontam para `path` (Linux: `/proc/self/fd`).
#[cfg(target_os = "linux")]
fn open_handles_to(path: &std::path::Path) -> usize {
    let target = std::fs::canonicalize(path).unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|e| std::fs::read_link(e.ok()?.path()).ok())
        .filter(|l| *l == target)
        .count()
}

/// GH#312, #641, #1300: o port nunca mantém o arquivo aberto/travado. O AppleWin abre com
/// `FILE_SHARE_READ` (DiskImageHelper.cpp:1848-1849) e não deixa outro programa gravar na imagem
/// montada (:1862-1863); no Windows isso é uma trava de compartilhamento, que só existe enquanto
/// há um handle aberto. O teste prova a causa no Linux: com imagens montadas (duas somente
/// leitura e uma de leitura e escrita) não resta nenhum descritor aberto para o arquivo, e outro
/// programa pode regravá-lo. Que não haja trava no Windows decorre de não haver handle, mas
/// esta suíte só roda no Linux.
#[test]
fn gh312_gh641_gh1300_mounted_files_are_not_locked() {
    let dir = std::env::temp_dir().join(format!("p23-lock-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("foo.dsk");
    std::fs::write(&p, disk(DOS33)).unwrap();
    let a = DiskImage::open_path(&p, Class::Floppy, true).unwrap();
    let b = DiskImage::open_path(&p, Class::Floppy, true).unwrap();
    assert!(a.is_write_protected() && b.is_write_protected());
    let rw = DiskImage::open_path(&p, Class::Floppy, false).unwrap();
    assert!(!rw.is_write_protected());
    #[cfg(target_os = "linux")]
    assert_eq!(
        open_handles_to(&p),
        0,
        "imagem montada não mantém o arquivo aberto"
    );
    // Outro programa (CiderPress, Cadius) grava no arquivo enquanto a imagem está montada.
    let mut changed = disk(DOS33);
    changed[0x1234] ^= 0xFF;
    std::fs::write(&p, &changed).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), changed);
    drop((a, b, rw));
    std::fs::remove_dir_all(&dir).ok();
}

/// GH#1433: ejetar o disco limpa a proteção contra escrita do drive (Disk.cpp:426).
#[test]
fn gh1433_eject_clears_write_protect() {
    let mut c = Disk2Card::new();
    let mut img = DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap();
    img.set_write_protected(true);
    c.insert_disk(0, img);
    assert!(c.is_write_protected(0));
    c.eject_disk(0);
    assert!(!c.is_write_protected(0));
}

// ---- Disk II ----

fn bare() -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(6, Box::new(Disk2Card::new())));
    for (i, b) in [0x4C, 0x00, 0x03].iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    m
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn card(m: &mut Machine) -> &mut Disk2Card {
    m.card_mut::<Disk2Card>(6).unwrap()
}

fn step_to(m: &mut Machine, on: u16, off: u16) {
    rd(m, 0xC0E0 + on * 2 + 1);
    m.run_cycles(20);
    rd(m, 0xC0E0 + off * 2);
    m.run_cycles(20);
}

/// GH#525: com o motor desligado e o drive parado, o braço não anda (Disk.cpp:538-545); dentro do
/// 1 s de giro residual, anda.
#[test]
fn gh525_head_does_not_move_while_the_drive_is_off() {
    let mut m = bare();
    card(&mut m).insert_disk(0, DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap());
    // Motor nunca ligado: ignorado.
    rd(&mut m, 0xC0E3);
    m.run_cycles(20);
    rd(&mut m, 0xC0E2);
    m.run_cycles(20);
    assert_eq!(card(&mut m).phase(0), 0.0);
    // Ligado: anda.
    rd(&mut m, 0xC0E9);
    step_to(&mut m, 1, 0);
    assert_eq!(card(&mut m).phase(0), 1.0);
    // Desligado, ainda girando: anda (o motor demora 1 s para parar).
    rd(&mut m, 0xC0E8);
    step_to(&mut m, 2, 1);
    assert_eq!(card(&mut m).phase(0), 2.0);
    // Depois do atraso de 1 s: ignorado.
    m.run_cycles(apple2_storage::disk2::SPINNING_CYCLES as u64 + 1000);
    assert!(!card(&mut m).is_spinning(0));
    step_to(&mut m, 3, 2);
    assert_eq!(card(&mut m).phase(0), 2.0);
}

/// GH#543: trocar os discos não troca o estado de giro: ele é do drive (Disk.cpp:2047-2051 troca
/// só `m_disk`).
#[test]
fn gh543_swapping_disks_keeps_the_motor_state_with_the_drive() {
    let mut m = bare();
    let c = card(&mut m);
    c.insert_disk(
        0,
        DiskImage::from_bytes("/d/Alpha.dsk", disk(DOS33)).unwrap(),
    );
    c.insert_disk(
        1,
        DiskImage::from_bytes("/d/Beta.po", disk("ProDOS_2_4_3.po")).unwrap(),
    );
    rd(&mut m, 0xC0E9);
    assert!(card(&mut m).is_spinning(0) && !card(&mut m).is_spinning(1));
    card(&mut m).swap_disks();
    let c = card(&mut m);
    assert_eq!((c.base_name(0), c.base_name(1)), ("Beta", "Alpha"));
    assert!(c.is_spinning(0), "o drive 1 continua girando");
    assert!(!c.is_spinning(1));
    assert_eq!(c.current_drive(), 0);
    assert!(c.is_motor_on());
}

/// GH#734, #767, #1133: o firmware de 13 setores é escolhido pelo WOZ do drive 1 (INFO
/// `bootSectorFormat`) ou forçado, e só a cada reset/`init_firmware` (Disk.cpp:2084-2099).
#[test]
fn gh734_gh767_gh1133_thirteen_sector_firmware_follows_the_woz_or_is_forced() {
    let mut m = bare();
    let rom16: Vec<u8> = (0..256u16)
        .map(|i| m.board_mut().read(0xC600 + i))
        .collect();
    assert_eq!(card(&mut m).current_firmware(), 16);
    // WOZ de 13 setores (bootSectorFormat = 2) no drive 1.
    let w13 = DiskImage::from_bytes("a.woz", woz2_from_dsk(disk(DOS33), 2)).unwrap();
    card(&mut m).insert_disk(0, w13);
    card(&mut m).init_firmware();
    assert_eq!(card(&mut m).current_firmware(), 13);
    // Um WOZ de 13 setores no drive 2 não conta: só o drive 1 decide.
    let mut m2 = bare();
    let w13b = DiskImage::from_bytes("b.woz", woz2_from_dsk(disk(DOS33), 2)).unwrap();
    card(&mut m2).insert_disk(1, w13b);
    card(&mut m2).init_firmware();
    assert_eq!(card(&mut m2).current_firmware(), 16);
    // Forçado (`-sN diskii13`), e a ROM do slot muda.
    let mut m3 = bare();
    card(&mut m3).set_force_13_sector_firmware(true);
    card(&mut m3).init_firmware();
    m3.power_cycle();
    assert_eq!(card(&mut m3).current_firmware(), 13);
    let rom13: Vec<u8> = (0..256u16)
        .map(|i| m3.board_mut().read(0xC600 + i))
        .collect();
    assert_ne!(rom13, rom16);
    // Um WOZ de 16 setores (bootSectorFormat = 1) mantém o de 16.
    let mut m4 = bare();
    let w16 = DiskImage::from_bytes("c.woz", woz2_from_dsk(disk(DOS33), 1)).unwrap();
    card(&mut m4).insert_disk(0, w16);
    card(&mut m4).init_firmware();
    assert_eq!(card(&mut m4).current_firmware(), 16);
}

/// GH#141, #726: mais de dois drives = um segundo Disk II no slot 5 (o AppleWin aceita Disk II
/// em qualquer slot, CardManager.cpp), cada um com seus dois drives e imagens.
#[test]
fn gh141_gh726_a_second_disk_ii_gives_four_drives() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(6, Box::new(Disk2Card::new())));
    assert!(m.insert_card(5, Box::new(Disk2Card::new())));
    m.card_mut::<Disk2Card>(6)
        .unwrap()
        .insert_disk(0, DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap());
    let c5 = m.card_mut::<Disk2Card>(5).unwrap();
    c5.insert_disk(
        0,
        DiskImage::from_bytes("b.po", disk("ProDOS_2_4_3.po")).unwrap(),
    );
    c5.insert_disk(1, DiskImage::from_bytes("c.dsk", disk(DOS33)).unwrap());
    assert_eq!(m.card::<Disk2Card>(6).unwrap().base_name(0), "a");
    assert!(m.card::<Disk2Card>(6).unwrap().is_drive_empty(1));
    assert_eq!(m.card::<Disk2Card>(5).unwrap().base_name(0), "b");
    assert_eq!(m.card::<Disk2Card>(5).unwrap().base_name(1), "c");
    // Os soft switches do slot 5 ($C0D0) ligam o motor só do cartão do slot 5.
    rd(&mut m, 0xC0D9);
    assert!(m.card::<Disk2Card>(5).unwrap().is_motor_on());
    assert!(!m.card::<Disk2Card>(6).unwrap().is_motor_on());
}

// ---- Disco rígido ----

fn hdd_card_machine(slot: u8, mode: HdcMode, image: DiskImage) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut c = HardDiskCard::new(slot, Model::Apple2eEnhanced, mode);
    c.insert(0, image);
    assert!(m.insert_card(slot, Box::new(c)));
    m.power_cycle();
    m
}

fn prodos_hdv() -> DiskImage {
    DiskImage::hard_disk_from_bytes("ProDOS.hdv", disk("ProDOS_2_4_3.po")).unwrap()
}

/// GH#324, #996, #1033: o disco rígido em outro slot (aqui o 5) funciona; o firmware é
/// independente de slot (Harddisk.cpp:1271, "fix GH#996") e o ProDOS inicia de S5,D1.
#[test]
fn gh324_gh996_gh1033_hard_disk_boots_from_slot_5() {
    let mut m = hdd_card_machine(5, HdcMode::Default, prodos_hdv());
    run_until(&mut m, 900, "ProDOS em S5,D1", |m| {
        screen_has(m, "S5,D1:/PRODOS.2.4.3")
    });
}

fn blk_at(m: &mut Machine, base: u16, cmd: u8, unit: u8, mem: u16, block: u16) -> u8 {
    let b = m.board_mut();
    b.write(base + 2, cmd);
    b.write(base + 3, unit);
    b.write(base + 4, mem as u8);
    b.write(base + 5, (mem >> 8) as u8);
    b.write(base + 6, block as u8);
    b.write(base + 7, (block >> 8) as u8);
    b.read(base)
}

/// GH#1186: mais de um controlador de disco rígido (um por slot, `CT_GenericHDD` em qualquer
/// slot: CardManager.cpp:79-80), cada um com as suas imagens.
#[test]
fn gh1186_two_hard_disk_controllers_hold_independent_images() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut a = HardDiskCard::new(7, Model::Apple2eEnhanced, HdcMode::SmartPort);
    a.insert(0, prodos_hdv());
    let mut b = HardDiskCard::new(5, Model::Apple2eEnhanced, HdcMode::SmartPort);
    let mut raw = vec![0u8; 800 * 1024];
    raw[0] = 0x5A;
    b.insert(0, DiskImage::hard_disk_from_bytes("b.hdv", raw).unwrap());
    assert!(m.insert_card(7, Box::new(a)));
    assert!(m.insert_card(5, Box::new(b)));
    for (i, b) in [0x4C, 0x00, 0x03].iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    m.run_cycles(10);
    // Slot 5: base $C0D0, unidade $50.
    blk_at(&mut m, 0xC0D0, 1, 0x50, 0x4000, 0);
    m.run_cycles(600);
    assert_eq!(m.peek(0x4000), 0x5A);
    // Slot 7: base $C0F0, unidade $70; bloco 0 do ProDOS não é 0x5A.
    blk_at(&mut m, 0xC0F0, 1, 0x70, 0x5000, 0);
    m.run_cycles(600);
    assert_ne!(m.peek(0x5000), 0x5A);
}

/// GH#1207: SmartPort (`$Cn07 = 0`) ou modo bloco (`$Cn07 = $3C`) com 2 ou 4 dispositivos
/// (Harddisk.cpp, opções `-s7 hdc` / modo de firmware).
#[test]
fn gh1207_hdc_mode_selects_smartport_or_block_firmware() {
    let mut m = hdd_card_machine(7, HdcMode::SmartPort, prodos_hdv());
    assert_eq!(rd(&mut m, 0xC707), 0x00);
    let mut m = hdd_card_machine(7, HdcMode::BlockMode2Devices, prodos_hdv());
    assert_eq!(rd(&mut m, 0xC707), 0x3C);
    assert_eq!(rd(&mut m, 0xC7FE) & 0x30, 0x10, "2 dispositivos");
    let mut m = hdd_card_machine(7, HdcMode::BlockMode4Devices, prodos_hdv());
    assert_eq!(
        (rd(&mut m, 0xC707), rd(&mut m, 0xC7FE) & 0x30),
        (0x3C, 0x30)
    );
}

/// GH#12: dá para desplugar a imagem de disco rígido (volta ao estado "sem imagem").
#[test]
fn gh12_hard_disk_image_can_be_unplugged() {
    let mut c = HardDiskCard::new(7, Model::Apple2eEnhanced, HdcMode::SmartPort);
    c.insert(0, prodos_hdv());
    assert!(c.image(0).is_some());
    let img = c.unplug(0);
    assert!(img.is_some());
    assert!(c.image(0).is_none());
    assert_eq!(c.base_name(0), "");
}

/// GH#61, #88, #152, #563, #1227, #1331: regras de detecção do HDD (DiskImageHelper.cpp:895-906):
/// `.hdv` aceita qualquer tamanho; outras extensões precisam de pelo menos 800K; `.po` de 16 MB e
/// 32 MB abrem; `.2mg` de 800K abre; um `.po` menor que 1600 blocos NÃO abre no AppleWin também.
#[test]
fn gh61_gh88_gh152_gh563_gh1227_gh1331_hard_disk_detection_matches_applewin() {
    let hd = |n: &str, len: usize| DiskImage::hard_disk_from_bytes(n, vec![0; len]);
    assert!(hd("big16.po", 16 * 1024 * 1024).is_ok());
    assert!(hd("big32.po", 32 * 1024 * 1024).is_ok());
    assert!(hd("oz.po", 800 * 1024).is_ok(), "OzDOS / UniDOS de 800K");
    assert!(hd("x.hdv", 800 * 1024).is_ok());
    assert!(
        hd("small.hdv", 1599 * 512).is_ok(),
        "qualquer tamanho com .hdv"
    );
    assert!(
        hd("small.po", 1599 * 512).is_err(),
        "menor que 800K com .po é recusado"
    );
    assert!(hd("small.po", 1600 * 512).is_ok());
    let two = two_img_v(1, 1, &vec![0; 800 * 1024]);
    assert!(DiskImage::hard_disk_from_bytes("x.2mg", two).is_ok());
    // Acima de 32 MB: recusado.
    assert!(hd("huge.hdv", 33 * 1024 * 1024).is_err());
}

/// GH#1469: o volume criado de 32 MB grava o total de blocos limitado a 65535 (e não 0);
/// ProDOS_FileSystem.h:303-307.
#[test]
fn gh1469_new_32mb_prodos_volume_has_a_valid_block_count() {
    let d = new_prodos_disk(32 * 1024 * 1024, ProDosFiles::default());
    // Cabeçalho do diretório raiz no bloco 2: total de blocos em +0x25 depois do ponteiro duplo.
    let total = u16::from_le_bytes([d[2 * 512 + 4 + 37], d[2 * 512 + 4 + 38]]);
    assert_eq!(total, 0xFFFF);
    let small = new_prodos_disk(800 * 1024, ProDosFiles::default());
    let total = u16::from_le_bytes([small[2 * 512 + 4 + 37], small[2 * 512 + 4 + 38]]);
    assert_eq!(total, 1600);
}

/// GH#452: um HD sem imagem devolve "dispositivo não conectado" ($28) e não o erro de I/O
/// ($27 genérico), Harddisk.cpp:604 e o teste `errors_unknown_unit...` de `hdd.rs`.
#[test]
fn gh452_unmounted_hard_disk_reports_no_device_connected() {
    let mut m = hdd_card_machine(7, HdcMode::SmartPort, prodos_hdv());
    for (i, b) in [0x4C, 0x00, 0x03].iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    m.run_cycles(10);
    let st = blk_at(&mut m, 0xC0F0, 1, 0xF0, 0x4000, 0);
    assert_eq!(st >> 1 & 0x3F, 0x28);
}

/// GH#1264: a chamada de status do driver ProDOS devolve o número de blocos em X (baixo) e Y
/// (alto). O firmware e os registradores são os do AppleWin (Harddisk.cpp registradores $C0n9/A).
#[test]
fn gh1264_prodos_status_returns_block_count_in_x_and_y() {
    let mut m = hdd_card_machine(7, HdcMode::BlockMode2Devices, prodos_hdv());
    // Entrada do driver de bloco: $C700 + byte em $C7FF.
    let entry = 0xC700 + rd(&mut m, 0xC7FF) as u16;
    let prog: [u8; 22] = [
        0xA9,
        0x00,
        0x85,
        0x42, // comando 0 = status
        0xA9,
        0x70,
        0x85,
        0x43, // unidade $70
        0xA9,
        0x00,
        0x85,
        0x44,
        0xA9,
        0x20,
        0x85,
        0x45, // buffer $2000
        0x20,
        entry as u8,
        (entry >> 8) as u8, // JSR driver
        0x4C,
        0x13,
        0x03, // fim: laço
    ];
    for (i, b) in prog.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    m.run_cycles(20_000);
    let cpu = m.cpu_mut();
    assert_eq!((cpu.x, cpu.y), (0x18, 0x01), "280 blocos");
}

/// GH#515: com o HDC no slot 7 vazio e um disquete em uso, plugar a imagem e reiniciar a frio
/// inicia o ProDOS do HD em um só boot (o autostart varre do slot 7 para baixo). C++ do a8f0d1d
/// reproduzido: `HarddiskInterfaceCard::Insert` (Harddisk.cpp:439-475) pluga com a máquina
/// rodando e nada mais é preciso; `Reset(powerCycle)` (:198-204) só zera erros e a fila, sem
/// apagar a imagem; o firmware inicia pelo HD quando o status do volume responde sem erro
/// (`firmware/HDD-SmartPort`, `Cn_Bootstrap`; `hddrvr.a65:215-222` `hdboot2`). Oráculo: C++
/// transcrito; o oráculo executável não pluga imagem com a máquina rodando, mas o estado depois
/// do plug (HDC com imagem, disquete em S6) é o do `--hdd --disk` dele.
#[test]
fn gh515_plugging_a_hard_disk_then_cold_boot_boots_from_it() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut f = Disk2Card::new();
    f.insert_disk(0, DiskImage::from_bytes("m.dsk", disk(DOS33)).unwrap());
    assert!(m.insert_card(6, Box::new(f)));
    let h = HardDiskCard::new(7, Model::Apple2eEnhanced, HdcMode::Default);
    assert!(m.insert_card(7, Box::new(h)));
    m.power_cycle();
    run_until(&mut m, 700, "DOS 3.3 do disquete", |m| {
        screen_has(m, "DOS VERSION 3.3")
    });
    m.card_mut::<HardDiskCard>(7)
        .unwrap()
        .insert(0, prodos_hdv());
    m.power_cycle();
    run_until(&mut m, 900, "ProDOS em S7,D1 depois de plugar", |m| {
        screen_has(m, "S7,D1:/PRODOS.2.4.3")
    });
    assert!(
        !screen_has(&m, "DOS VERSION 3.3"),
        "um só reboot basta: o disquete não inicia de novo"
    );
}

fn at_prompt(m: &Machine) -> bool {
    screen(m).lines().any(|l| l.starts_with(']'))
}

/// Digita `cmd`, espera o motor parar e o prompt voltar.
fn dos_command(m: &mut Machine, cmd: &str) {
    type_text(m, cmd);
    type_text(m, "\n");
    for _ in 0..30 {
        m.run_frame();
    }
    run_until(m, 900, cmd, |m| {
        !m.card::<Disk2Card>(6).unwrap().is_motor_on()
    });
    for _ in 0..90 {
        m.run_frame();
    }
}

/// GH#821: o `_ASSERT(0)` de `Disk2InterfaceCard::ReadTrack()` (Disk.cpp:312-326) dispara quando
/// a cabeça está além do número de trilhas de uma imagem que não aceita "zero trilhas". O roteiro
/// da própria issue (comentário em Disk.cpp:318-323): DOS 3.3 Master no D1, disco em branco no
/// D2, `INIT HELLO,D2`, `RUN HELLO`, reiniciar com F2 e `RUN HELLO,D2`. No AppleWin de release o
/// ramo marca a trilha como sem dados e volta (:324-325); o port tem de formatar o disco, e o
/// HELLO gravado tem de rodar depois do reinício. O INIT roda no DOS 3.3 real, em cópias em
/// memória do BLANK.DSK (140K sem formatação) e de um disco de zero bytes (imagem vazia, o caso
/// que `ImageIsZeroTracksValidForThisType` protege).
#[test]
fn gh821_init_on_a_blank_disk_then_reboot_and_run_hello_from_d2() {
    for (nome, blank) in [("BLANK.DSK", disk("BLANK.DSK")), ("vazio", Vec::new())] {
        let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
        let mut c = Disk2Card::new();
        c.insert_disk(0, DiskImage::from_bytes("m.dsk", disk(DOS33)).unwrap());
        c.insert_disk(1, DiskImage::from_bytes("b.dsk", blank).unwrap());
        assert!(m.insert_card(6, Box::new(c)));
        m.power_cycle();
        run_until(&mut m, 1500, "DOS 3.3 prompt", |m| {
            screen_has(m, "DOS VERSION 3.3") && at_prompt(m)
        });
        dos_command(&mut m, "NEW");
        dos_command(&mut m, "10 PRINT \"OLA DO D2\"");
        type_text(&mut m, "INIT HELLO,D2\n");
        let mut started = false;
        run_until(&mut m, 6000, "fim do INIT", |m| {
            let c = m.card::<Disk2Card>(6).unwrap();
            started |= c.is_motor_on() && c.current_drive() == 1;
            started
                && !c.is_motor_on()
                && screen(m).lines().filter(|l| l.starts_with(']')).count() >= 4
        });
        for _ in 0..120 {
            m.run_frame();
        }
        // F2: reinicia a máquina com as mesmas imagens, e o HELLO gravado roda do D2
        m.power_cycle();
        run_until(&mut m, 1500, "DOS 3.3 de novo", |m| {
            screen_has(m, "DOS VERSION 3.3") && at_prompt(m)
        });
        dos_command(&mut m, "RUN HELLO,D2");
        assert!(screen_has(&m, "OLA DO D2"), "{nome}:\n{}", screen(&m));
        dos_command(&mut m, "CATALOG,D2");
        assert!(screen_has(&m, "DISK VOLUME 254"), "{nome}:\n{}", screen(&m));
    }
}

/// GH#323 (PR #370 do AppleWin): com o botão 0 (Open Apple) apertado no boot, o firmware do HDC
/// pula o disco rígido. Oráculo: firmware transcrito do a8f0d1d, `firmware/HDD/hddrvr.a65:218-219`
/// (`bit BUTTON0 / bmi BootSlot6`: vai ao boot do slot 6) e `firmware/HDD-SmartPort`
/// `Cn_Bootstrap` (`bit IO_IIE_OPENAPPLE / bmi Cn_Sloop`: segue a varredura de slots). Os bytes
/// dos três firmwares do port são idênticos aos do a8f0d1d (SHA-256 em rom-provenance.md). O
/// oráculo executável (`--hdd`) não tem como apertar o botão, então só confirma o controle.
/// Montagem: HDC no slot 7 com ProDOS, Disk II no slot 6 com DOS 3.3.
#[test]
fn gh323_open_apple_at_boot_skips_the_hard_disk() {
    for (mode, nome) in [
        (HdcMode::Default, "SmartPort"),
        (HdcMode::BlockMode2Devices, "bloco"),
    ] {
        for pressed in [false, true] {
            let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
            let mut f = Disk2Card::new();
            f.insert_disk(0, DiskImage::from_bytes("m.dsk", disk(DOS33)).unwrap());
            assert!(m.insert_card(6, Box::new(f)));
            let mut h = HardDiskCard::new(7, Model::Apple2eEnhanced, mode);
            h.insert(0, prodos_hdv());
            assert!(m.insert_card(7, Box::new(h)));
            m.power_cycle();
            m.set_open_apple(pressed); // segurado desde antes do boot
            if pressed {
                run_until(&mut m, 700, &format!("{nome}: DOS 3.3 do slot 6"), |m| {
                    screen_has(m, "DOS VERSION 3.3")
                });
                assert!(
                    !screen_has(&m, "PRODOS"),
                    "{nome}: o HD foi pulado com o Open Apple"
                );
            } else {
                run_until(&mut m, 900, &format!("{nome}: ProDOS do HD"), |m| {
                    screen_has(m, "S7,D1:/PRODOS.2.4.3")
                });
                assert!(!screen_has(&m, "DOS VERSION 3.3"), "{nome}: controle");
            }
        }
    }
}

/// GH#460: o reset (Ctrl+Reset) desliga o motor e as chaves mas o drive ainda gira até 1 s; a
/// partida a frio (F2, `ResetMachineState`) zera o giro e o LED também (Disk.cpp:1797-1818).
#[test]
fn gh460_cold_start_stops_a_spinning_drive_and_warm_reset_keeps_it_spinning() {
    let mut m = bare();
    card(&mut m).insert_disk(0, DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap());
    rd(&mut m, 0xC0E9);
    assert!(card(&mut m).is_motor_on() && card(&mut m).is_spinning(0));
    m.reset();
    assert!(!card(&mut m).is_motor_on(), "RESET' desliga as chaves");
    assert!(card(&mut m).is_spinning(0), "o giro residual continua");
    rd(&mut m, 0xC0E9);
    m.power_cycle();
    assert!(!card(&mut m).is_motor_on());
    assert!(
        !card(&mut m).is_spinning(0),
        "partida a frio zera o giro (GH#460)"
    );
}

/// GH#971, #991: um WOZ com a flag "protegido" no INFO abre protegido (DiskImageHelper.cpp:2044),
/// mas o AppleWin permite desproteger o drive (Disk.cpp:1000 `SetProtect`) e então grava; o WOZ
/// aceita escrita (caso `disk-micro-write-sector-woz`).
#[test]
fn gh971_gh991_woz_write_protect_flag_can_be_cleared_by_the_user() {
    let mut woz = woz2_from_dsk(disk(DOS33), 1);
    assert_eq!(&woz[12..16], b"INFO");
    woz[20 + 2] = 1; // INFO.writeProtected
    apple2_storage::woz::update_crc(&mut woz);
    let img = DiskImage::from_bytes("prot.woz", woz).unwrap();
    assert!(img.is_write_protected());
    let mut c = Disk2Card::new();
    c.insert_disk(0, img);
    assert!(c.is_write_protected(0));
    c.set_write_protect(0, false);
    assert!(!c.is_write_protected(0));
}

/// GH#1277: o firmware v1/v2 antigo do HDC é selecionável e difere do SmartPort atual.
#[test]
fn gh1277_old_hdc_firmware_differs_from_current() {
    use apple2_storage::hdd::HdcFirmware;
    let rom = |fw| {
        let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
        let c = HardDiskCard::with_firmware(7, Model::Apple2eEnhanced, HdcMode::Default, fw);
        assert!(m.insert_card(7, Box::new(c)));
        m.power_cycle();
        (0..256u16)
            .map(|i| rd(&mut m, 0xC700 + i))
            .collect::<Vec<u8>>()
    };
    let (cur, v1, v2) = (
        rom(HdcFirmware::SmartPort),
        rom(HdcFirmware::HddrvrV1),
        rom(HdcFirmware::HddrvrV2),
    );
    assert!(cur != v1 && cur != v2 && v1 != v2);
}
