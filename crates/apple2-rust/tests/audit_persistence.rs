//! Auditoria wave0-core, areas 5 e 6 (HU-0301/0302/0402): o que acontece com a escrita pendente
//! do disco e com o arquivo de save-state nos caminhos do frontend.
//!
//! C++: Disk.cpp EjectDisk/FlushCurrentTrack (chamados por Destroy, InsertDisk, LoadSnapshot,
//! ControlStepperDeferred, DriveSwap) e Snapshot_SaveState (SaveState.cpp).

use apple2_config::card::DriveSpec;
use apple2_config::{Settings, SlotCardSpec};
use apple2_cpu::Bus;
use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::{Command, Emulator};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use apple2_storage::nibble::NIBBLES_PER_TRACK;
use std::path::{Path, PathBuf};

const MOTOR_ON: u16 = 0xC0E9;
const Q6H: u16 = 0xC0ED;
const Q6L: u16 = 0xC0EC;
const Q7H: u16 = 0xC0EF;
const Q7L: u16 = 0xC0EE;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("apple2-rust-audit-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn dos33_bytes() -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("assets/disks/DOS 3.3 System Master - 680-0210-A.dsk");
    std::fs::read(p).unwrap()
}

/// Emulador com um disco gravavel (copia) no drive 1 e a CPU parada num laco.
fn emulator_with_writable_disk(img: &Path) -> (Emulator, Settings) {
    let mut s = Settings::default();
    s.slots.0[6] = SlotCardSpec::Disk2 {
        firmware_13_sector: false,
        drives: [
            DriveSpec {
                image: Some(img.to_path_buf()),
                write_protect: false,
                connected: true,
            },
            Default::default(),
        ],
    };
    let mut emu = Emulator::new(s.clone(), Box::new(DefaultBackend), 48_000).unwrap();
    park_and_dirty_track(&mut emu, 0);
    (emu, s)
}

/// Deixa a trilha 0 suja no buffer da carta (um nibble de dados alterado, no `nth`-esimo campo), sem descarregar.
fn park_and_dirty_track(emu: &mut Emulator, nth: usize) {
    let m = &mut emu.machine;
    for (i, b) in [0x4C, 0x00, 0x03].iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    m.card_mut::<Disk2Card>(6).unwrap().set_enhance_disk(true);
    let mut reference = DiskImage::from_bytes("r.dsk", dos33_bytes()).unwrap();
    let mut track = vec![0u8; NIBBLES_PER_TRACK];
    let (n, _) = reference.read_track(0.0, &mut track, true);
    let at = track[..n]
        .windows(3)
        .enumerate()
        .filter(|(_, w)| *w == [0xD5, 0xAA, 0xAD])
        .nth(nth)
        .unwrap()
        .0
        + 3;
    track[at] = if track[at] == 0x96 { 0x97 } else { 0x96 };
    m.board_mut().read(MOTOR_ON);
    m.run_cycles(2000);
    m.board_mut().read(Q6L);
    let pos = m.card::<Disk2Card>(6).unwrap().snapshot().drives[0].byte as usize;
    m.board_mut().read(Q7H);
    for j in 0..n {
        m.board_mut().write(Q6H, track[(pos + j) % n]);
        m.board_mut().read(Q6L);
        m.run_cycles(32);
    }
    m.board_mut().read(Q7L);
    assert!(m.card::<Disk2Card>(6).unwrap().snapshot().drives[0].track_dirty);
}

#[test]
fn quitting_flushes_the_pending_track() {
    let dir = temp_dir("quit");
    let img = dir.join("work.dsk");
    std::fs::write(&img, dos33_bytes()).unwrap();
    let (mut emu, _) = emulator_with_writable_disk(&img);
    assert!(std::fs::read(&img).unwrap() == dos33_bytes());
    emu.apply(Command::Shutdown);
    assert!(
        std::fs::read(&img).unwrap() != dos33_bytes(),
        "perdeu a escrita"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn applying_settings_flushes_the_pending_track_before_dropping_the_machine() {
    // emu.rs:396-405 (`rebuild`): `self.machine = m` descarta a maquina antiga e a Disk2Card com a
    // trilha suja; nao ha `disks.flush` antes (nem `impl Drop` que descarregue).
    let dir = temp_dir("apply");
    let img = dir.join("work.dsk");
    std::fs::write(&img, dos33_bytes()).unwrap();
    let (mut emu, settings) = emulator_with_writable_disk(&img);
    emu.apply(Command::ApplySettings(Box::new(settings)));
    assert!(
        std::fs::read(&img).unwrap() != dos33_bytes(),
        "ApplySettings perdeu a trilha pendente"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn loading_a_state_flushes_the_pending_track_first() {
    // emu.rs:~486 (`try_load_state`): `self.machine = machine` sem descarregar a antiga; o AppleWin
    // chama EjectDisk -> FlushCurrentTrack em LoadSnapshot. A maquina nova ja reabriu o arquivo.
    let dir = temp_dir("load");
    let img = dir.join("work.dsk");
    std::fs::write(&img, dos33_bytes()).unwrap();
    let state = dir.join("s.aws.yaml");
    let (mut emu, _) = emulator_with_writable_disk(&img);
    emu.machine.card_mut::<Disk2Card>(6).unwrap().flush();
    emu.apply(Command::SaveState(state.clone()));
    // suja de novo e carrega o estado salvo
    park_and_dirty_track(&mut emu, 1);
    let before = std::fs::read(&img).unwrap();
    emu.apply(Command::LoadState(state));
    assert!(
        std::fs::read(&img).unwrap() != before,
        "LoadState perdeu a trilha pendente"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_state_replaces_the_file_atomically() {
    // emu.rs:427 usa `std::fs::write(path, ..)`: trunca o arquivo no lugar. Um hard link para o
    // save anterior (mesmo inode) mostra a diferenca: com "temporario + rename" o link continua
    // com o conteudo antigo valido; com truncar-e-escrever ele muda junto (e uma queda no meio
    // destroi o ultimo save valido).
    let dir = temp_dir("atomic");
    let target = dir.join("SaveState.aws.yaml");
    let twin = dir.join("previous-valid.aws.yaml");
    std::fs::write(&target, "OLD VALID CONTENT\n").unwrap();
    std::fs::hard_link(&target, &twin).unwrap();
    let mut emu = Emulator::new(Settings::default(), Box::new(DefaultBackend), 48_000).unwrap();
    emu.apply(Command::SaveState(target.clone()));
    assert!(
        std::fs::read_to_string(&target)
            .unwrap()
            .contains("AppleWin Save State"),
        "{}",
        emu.status().message
    );
    assert_eq!(
        std::fs::read_to_string(&twin).unwrap(),
        "OLD VALID CONTENT\n",
        "o save anterior foi sobrescrito no lugar (nao atomico)"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn settings_file_is_replaced_atomically() {
    // store.rs:82-84: grava `settings.toml.tmp` e renomeia. O hard link preserva o antigo.
    let dir = temp_dir("settings");
    let target = dir.join("settings.toml");
    let twin = dir.join("old.toml");
    std::fs::write(&target, "# old\n").unwrap();
    std::fs::hard_link(&target, &twin).unwrap();
    Settings::default().save(&target).unwrap();
    assert_eq!(std::fs::read_to_string(&twin).unwrap(), "# old\n");
    assert_ne!(std::fs::read_to_string(&target).unwrap(), "# old\n");
    std::fs::remove_dir_all(dir).unwrap();
}
