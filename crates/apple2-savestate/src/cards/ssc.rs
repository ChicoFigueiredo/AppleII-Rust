//! Super Serial Card (SerialComms.cpp:1407-1520).

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::Card;
use apple2_hostio::serial::{Parity, StopBits};
use apple2_hostio::ssc::{DipSwitches, FirmwareMode, SscSnapshot, SuperSerialCard};

/// `kUNIT_VERSION` (SerialComms.cpp:1407).
const VERSION: u32 = 2;

pub const MAP: Mapping = Mapping { aw_names: &["Super Serial Card"], our_name: "Super Serial Card", save, load };

/// Win32 `NOPARITY`..`SPACEPARITY`.
fn parity_code(p: Parity) -> u64 {
    match p {
        Parity::None => 0,
        Parity::Odd => 1,
        Parity::Even => 2,
        Parity::Mark => 3,
        Parity::Space => 4,
    }
}

fn parity_from(v: u32) -> Parity {
    match v {
        1 => Parity::Odd,
        2 => Parity::Even,
        3 => Parity::Mark,
        4 => Parity::Space,
        _ => Parity::None,
    }
}

/// Win32 `ONESTOPBIT`, `ONE5STOPBITS`, `TWOSTOPBITS`.
fn stop_code(s: StopBits) -> u64 {
    match s {
        StopBits::One => 0,
        StopBits::OnePointFive => 1,
        StopBits::Two => 2,
    }
}

fn stop_from(v: u32) -> StopBits {
    match v {
        1 => StopBits::OnePointFive,
        2 => StopBits::Two,
        _ => StopBits::One,
    }
}

fn mode_from(v: u32) -> FirmwareMode {
    match v {
        1 => FirmwareMode::SicP8,
        2 => FirmwareMode::Ppc,
        3 => FirmwareMode::SicP8a,
        _ => FirmwareMode::Cic,
    }
}

/// `SaveSnapshotDIPSW` (:1436).
fn save_dip(w: &mut Writer, key: &str, d: &DipSwitches) {
    w.map(key, |w| {
        w.uint("Baud Rate", d.baud as u64);
        w.uint("Firmware mode", d.firmware_mode as u64);
        w.uint("Stop Bits", stop_code(d.stop_bits));
        w.uint("Byte Size", d.data_bits as u64);
        w.uint("Parity", parity_code(d.parity));
        w.bool("Linefeed", d.linefeed);
        w.bool("Interrupts", d.interrupts);
    });
}

/// `LoadSnapshotDIPSW` (:1464).
fn load_dip(st: &Map, key: &str) -> Result<DipSwitches, Error> {
    let m = st.sub(key).ok_or_else(|| Error::Format(format!("Card: Expected key: {key}")))?;
    Ok(DipSwitches {
        baud: m.u32("Baud Rate")?,
        firmware_mode: mode_from(m.u32("Firmware mode")?),
        stop_bits: stop_from(m.u32("Stop Bits")?),
        data_bits: m.u32("Byte Size")? as u8,
        parity: parity_from(m.u32("Parity")?),
        linefeed: m.bool("Linefeed")?,
        interrupts: m.bool("Interrupts")?,
    })
}

/// `SaveSnapshot` (:1448). `DIPSW Default` is AppleWin's static default (9600-8-N-1), `Support
/// DCD` is false and the port name is "TCP": those belong to the host's back end.
fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<SuperSerialCard>()) else { return false };
    let s = c.snapshot();
    w.slot(slot, "Super Serial Card", VERSION, |w| {
        save_dip(w, "DIPSW Default", &DipSwitches::default());
        save_dip(w, "DIPSW Current", &s.dip);
        w.hex8("Control Byte", s.control as u32);
        w.hex8("Command Byte", s.command as u32);
        w.bool("TX IRQ Pending", s.tx_irq_pending);
        w.bool("RX IRQ Pending", s.rx_irq_pending);
        w.bool("Written TX", s.tx_empty);
        w.bool("Support DCD", false);
        w.string("Serial Port Name", "TCP");
    });
    true
}

/// `LoadSnapshot` (:1487), unit versions 1 and 2. The serial back end comes from the host.
fn load(ctx: &mut LoadCtx, slot: u8, version: u32, st: Map) -> Result<Option<Box<dyn Card>>, Error> {
    if !(1..=VERSION).contains(&version) {
        return Err(Error::Format(format!("Version {version} is not supported for card 'Super Serial Card'.")));
    }
    load_dip(&st, "DIPSW Default")?;
    let dip = load_dip(&st, "DIPSW Current")?;
    if version == 1 {
        // Redundant/obsolete keys, consumed (they must exist).
        for k in ["Parity", "Baud Rate", "Stop Bits", "Byte Size", "Comm Inactivity"] {
            st.u32(k)?;
        }
        st.bool("TX IRQ Enabled")?;
        st.bool("RX IRQ Enabled")?;
    } else {
        st.bool("Support DCD")?;
    }
    let command = st.u32("Command Byte")? as u8;
    let control = st.u32("Control Byte")? as u8;
    let snap = SscSnapshot {
        dip,
        command,
        control,
        tx_irq_pending: st.bool("TX IRQ Pending")?,
        rx_irq_pending: st.bool("RX IRQ Pending")?,
        tx_empty: st.bool("Written TX")?,
    };
    st.string("Serial Port Name")?;
    let Some(mut card) = make(ctx, "Super Serial Card", slot) else { return Ok(None) };
    let Some(c) = card.as_any_mut().and_then(|a| a.downcast_mut::<SuperSerialCard>()) else {
        return Err(Error::Unsupported("host returned a card that is not a Super Serial Card".into()));
    };
    c.apply_snapshot(&snap);
    Ok(Some(card))
}
