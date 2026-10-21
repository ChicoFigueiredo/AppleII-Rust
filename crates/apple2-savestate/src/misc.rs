//! `Unit: Miscellaneous` (No Slot Clock: SaveState.cpp:550, NoSlotClock.cpp:196-220) and the
//! dongle part of `Unit: Game I/O Connector` (CopyProtectionDongles.cpp:150-330).
//!
//! The shift-key mod (`Shift Key Mod`) is a frontend setting: it is written as false and its
//! loaded value is ignored.

use crate::host::LoadHost;
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_cards::{Dongle, DongleKind, NoSlotClock, NscSnapshot};
use apple2_core::{Machine, MachineState};

/// `UNIT_MISC_VER` (SaveState.cpp:76).
pub const UNIT_MISC_VER: u32 = 1;

/// `GetSnapshotStructName_*` (CopyProtectionDongles.cpp:187-216), the `Device` value.
const DEVICES: [(DongleKind, &str); 6] = [
    (DongleKind::SdsSpeedStar, "SDS SpeedStar dongle"),
    (
        DongleKind::CodeWriter,
        "Cortechs Corp - CodeWriter protection key",
    ),
    (
        DongleKind::Robocom500,
        "Robocom Ltd - Robo 500 Interface Module",
    ),
    (
        DongleKind::Robocom1000,
        "Robocom Ltd - Robo 1000 Interface Module",
    ),
    (
        DongleKind::Robocom1500,
        "Robocom Ltd - Robo 1500 Interface Module",
    ),
    (
        DongleKind::HaydenCompiler,
        "Hayden - Applesoft Compiler protection key",
    ),
];

/// Body of `Unit: Game I/O Connector`: `ShiftKeyModSaveSnapshot` then, with a dongle attached,
/// `CopyProtectionDongleSaveSnapshot`.
pub fn save_game_io(w: &mut Writer, machine: &Machine) {
    w.bool("Shift Key Mod", false);
    let Some(d) = machine
        .game_io()
        .and_then(|g| g.as_any())
        .and_then(|a| a.downcast_ref::<Dongle>())
    else {
        return;
    };
    let name = DEVICES
        .iter()
        .find(|(k, _)| *k == d.kind())
        .map_or("", |(_, n)| n);
    w.string("Device", name);
    if d.kind() == DongleKind::CodeWriter {
        w.hex8("LFSR", d.lfsr() as u32);
    }
}

/// The `Unit: Miscellaneous` registers, if the machine has a No-Slot Clock.
pub fn nsc_of(st: &MachineState) -> Option<NscSnapshot> {
    st.rom_tap
        .as_deref()
        .and_then(NoSlotClock::snapshot_of_saved)
}

/// `CNoSlotClock::SaveSnapshot` inside the unit's `State`.
pub fn save_nsc(w: &mut Writer, s: &NscSnapshot) {
    w.map("No Slot Clock", |w| {
        w.bool("Clock Register Enabled", s.clock_register_enabled);
        w.bool("Write Enabled", s.write_enabled);
        w.hex64("Clock Register Mask", s.clock_register_mask);
        w.hex64("Clock Register", s.clock_register);
        w.hex64("Comparison Register Mask", s.comparison_register_mask);
        w.hex64("Comparison Register", s.comparison_register);
    });
}

/// `CNoSlotClock::LoadSnapshot`: no `No Slot Clock` map means no registers.
pub fn parse_nsc(state: Map) -> Result<Option<NscSnapshot>, Error> {
    let Some(m) = state.sub("No Slot Clock") else {
        return Ok(None);
    };
    Ok(Some(NscSnapshot {
        clock_register_enabled: m.bool("Clock Register Enabled")?,
        write_enabled: m.bool("Write Enabled")?,
        clock_register_mask: m.u64("Clock Register Mask")?,
        clock_register: m.u64("Clock Register")?,
        comparison_register_mask: m.u64("Comparison Register Mask")?,
        comparison_register: m.u64("Comparison Register")?,
    }))
}

/// What `Unit: Game I/O Connector` says about the dongle.
pub struct GameIo {
    pub device: Option<(String, Option<u8>)>,
}

/// `ShiftKeyModLoadSnapshot` + `CopyProtectionDongleLoadSnapshot`.
pub fn parse_game_io(version: u32, state: Map, max_version: u32) -> Result<GameIo, Error> {
    if !(1..=max_version).contains(&version) {
        return Err(Error::Format(format!(
            "Version {version} is not supported for game I/O device."
        )));
    }
    if version >= 4 {
        state.bool("Shift Key Mod")?;
    }
    let Some(device) = state.str_opt("Device") else {
        return Ok(GameIo { device: None });
    };
    let device = state
        .string("Device")
        .unwrap_or_else(|_| device.to_string());
    let lfsr = if device == DEVICES[1].1 {
        Some(state.u32("LFSR")? as u8)
    } else {
        None
    };
    Ok(GameIo {
        device: Some((device, lfsr)),
    })
}

/// What the loaded file says about the devices outside the slots.
pub struct DeviceUnits {
    /// `Apple2` unit version (before 6 the No-Slot Clock is always inserted).
    pub apple2_version: u32,
    pub misc_unit: bool,
    pub nsc: Option<NscSnapshot>,
    pub game_io: Option<GameIo>,
    /// The extension's state has a ROM tap / game I/O device (so the machine must too).
    pub ext_rom_tap: bool,
    pub ext_game_io: bool,
}

/// Installs the No-Slot Clock and the dongle the file asks for.
pub fn install(
    machine: &mut Machine,
    host: &mut dyn LoadHost,
    u: &DeviceUnits,
    warnings: &mut Vec<String>,
) {
    if u.apple2_version < 6 || u.misc_unit || u.ext_rom_tap {
        match host.make_no_slot_clock() {
            Some(tap) => {
                machine.set_rom_tap(Some(tap));
                if let Some(s) = &u.nsc {
                    let ok = machine
                        .rom_tap_mut()
                        .and_then(|t| t.as_any_mut())
                        .and_then(|a| a.downcast_mut::<NoSlotClock>())
                        .is_some_and(|c| c.apply_snapshot(s));
                    if !ok {
                        warnings.push(
                            "Miscellaneous: the No-Slot Clock registers could not be applied"
                                .into(),
                        );
                    }
                }
            }
            None => warnings.push("Miscellaneous: the host cannot provide a No-Slot Clock".into()),
        }
    }
    let Some((name, lfsr)) = u.game_io.as_ref().and_then(|g| g.device.as_ref()) else {
        return;
    };
    let Some((kind, _)) = DEVICES.iter().find(|(_, n)| n == name) else {
        warnings.push(format!(
            "Game I/O Connector: unknown device '{name}' (skipped)"
        ));
        return;
    };
    match host.make_dongle(*kind) {
        Some(dev) => {
            machine.set_game_io(Some(dev));
            if let (Some(l), Some(d)) = (
                lfsr,
                machine
                    .game_io_mut()
                    .and_then(|g| g.as_any_mut())
                    .and_then(|a| a.downcast_mut::<Dongle>()),
            ) {
                d.set_lfsr(*l);
            }
        }
        None => warnings.push(format!(
            "Game I/O Connector: the host cannot provide the '{name}' device"
        )),
    }
}
