//! HU-0501, AUD-P-006 / AUD-P-007: one AY type per socket and the "bad 6522" switch.
//! Oracle: AppleWin `MockingboardCard::SetSocketAY891x` (Mockingboard.cpp:136-151) and
//! `SY6522::Read` (6522.cpp:355).

use apple2_audio::ay8910::AyType;
use apple2_audio::{AudioBus, MockingboardCard, Sockets, Variant};
use apple2_core::{Machine, MachineConfig, Model, VideoStandard};

fn machine() -> Machine {
    let mut rom = vec![0xEAu8; 0x3000];
    for (vec, target) in [(0xFFFAu16, 0x0500u16), (0xFFFC, 0x0300), (0xFFFE, 0x0400)] {
        let o = vec as usize - 0xD000;
        rom[o] = target as u8;
        rom[o + 1] = (target >> 8) as u8;
    }
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.video_standard = VideoStandard::Ntsc;
    cfg.custom_rom = Some(rom);
    Machine::new(cfg)
}

#[test]
fn each_ay_socket_keeps_its_own_type() {
    // Socket s -> 6522 unit (s & 1), AY (s >> 1): the Phasor's sockets 2 and 3 are the second
    // AY of each unit.
    let bus = AudioBus::new(44_100, VideoStandard::Ntsc);
    let sockets = Sockets {
        ay: [
            AyType::Ay3_8910,
            AyType::Ay3_8912,
            AyType::Ym2149f,
            AyType::Empty,
        ],
        ..Sockets::default()
    };
    let card = MockingboardCard::with_sockets(Variant::Phasor, &bus, sockets);
    for (s, t) in sockets.ay.iter().enumerate() {
        assert_eq!(card.ay_type(s), *t, "socket {s}");
    }
}

#[test]
fn default_sockets_are_all_ay_3_8913_like_applewin() {
    // AY8913::AY8913() sets m_type = AY_3_8913 (AY8910.cpp:127).
    assert_eq!(Sockets::default().ay, [AyType::Ay3_8913; 4]);
    assert_eq!(Sockets::default().bad_6522, [false; 2]);
}

/// `LDA #$FF : STA ddrb : LDA ddrb : STA $0200 : JMP *`; returns what the `LDA` read back.
fn ddrb_readback(bad: [bool; 2], addr: u16) -> u8 {
    let bus = AudioBus::new(44_100, VideoStandard::Ntsc);
    let mut m = machine();
    let sockets = Sockets {
        bad_6522: bad,
        ..Sockets::default()
    };
    assert!(m.insert_card(
        4,
        Box::new(MockingboardCard::with_sockets(
            Variant::MockingboardC,
            &bus,
            sockets
        ))
    ));
    let (lo, hi) = (addr as u8, (addr >> 8) as u8);
    let code = [
        0xA9, 0xFF, 0x8D, lo, hi, 0xAD, lo, hi, 0x8D, 0x00, 0x02, 0x4C, 0x0B, 0x03,
    ];
    for (i, b) in code.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.run_cycles(200);
    m.peek(0x0200)
}

#[test]
fn bad_6522_a_loses_ddrb_bit_0_and_only_unit_a() {
    assert_eq!(ddrb_readback([false, false], 0xC402), 0xFF);
    assert_eq!(ddrb_readback([true, false], 0xC402), 0xFE);
    assert_eq!(
        ddrb_readback([false, true], 0xC402),
        0xFF,
        "6522b is a different chip"
    );
}

#[test]
fn bad_6522_b_loses_ddrb_bit_0_and_only_unit_b() {
    assert_eq!(ddrb_readback([false, true], 0xC482), 0xFE);
    assert_eq!(ddrb_readback([true, false], 0xC482), 0xFF);
}

#[test]
fn bad_6522_is_a_property_of_the_chip() {
    let bus = AudioBus::new(44_100, VideoStandard::Ntsc);
    let sockets = Sockets {
        bad_6522: [true, false],
        ..Sockets::default()
    };
    let card = MockingboardCard::with_sockets(Variant::MockingboardC, &bus, sockets);
    assert!(card.via(0).bad && !card.via(1).bad);
}
