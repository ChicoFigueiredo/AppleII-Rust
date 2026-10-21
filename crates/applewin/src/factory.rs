//! The card factory: turns [`Settings`] into a [`Machine`] with every peripheral AppleWin can
//! put in a slot (`CardManager::InsertInternal`, CardManager.cpp:48).
//!
//! [`build_card`] matches [`SlotCardSpec`] exhaustively, so a new card type fails to compile
//! until it is decided here. Cards that AppleWin models as a `DummyCard` (Generic Clock, Echo)
//! and the RAM cards that live in the core's [`MachineConfig`] (Language Card, Saturn, 80-column
//! and RamWorks aux cards) yield no slot card.

use apple2_audio::ay8910::AyType;
use apple2_audio::ssi263::SsiType;
use apple2_audio::{AudioBus, MockingboardCard, SamCard, Sockets, Variant};
use apple2_cards::snesmax::ButtonMap;
use apple2_cards::{DongleKind, HostClock, NoSlotClock, WallTime};
use apple2_config::card::{AyChip, HdcFirmware, PrinterSpec, SoundSockets, Ssi263Chip};
use apple2_config::settings::DongleType;
use apple2_config::{CardType, Settings, SlotCardSpec};
use apple2_core::{Card, Machine, Model};
use apple2_hostio::network::{NetworkBackend, NullNetwork};
use apple2_hostio::printer::{PrinterEncoding, PrinterOptions};
use apple2_hostio::serial::SerialBackend;
use apple2_hostio::{
    FileSink, LoopbackSerial, ParallelPrinterCard, SuperSerialCard, TcpServerSerial, Uthernet1,
    Uthernet2,
};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::hdd::{HardDiskCard, HdcFirmware as HddFirmware, HdcMode};
use std::path::PathBuf;

/// The host's local wall-clock time for the clock cards (`GetLocalTime`).
pub struct SystemClock;

impl HostClock for SystemClock {
    fn now(&self) -> WallTime {
        use chrono::{Datelike, Timelike};
        let t = chrono::Local::now();
        WallTime {
            year: t.year().clamp(0, i32::from(u16::MAX)) as u16,
            month: t.month() as u8,
            day: t.day() as u8,
            weekday: t.weekday().num_days_from_sunday() as u8,
            hour: t.hour() as u8,
            minute: t.minute() as u8,
            second: t.second() as u8,
            centisecond: (t.nanosecond() / 10_000_000).min(99) as u8,
        }
    }
}

/// Printer file when the settings give none (ParallelPrinter.cpp:162).
pub const DEFAULT_PRINTER_FILE: &str = "Printer.txt";

fn ay_type(c: AyChip) -> AyType {
    match c {
        AyChip::Empty => AyType::Empty,
        AyChip::Ay8910 => AyType::Ay3_8910,
        AyChip::Ay8912 => AyType::Ay3_8912,
        AyChip::Ay8913 => AyType::Ay3_8913,
        AyChip::Ym2149 => AyType::Ym2149f,
    }
}

fn ssi_type(c: Ssi263Chip) -> SsiType {
    match c {
        Ssi263Chip::Empty => SsiType::Empty,
        Ssi263Chip::Ssi263P => SsiType::P,
        Ssi263Chip::Ssi263AP => SsiType::Ap,
    }
}

/// Socket population as the sound card crate models it (one AY type per card; the Phasor's
/// third and fourth sockets and the "bad 6522" switches are not modelled there).
pub fn sockets(s: &SoundSockets) -> Sockets {
    Sockets {
        ssi263: [ssi_type(s.ssi263[0]), ssi_type(s.ssi263[1])],
        sc01: s.sc01,
        ay: ay_type(s.ay[0]),
    }
}

pub fn dongle_kind(t: DongleType) -> Option<DongleKind> {
    Some(match t {
        DongleType::Empty => return None,
        DongleType::SdsSpeedStar => DongleKind::SdsSpeedStar,
        DongleType::Codewriter => DongleKind::CodeWriter,
        DongleType::Robocom500 => DongleKind::Robocom500,
        DongleType::Robocom1000 => DongleKind::Robocom1000,
        DongleType::Robocom1500 => DongleKind::Robocom1500,
        DongleType::HaydenCompiler => DongleKind::HaydenCompiler,
    })
}

