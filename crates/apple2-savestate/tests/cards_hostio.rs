//! Host-I/O cards (apple2-hostio) in AppleWin's `.aws.yaml`: golden loads (text transcribed from
//! the C++ writers), exact writes, and full-machine round trips.

use apple2_core::{Card, Machine, MachineConfig, Model};
use apple2_hostio::serial::{Parity, StopBits};
use apple2_hostio::ssc::{DipSwitches, FirmwareMode, SscSnapshot, SuperSerialCard};
use apple2_hostio::printer::{MemorySink, ParallelPrinterCard, PrinterEncoding, PrinterOptions, PrinterSnapshot};
use apple2_hostio::network::MockNetwork;
use apple2_hostio::uthernet1::{Uthernet1, Uthernet1Snapshot};
use apple2_hostio::LoopbackSerial;
use apple2_savestate::cards::{self, LoadCtx};
use apple2_savestate::yaml::{self, Map, Writer};
use apple2_savestate::{default_card, load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::image::DiskImage;

/// Builds the host-bound cards with in-memory back ends.
#[derive(Default)]
struct Host {
    sink: MemorySink,
}

impl LoadHost for Host {
    fn open_image(&mut self, _req: &ImageRequest) -> Result<DiskImage, String> {
        Err("no images".into())
    }

    fn make_card(&mut self, name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
        match name {
            "Super Serial Card" => Some(Box::new(SuperSerialCard::new(Box::new(LoopbackSerial::new().0)))),
            "Generic Printer" => Some(Box::new(ParallelPrinterCard::new(Box::new(self.sink.clone()), PrinterOptions::default()))),
            "Uthernet" => Some(Box::new(Uthernet1::new(Box::new(MockNetwork::new().0)))),
            _ => default_card(name, slot, model),
        }
    }
}

/// Loads the `<slot>:` entry in `text` through the mapping named `aw_name`.
fn load_entry(text: &str, aw_name: &str) -> Box<dyn Card> {
    load_entry_with(&mut Host::default(), text, aw_name)
}

fn load_entry_with(host: &mut Host, text: &str, aw_name: &str) -> Box<dyn Card> {
    let top = yaml::parse(text).unwrap();
    let (key, node) = &top[0];
    let yaml::Node::Map(entry) = node else { panic!("not a map") };
    let m = Map { name: key, entries: entry };
    let version = m.u32("Version").unwrap();
    let state = m.need_sub("State").unwrap();
    let mut warnings = Vec::new();
    let mut ctx = LoadCtx { host, model: Model::Apple2eEnhanced, warnings: &mut warnings };
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
    let loaded = load_aws_yaml(&saved.text, &mut Host::default()).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(a.save_state(), loaded.machine.save_state());
    loaded.machine
}

fn no_ext(a: &Machine) -> Machine {
    let saved = save_aws_yaml(a, &SaveOptions { extension: false, ..Default::default() }).unwrap();
    load_aws_yaml(&saved.text, &mut Host::default()).unwrap().machine
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

// ---------------------------------------------------------------------------------------------
// Generic Printer

const PRINTER_OPEN: &str = "\
1:
  Card: Generic Printer
  Version: 1
  State:
    Inactivity: 1234
    Printer Idle Limit: 7
    Print Filename: C:\\Temp\\Printer.txt
    Is File Open: true
    Dump To Printer: true
    Convert Encoding: true
    Filter Unprintable: true
    Printer Append: false
    Enable Dump To Real Printer: true
";

fn printer_snapshot(c: &dyn Card) -> PrinterSnapshot {
    c.as_any().unwrap().downcast_ref::<ParallelPrinterCard>().unwrap().snapshot()
}

#[test]
fn printer_golden_resumes_the_open_job_in_append_mode() {
    let mut host = Host::default();
    let c = load_entry_with(&mut host, PRINTER_OPEN, "Generic Printer");
    let expected = PrinterSnapshot {
        inactivity: 0, // CheckPrint restarts the counter after the load
        options: PrinterOptions {
            idle_limit: 7,
            dump_to_printer: true,
            convert_encoding: Some(PrinterEncoding::Pravets82),
            filter_unprintable: true,
            append: true, // forced when a job is resumed
        },
        is_open: true,
    };
    assert_eq!(printer_snapshot(&*c), expected);
    assert!(host.sink.is_open());
}

#[test]
fn printer_golden_closed_job_keeps_its_fields() {
    let t = PRINTER_OPEN.replace("Is File Open: true", "Is File Open: false").replace("Printer Append: false", "Printer Append: true");
    let mut host = Host::default();
    let c = load_entry_with(&mut host, &t, "Generic Printer");
    let s = printer_snapshot(&*c);
    assert_eq!((s.inactivity, s.is_open, s.options.append, s.options.idle_limit), (1234, false, true, 7));
    assert!(!host.sink.is_open());
}

#[test]
fn printer_write_matches_the_cpp_writer() {
    let c = load_entry(&PRINTER_OPEN.replace("Is File Open: true", "Is File Open: false"), "Generic Printer");
    let expected = "\
1:
  Card: Generic Printer
  Version: 1
  State:
    Inactivity: 1234
    Printer Idle Limit: 7
    Print Filename: Printer.txt
    Is File Open: false
    Dump To Printer: true
    Convert Encoding: true
    Filter Unprintable: true
    Printer Append: false
    Enable Dump To Real Printer: false
";
    assert_eq!(save_entry(1, &*c), expected);
}

#[test]
fn printer_full_machine_roundtrip() {
    let mut a = machine_with(1, Box::new(ParallelPrinterCard::new(Box::new(MemorySink::new()), PrinterOptions::default())));
    a.with_card(1, |c, ctx| c.io_write(0, b'X', ctx));
    a.run_frame();
    let snap = printer_snapshot(a.board().slots.card(1).unwrap());
    assert!(snap.is_open && snap.inactivity > 0);
    // The load resumes the job in append mode, then `insert_card`'s power-on reset closes it: the
    // next print reopens it (appending), so only the flag differs.
    let want = PrinterSnapshot { inactivity: 0, options: PrinterOptions { append: true, ..snap.options }, is_open: false };
    let b = roundtrip(&a);
    assert_eq!(printer_snapshot(b.board().slots.card(1).unwrap()), want);
    let c = no_ext(&a);
    assert_eq!(printer_snapshot(c.board().slots.card(1).unwrap()), want);
}

// ---------------------------------------------------------------------------------------------
// Uthernet (I)

/// `YamlSaveHelper::SaveMemory` layout, written independently of the crate's `Writer`: 64 bytes
/// per line, `%04X: ` prefix, `indent` spaces.
fn dump(data: &[u8], indent: usize) -> String {
    let mut s = String::new();
    for (i, chunk) in data.chunks(64).enumerate() {
        s += &format!("{:indent$}{:04X}: ", "", i * 64);
        for b in chunk {
            s += &format!("{b:02X}");
        }
        s.push('\n');
    }
    s
}

fn u1_pp() -> Vec<u8> {
    let mut pp = vec![0u8; 0x1000];
    pp[0..4].copy_from_slice(&[0x0E, 0x63, 0x00, 0x07]);
    pp[0x104..0x106].copy_from_slice(&0x0E80u16.to_le_bytes()); // RxCTL: broadcast, mac, multicast, promiscuous
    pp[0x150..0x158].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]); // hash filter
    pp[0x158..0x15E].copy_from_slice(&[0x02, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]); // MAC
    pp[0x400..0x404].copy_from_slice(&[0x04, 0x01, 0x40, 0x00]);
    pp[0xFFF] = 0x5A;
    pp
}

