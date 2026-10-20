//! AppleWin-terms snapshots of the Mockingboard family.

use apple2_audio::{AudioBus, MockingboardCard, Variant};
use apple2_core::VideoStandard;

#[test]
fn a_snapshot_applied_to_a_fresh_card_reads_back_identically() {
    let bus = AudioBus::new(44_100, VideoStandard::Ntsc);
    let mut a = MockingboardCard::new(Variant::Phasor, &bus);
    let mut s = a.snapshot();
    s.phasor_mode = 5;
    s.units[1].via.t1_counter = 0x1234;
    s.units[1].via.t1_active = true;
    s.units[1].via.ier = 0x40;
    s.units[1].via.ifr = 0xC0; // bit 7 follows IFR & IER
    s.units[0].ay[1].tone_tick = [5, 6, 7];
    s.units[0].ay[1].rng = 0xABCDE;
    s.units[0].ay[1].env_counter = 9;
    s.units[0].ay[1].regs[13] = 0x0A;
    s.units[0].state = [6, 7];
    s.units[1].ssi.ctrl_art_amp |= 0x80; // power-down: nothing to restart
    s.units[1].ssi.inflection = 0x33;
    a.apply_snapshot(&s);
    // `tick_incr` follows the clocks (Phasor mode doubles the AY clock) and is not applied.
    let tick_incr = a.snapshot().units[0].ay[0].tick_incr;
    for ay in s.units.iter_mut().flat_map(|u| u.ay.iter_mut()) {
        ay.tick_incr = tick_incr;
    }
    assert_eq!(a.snapshot(), s);

    let mut b = MockingboardCard::new(Variant::Phasor, &bus);
    b.apply_snapshot(&s);
    assert_eq!(b.snapshot(), s);
}