pub fn printer_encoding(model: Model) -> Option<PrinterEncoding> {
    match model {
        Model::Pravets8M | Model::Pravets82 | Model::BadPravets82 | Model::BadPravets8M => {
            Some(PrinterEncoding::Pravets82)
        }
        Model::Pravets8A => Some(PrinterEncoding::Pravets8A),
        _ => None,
    }
}

pub fn printer_options(spec: &PrinterSpec, model: Model, real_printer: bool) -> PrinterOptions {
    PrinterOptions {
        idle_limit: spec.idle_limit,
        dump_to_printer: spec.dump_to_printer || real_printer,
        convert_encoding: if spec.convert_encoding {
            printer_encoding(model)
        } else {
            None
        },
        filter_unprintable: spec.filter_unprintable,
        append: spec.append,
    }
}

pub fn printer_card(spec: &PrinterSpec, model: Model, real_printer: bool) -> ParallelPrinterCard {
    let path: PathBuf = spec
        .filename
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_PRINTER_FILE));
    ParallelPrinterCard::new(
        Box::new(FileSink::new(path)),
        printer_options(spec, model, real_printer),
    )
}

/// SSC back end: TCP server when a port is set, else the named host serial port (feature
/// `serialport`), else a loopback (nothing connected, bytes sent come back like a jumpered plug).
pub fn ssc_backend(
    serial_port: &str,
    tcp_port: Option<u16>,
    support_dcd: bool,
    warnings: &mut Vec<String>,
) -> Box<dyn SerialBackend> {
    if let Some(p) = tcp_port {
        return Box::new(TcpServerSerial::new(if p == 0 {
            apple2_hostio::serial::TCP_SERIAL_PORT_DEFAULT
        } else {
            p
        }));
    }
    if !serial_port.is_empty() {
        #[cfg(feature = "serialport")]
        {
            let mut port = apple2_hostio::serial::HostSerialPort::new(serial_port);
            port.support_dcd = support_dcd;
            return Box::new(port);
        }
        #[cfg(not(feature = "serialport"))]
        warnings.push(format!(
            "SSC: serial port '{serial_port}' needs the `serialport` feature; using loopback"
        ));
    }
    let _ = (support_dcd, &warnings);
    Box::new(LoopbackSerial::new().0)
}

/// Uthernet back end: libpcap on the named interface (feature `pcap`), else a null network (the
/// Uthernet II keeps its host-socket and virtual-DNS modes).
pub fn network_backend(interface: &str, warnings: &mut Vec<String>) -> Box<dyn NetworkBackend> {
    if !interface.is_empty() {
        #[cfg(feature = "pcap")]
        return Box::new(apple2_hostio::pcap_backend::PcapNetwork::open(interface));
        #[cfg(not(feature = "pcap"))]
        warnings.push(format!(
            "Uthernet: interface '{interface}' needs the `pcap` feature; no network attached"
        ));
    }
    let _ = &warnings;
    Box::new(NullNetwork)
}

fn hdc_mode(f: HdcFirmware) -> HdcMode {
    match f {
        HdcFirmware::Default => HdcMode::Default,
        HdcFirmware::SmartPort => HdcMode::SmartPort,
        HdcFirmware::BlockMode2Devices => HdcMode::BlockMode2Devices,
        HdcFirmware::BlockMode4Devices => HdcMode::BlockMode4Devices,
    }
}

/// Everything a card needs from the shell besides its own spec.
pub struct Env<'a> {
    pub settings: &'a Settings,
    pub bus: &'a AudioBus,
}

