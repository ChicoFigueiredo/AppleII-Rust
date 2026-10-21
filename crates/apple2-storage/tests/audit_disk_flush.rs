//! Auditoria wave0-core, area 5 (HU-0301): escrita pendente e descarga da trilha suja.
//!
//! C++: Disk.cpp:1996-2001 (GH#1444: "motor off and not spinning: write back any dirty track")
//! e FlushCurrentTrack em Eject/Destroy/ControlStepperDeferred.
//!

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_storage::disk2::{Disk2Card, SPINNING_CYCLES};
use apple2_storage::image::DiskImage;
use apple2_storage::store::MemStore;
use common::*;

const MOTOR_OFF: u16 = 0xC0E8;
const MOTOR_ON: u16 = 0xC0E9;
const Q6H: u16 = 0xC0ED;
const Q6L: u16 = 0xC0EC;
const Q7H: u16 = 0xC0EF;
const Q7L: u16 = 0xC0EE;

fn setup(wp: bool) -> (Machine, MemStore, Vec<u8>) {
    let original = disk(DOS33);
    let store = MemStore::new(original.clone());
    let mut img = DiskImage::from_bytes("w.dsk", original.clone())
        .unwrap()
        .with_store(Box::new(store.clone()));
    img.set_write_protected(wp);
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut card = Disk2Card::new();
    card.insert_disk(0, img);
    assert!(m.insert_card(6, Box::new(card)));
    for (i, b) in [0x4C, 0x00, 0x03].iter().enumerate() {
        m.poke(0x300 + i as u16, *b); // JMP $0300: nao deixa o autostart bootar
    }
    m.cpu_mut().pc = 0x300;
    (m, store, original)
}

/// Regrava a trilha 0 inteira pelos soft switches (Q7H + Q6H data load + Q6L shift), igual a
/// original exceto por um nibble do primeiro campo de dados, de modo que o arquivo muda se (e
/// so se) a trilha for descarregada. Usa o modo "enhanced" da carta (sem salto por tempo).
fn write_some_nibbles(m: &mut Machine) {
    use apple2_storage::nibble::NIBBLES_PER_TRACK;
    m.card_mut::<Disk2Card>(6).unwrap().set_enhance_disk(true);
    let mut reference = DiskImage::from_bytes("r.dsk", disk(DOS33)).unwrap();
    let mut track = vec![0u8; NIBBLES_PER_TRACK];
    let (n, _) = reference.read_track(0.0, &mut track, true);
    let at = track[..n]
        .windows(3)
        .position(|w| w == [0xD5, 0xAA, 0xAD])
        .unwrap()
        + 3;
    track[at] = if track[at] == 0x96 { 0x97 } else { 0x96 };

    let rd = |m: &mut Machine, a| m.board_mut().read(a);
    rd(m, MOTOR_ON);
    m.run_cycles(2000);
    rd(m, Q6L); // le: carrega a trilha no buffer
    let pos = m.card::<Disk2Card>(6).unwrap().snapshot().drives[0].byte as usize;
    assert!(pos < n);
    rd(m, Q7H);
    for j in 0..n {
        m.board_mut().write(Q6H, track[(pos + j) % n]);
        rd(m, Q6L);
        m.run_cycles(32);
    }
    rd(m, Q7L);
}

fn dirty(m: &Machine) -> bool {
    m.card::<Disk2Card>(6).unwrap().snapshot().drives[0].track_dirty
}

#[test]
fn dirty_track_is_flushed_one_second_after_the_motor_stops_gh1444() {
    let (mut m, store, original) = setup(false);
    write_some_nibbles(&mut m);
    assert!(dirty(&m), "a gravacao deveria sujar a trilha");
    assert!(store.bytes() == original, "ainda so na memoria");
    m.board_mut().read(MOTOR_OFF);
    m.run_cycles(SPINNING_CYCLES as u64 / 2);
    assert!(dirty(&m), "ainda girando: nao descarrega");
    m.run_cycles(SPINNING_CYCLES as u64 / 2 + 5000);
    assert!(
        !dirty(&m),
        "motor parado e sem girar: a trilha suja deve ser descarregada (GH#1444)"
    );
    assert!(store.bytes() != original, "o arquivo deve ter mudado");
}

#[test]
fn write_protected_drive_never_reaches_the_store() {
    let (mut m, store, original) = setup(true);
    write_some_nibbles(&mut m);
    m.board_mut().read(MOTOR_OFF);
    m.run_cycles(SPINNING_CYCLES as u64 + 5000);
    apple2_storage::disk2::manager::flush_all(&mut m);
    assert!(store.bytes() == original, "disco protegido foi alterado");
}

#[test]
fn flush_on_quit_writes_the_pending_track() {
    let (mut m, _store, _original) = setup(false);
    write_some_nibbles(&mut m);
    assert!(dirty(&m));
    apple2_storage::disk2::manager::flush_all(&mut m); // o que o frontend chama ao fechar
    assert!(!dirty(&m));
}

#[test]
fn eject_flushes_the_pending_track() {
    let (mut m, _store, _original) = setup(false);
    write_some_nibbles(&mut m);
    let img = m.card_mut::<Disk2Card>(6).unwrap().eject_disk(0);
    assert!(img.is_some());
    assert!(!dirty(&m));
}
