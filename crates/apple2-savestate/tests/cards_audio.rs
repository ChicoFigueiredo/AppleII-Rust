//! Mockingboard family (Mockingboard.cpp:1174-1520, 6522.cpp:634, AY8910.cpp:1098,
//! SSI263.cpp:1005): golden text from the C++ writers, our writer, and full-machine round trips.

mod common;
use apple2_audio::ay8910::AyType;
use apple2_audio::ssi263::SsiType;
use apple2_audio::{
    AudioBus, AySnapshot, MockingboardCard, MockingboardSnapshot, SsiSnapshot, UnitSnapshot,
    Variant, ViaSnapshot,
};
use apple2_core::{Card, Machine, MachineConfig, Model};
use apple2_savestate::{
    default_card, load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions,
};
use apple2_storage::image::DiskImage;

struct Host {
    bus: AudioBus,
}

impl Host {
    fn new() -> Self {
        Host {
            bus: AudioBus::new(44_100, apple2_core::VideoStandard::Ntsc),
        }
    }
}

impl LoadHost for Host {
    fn open_image(&mut self, _req: &ImageRequest) -> Result<DiskImage, String> {
        Err("no images".into())
    }
    fn make_card(&mut self, name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
        let v = match name {
            "Mockingboard C" => Variant::MockingboardC,
            "Phasor" => Variant::Phasor,
            "MEGA Audio" => Variant::MegaAudio,
            "SD Music" => Variant::SdMusic,
            _ => return default_card(name, slot, model),
        };
        Some(Box::new(MockingboardCard::new(v, &self.bus)))
    }
}

fn machine_with(variant: Variant, slot: u8, bus: &AudioBus) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(slot, Box::new(MockingboardCard::new(variant, bus))));
    m.power_cycle();
    m
}