/// Builds the card for `slot`, or `None` when the slot stays empty / the core owns the card.
/// Non-fatal problems (an image that does not open) are appended to `warnings`.
pub fn build_card(
    spec: &SlotCardSpec,
    slot: u8,
    env: &Env,
    warnings: &mut Vec<String>,
) -> Option<Box<dyn Card>> {
    let s = env.settings;
    let sound = !s.sound.disable_sound;
    let mockingboard = sound && !s.sound.disable_mockingboard;
    let mb = |variant: Variant, k: &SoundSockets| -> Option<Box<dyn Card>> {
        mockingboard.then(|| {
            Box::new(MockingboardCard::with_sockets(variant, env.bus, sockets(k))) as Box<dyn Card>
        })
    };
    match spec {
        SlotCardSpec::Empty => None,
        SlotCardSpec::Disk2 {
            firmware_13_sector,
            drives,
        } => {
            let mut c = Disk2Card::new();
            c.set_force_13_sector_firmware(*firmware_13_sector);
            c.set_enhance_disk(s.storage.enhance_disk_speed);
            for (d, drive) in drives.iter().enumerate() {
                if !drive.connected {
                    c.unplug_drive(d);
                }
                if let Some(path) = &drive.image {
                    if let Err(e) = c.insert_path(d, path, drive.write_protect) {
                        warnings.push(format!(
                            "slot {slot} drive {}: {}: {e:?}",
                            d + 1,
                            path.display()
                        ));
                    }
                }
            }
            Some(Box::new(c))
        }
        SlotCardSpec::Ssc {
            serial_port,
            tcp_port,
            support_dcd,
        } => Some(Box::new(SuperSerialCard::new(ssc_backend(
            serial_port,
            *tcp_port,
            *support_dcd,
            warnings,
        )))),
        SlotCardSpec::MockingboardC(k) => mb(Variant::MockingboardC, k),
        SlotCardSpec::Phasor(k) => mb(Variant::Phasor, k),
        SlotCardSpec::MegaAudio(k) => mb(Variant::MegaAudio, k),
        SlotCardSpec::SdMusic(k) => mb(Variant::SdMusic, k),
        SlotCardSpec::GenericPrinter(p) => Some(Box::new(printer_card(
            p,
            s.machine.model,
            s.misc.enable_dump_to_real_printer,
        ))),
        SlotCardSpec::GenericHdd { firmware, images } => {
            let fw = match (s.misc.hdc_firmware_v1, s.misc.hdc_firmware_v2) {
                (true, _) => HddFirmware::HddrvrV1,
                (_, true) => HddFirmware::HddrvrV2,
                _ => HddFirmware::SmartPort,
            };
            let mut c = HardDiskCard::with_firmware(slot, s.machine.model, hdc_mode(*firmware), fw);
            if let Some(n) = s.storage.harddisk_num_blocks {
                c.set_user_num_blocks(n);
            }
            for (u, path) in images.iter().enumerate() {
                if let Some(path) = path {
                    if let Err(e) = c.insert_path(u, path) {
                        warnings.push(format!(
                            "slot {slot} hard disk {}: {}: {e:?}",
                            u + 1,
                            path.display()
                        ));
                    }
                }
            }
            Some(Box::new(c))
        }
        // AppleWin: `new DummyCard(...)`.
        SlotCardSpec::GenericClock | SlotCardSpec::Echo => None,
        SlotCardSpec::MouseInterface { .. } => Some(Box::new(apple2_cards::MouseCard::new())),
        SlotCardSpec::Z80 => Some(Box::new(apple2_cards::SoftCard::new())),
        SlotCardSpec::Sam => sound.then(|| Box::new(SamCard::new(env.bus)) as Box<dyn Card>),
        // Built into the core's MachineConfig (aux slot, slot 0, Saturn).
        SlotCardSpec::Col80
        | SlotCardSpec::Extended80Col
        | SlotCardSpec::RamWorksIII { .. }
        | SlotCardSpec::LanguageCard
        | SlotCardSpec::LanguageCardIIe
        | SlotCardSpec::Saturn128K { .. } => None,
        SlotCardSpec::Uthernet { interface } => Some(Box::new(Uthernet1::new(network_backend(
            interface, warnings,
        )))),
        SlotCardSpec::Uthernet2 {
            interface,
            virtual_dns,
        } => Some(Box::new(Uthernet2::new(
            network_backend(interface, warnings),
            *virtual_dns,
        ))),
        SlotCardSpec::FourPlay => Some(Box::new(apple2_cards::FourPlay::new())),
        SlotCardSpec::SnesMax {
            alt_controller,
            user_mapping,
        } => Some(Box::new(snes_max_card(
            *alt_controller,
            user_mapping,
            warnings,
        ))),
        SlotCardSpec::VidHd => Some(Box::new(apple2_cards::VidHd::new())),
        SlotCardSpec::BreakpointCard => Some(Box::new(apple2_cards::BreakpointCard::new())),
    }
}

