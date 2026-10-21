//! Hard disk controller (Harddisk.cpp:1267-1540).

use super::{make, LoadCtx, Mapping};
use crate::host::ImageRequest;
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::Card;
use apple2_storage::hdd::{HardDiskCard, HdcFirmware, HdcMode, HddSnapshot, HddUnitSnapshot};

/// `kUNIT_VERSION` (Harddisk.cpp:1277).
const VERSION: u32 = 6;

pub const MAP: Mapping = Mapping {
    aw_names: &["Hard Disk Controller", "Generic HDD"],
    our_name: "Hard Disk Controller",
    save,
    load,
};

fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<HardDiskCard>()) else {
        return false;
    };
    let s = c.snapshot();
    w.slot(slot, "Hard Disk Controller", VERSION, |w| {
        w.line(&format!(
            "Current Unit: {} # b7=unit for ProDOS BLK device",
            s.unit_num
        ));
        w.hex8("Command", s.command as u32);
        w.hex64("Not Busy Cycle", s.not_busy_cycle);
        w.hex8("SP Status Code", s.status_code as u32);
        w.hex8("FIFO Index", s.fifo_idx as u32);
        w.map("Firmware", |w| w.memory(&s.firmware));
        for (i, u) in &s.units {
            // Unit0 is the SmartPort controller: images start at Unit1.
            w.map(&format!("Unit{}", i + 1), |w| {
                w.string("Filename", &u.filename);
                w.string("Absolute Path", &u.pathname);
                w.hex8("Error", u.error as u32);
                w.hex16("MemBlock", u.memblock as u32);
                w.hex32("DiskBlock", u.diskblock);
                w.bool("ImageLoaded", true);
                w.uint("Status Next", u.status_next as u64);
                w.uint("Status Prev", u.status_prev as u64);
                w.hex16("Buffer Offset", u.buf_ptr as u32);
                w.map("Buffer", |w| w.memory(&u.buf));
            });
        }
    });
    true
}

fn load(
    ctx: &mut LoadCtx,
    slot: u8,
    version: u32,
    st: Map,
) -> Result<Option<Box<dyn Card>>, Error> {
    if !(1..=VERSION).contains(&version) {
        return Err(Error::Format(format!(
            "Version {version} is not supported for card 'Hard Disk Controller'."
        )));
    }
    let Some(mut card) = make(ctx, "Hard Disk Controller", slot) else {
        return Ok(None);
    };
    // Harddisk.cpp:1455: a state older than v5 does not carry the firmware; v1-v3 ran
    // Hddrvr.bin and v4 Hddrvr-v2.bin (InitializeIO picks them through m_saveStateFirmwareV1/V2).
    if version < 5 {
        let fw = if version < 4 {
            HdcFirmware::HddrvrV1
        } else {
            HdcFirmware::HddrvrV2
        };
        card = Box::new(HardDiskCard::with_firmware(
            slot,
            ctx.model,
            HdcMode::Default,
            fw,
        ));
    }
    let Some(c) = card
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<HardDiskCard>())
    else {
        return Err(Error::Unsupported(
            "host returned a card that is not a hard disk controller".into(),
        ));
    };
    let mut s = HddSnapshot {
        unit_num: st.u32("Current Unit")? as u8,
        command: st.u32("Command")? as u8,
        not_busy_cycle: if version >= 3 {
            st.u64("Not Busy Cycle")?
        } else {
            0
        },
        status_code: 0,
        fifo_idx: 0,
        firmware: Vec::new(),
        units: Vec::new(),
    };
    if version >= 5 {
        s.status_code = st.u32("SP Status Code")? as u8;
        s.fifo_idx = st.u32("FIFO Index")? as u8;
        let fw = st
            .need_sub("Firmware")
            .map_err(|_| Error::Format("HDC: Missing: Firmware".into()))?;
        let mut b = vec![0u8; 256];
        fw.load_memory(&mut b)?;
        s.firmware = b;
    }
    let base = if version >= 5 { 1 } else { 0 };
    for unit in 0..2usize {
        let Some(u) = st.sub(&format!("Unit{}", base + unit)) else {
            continue;
        };
        let filename = u.string("Filename")?;
        let absolute = if version >= 6 {
            u.string("Absolute Path")?
        } else {
            String::new()
        };
        let mut snap = HddUnitSnapshot {
            filename: filename.clone(),
            pathname: absolute.clone(),
            error: u.u32("Error")? as u8,
            memblock: u.u32("MemBlock")? as u16,
            diskblock: u.u32("DiskBlock")?,
            status_next: u.u32("Status Next")? as u8,
            status_prev: u.u32("Status Prev")? as u8,
            buf_ptr: u.u32("Buffer Offset")? as u16,
            buf: vec![0; 512],
        };
        u.bool("ImageLoaded")?;
        let b = u
            .sub("Buffer")
            .ok_or_else(|| Error::Format(format!("Unit{}: Missing: Buffer", base + unit)))?;
        b.load_memory(&mut snap.buf)?;
        if !filename.is_empty() {
            let req = ImageRequest {
                filename: &filename,
                absolute_path: &absolute,
                write_protected: false,
                hard_disk: true,
            };
            match ctx.host.open_image(&req) {
                Ok(img) => {
                    c.insert(unit, img);
                }
                Err(e) => {
                    ctx.warnings
                        .push(format!("HDC: cannot open '{filename}': {e}"));
                    continue;
                }
            }
        }
        s.units.push((unit, snap));
    }
    c.apply_snapshot(&s);
    if version < 5 {
        c.fixup_unit_num(); // Harddisk.cpp:1447
    }
    Ok(Some(card))
}
