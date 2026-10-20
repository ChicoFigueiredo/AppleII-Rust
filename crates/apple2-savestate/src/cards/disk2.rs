//! Disk II (Disk.cpp:2256-2700, DiskFormatTrack.cpp:341).

use super::{make, LoadCtx, Mapping};
use crate::host::ImageRequest;
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::Card;
use apple2_storage::disk2::{Disk2Card, Disk2Snapshot, DriveSnapshot};

/// `kUNIT_VERSION` (Disk.cpp:2261).
const VERSION: u32 = 9;

pub const MAP: Mapping = Mapping { aw_names: &["Disk II", "Disk]["], our_name: "Disk II", save, load };

fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<Disk2Card>()) else { return false };
    let s = c.snapshot();
    w.slot(slot, "Disk II", VERSION, |w| {
        w.uint("Current Drive", s.current_drive as u64);
        w.hex4("Phases", s.magnet_states as u32);
        w.bool("Enhance Disk", s.enhance_disk);
        w.hex8("Floppy Latch", s.floppy_latch as u32);
        w.bool("Floppy Motor On", s.motor_on);
        w.hex64("Last Cycle", s.last_cycle);
        w.hex64("Last Read Latch Cycle", s.last_read_latch_cycle);
        w.hex8("LSS Shift Reg", s.shift_reg as u32);
        w.int("LSS Latch Delay", s.latch_delay as i64);
        w.int("LSS Sequencer Function", s.seq_func as i64);
        w.bool("Deferred Stepper Event", s.deferred_stepper_event);
        w.hex16("Deferred Stepper Address", s.deferred_stepper_address as u32);
        w.hex64("Deferred Stepper Cycle", s.deferred_stepper_cycles);
        // FormatTrack::SaveSnapshot: the format-track tracker is not saved by our card; write
        // the idle state ("waiting for the first write").
        w.map("Format Track state", |w| {
            w.hex16("Written Sector Address Fields", 0);
            w.hex16("Write Track Start Index", 0xFFFF);
            w.bool("Write Track Wrapped", false);
            w.hex8("Write Data Field Prologue Count", 0);
            w.hex8("Track State", 0);
            w.hex24("Last 3 bytes", 0);
            w.hex64("Vol,Trk,Sec,Chk (4 and 4)", 0);
            w.hex4("Index (4 and 4)", 0);
        });
        for (n, d) in s.drives.iter().enumerate() {
            w.map(&format!("Unit{n}"), |w| {
                w.bool("Drive Connected", d.connected);
                w.uint("Phase", d.phase as u32 as u64);
                w.float("Phase (precise)", d.phase_precise as f64);
                w.hex4("Head Window", d.head_window as u32);
                w.hex64("Last Stepper Cycle", d.last_stepper_cycle);
                w.hex64("Motor On Cycle", d.motor_on_cycle);
                w.uint("Spinning", d.spinning as u64);
                w.uint("Write Light", d.write_light as u64);
                w.map("Floppy", |w| {
                    w.string("Filename", &d.filename);
                    w.string("Absolute Path", &d.pathname);
                    w.hex16("Byte", d.byte);
                    w.hex16("Nibbles", d.nibbles);
                    w.hex32("Bit Offset", d.bit_offset);
                    w.hex32("Bit Count", d.bit_count);
                    w.float("Extra Cycles", d.extra_cycles);
                    w.bool("Write Protected", d.write_protected);
                    w.uint("Track Image Data", d.track_data as u64);
                    w.uint("Track Image Dirty", d.track_dirty as u64);
                    if let Some(t) = &d.track {
                        w.map("Track Image", |w| {
                            let mut t = t.clone();
                            t.resize(t.len().div_ceil(8) * 8, 0);
                            w.memory(&t)
                        });
                    }
                });
            });
        }
    });
    true
}