/// The SNES MAX with the `-snes-max-alt-joy1/2` and `-snes-max-user-joy1/2` command-line options
/// applied (`SNESMAXCard` constructor, SNESMAX.cpp / CmdLine.cpp:727-748). A user mapping file
/// that parses enables the alternative controller for that joystick too
/// (`ParseControllerMappingFile`); one that does not leaves the defaults and adds a warning with
/// AppleWin's message (which shows it in a message box).
pub fn snes_max_card(
    alt_controller: [bool; 2],
    user_mapping: &[Option<PathBuf>; 2],
    warnings: &mut Vec<String>,
) -> apple2_cards::SnesMax {
    let mut c = apple2_cards::SnesMax::new();
    for n in 0..2 {
        c.set_alt_controller(n, alt_controller[n]);
        if let Some(path) = &user_mapping[n] {
            match parse_controller_mapping_file(path) {
                Ok(map) => {
                    c.set_alt_map(n, map);
                    c.set_alt_controller(n, true);
                }
                Err(e) => warnings.push(format!("Error with yaml file: {}\n{e}", path.display())),
            }
        }
    }
    c
}

/// `SNESMAXCard::ParseControllerMappingFile` (SNESMAX.cpp): an `AppleWin Controller Button
/// Remapping` v1 document whose first `Unit` has `HID`, `Description` and `1`..`16` giving the
/// card button (`A B X Y LB RB SELECT START`, empty = unused) of each host button. Only the
/// first unit is read (the source breaks out of its loop: "TODO: extend to support multiple
/// controllers"). A file with no unit is accepted and keeps the default table. Unlike AppleWin,
/// which edits its table while reading, a failure leaves the table untouched.
pub fn parse_controller_mapping_file(path: &std::path::Path) -> Result<ButtonMap, String> {
    use apple2_savestate::yaml::{self, Map, Node};
    let text = std::fs::read_to_string(path)
        .map_err(|_| "Controller mapping file: Failed to initialize parser or open file")?;
    let top = yaml::parse(&text).map_err(|e| e.to_string())?;
    let mut entries = top.iter();
    let hdr = match entries.next() {
        Some((k, Node::Map(m))) if k == "File_hdr" => Map {
            name: "File_hdr",
            entries: m,
        },
        Some(_) => return Err("Failed to find file header".into()),
        None => return Err("File_hdr: Failed to find scalar".into()),
    };
    let tag = hdr.string("Tag").map_err(|e| e.to_string())?;
    if tag != "AppleWin Controller Button Remapping" {
        return Err("File_hdr: Bad tag".into());
    }
    if hdr.u32("Version").map_err(|e| e.to_string())? != 1 {
        return Err("Controller mapping file: Version mismatch".into());
    }
    let mut map = ButtonMap::ALT_DEFAULT;
    match entries.next() {
        None => {}
        Some((k, Node::Map(unit))) if k == "Unit" => {
            let u = Map {
                name: "Unit",
                entries: unit,
            };
            u.string("HID").map_err(|e| e.to_string())?;
            u.string("Description").map_err(|e| e.to_string())?;
            for (i, slot) in map.0.iter_mut().enumerate() {
                // A missing key is UNUSED (`LoadString_NoThrow` not found).
                *slot = match u.str_opt(&(i + 1).to_string()) {
                    Some(name) => ButtonMap::parse_name(name)?,
                    None => None,
                };
            }
        }
        Some((k, _)) => return Err(format!("Unknown top-level scalar: {k}")),
    }
    Ok(map)
}

