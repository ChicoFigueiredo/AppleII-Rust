//! Host-I/O cards (apple2-hostio) in AppleWin's `.aws.yaml`: golden loads (text transcribed from
//! the C++ writers), exact writes, and full-machine round trips.

use apple2_core::{Card, Machine, MachineConfig, Model};
use apple2_hostio::serial::{Parity, StopBits};
use apple2_hostio::ssc::{DipSwitches, FirmwareMode, SscSnapshot, SuperSerialCard};
use apple2_hostio::LoopbackSerial;
use apple2_savestate::cards::{self, LoadCtx};
use apple2_savestate::yaml::{self, Map, Writer};
use apple2_savestate::{default_card, load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::image::DiskImage;

/// Builds the host-bound cards with in-memory back ends.
struct Host;

impl LoadHost for Host {
    fn open_image(&mut self, _req: &ImageRequest) -> Result<DiskImage, String> {
        Err("no images".into())
    }

    fn make_card(&mut self, name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
        match name {
            "Super Serial Card" => Some(Box::new(SuperSerialCard::new(Box::new(LoopbackSerial::new().0)))),
            _ => default_card(name, slot, model),
        }
    }
}

/// Loads the `<slot>:` entry in `text` through the mapping named `aw_name`.
fn load_entry(text: &str, aw_name: &str) -> Box<dyn Card> {
    let top = yaml::parse(text).unwrap();
    let (key, node) = &top[0];
    let yaml::Node::Map(entry) = node else { panic!("not a map") };
    let m = Map { name: key, entries: entry };
    let version = m.u32("Version").unwrap();
    let state = m.need_sub("State").unwrap();
    let mut host = Host;
    let mut warnings = Vec::new();
    let mut ctx = LoadCtx { host: &mut host, model: Model::Apple2eEnhanced, warnings: &mut warnings };
    let map = cards::by_aw_name(aw_name).unwrap();
    let card = (map.load)(&mut ctx, key.parse().unwrap(), version, state).unwrap().unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    card
}

/// The text `save` writes for `card` in `slot` (the slot entry only).
fn save_entry(slot: u8, card: &dyn Card) -> String {
    let mut w = Writer::new("", "");
    assert!(cards::save_card(&mut w, slot, card));
    let t = w.finish();
    t.split_once("---\n").unwrap().1.to_string()
}

fn machine_with(slot: u8, card: Box<dyn Card>) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(slot, card));
    m.power_cycle();
    m
}

fn roundtrip(a: &Machine) -> Machine {
    let saved = save_aws_yaml(a, &SaveOptions::default()).unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
    let loaded = load_aws_yaml(&saved.text, &mut Host).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(a.save_state(), loaded.machine.save_state());
    loaded.machine
}

fn no_ext(a: &Machine) -> Machine {
    let saved = save_aws_yaml(a, &SaveOptions { extension: false, ..Default::default() }).unwrap();
    load_aws_yaml(&saved.text, &mut Host).unwrap().machine
}

// ---------------------------------------------------------------------------------------------
// Super Serial Card

const SSC_V2: &str = "\
2:
  Card: Super Serial Card
  Version: 2
  State:
    DIPSW Default:
      Baud Rate: 9600
      Firmware mode: 0
      Stop Bits: 0
      Byte Size: 8
      Parity: 0
      Linefeed: false
      Interrupts: true
    DIPSW Current:
      Baud Rate: 1200
      Firmware mode: 2
      Stop Bits: 2
      Byte Size: 7
      Parity: 2
      Linefeed: true
      Interrupts: false
    Control Byte: 0x1E
    Command Byte: 0x0B
    TX IRQ Pending: true
    RX IRQ Pending: false
    Written TX: false
    Support DCD: true
    Serial Port Name: COM3
";

fn ssc_expected() -> SscSnapshot {
    SscSnapshot {
        dip: DipSwitches {
            baud: 1200,
            firmware_mode: FirmwareMode::Ppc,
            stop_bits: StopBits::Two,
            data_bits: 7,
            parity: Parity::Even,
            linefeed: true,
            interrupts: false,
        },
        command: 0x0B,
        control: 0x1E,
        tx_irq_pending: true,
        rx_irq_pending: false,
        tx_empty: false,
    }
}

fn ssc_snapshot(c: &dyn Card) -> SscSnapshot {
    c.as_any().unwrap().downcast_ref::<SuperSerialCard>().unwrap().snapshot()
}

#[test]
fn ssc_golden_v2_loads() {
    let c = load_entry(SSC_V2, "Super Serial Card");
    assert_eq!(ssc_snapshot(&*c), ssc_expected());
}

#[test]
fn ssc_golden_v1_consumes_obsolete_keys() {
    let v1 = "\
2:
  Card: Super Serial Card
  Version: 1
  State:
    DIPSW Default:
      Baud Rate: 9600
      Firmware mode: 0
      Stop Bits: 0
      Byte Size: 8
      Parity: 0
      Linefeed: false
      Interrupts: true
    DIPSW Current:
      Baud Rate: 1200
      Firmware mode: 2
      Stop Bits: 2
      Byte Size: 7
      Parity: 2
      Linefeed: true
      Interrupts: false
    Parity: 2
    TX IRQ Enabled: false
    RX IRQ Enabled: true
    Baud Rate: 1200
    Stop Bits: 2
    Byte Size: 7
    Comm Inactivity: 0
    Control Byte: 0x1E
    Command Byte: 0x0B
    TX IRQ Pending: true
    RX IRQ Pending: false
    Written TX: false
    Serial Port Name: COM3
";
    let c = load_entry(v1, "Super Serial Card");
    assert_eq!(ssc_snapshot(&*c), ssc_expected());
}

#[test]
fn ssc_write_matches_the_cpp_writer() {
    let c = load_entry(SSC_V2, "Super Serial Card");
    let expected = "\
2:
  Card: Super Serial Card
  Version: 2
  State:
    DIPSW Default:
      Baud Rate: 9600
      Firmware mode: 0
      Stop Bits: 0
      Byte Size: 8
      Parity: 0
      Linefeed: false
      Interrupts: true
    DIPSW Current:
      Baud Rate: 1200
      Firmware mode: 2
      Stop Bits: 2
      Byte Size: 7
      Parity: 2
      Linefeed: true
      Interrupts: false
    Control Byte: 0x1E
    Command Byte: 0x0B
    TX IRQ Pending: true
    RX IRQ Pending: false
    Written TX: false
    Support DCD: false
    Serial Port Name: TCP
";
    assert_eq!(save_entry(2, &*c), expected);
}

#[test]
fn ssc_full_machine_roundtrip() {
    let mut a = machine_with(2, Box::new(SuperSerialCard::new(Box::new(LoopbackSerial::new().0))));
    a.with_card(2, |c, ctx| {
        c.io_write(0xB, 0x1E, ctx);
        c.io_write(0xA, 0x0B, ctx);
        c.io_write(0x8, b'A', ctx);
    });
    for _ in 0..3 {
        a.run_frame();
    }
    let snap = ssc_snapshot(a.board().slots.card(2).unwrap());
    assert_eq!((snap.command, snap.control), (0x0B, 0x1E));
    let b = roundtrip(&a);
    assert_eq!(ssc_snapshot(b.board().slots.card(2).unwrap()), snap);
    let c = no_ext(&a);
    assert_eq!(ssc_snapshot(c.board().slots.card(2).unwrap()), snap);
}