fn load_floppy(ctx: &mut LoadCtx, f: Map, version: u32, d: &mut DriveSnapshot) -> Result<Option<apple2_storage::image::DiskImage>, Error> {
    let filename = f.string("Filename")?;
    let absolute = if version >= 9 { f.string("Absolute Path")? } else { String::new() };
    d.write_protected = f.bool("Write Protected")?;
    d.byte = f.u32("Byte")?;
    d.nibbles = f.u32("Nibbles")?;
    d.track_data = f.u32("Track Image Data")? != 0;
    d.track_dirty = f.u32("Track Image Dirty")? != 0;
    if version >= 4 {
        d.bit_offset = f.u32("Bit Offset")?;
        d.bit_count = f.u32("Bit Count")?;
        d.extra_cycles = f.f64("Extra Cycles")?;
        if d.bit_count != 0 && d.bit_offset >= d.bit_count {
            return Err(Error::Format("Disk image: bitOffset >= bitCount".into()));
        }
    }
    let track = f.sub("Track Image");
    d.filename = filename.clone();
    d.pathname = absolute.clone();
    let mut image = None;
    if !filename.is_empty() {
        let req = ImageRequest { filename: &filename, absolute_path: &absolute, write_protected: d.write_protected, hard_disk: false };
        match ctx.host.open_image(&req) {
            Ok(img) => image = Some(img),
            Err(e) => ctx.warnings.push(format!("Disk II: cannot open '{filename}': {e}")),
        }
    }
    if let (Some(t), Some(img)) = (track, image.as_ref()) {
        let mut buf = vec![0u8; img.max_nibbles_per_track().max(apple2_storage::nibble::NIBBLES_PER_TRACK)];
        t.load_memory(&mut buf)?;
        buf.truncate(img.max_nibbles_per_track());
        d.track = Some(buf);
    }
    Ok(image)
}

fn load(ctx: &mut LoadCtx, slot: u8, version: u32, st: Map) -> Result<Option<Box<dyn Card>>, Error> {
    if !(1..=VERSION).contains(&version) {
        return Err(Error::Format(format!("Version {version} is not supported for card 'Disk II'.")));
    }
    let Some(mut card) = make(ctx, "Disk II", slot) else { return Ok(None) };
    let Some(c) = card.as_any_mut().and_then(|a| a.downcast_mut::<Disk2Card>()) else {
        return Err(Error::Unsupported("host returned a card that is not a Disk II".into()));
    };
    let mut s = Disk2Snapshot {
        current_drive: st.u32("Current Drive")? as u8,
        magnet_states: st.u32("Phases")? as u8,
        enhance_disk: st.bool("Enhance Disk")?,
        floppy_latch: st.u32("Floppy Latch")? as u8,
        motor_on: st.bool("Floppy Motor On")?,
        ..Default::default()
    };
    if version >= 2 {
        s.last_cycle = st.u64("Last Cycle")?;
        st.need_sub("Format Track state")?;
    }
    if version >= 3 {
        s.last_read_latch_cycle = st.u64("Last Read Latch Cycle")?;
    }
    if version >= 4 {
        s.shift_reg = st.u32("LSS Shift Reg")? as u8;
        s.latch_delay = st.i32("LSS Latch Delay")?;
    }
    s.seq_func = if version >= 5 { (st.i32("LSS Sequencer Function")? & 3) as u8 } else { st.bool("Floppy Write Mode")? as u8 };
    if version >= 8 {
        s.deferred_stepper_event = st.bool("Deferred Stepper Event")?;
        s.deferred_stepper_address = st.u32("Deferred Stepper Address")? as u8;
        s.deferred_stepper_cycles = st.u64("Deferred Stepper Cycle")?;
    }
    let mut images = [None, None];
    for (n, (d, image)) in s.drives.iter_mut().zip(images.iter_mut()).enumerate() {
        let name = format!("Unit{n}");
        let u = st.sub(&name).ok_or_else(|| Error::Format(format!("Card: Expected key: {name}")))?;
        d.connected = true;
        if version <= 3 {
            *image = load_floppy(ctx, u, version, d)?;
            u.u32("Track")?;
            d.phase = u.u32("Phase")? as i32;
            d.phase_precise = d.phase as f32;
            d.spinning = u.u32("Spinning")?;
            d.write_light = u.u32("Write Light")?;
        } else {
            let f = u.sub("Floppy").ok_or_else(|| Error::Format("Card: Expected key: Floppy".into()))?;
            *image = load_floppy(ctx, f, version, d)?;
            d.phase = u.u32("Phase")? as i32;
            d.phase_precise = u.f64("Phase (precise)")? as f32;
            d.head_window = (u.u32("Head Window")? & 0xF) as u8;
            d.last_stepper_cycle = u.u64("Last Stepper Cycle")?;
            d.spinning = u.u32("Spinning")?;
            d.write_light = u.u32("Write Light")?;
            if version >= 6 {
                d.connected = u.bool("Drive Connected")?;
                d.motor_on_cycle = u.u64("Motor On Cycle")?;
            }
        }
    }
    for (n, img) in images.into_iter().enumerate() {
        if let Some(img) = img {
            c.insert_disk(n, img);
        }
    }
    c.apply_snapshot(&s);
    Ok(Some(card))
}