/// Slot configuration checks done before building, with AppleWin's behaviour (the oracle wins,
/// ADR 0004): `CardManager::InsertInternal` (CardManager.cpp:52-131) takes the slots in ascending
/// order and, for the cards it keeps a single pointer to (Mouse Interface, SSC, printer, Z80,
/// VidHD), leaves a later duplicate's slot EMPTY (`_ASSERT` in debug builds). The first one stays.
/// Uthernet, Uthernet II, 4Play and SNES MAX have no such check there (only the slot dialog
/// refuses them), so they are built as configured. Returns the warnings (our extension, shown in
/// the status line and on stderr; they do not change what is emulated) and the slots whose card
/// must be left empty.
pub fn check_slots(s: &Settings) -> (Vec<String>, [bool; 8]) {
    let mut warnings = Vec::new();
    let mut ignored = [false; 8];
    let types: Vec<CardType> = s.slots.0.iter().map(SlotCardSpec::card_type).collect();
    let slots_of = |ty: CardType| -> Vec<usize> { (1..8).filter(|&i| types[i] == ty).collect() };
    let list = |v: &[usize]| {
        v.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    // The cards CardManager.cpp:64-117 refuses a second time.
    for ty in [
        CardType::MouseInterface,
        CardType::Ssc,
        CardType::GenericPrinter,
        CardType::Z80,
        CardType::VidHd,
    ] {
        let at = slots_of(ty);
        if let Some((first, rest)) = at.split_first() {
            for &n in rest {
                ignored[n] = true;
            }
            if !rest.is_empty() {
                warnings.push(format!(
                    "{ty:?} can be plugged in only one slot: kept in slot {first}, slots {} left empty (as AppleWin)",
                    list(rest)
                ));
            }
        }
    }
    // Only the slot dialog prevents these in AppleWin; the card manager builds them all.
    for ty in [
        CardType::FourPlay,
        CardType::SnesMax,
        CardType::Uthernet,
        CardType::Uthernet2,
    ] {
        let at = slots_of(ty);
        if at.len() > 1 {
            warnings.push(format!(
                "{ty:?} is in several slots ({}); AppleWin's slot dialog would refuse this",
                list(&at)
            ));
        }
    }
    let (u1, u2) = (slots_of(CardType::Uthernet), slots_of(CardType::Uthernet2));
    if !u1.is_empty() && !u2.is_empty() {
        warnings.push(format!(
            "Uthernet (slot {}) and Uthernet II (slot {}) together: AppleWin's slot dialog would refuse this",
            list(&u1),
            list(&u2)
        ));
    }
    let slot0 = types[0];
    let ok0 = if s.machine.model.is_apple2() {
        matches!(
            slot0,
            CardType::Empty | CardType::LanguageCard | CardType::Saturn128K
        )
    } else {
        matches!(slot0, CardType::Empty | CardType::LanguageCardIIe)
    };
    if !ok0 {
        warnings.push(format!(
            "Slot 0: {slot0:?} is not valid in slot 0 of this model and was ignored"
        ));
    }
    for ty in [CardType::LanguageCard, CardType::LanguageCardIIe] {
        let at = slots_of(ty);
        if !at.is_empty() {
            warnings.push(format!(
                "Slot {}: {ty:?} only works in slot 0 and was ignored",
                list(&at)
            ));
        }
    }
    (warnings, ignored)
}

/// The machine for `settings`: core configuration (model, CPU, ROMs, RAM cards, aux card), the
/// slot cards, the No-Slot Clock and the game-port dongle.
pub fn build_machine(settings: &Settings, bus: &AudioBus) -> Result<Machine, String> {
    build_machine_report(settings, bus).map(|(m, _)| m)
}

/// [`build_machine`] plus the non-fatal warnings.
pub fn build_machine_report(
    settings: &Settings,
    bus: &AudioBus,
) -> Result<(Machine, Vec<String>), String> {
    let (slot_warnings, ignored) = check_slots(settings);
    let roms = settings.resolve_roms();
    let mut m = Machine::try_new(roms.machine).map_err(|e| e.to_string())?;
    let mut warnings = roms.warnings;
    warnings.extend(slot_warnings);
    let env = Env { settings, bus };
    for (slot, spec) in settings.slots.0.iter().enumerate().skip(1) {
        if ignored[slot] {
            continue;
        }
        if let Some(card) = build_card(spec, slot as u8, &env, &mut warnings) {
            if !m.insert_card(slot as u8, card) {
                warnings.push(format!("slot {slot}: card not accepted"));
            }
        }
    }
    if settings.machine.no_slot_clock {
        m.set_rom_tap(Some(Box::new(NoSlotClock::new(Box::new(SystemClock)))));
    }
    if let Some(kind) = dongle_kind(settings.joystick.dongle) {
        m.set_game_io(Some(Box::new(apple2_cards::Dongle::new(kind))));
    }
    m.power_cycle();
    Ok((m, warnings))
}

/// First slot holding a card of type `T`.
pub fn slot_of<T: Card + 'static>(m: &Machine) -> Option<u8> {
    (0..8u8).find(|&s| m.card::<T>(s).is_some())
}