/// A plain machine's file with `slots_body` (entries indented by 4) as its `Slots` unit.
fn file_with_slots(slots_body: &str) -> String {
    let m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let base = save_aws_yaml(
        &m,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .unwrap()
    .text;
    let marker = "\nUnit:\n  Type: Game I/O Connector";
    let at = base.find(marker).expect("game I/O unit");
    let mut body = String::new();
    for l in slots_body.lines() {
        body.push_str(&format!("    {l}\n"));
    }
    format!(
        "{}\nUnit:\n  Type: Slots\n  Version: 1\n  State:\n{}{}",
        &base[..at],
        body,
        &base[at..]
    )
}

/// The slots body of a saved file (the lines of `Unit: Slots` State, dedented by 4).
fn slots_body(text: &str) -> String {
    let start = text
        .find("  Type: Slots\n  Version: 1\n  State:\n")
        .expect("slots unit");
    let rest = &text[start + "  Type: Slots\n  Version: 1\n  State:\n".len()..];
    let end = rest.find("\nUnit:").unwrap_or(rest.len());
    rest[..end]
        .lines()
        .map(|l| format!("{}\n", l.strip_prefix("    ").unwrap_or(l)))
        .collect()
}

fn load_card(text: &str, host: &mut Host, slot: u8) -> (Machine, Vec<String>) {
    let loaded = load_aws_yaml(text, host).unwrap();
    let _ = slot;
    (loaded.machine, loaded.warnings)
}

// ---- golden: Mockingboard C, unit version 15 -------------------------------------------------

const MB_C_V15: &str = "\
4:
  Card: Mockingboard C
  Version: 15
  State:
    Unit0:
      SY6522:
        ORB: 0x12
        ORA: 0x34
        DDRB: 0xFF
        DDRA: 0xF0
        Timer1 Counter: 0x1234
        Timer1 Latch: 0xABCD
        Timer1 IRQ Delay: 0
        Timer1 Active: true
        Timer2 Counter: 0x0042
        Timer2 Latch: 0x0100
        Timer2 IRQ Delay: 1
        Timer2 Active: false
        Serial Shift: 0x00
        ACR: 0x40
        PCR: 0x0C
        IFR: 0xC0
        IER: 0xC0
      AY891x:
        Type: AY-3-8913
        Tone0 Tick: 11
        Tone1 Tick: 22
        Tone2 Tick: 33
        Tone0 High: 1
        Tone1 High: 0
        Tone2 High: 1
        Noise Tick: 5
        Tone Subcycles: 6
        Env Subcycles: 7
        Env Internal Tick: 8
        Env Tick: 9
        Tick Incr: 4321
        Tone0 Period: 284
        Tone1 Period: 213
        Tone2 Period: 767
        Noise Period: 10
        RNG: 123456
        Noise Toggle: 1
        Env First: 0
        Env Rev: 1
        Env Counter: 12
        Registers:
          Tone0 Period: 0x11C
          Tone1 Period: 0x0D5
          Tone2 Period: 0x2FF
          Noise Period: 0x0A
          Mixer: 0x38
          Vol0: 0x0F
          Vol1: 0x0B
          Vol2: 0x00
          Env Period: 0x1000
          Env Shape: 0xA
          PortA: 0x00
          PortB: 0x00
      SSI263:
        Type: Empty
      SC01:
        Type: SC01
        SC01 Phoneme: 0x15
        SC01 Active Phoneme: true
      Unit State: 0x2
      AY Current Register: 0x07
      Chip Select A: true
      Reg Address Latch Valid A: true
      Bus Driven by AY: false
    Unit1:
      SY6522:
        ORB: 0x00
        ORA: 0x01
        DDRB: 0x00
        DDRA: 0x00
        Timer1 Counter: 0x0000
        Timer1 Latch: 0xFFFF
        Timer1 IRQ Delay: 0
        Timer1 Active: false
        Timer2 Counter: 0x0000
        Timer2 Latch: 0x0000
        Timer2 IRQ Delay: 0
        Timer2 Active: false
        Serial Shift: 0x00
        ACR: 0x00
        PCR: 0x00
        IFR: 0x20
        IER: 0x40
      AY891x:
        Type: AY-3-8913
        Tone0 Tick: 0
        Tone1 Tick: 0
        Tone2 Tick: 0
        Tone0 High: 0
        Tone1 High: 0
        Tone2 High: 0
        Noise Tick: 0
        Tone Subcycles: 0
        Env Subcycles: 0
        Env Internal Tick: 0
        Env Tick: 0
        Tick Incr: 4321
        Tone0 Period: 1
        Tone1 Period: 1
        Tone2 Period: 1
        Noise Period: 0
        RNG: 1
        Noise Toggle: 0
        Env First: 1
        Env Rev: 0
        Env Counter: 15
        Registers:
          Tone0 Period: 0x000
          Tone1 Period: 0x000
          Tone2 Period: 0x000
          Noise Period: 0x00
          Mixer: 0x00
          Vol0: 0x00
          Vol1: 0x00
          Vol2: 0x00
          Env Period: 0x0000
          Env Shape: 0x0
          PortA: 0x00
          PortB: 0x00
      SSI263:
        Type: SSI263AP
        Duration / Phoneme: 0x4C
        Inflection: 0x11
        Rate / Inflection: 0x22
        Control / Articulation / Amplitude: 0x8A
        Filter Frequency: 0x33
        Current Mode: 0x61
      Unit State: 0x3
      AY Current Register: 0x0F
      Chip Select A: false
      Reg Address Latch Valid A: false
      Bus Driven by AY: true
";

fn via0() -> ViaSnapshot {
    ViaSnapshot {
        orb: 0x12,
        ora: 0x34,
        ddrb: 0xFF,
        ddra: 0xF0,
        t1_counter: 0x1234,
        t1_latch: 0xABCD,
        t1_irq_delay: false,
        t1_active: true,
        t2_counter: 0x42,
        t2_latch: 0x100,
        t2_irq_delay: true,
        t2_active: false,
        serial_shift: 0,
        acr: 0x40,
        pcr: 0x0C,
        ifr: 0xC0,
        ier: 0xC0,
    }
}

fn via1() -> ViaSnapshot {
    ViaSnapshot {
        orb: 0,
        ora: 1,
        ddrb: 0,
        ddra: 0,
        t1_counter: 0,
        t1_latch: 0xFFFF,
        t1_irq_delay: false,
        t1_active: false,
        t2_counter: 0,
        t2_latch: 0,
        t2_irq_delay: false,
        t2_active: false,
        serial_shift: 0,
        acr: 0,
        pcr: 0,
        ifr: 0x20,
        ier: 0x40,
    }
}

fn ay0() -> AySnapshot {
    AySnapshot {
        ty: AyType::Ay3_8913,
        tone_tick: [11, 22, 33],
        tone_high: [true, false, true],
        noise_tick: 5,
        tone_subcycles: 6,
        env_subcycles: 7,
        env_internal_tick: 8,
        env_tick: 9,
        tick_incr: 4321,
        tone_period: [284, 213, 767],
        noise_period: 10,
        rng: 123456,
        noise_toggle: true,
        env_first: false,
        env_rev: true,
        env_counter: 12,
        regs: [
            0x1C, 0x01, 0xD5, 0x00, 0xFF, 0x02, 0x0A, 0x38, 0x0F, 0x0B, 0x00, 0x00, 0x10, 0x0A, 0,
            0,
        ],
    }
}

fn ay_idle() -> AySnapshot {
    AySnapshot {
        ty: AyType::Ay3_8913,
        tone_tick: [0; 3],
        tone_high: [false; 3],
        noise_tick: 0,
        tone_subcycles: 0,
        env_subcycles: 0,
        env_internal_tick: 0,
        env_tick: 0,
        tick_incr: 4321,
        tone_period: [1; 3],
        noise_period: 0,
        rng: 1,
        noise_toggle: false,
        env_first: true,
        env_rev: false,
        env_counter: 15,
        regs: [0; 16],
    }
}

fn ssi_empty_sc01() -> SsiSnapshot {
    SsiSnapshot {
        // An empty socket keeps the chip's power-on registers (SSI263.cpp:1048).
        ty: SsiType::Empty,
        duration_phoneme: 0xC0,
        inflection: 0,
        rate_inflection: 0,
        ctrl_art_amp: 0x80,
        filter_freq: 0xFF,
        current_mode: 0,
        has_sc01: true,
        votrax_phoneme: 0x15,
        is_votrax_phoneme: true,
    }
}

fn ssi_ap() -> SsiSnapshot {
    SsiSnapshot {
        ty: SsiType::Ap,
        duration_phoneme: 0x4C,
        inflection: 0x11,
        rate_inflection: 0x22,
        ctrl_art_amp: 0x8A,
        filter_freq: 0x33,
        current_mode: 0x61,
        has_sc01: false,
        votrax_phoneme: 0,
        is_votrax_phoneme: false,
    }
}

fn mb_c_expected() -> MockingboardSnapshot {
    MockingboardSnapshot {
        phasor_mode: 0,
        units: [
            UnitSnapshot {
                via: via0(),
                ay: [ay0(), ay_idle()],
                ssi: ssi_empty_sc01(),
                state: [2, 2],
                ay_current_register: [7, 0],
                chip_selected: [true, false],
                latch_valid: [true, false],
                bus_driven: false,
            },
            UnitSnapshot {
                via: via1(),
                ay: [ay_idle(), ay_idle()],
                ssi: ssi_ap(),
                state: [3, 2],
                ay_current_register: [0x0F, 0],
                chip_selected: [false, false],
                latch_valid: [false, false],
                bus_driven: true,
            },
        ],
    }
}

fn snapshot_in(m: &Machine, slot: u8) -> MockingboardSnapshot {
    m.card::<MockingboardCard>(slot)
        .expect("mockingboard card")
        .snapshot()
}

#[test]
fn golden_mockingboard_c_v15_loads_into_the_card() {
    let mut host = Host::new();
    let (m, warnings) = load_card(&file_with_slots(MB_C_V15), &mut host, 4);
    assert!(warnings.is_empty(), "{warnings:?}");
    // The AY's internal phase (ticks, RNG, envelope) is not part of the card's own state bytes
    // (`Card::save` keeps type + registers), which is how the loader hands it to the machine.
    let (mut got, mut want) = (snapshot_in(&m, 4), mb_c_expected());
    regs_only(&mut got);
    regs_only(&mut want);
    assert_eq!(got, want);
}

fn regs_only(s: &mut MockingboardSnapshot) {
    for a in s.units.iter_mut().flat_map(|u| u.ay.iter_mut()) {
        *a = AySnapshot {
            ty: a.ty,
            regs: a.regs,
            ..ay_idle()
        };
    }
}

#[test]
fn mockingboard_c_writes_exactly_the_cpp_text() {
    let bus = AudioBus::new(44_100, apple2_core::VideoStandard::Ntsc);
    let mut m = machine_with(Variant::MockingboardC, 4, &bus);
    m.card_mut::<MockingboardCard>(4)
        .unwrap()
        .apply_snapshot(&mb_c_expected());
    let text = save_aws_yaml(
        &m,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .unwrap()
    .text;
    let tick_incr = m.card::<MockingboardCard>(4).unwrap().snapshot().units[0].ay[0].tick_incr;
    assert_eq!(
        slots_body(&text),
        MB_C_V15.replace("Tick Incr: 4321", &format!("Tick Incr: {tick_incr}"))
    );
}

// ---- golden: Phasor, unit version 15 ---------------------------------------------------------

/// Body of a power-on `AY891x` entry, at indent 0.
const AY_IDLE_BODY: &str = "\
Type: AY-3-8913
Tone0 Tick: 0
Tone1 Tick: 0
Tone2 Tick: 0
Tone0 High: 0
Tone1 High: 0
Tone2 High: 0
Noise Tick: 0
Tone Subcycles: 0
Env Subcycles: 0
Env Internal Tick: 0
Env Tick: 0
Tick Incr: 4321
Tone0 Period: 1
Tone1 Period: 1
Tone2 Period: 1
Noise Period: 0
RNG: 1
Noise Toggle: 0
Env First: 1
Env Rev: 0
Env Counter: 15
Registers:
  Tone0 Period: 0x000
  Tone1 Period: 0x000
  Tone2 Period: 0x000
  Noise Period: 0x00
  Mixer: 0x00
  Vol0: 0x00
  Vol1: 0x00
  Vol2: 0x00
  Env Period: 0x0000
  Env Shape: 0x0
  PortA: 0x00
  PortB: 0x00
";

/// Body of an `SY6522` entry with only the timer-2 latch and IER set.
const VIA_BODY: &str = "\
ORB: 0x00
ORA: 0x00
DDRB: 0x00
DDRA: 0x00
Timer1 Counter: 0x0000
Timer1 Latch: 0xFFFF
Timer1 IRQ Delay: 0
Timer1 Active: false
Timer2 Counter: 0x0000
Timer2 Latch: 0x0777
Timer2 IRQ Delay: 0
Timer2 Active: false
Serial Shift: 0x00
ACR: 0x00
PCR: 0x00
IFR: 0x00
IER: 0x40
";

fn indent(s: &str, n: usize) -> String {
    s.lines()
        .map(|l| format!("{}{l}\n", " ".repeat(n)))
        .collect()
}

fn phasor_v15() -> String {
    let unit = |ssi: &str, tail: &str| {
        format!(
            "SY6522:\n{}AY891x-A:\n{}AY891x-B:\n{}{ssi}{tail}",
            indent(VIA_BODY, 2),
            indent(AY_IDLE_BODY, 2),
            indent(AY_IDLE_BODY, 2)
        )
    };
    let u0 = unit(
        "SSI263:\n  Type: Empty\nSC01:\n  Type: SC01\n  SC01 Phoneme: 0x15\n  SC01 Active Phoneme: false\n",
        "Unit State: 0x6\nUnit State-B: 0x7\nAY Current Register: 0x07\nAY Current Register B: 0x09\n\
Chip Select A: true\nChip Select B: true\nReg Address Latch Valid A: true\nReg Address Latch Valid B: false\n\
Bus Driven by AY: true\n",
    );
    let u1 = unit(
        "SSI263:\n  Type: SSI263AP\n  Duration / Phoneme: 0x4C\n  Inflection: 0x11\n  Rate / Inflection: 0x22\n  \
Control / Articulation / Amplitude: 0x8A\n  Filter Frequency: 0x33\n  Current Mode: 0x61\n",
        "Unit State: 0x2\nUnit State-B: 0x3\nAY Current Register: 0x01\nAY Current Register B: 0x02\n\
Chip Select A: false\nChip Select B: true\nReg Address Latch Valid A: false\nReg Address Latch Valid B: true\n\
Bus Driven by AY: false\n",
    );
    format!(
        "4:\n  Card: Phasor\n  Version: 15\n  State:\n    Mode: 5\n    Unit0:\n{}    Unit1:\n{}",
        indent(&u0, 6),
        indent(&u1, 6)
    )
}

fn phasor_expected() -> MockingboardSnapshot {
    let via = ViaSnapshot {
        ora: 0,
        ifr: 0,
        t2_latch: 0x777,
        ier: 0x40,
        ..via1()
    };
    let u0 = UnitSnapshot {
        via: via.clone(),
        ay: [ay_idle(), ay_idle()],
        ssi: SsiSnapshot {
            is_votrax_phoneme: false,
            ..ssi_empty_sc01()
        },
        state: [6, 7],
        ay_current_register: [7, 9],
        chip_selected: [true, true],
        latch_valid: [true, false],
        bus_driven: true,
    };
    let u1 = UnitSnapshot {
        via,
        ay: [ay_idle(), ay_idle()],
        ssi: ssi_ap(),
        state: [2, 3],
        ay_current_register: [1, 2],
        chip_selected: [false, true],
        latch_valid: [false, true],
        bus_driven: false,
    };
    MockingboardSnapshot {
        phasor_mode: 5,
        units: [u0, u1],
    }
}

#[test]
fn golden_phasor_v15_loads_into_the_card() {
    let mut host = Host::new();
    let (m, warnings) = load_card(&file_with_slots(&phasor_v15()), &mut host, 4);
    assert!(warnings.is_empty(), "{warnings:?}");
    let (mut got, mut want) = (snapshot_in(&m, 4), phasor_expected());
    regs_only(&mut got);
    regs_only(&mut want);
    assert_eq!(got, want);
}

#[test]
fn phasor_writes_exactly_the_cpp_text() {
    let bus = AudioBus::new(44_100, apple2_core::VideoStandard::Ntsc);
    let mut m = machine_with(Variant::Phasor, 4, &bus);
    m.card_mut::<MockingboardCard>(4)
        .unwrap()
        .apply_snapshot(&phasor_expected());
    let text = save_aws_yaml(
        &m,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .unwrap()
    .text;
    let tick = m.card::<MockingboardCard>(4).unwrap().snapshot().units[0].ay[0].tick_incr;
    assert_eq!(
        slots_body(&text),
        phasor_v15().replace("Tick Incr: 4321", &format!("Tick Incr: {tick}"))
    );
}

// ---- older unit versions ---------------------------------------------------------------------

/// An `AY8910` (pre-v15) body: no `Type`, `Vol0` tells the chips apart.
fn ay_old(vol0: u8) -> String {
    let body: String = AY_IDLE_BODY
        .lines()
        .skip(1)
        .map(|l| format!("{l}\n"))
        .collect();
    indent(
        &body.replace("Vol0: 0x00", &format!("Vol0: 0x{vol0:02X}")),
        6,
    )
}

fn via_old(extra: &str, t1_latch: &str) -> String {
    let body = VIA_BODY
        .replace("Timer1 Latch: 0xFFFF", &format!("Timer1 Latch: {t1_latch}"))
        .lines()
        .filter(|l| extra.contains("delays") || !l.contains("IRQ Delay"))
        .filter(|l| extra.contains("active") || !l.contains("Active"))
        .map(|l| format!("{l}\n"))
        .collect::<String>();
    indent(&body, 6)
}

#[test]
fn golden_mockingboard_c_v3_reads_the_old_layout() {
    // v3: timers' active flags at unit level, redundant IRQ flags, no chip-select keys.
    let unit = |ssi_mode: &str, state: &str| {
        format!(
            "    SY6522:\n{}    AY8910:\n{}    SSI263:\n      Duration / Phoneme: 0x12\n      Inflection: 0x00\n      \
Rate / Inflection: 0x00\n      Control / Articulation / Amplitude: 0x00\n      Filter Frequency: 0x00\n      \
Current Mode: {ssi_mode}\n    AY Current Register: 0x05\n    Timer1 Active: true\n    Timer2 Active: false\n    \
Timer1 IRQ Pending: false\n    Timer2 IRQ Pending: false\n    Speech IRQ Pending: false\n    Unit State: {state}\n",
            via_old("", "0x0000"),
            ay_old(9),
        )
    };
    let text = format!(
        "4:\n  Card: Mockingboard C\n  Version: 3\n  State:\n    Unit0:\n{}    Unit1:\n{}",
        indent(&unit("0x00", "0x3"), 2),
        indent(&unit("0x40", "0x2"), 2)
    );
    let mut host = Host::new();
    let (m, warnings) = load_card(&file_with_slots(&text), &mut host, 4);
    assert!(warnings.is_empty(), "{warnings:?}");
    let s = snapshot_in(&m, 4);
    let (u0, u1) = (&s.units[0], &s.units[1]);
    assert_eq!(
        u0.via.t1_latch, 0xFFFF,
        "a zero latch is lifted so detection works (6522.cpp:690)"
    );
    assert_eq!(u0.via.t2_latch, 0x777);
    assert!(u0.via.t1_active && !u0.via.t2_active && !u0.via.t1_irq_delay);
    assert_eq!(u0.ay[0].regs[8], 9);
    assert_eq!(u0.ay_current_register[0], 5);
    assert_eq!(u0.ssi.ty, SsiType::P, "no Type before v14");
    // v<12: function 0 is replaced by the usual one with interrupts off; otherwise interrupts on.
    assert_eq!(u0.ssi.current_mode, 0xC0);
    assert_eq!(u1.ssi.current_mode, 0x60);
    assert_eq!(
        (u0.state[0], u0.bus_driven),
        (3, true),
        "bus driven = AY_READ"
    );
    assert_eq!((u1.state[0], u1.bus_driven), (2, false));
    assert_eq!(u0.state[1], 2);
}

#[test]
fn golden_phasor_v8_swaps_the_ay_pairs() {
    // v5-8: unit file i holds AYs of both units (Mockingboard.cpp:1451).
    let unit = |a: u8, b: u8| {
        format!(
            "    SY6522:\n{}    AY8910-A:\n{}    AY8910-B:\n{}    SSI263:\n      Duration / Phoneme: 0x00\n      \
Inflection: 0x00\n      Rate / Inflection: 0x00\n      Control / Articulation / Amplitude: 0x80\n      \
Filter Frequency: 0xFF\n      Current Mode: 0x00\n      Active Phoneme: false\n    AY Current Register: 0x00\n    \
Unit State: 0x2\n    Unit State-B: 0x2\n",
            via_old("delays active", "0xFFFF"),
            ay_old(a),
            ay_old(b),
        )
    };
    let text = format!(
        "4:\n  Card: Phasor\n  Version: 8\n  State:\n    Mode: 5\n    Votrax Phoneme: false\n    Unit0:\n{}    Unit1:\n{}",
        indent(&unit(1, 2), 2),
        indent(&unit(3, 4), 2)
    );
    let mut host = Host::new();
    let (m, warnings) = load_card(&file_with_slots(&text), &mut host, 4);
    assert!(warnings.is_empty(), "{warnings:?}");
    let c = m.card::<MockingboardCard>(4).unwrap();
    let vol = |unit: usize, ay: usize| c.ay_regs(unit, ay)[8];
    assert_eq!([vol(0, 1), vol(1, 1), vol(0, 0), vol(1, 0)], [1, 2, 3, 4]);
    assert_eq!(c.phasor_mode(), 5);
}

#[test]
fn every_variant_is_written_under_its_applewin_card_name() {
    let bus = AudioBus::new(44_100, apple2_core::VideoStandard::Ntsc);
    for (v, name) in [
        (Variant::MockingboardC, "Mockingboard C"),
        (Variant::Phasor, "Phasor"),
        (Variant::MegaAudio, "MEGA Audio"),
        (Variant::SdMusic, "SD Music"),
    ] {
        let m = machine_with(v, 5, &bus);
        let text = save_aws_yaml(
            &m,
            &SaveOptions {
                extension: false,
                ..Default::default()
            },
        )
        .unwrap()
        .text;
        assert!(
            slots_body(&text)
                .starts_with(&format!("5:\n  Card: {name}\n  Version: 15\n  State:\n")),
            "{name}"
        );
    }
}

// ---- full machine round trips ----------------------------------------------------------------

/// Programs the 6522s and the AY like a player would: free-running timer 1 with its interrupt
/// enabled, a tone on AY A, a one-shot timer 2, and the speech chips' registers.
fn play(m: &mut Machine, slot: u8, variant: Variant) {
    m.with_card(slot, |c, ctx| {
        if variant == Variant::Phasor {
            c.io_write(0x0D, 0, ctx); // native Phasor mode
        }
        let bases = if variant == Variant::Phasor {
            [0x10u8, 0x80]
        } else {
            [0x00, 0x80]
        };
        for base in bases {
            let w =
                |c: &mut dyn Card, ctx: &mut _, r: u8, v: u8| c.slot_rom_write(base + r, v, ctx);
            w(c, ctx, 0x02, 0xFF); // DDRB
            w(c, ctx, 0x03, 0xFF); // DDRA
            w(c, ctx, 0x00, 0x04); // ORB: AY idle (reset released)
            w(c, ctx, 0x0B, 0x40); // ACR: free-running timer 1
            w(c, ctx, 0x0E, 0xC0); // IER: timer 1 interrupt
            w(c, ctx, 0x04, 0x34);
            w(c, ctx, 0x05, 0x12); // T1C-H: starts timer 1 ($1234)
            w(c, ctx, 0x08, 0x00);
            w(c, ctx, 0x09, 0x40); // T2C-H: timer 2 ($4000)
            for (reg, val) in [(0u8, 0x1Cu8), (1, 0x01), (7, 0x3E), (8, 0x0F)] {
                w(c, ctx, 0x01, reg); // ORA: register number
                w(c, ctx, 0x00, 0x07); // latch address
                w(c, ctx, 0x00, 0x04);
                w(c, ctx, 0x01, val);
                w(c, ctx, 0x00, 0x06); // write
                w(c, ctx, 0x00, 0x04);
            }
        }
    });
}

fn round_trip(variant: Variant) {
    let bus = AudioBus::new(44_100, apple2_core::VideoStandard::Ntsc);
    let mut a = machine_with(variant, 4, &bus);
    play(&mut a, 4, variant);
    for _ in 0..3 {
        a.run_frame();
    }
    let before = snapshot_in(&a, 4);
    assert!(before.units[0].via.t1_active, "timer 1 is running");
    assert_ne!(before.units[0].ay[0].regs, [0; 16], "the AY was programmed");

    // With the extension: identical MachineState.
    let saved = save_aws_yaml(&a, &SaveOptions::default()).unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
    let mut host = Host::new();
    let b = load_aws_yaml(&saved.text, &mut host).unwrap();
    assert!(b.warnings.is_empty(), "{:?}", b.warnings);
    assert_eq!(
        a.save_state(),
        b.machine.save_state(),
        "{variant:?}: exact state"
    );

    // Without it: the card's AppleWin state, and the machine keeps running.
    let saved = save_aws_yaml(
        &a,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .unwrap();
    let mut host = Host::new();
    let mut c = load_aws_yaml(&saved.text, &mut host).unwrap();
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    let (mut got, mut want) = (snapshot_in(&c.machine, 4), before);
    regs_only(&mut got);
    regs_only(&mut want);
    assert_eq!(got, want, "{variant:?}: card state through AppleWin units");
    c.machine.with_card(4, |card, ctx| {
        card.slot_rom_write(
            if variant == Variant::Phasor {
                0x1D
            } else {
                0x0D
            },
            0x40,
            ctx,
        ); // IFR: clear timer 1 flag
    });
    assert_eq!(snapshot_in(&c.machine, 4).units[0].via.ifr & 0x40, 0);
    for _ in 0..3 {
        c.machine.run_frame();
    }
    // Timer 1 free-runs again after the load: its flag shows up in IFR.
    assert_ne!(
        snapshot_in(&c.machine, 4).units[0].via.ifr & 0x40,
        0,
        "{variant:?}: timer 1 events re-armed"
    );
}

#[test]
fn a_phoneme_playing_when_saved_restarts_and_completes_after_the_load() {
    // Unit 1 of MB_C_V15 with CTL low (phoneme active) instead of power-down.
    let text = MB_C_V15.replace(
        "Control / Articulation / Amplitude: 0x8A",
        "Control / Articulation / Amplitude: 0x0A",
    );
    let text = text.replace("Current Mode: 0x61", "Current Mode: 0x60"); // D7 clear, interrupts on
    let mut host = Host::new();
    let (mut m, warnings) = load_card(&file_with_slots(&text), &mut host, 4);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(snapshot_in(&m, 4).units[1].ssi.current_mode & 1, 0);
    for _ in 0..10 {
        m.run_frame();
    }
    let s = snapshot_in(&m, 4);
    assert_eq!(
        s.units[1].ssi.current_mode & 1,
        1,
        "phoneme complete: D7 set"
    );
    assert_ne!(
        s.units[1].via.ifr & 0x02,
        0,
        "and the SSI263 flag raised in the 6522"
    );
}

#[test]
fn mockingboard_c_round_trips() {
    round_trip(Variant::MockingboardC);
}

#[test]
fn phasor_round_trips() {
    round_trip(Variant::Phasor);
}

#[test]
fn mega_audio_round_trips() {
    round_trip(Variant::MegaAudio);
}

#[test]
fn sd_music_round_trips() {
    round_trip(Variant::SdMusic);
}
