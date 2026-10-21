//! GH#1340: "Enhanced disk speed" must not speed up while the speaker or a Mockingboard is
//! active. AppleWin.cpp:183: `IsConditionForFullSpeed() && !Spkr_IsActive() &&
//! !MockingboardCardManager::IsActiveToPreventFullSpeed()`. Speaker window 0.2 s
//! (Speaker.cpp:451), Mockingboard window 0.1 s (Mockingboard.cpp:521).

use apple2_config::{CardType, Settings, SlotCardSpec};
use apple2_cpu::Bus;
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::Emulator;

const MOTOR_ON: u16 = 0xC0E9;
const SPKR: u16 = 0xC030;

fn emu(mockingboard: bool) -> Emulator {
    let mut s = Settings::default();
    assert!(s.storage.enhance_disk_speed);
    if mockingboard {
        s.slots.0[4] = SlotCardSpec::new(CardType::MockingboardC);
    }
    let mut e = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    // Park the CPU so the autostart ROM does not interfere.
    for (i, b) in [0x4C, 0x00, 0x03].iter().enumerate() {
        e.machine.poke(0x300 + i as u16, *b);
    }
    e.machine.cpu_mut().pc = 0x300;
    e
}

fn spin_drive(e: &mut Emulator) {
    e.machine.board_mut().read(MOTOR_ON);
}

/// AY register write through the 6522: latch `reg`, then write `val` (ORA/ORB/DDRA/DDRB at
/// $C400+0/1/2/3 of slot 4).
fn ay_write(e: &mut Emulator, reg: u8, val: u8) {
    let b = e.machine.board_mut();
    b.write(0xC403, 0xFF); // DDRA
    b.write(0xC402, 0x07); // DDRB
    b.write(0xC401, reg);
    b.write(0xC400, 0x07); // latch
    b.write(0xC400, 0x04); // inactive
    b.write(0xC401, val);
    b.write(0xC400, 0x06); // write
    b.write(0xC400, 0x04);
}

fn cycles_per(e: &Emulator, seconds: f64) -> u64 {
    (e.clock_hz() * seconds) as u64
}

#[test]
fn drive_spinning_with_speaker_playing_does_not_accelerate() {
    let mut e = emu(false);
    spin_drive(&mut e);
    e.machine.board_mut().read(SPKR);
    e.machine.run_cycles(1000);
    assert!(e.machine.speaker_is_active());
    assert!(!e.effective_full_speed());
}

#[test]
fn drive_spinning_without_sound_accelerates() {
    let mut e = emu(false);
    spin_drive(&mut e);
    e.machine.run_cycles(1000);
    assert!(!e.machine.speaker_is_active());
    assert!(e.effective_full_speed());
}

#[test]
fn speaker_idle_beyond_200ms_accelerates_again() {
    let mut e = emu(false);
    spin_drive(&mut e);
    e.machine.board_mut().read(SPKR);
    e.machine.run_cycles(cycles_per(&e, 0.15));
    assert!(!e.effective_full_speed(), "inside the 0.2 s window");
    e.machine.run_cycles(cycles_per(&e, 0.1));
    assert!(e.effective_full_speed(), "0.25 s after the last toggle");
}

#[test]
fn speaker_activity_is_not_part_of_the_saved_state() {
    let mut a = emu(false);
    let b = emu(false);
    a.machine.board_mut().read(SPKR);
    let _ = a.machine.drain_speaker_toggles().count();
    assert!(a.machine.speaker_is_active(), "survives draining the log");
    assert_eq!(
        serde_json::to_string(&a.machine.board().speaker).unwrap(),
        serde_json::to_string(&b.machine.board().speaker).unwrap()
    );
}

#[test]
fn drive_spinning_with_mockingboard_active_does_not_accelerate() {
    let mut e = emu(true);
    spin_drive(&mut e);
    ay_write(&mut e, 0, 0x55);
    e.machine.run_cycles(1000);
    assert!(!e.machine.speaker_is_active());
    assert!(!e.effective_full_speed());
}

#[test]
fn drive_spinning_with_mockingboard_idle_beyond_window_accelerates() {
    let mut e = emu(true);
    spin_drive(&mut e);
    ay_write(&mut e, 0, 0x55);
    e.machine.run_cycles(cycles_per(&e, 0.05));
    assert!(!e.effective_full_speed(), "inside the 0.1 s window");
    e.machine.run_cycles(cycles_per(&e, 0.1));
    assert!(e.effective_full_speed(), "0.15 s after the last AY write");
}

#[test]
fn mockingboard_present_but_never_written_accelerates() {
    let mut e = emu(true);
    spin_drive(&mut e);
    e.machine.run_cycles(1000);
    assert!(e.effective_full_speed());
}

#[test]
fn scroll_lock_full_speed_ignores_sound() {
    let mut e = emu(false);
    e.machine.board_mut().read(SPKR);
    e.full_speed = true;
    assert!(e.effective_full_speed());
}
