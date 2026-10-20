//! Disk II controller behaviour driven through its soft switches (UTAIIe chapter 9).

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_storage::disk2::{Disk2Card, DiskStatus, SPINNING_CYCLES};
use apple2_storage::image::DiskImage;
use common::*;

const MOTOR_OFF: u16 = 0xC0E8;
const MOTOR_ON: u16 = 0xC0E9;
const DRIVE1: u16 = 0xC0EA;
const DRIVE2: u16 = 0xC0EB;
const Q6L: u16 = 0xC0EC;
const Q6H: u16 = 0xC0ED;
const Q7L: u16 = 0xC0EE;
const Q7H: u16 = 0xC0EF;

fn bare() -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(6, Box::new(Disk2Card::new())));
    // Park the CPU in a loop so the autostart ROM does not boot the drive.
    for (i, b) in [0x4C, 0x00, 0x03].iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    m
}

fn card(m: &mut Machine) -> &mut Disk2Card {
    m.card_mut::<Disk2Card>(6).unwrap()
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

/// One stepper switch: `LDA $C0E0+phase*2+on`.
fn phase(m: &mut Machine, p: u16, on: bool) {
    rd(m, 0xC0E0 + p * 2 + on as u16);
}

#[test]
fn stepper_moves_the_head_a_half_track_per_phase_in_dsk_images() {
    let mut m = bare();
    card(&mut m).insert_disk(0, DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap());
    rd(&mut m, MOTOR_ON);
    // Phase 1 on, then 0 off (the DOS 3.3 SEEK order): 0 -> half track 1.
    phase(&mut m, 1, true);
    m.run_cycles(20); // the magnet takes effect 10 cycles later
    phase(&mut m, 0, false);
    m.run_cycles(20);
    assert_eq!(card(&mut m).phase(0), 1.0);
    for (on, off) in [(2, 1), (3, 2), (0, 3)] {
        phase(&mut m, on, true);
        m.run_cycles(20);
        phase(&mut m, off, false);
        m.run_cycles(20);
    }
    assert_eq!(card(&mut m).phase(0), 4.0);
    assert_eq!(card(&mut m).track(0), 2);
    // Moving back below 0 is clamped.
    for _ in 0..3 {
        for (on, off) in [(3, 0), (2, 3), (1, 2), (0, 1)] {
            phase(&mut m, on, true);
            m.run_cycles(20);
            phase(&mut m, off, false);
            m.run_cycles(20);
        }
    }
    assert_eq!(card(&mut m).phase(0), 0.0);
}

#[test]
fn woz_images_get_quarter_tracks_when_two_adjacent_magnets_are_on() {
    let mut m = bare();
    card(&mut m).insert_disk(0, DiskImage::from_bytes("a.woz", woz2_from_dsk(disk(DOS33), 0)).unwrap());
    rd(&mut m, MOTOR_ON);
    phase(&mut m, 1, true);
    m.run_cycles(20);
    phase(&mut m, 0, true); // phases 0 and 1 on: half way
    m.run_cycles(20);
    assert_eq!(card(&mut m).phase(0), 0.5);
    phase(&mut m, 0, false);
    m.run_cycles(20);
    assert_eq!(card(&mut m).phase(0), 1.0);
}

#[test]
fn a_pending_magnet_change_completes_before_the_next_one_is_queued() {
    let mut m = bare();
    card(&mut m).insert_disk(0, DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap());
    rd(&mut m, MOTOR_ON);
    phase(&mut m, 1, true);
    m.run_cycles(20);
    phase(&mut m, 0, false);
    m.run_cycles(20);
    assert_eq!(card(&mut m).phase(0), 1.0);
    // Phase 2 on, then phase 1 off within 10 cycles: the first change completes, then the second is queued.
    phase(&mut m, 2, true);
    phase(&mut m, 1, false);
    m.run_cycles(20);
    assert_eq!(card(&mut m).phase(0), 1.0 + 1.0);
}

#[test]
fn motor_off_keeps_spinning_for_a_second_and_the_led_follows() {
    let mut m = bare();
    card(&mut m).insert_disk(0, DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap());
    assert_eq!(card(&mut m).light_status(0), DiskStatus::Off);
    rd(&mut m, MOTOR_ON);
    assert_eq!(card(&mut m).light_status(0), DiskStatus::Read);
    assert!(card(&mut m).is_condition_for_full_speed());
    rd(&mut m, MOTOR_OFF);
    assert!(!card(&mut m).is_condition_for_full_speed());
    assert_eq!(card(&mut m).current_state(), DiskStatus::Spin);
    m.run_cycles(SPINNING_CYCLES as u64 / 2);
    assert!(card(&mut m).is_spinning(0));
    m.run_cycles(SPINNING_CYCLES as u64 / 2 + 100);
    assert!(!card(&mut m).is_spinning(0));
    assert_eq!(card(&mut m).current_state(), DiskStatus::Off);
}

#[test]
fn write_protect_is_sensed_through_q6h_q7l() {
    let mut m = bare();
    card(&mut m).insert_disk(0, DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap());
    rd(&mut m, MOTOR_ON);
    rd(&mut m, Q6H);
    assert_eq!(rd(&mut m, Q7L) & 0x80, 0, "writable disk");
    card(&mut m).set_write_protect(0, true);
    rd(&mut m, Q6H);
    assert_eq!(rd(&mut m, Q7L) & 0x80, 0x80, "write-protected");
    assert_eq!(card(&mut m).light_status(0), DiskStatus::Prot);
    // Write mode with a protected disk never changes the image.
    rd(&mut m, Q7H);
    rd(&mut m, Q6L);
    assert!(card(&mut m).image(0).unwrap().data() == &disk(DOS33)[..]);
}

#[test]
fn empty_and_unplugged_drives_read_noise_or_0x80() {
    let mut m = bare();
    rd(&mut m, MOTOR_ON);
    assert_eq!(rd(&mut m, Q6L), 0x80, "right after the motor starts the LSS has not settled");
    m.run_cycles(2000);
    let seen: std::collections::HashSet<u8> = (0..50).map(|_| rd(&mut m, Q6L)).collect();
    assert!(seen.len() > 5, "an empty drive returns random nibbles (GH#748)");
    card(&mut m).unplug_drive(0);
    m.run_cycles(2000);
    assert_eq!(rd(&mut m, Q6L), 0x80);
    assert!(!card(&mut m).is_drive_connected(0));
}

#[test]
fn media_management() {
    let mut m = bare();
    let a = DiskImage::from_bytes("/disks/Alpha.dsk", disk(DOS33)).unwrap();
    let b = DiskImage::from_bytes("/disks/Beta.po", disk("ProDOS_2_4_3.po")).unwrap();
    let c = card(&mut m);
    assert!(c.is_drive_empty(0) && c.light_status(0) == DiskStatus::Off);
    c.insert_disk(0, a);
    c.insert_disk(1, b);
    assert_eq!((c.base_name(0), c.full_name(0), c.base_name(1)), ("Alpha", "Alpha.dsk", "Beta"));
    c.swap_disks();
    assert_eq!((c.base_name(0), c.base_name(1)), ("Beta", "Alpha"));
    // Inserting a disk already in the other drive moves it.
    let again = DiskImage::from_bytes("/disks/Alpha.dsk", disk(DOS33)).unwrap();
    c.insert_disk(0, again);
    assert!(c.is_drive_empty(1) && c.base_name(0) == "Alpha");
    let ejected = c.eject_disk(0).unwrap();
    assert_eq!(ejected.pathname(), "/disks/Alpha.dsk");
    assert!(c.is_drive_empty(0));
    assert!(c.eject_disk(0).is_none());
    let events = c.drain_events();
    assert!(events.len() >= 4 && c.drain_events().is_empty());
    // Drive 2 can be selected and is independent.
    rd(&mut m, DRIVE2);
    assert_eq!(card(&mut m).current_drive(), 1);
    rd(&mut m, DRIVE1);
    assert_eq!(card(&mut m).current_drive(), 0);
}

#[test]
fn state_round_trips_through_the_slots_save() {
    let mut m = machine_with_disk(DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap());
    for _ in 0..60 {
        m.run_frame();
    }
    let saved = m.card::<Disk2Card>(6).unwrap();
    let (phase, motor, track) = (saved.phase(0), saved.is_motor_on(), saved.track(0));
    let state = m.board().slots.save();
    let mut m2 = machine_with_disk(DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap());
    assert!(m2.board_mut().slots.load(&state));
    let c = m2.card::<Disk2Card>(6).unwrap();
    assert_eq!((c.phase(0), c.is_motor_on(), c.track(0)), (phase, motor, track));
}

#[test]
fn authentic_mode_skews_the_sectors_and_enhanced_does_not() {
    let mut a = DiskImage::from_bytes("a.dsk", disk(DOS33)).unwrap();
    let (mut e, mut s) = (vec![0u8; 0x1A18], vec![0u8; 0x1A18]);
    let (n1, _) = a.read_track(6.0, &mut e, true);
    let (n2, _) = a.read_track(6.0, &mut s, false);
    assert_eq!(n1, n2);
    assert_ne!(e[..n1], s[..n1]);
    let skew = (3 * 768) % n1;
    assert_eq!(s[..n1 - skew], e[skew..n1]);
}