fn u1_io() -> [u8; 16] {
    let mut io = [0u8; 16];
    io[0x0A..0x0C].copy_from_slice(&0x0124u16.to_le_bytes());
    io[0] = 0x12;
    io[0x0C] = 0x34;
    io
}

fn u1_text(enabled: bool, iface: &str) -> String {
    format!(
        "5:\n  Card: Uthernet\n  Version: 1\n  State:\n    Enabled: {enabled}\n    Network Interface: {iface}\n    Started Tx: false\n    Cannot Use: false\n    Tx Collect Buffer: 0x0A12\n    Rx Buffer: 0x0402\n    CS8900A Registers:\n{}    PacketPage Registers:\n{}",
        dump(&u1_io(), 6),
        dump(&u1_pp(), 6)
    )
}

#[test]
fn uthernet1_golden_loads_registers_and_derives_filters() {
    let c = load_entry(&u1_text(true, "Ethernet 2"), "Uthernet");
    let u = c.as_any().unwrap().downcast_ref::<Uthernet1>().unwrap();
    let s = u.snapshot();
    assert_eq!(s.io, u1_io());
    assert_eq!(s.packet_page, u1_pp());
    assert_eq!((s.txcollect_buffer, s.rx_buffer), (0x0A12, 0x0402));
    assert_eq!(u.mac(), [0x02, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE], "derived from the packet page");
}

#[test]
fn uthernet1_write_matches_the_cpp_writer() {
    let c = load_entry(&u1_text(false, "\"\""), "Uthernet");
    // The mock back end is valid and called "mock".
    assert_eq!(save_entry(5, &*c), u1_text(true, "mock"));
}

#[test]
fn uthernet1_full_machine_roundtrip() {
    let mut a = machine_with(5, Box::new(Uthernet1::new(Box::new(MockNetwork::new().0))));
    a.with_card(5, |c, ctx| {
        // Point the packet page pointer at RxCTL and write 0x0F00 through the data port.
        c.io_write(0x0A, 0x04, ctx);
        c.io_write(0x0B, 0x01, ctx);
        c.io_write(0x0C, 0x00, ctx);
        c.io_write(0x0D, 0x0F, ctx);
    });
    a.run_frame();
    let snap = |m: &Machine| -> Uthernet1Snapshot { m.card::<Uthernet1>(5).unwrap().snapshot() };
    let want = snap(&a);
    assert_eq!(want.packet_page[0x104..0x106], [0x00, 0x0F]);
    let b = roundtrip(&a);
    assert_eq!(snap(&b), want);
    assert_eq!(snap(&no_ext(&a)), want);
}
