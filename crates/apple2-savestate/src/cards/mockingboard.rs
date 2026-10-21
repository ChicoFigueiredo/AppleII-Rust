//! Mockingboard C, Phasor, MEGA Audio and SD Music (Mockingboard.cpp:1189-1521) with the 6522
//! (6522.cpp:612-738), AY8913 (AY8910.cpp:1032-1250) and SSI263 / SC-01 (SSI263.cpp:989-1161)
//! sub-states.

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_audio::ay8910::AyType;
use apple2_audio::ssi263::SsiType;
use apple2_audio::{AySnapshot, MockingboardCard, SsiSnapshot, Variant, ViaSnapshot};
use apple2_core::{Card, CardCtx};

/// `kUNIT_VERSION` (Mockingboard.cpp:1215).
const VERSION: u32 = 15;
/// `MockingboardUnitState_e::AY_INACTIVE` and `AY_READ`.
const AY_INACTIVE: u8 = 2;
const AY_READ: u8 = 3;
const PH_MOCKINGBOARD: u32 = 0;
const PH_PHASOR: u32 = 5;

macro_rules! per_variant {
    ($name:literal, $v:expr) => {
        Mapping {
            aw_names: &[$name],
            our_name: $name,
            save: |w, slot, c| save(w, slot, c, $v),
            load: |ctx, slot, ver, st| load(ctx, slot, ver, st, $name, $v),
        }
    };
}

pub const MOCKINGBOARD_C: Mapping = per_variant!("Mockingboard C", Variant::MockingboardC);
pub const PHASOR: Mapping = per_variant!("Phasor", Variant::Phasor);
pub const MEGA_AUDIO: Mapping = per_variant!("MEGA Audio", Variant::MegaAudio);
pub const SD_MUSIC: Mapping = per_variant!("SD Music", Variant::SdMusic);

// ---- save --------------------------------------------------------------------------------------

fn save(w: &mut Writer, slot: u8, card: &dyn Card, variant: Variant) -> bool {
    let Some(c) = card
        .as_any()
        .and_then(|a| a.downcast_ref::<MockingboardCard>())
    else {
        return false;
    };
    if c.variant() != variant {
        return false;
    }
    let s = c.snapshot();
    let phasor = variant == Variant::Phasor;
    w.slot(slot, card.name(), VERSION, |w| {
        if phasor {
            w.uint("Mode", s.phasor_mode as u64); // Mockingboard.cpp:1386
        }
        for (i, u) in s.units.iter().enumerate() {
            w.map(&format!("Unit{i}"), |w| {
                save_via(w, &u.via);
                if phasor {
                    save_ay(w, "-A", &u.ay[0]);
                    save_ay(w, "-B", &u.ay[1]);
                } else {
                    save_ay(w, "", &u.ay[0]);
                }
                save_ssi(w, &u.ssi, i == 0);
                w.hex4("Unit State", u.state[0] as u32);
                if phasor {
                    w.hex4("Unit State-B", u.state[1] as u32);
                }
                w.hex8("AY Current Register", u.ay_current_register[0] as u32);
                if phasor {
                    w.hex8("AY Current Register B", u.ay_current_register[1] as u32);
                }
                w.bool("Chip Select A", u.chip_selected[0]);
                if phasor {
                    w.bool("Chip Select B", u.chip_selected[1]);
                }
                w.bool("Reg Address Latch Valid A", u.latch_valid[0]);
                if phasor {
                    w.bool("Reg Address Latch Valid B", u.latch_valid[1]);
                }
                w.bool("Bus Driven by AY", u.bus_driven);
            });
        }
    });
    true
}

/// `SY6522::SaveSnapshot` (6522.cpp:634).
fn save_via(w: &mut Writer, v: &ViaSnapshot) {
    w.map("SY6522", |w| {
        w.hex8("ORB", v.orb as u32);
        w.hex8("ORA", v.ora as u32);
        w.hex8("DDRB", v.ddrb as u32);
        w.hex8("DDRA", v.ddra as u32);
        w.hex16("Timer1 Counter", v.t1_counter as u32);
        w.hex16("Timer1 Latch", v.t1_latch as u32);
        w.uint("Timer1 IRQ Delay", v.t1_irq_delay as u64);
        w.bool("Timer1 Active", v.t1_active);
        w.hex16("Timer2 Counter", v.t2_counter as u32);
        w.hex16("Timer2 Latch", v.t2_latch as u32);
        w.uint("Timer2 IRQ Delay", v.t2_irq_delay as u64);
        w.bool("Timer2 Active", v.t2_active);
        w.hex8("Serial Shift", v.serial_shift as u32);
        w.hex8("ACR", v.acr as u32);
        w.hex8("PCR", v.pcr as u32);
        w.hex8("IFR", v.ifr as u32);
        w.hex8("IER", v.ier as u32);
    });
}

fn ay_type_name(t: AyType) -> &'static str {
    match t {
        AyType::Empty => "Empty",
        AyType::Ay3_8910 => "AY-3-8910",
        AyType::Ay3_8912 => "AY-3-8912",
        AyType::Ay3_8913 => "AY-3-8913",
        AyType::Ym2149f => "YM2149F",
    }
}

/// `AY8913::SaveSnapshot` (AY8910.cpp:1098). No `Change` list: writes are applied immediately.
fn save_ay(w: &mut Writer, suffix: &str, a: &AySnapshot) {
    w.map(&format!("AY891x{suffix}"), |w| {
        w.string("Type", ay_type_name(a.ty));
        if a.ty == AyType::Empty {
            return;
        }
        for (i, v) in a.tone_tick.iter().enumerate() {
            w.uint(&format!("Tone{i} Tick"), *v as u64);
        }
        for (i, v) in a.tone_high.iter().enumerate() {
            w.uint(&format!("Tone{i} High"), *v as u64);
        }
        w.uint("Noise Tick", a.noise_tick as u64);
        w.uint("Tone Subcycles", a.tone_subcycles as u64);
        w.uint("Env Subcycles", a.env_subcycles as u64);
        w.uint("Env Internal Tick", a.env_internal_tick as u64);
        w.uint("Env Tick", a.env_tick as u64);
        w.uint("Tick Incr", a.tick_incr as u64);
        for (i, v) in a.tone_period.iter().enumerate() {
            w.uint(&format!("Tone{i} Period"), *v as u64);
        }
        w.uint("Noise Period", a.noise_period as u64);
        w.uint("RNG", a.rng as u64);
        w.uint("Noise Toggle", a.noise_toggle as u64);
        w.uint("Env First", a.env_first as u64);
        w.uint("Env Rev", a.env_rev as u64);
        w.uint("Env Counter", a.env_counter as u32 as u64);
        let r = &a.regs;
        w.map("Registers", |w| {
            for i in 0..3 {
                w.hex12(
                    &format!("Tone{i} Period"),
                    (r[2 * i + 1] as u32) << 8 | r[2 * i] as u32,
                );
            }
            w.hex8("Noise Period", r[6] as u32);
            w.hex8("Mixer", r[7] as u32);
            for i in 0..3 {
                w.hex8(&format!("Vol{i}"), r[8 + i] as u32);
            }
            w.hex16("Env Period", (r[12] as u32) << 8 | r[11] as u32);
            w.hex4("Env Shape", r[13] as u32);
            w.hex8("PortA", r[14] as u32);
            w.hex8("PortB", r[15] as u32);
        });
    });
}

/// `SSI263::SaveSnapshot` (SSI263.cpp:1005) and, for unit 0, `SC01_SaveSnapshot` (:1122).
fn save_ssi(w: &mut Writer, s: &SsiSnapshot, has_sc01_slot: bool) {
    w.map("SSI263", |w| {
        w.string(
            "Type",
            match s.ty {
                SsiType::Empty => "Empty",
                SsiType::P => "SSI263P",
                SsiType::Ap => "SSI263AP",
            },
        );
        if s.ty != SsiType::Empty {
            w.hex8("Duration / Phoneme", s.duration_phoneme as u32);
            w.hex8("Inflection", s.inflection as u32);
            w.hex8("Rate / Inflection", s.rate_inflection as u32);
            w.hex8("Control / Articulation / Amplitude", s.ctrl_art_amp as u32);
            w.hex8("Filter Frequency", s.filter_freq as u32);
            w.hex8("Current Mode", s.current_mode as u32);
        }
    });
    if has_sc01_slot {
        w.map("SC01", |w| {
            w.string("Type", if s.has_sc01 { "SC01" } else { "Empty" });
            if s.has_sc01 {
                w.hex8("SC01 Phoneme", s.votrax_phoneme as u32);
                w.bool("SC01 Active Phoneme", s.is_votrax_phoneme);
            }
        });
    }
}

// ---- load --------------------------------------------------------------------------------------

fn load(
    ctx: &mut LoadCtx,
    slot: u8,
    version: u32,
    st: Map,
    name: &str,
    variant: Variant,
) -> Result<Option<Box<dyn Card>>, Error> {
    if slot == 0 || slot == 3 {
        return Err(Error::Format("Card: wrong slot".into()));
    }
    if !(1..=VERSION).contains(&version) {
        return Err(Error::Format("Card: wrong version".into()));
    }
    let Some(mut card) = make(ctx, name, slot) else {
        return Ok(None);
    };
    let Some(c) = card
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<MockingboardCard>())
    else {
        return Err(Error::Unsupported(format!(
            "host returned a card that is not a '{name}'"
        )));
    };
    let phasor = variant == Variant::Phasor;
    let mut s = c.snapshot();

    if phasor {
        // Phasor_LoadSnapshot (Mockingboard.cpp:1413).
        if version < 6 {
            st.u32("Clock Scale Factor")?; // consumed
        }
        let mut mode = st.u32("Mode")?;
        if version < 6 {
            mode = if mode == 0 {
                PH_MOCKINGBOARD
            } else {
                PH_PHASOR
            };
        }
        s.phasor_mode = mode as u8;
    }
    if (6..=12).contains(&version) {
        s.units[0].ssi.is_votrax_phoneme = st.bool("Votrax Phoneme")?; // SC01 only in unit 0
    }

    for unit in 0..2usize {
        let key = format!("Unit{unit}");
        let u = st
            .sub(&key)
            .ok_or_else(|| Error::Format(format!("Card: Expected key: {key}")))?;
        load_via(u, version, &mut s.units[unit].via)?;
        if phasor {
            // The AY pairs were swapped in versions 5-9 (Mockingboard.cpp:1451-1467).
            for (suffix, k) in [("-A", 0usize), ("-B", 1)] {
                let (sub, dev) = match version {
                    5..=8 => (k, if unit == 0 { 1 } else { 0 }),
                    1..=4 | 9 => (k, if unit == 0 { 0 } else { 1 }),
                    _ => (unit, k),
                };
                let base = s.units[sub].ay[dev].clone();
                s.units[sub].ay[dev] = load_ay(u, suffix, version, base)?;
            }
        } else {
            let base = s.units[unit].ay[0].clone();
            s.units[unit].ay[0] = load_ay(u, "", version, base)?;
        }
        load_ssi(u, version, unit == 0, &mut s.units[unit].ssi)?;

        let us = &mut s.units[unit];
        us.ay_current_register[0] = u.u32("AY Current Register")? as u8;
        if version >= 10 && phasor {
            us.ay_current_register[1] = u.u32("AY Current Register B")? as u8;
        }
        if version == 1 {
            // SY6522::StartTimer1_LoadStateV1 (6522.cpp:72): Timer1 is restarted only if its
            // interrupt is enabled; Timer2 is never restarted (GH#309 fixture Cybernoid).
            us.via.t1_active = us.via.ier & 0x40 != 0;
            us.via.t2_active = false;
        } else if (2..=7).contains(&version) {
            us.via.t1_active = u.bool("Timer1 Active")?;
            us.via.t2_active = u.bool("Timer2 Active")?;
        }
        if version <= 7 {
            // Redundant data, but AppleWin requires the keys.
            u.bool("Timer1 IRQ Pending")?;
            u.bool("Timer2 IRQ Pending")?;
            u.bool("Speech IRQ Pending")?;
        }
        us.state = [AY_INACTIVE; 2];
        if version >= 3 {
            us.state[0] = (u.u32("Unit State")? & 7) as u8;
        }
        if phasor && version >= 5 {
            us.state[1] = (u.u32("Unit State-B")? & 7) as u8;
        }
        if version >= 10 {
            us.chip_selected[0] = u.bool("Chip Select A")?;
            us.latch_valid[0] = u.bool("Reg Address Latch Valid A")?;
            if phasor {
                us.chip_selected[1] = u.bool("Chip Select B")?;
                us.latch_valid[1] = u.bool("Reg Address Latch Valid B")?;
            }
        }
        us.bus_driven = us.state[0] == AY_READ || (phasor && us.state[1] == AY_READ);
        if version >= 11 {
            us.bus_driven = u.bool("Bus Driven by AY")?;
        }
    }
    c.apply_snapshot(&s);
    Ok(Some(card))
}

/// `SY6522::LoadSnapshot` (6522.cpp:660).
fn load_via(unit: Map, version: u32, v: &mut ViaSnapshot) -> Result<(), Error> {
    let m = unit
        .sub("SY6522")
        .ok_or_else(|| Error::Format("Card: Expected key: SY6522".into()))?;
    v.orb = m.u32("ORB")? as u8;
    v.ora = m.u32("ORA")? as u8;
    v.ddrb = m.u32("DDRB")? as u8;
    v.ddra = m.u32("DDRA")? as u8;
    v.t1_counter = m.u32("Timer1 Counter")? as u16;
    v.t1_latch = m.u32("Timer1 Latch")? as u16;
    v.t2_counter = m.u32("Timer2 Counter")? as u16;
    v.t2_latch = m.u32("Timer2 Latch")? as u16;
    v.serial_shift = m.u32("Serial Shift")? as u8;
    v.acr = m.u32("ACR")? as u8;
    v.pcr = m.u32("PCR")? as u8;
    v.ifr = m.u32("IFR")? as u8;
    v.ier = m.u32("IER")? as u8;
    v.t1_irq_delay = false;
    v.t2_irq_delay = false;
    if version >= 4 {
        v.t1_irq_delay = m.u32("Timer1 IRQ Delay")? != 0;
        v.t2_irq_delay = m.u32("Timer2 IRQ Delay")? != 0;
    }
    if version < 7 && v.t1_latch == 0 {
        v.t1_latch = 0xFFFF; // allow Mockingboard detection to succeed
    }
    if version >= 8 {
        v.t1_active = m.bool("Timer1 Active")?;
        v.t2_active = m.bool("Timer2 Active")?;
    }
    Ok(())
}

/// `AY8913::LoadSnapshot` (AY8910.cpp:1157); `Change` is ignored.
fn load_ay(unit: Map, suffix: &str, version: u32, mut a: AySnapshot) -> Result<AySnapshot, Error> {
    let key = format!(
        "{}{suffix}",
        if version >= 15 { "AY891x" } else { "AY8910" }
    );
    let m = unit
        .sub(&key)
        .ok_or_else(|| Error::Format(format!("Card: Expected key: {key}")))?;
    a.ty = AyType::Ay3_8913;
    if version >= 15 {
        a.ty = match m.string("Type")?.as_str() {
            "Empty" => AyType::Empty,
            "AY-3-8910" => AyType::Ay3_8910,
            "AY-3-8912" => AyType::Ay3_8912,
            "YM2149F" => AyType::Ym2149f,
            _ => AyType::Ay3_8913,
        };
        if a.ty == AyType::Empty {
            return Ok(a);
        }
    }
    for i in 0..3 {
        a.tone_tick[i] = m.u32(&format!("Tone{i} Tick"))?;
    }
    for i in 0..3 {
        a.tone_high[i] = m.u32(&format!("Tone{i} High"))? != 0;
    }
    a.noise_tick = m.u32("Noise Tick")?;
    a.tone_subcycles = m.u32("Tone Subcycles")?;
    a.env_subcycles = m.u32("Env Subcycles")?;
    a.env_internal_tick = m.u32("Env Internal Tick")?;
    a.env_tick = m.u32("Env Tick")?;
    a.tick_incr = m.u32("Tick Incr")?;
    for i in 0..3 {
        a.tone_period[i] = m.u32(&format!("Tone{i} Period"))?;
    }
    a.noise_period = m.u32("Noise Period")?;
    a.rng = m.u32("RNG")?;
    a.noise_toggle = m.u32("Noise Toggle")? != 0;
    a.env_first = m.u32("Env First")? != 0;
    a.env_rev = m.u32("Env Rev")? != 0;
    a.env_counter = m.u32("Env Counter")? as i32;
    let r = m
        .sub("Registers")
        .ok_or_else(|| Error::Format("Card: Expected key: Registers".into()))?;
    for i in 0..3 {
        let p = r.u32(&format!("Tone{i} Period"))? as u16;
        a.regs[2 * i] = p as u8;
        a.regs[2 * i + 1] = ((p >> 8) & 0xF) as u8;
    }
    a.regs[6] = r.u32("Noise Period")? as u8;
    a.regs[7] = r.u32("Mixer")? as u8;
    for i in 0..3 {
        a.regs[8 + i] = r.u32(&format!("Vol{i}"))? as u8;
    }
    let p = r.u32("Env Period")? as u16;
    a.regs[11] = p as u8;
    a.regs[12] = (p >> 8) as u8;
    a.regs[13] = r.u32("Env Shape")? as u8;
    a.regs[14] = r.u32("PortA")? as u8;
    a.regs[15] = r.u32("PortB")? as u8;
    Ok(a)
}

/// `SSI263::LoadSnapshot` (SSI263.cpp:1032) and, for unit 0, `SC01_LoadSnapshot` (:1136).
fn load_ssi(unit: Map, version: u32, unit0: bool, s: &mut SsiSnapshot) -> Result<(), Error> {
    let m = unit
        .sub("SSI263")
        .ok_or_else(|| Error::Format("Card: Expected key: SSI263".into()))?;
    s.ty = SsiType::P; // default prior to v14
    if version >= 14 {
        s.ty = match m.string("Type")?.as_str() {
            "Empty" => SsiType::Empty,
            "SSI263P" => SsiType::P,
            _ => SsiType::Ap,
        };
    }
    if s.ty != SsiType::Empty {
        s.duration_phoneme = m.u32("Duration / Phoneme")? as u8;
        s.inflection = m.u32("Inflection")? as u8;
        s.rate_inflection = m.u32("Rate / Inflection")? as u8;
        s.ctrl_art_amp = m.u32("Control / Articulation / Amplitude")? as u8;
        s.filter_freq = m.u32("Filter Frequency")? as u8;
        s.current_mode = m.u32("Current Mode")? as u8;
        if (7..13).contains(&version) {
            m.bool("Active Phoneme")?; // consumed
        }
        if version < 12 {
            if s.current_mode >> 6 == 0 {
                // Invalid function, but older versions accepted it: the usual one, no interrupts.
                s.current_mode = (s.current_mode & 0x1F) | 3 << 6;
            } else {
                s.current_mode |= 0x20;
            }
        }
    }
    if unit0 {
        if version < 12 {
            s.votrax_phoneme = 0;
        } else {
            let c = unit
                .sub("SC01")
                .ok_or_else(|| Error::Format("Card: Expected key: SC01".into()))?;
            s.has_sc01 = if version >= 14 {
                c.string("Type")? == "SC01"
            } else {
                true
            };
            if s.has_sc01 {
                s.votrax_phoneme = c.u32("SC01 Phoneme")? as u8;
                s.is_votrax_phoneme = c.bool("SC01 Active Phoneme")?;
            }
        }
    }
    Ok(())
}

/// Re-creates what the AppleWin units cannot hold: the pending 6522 timer events, a speech
/// phoneme restarted from its beginning and the IRQ lines. Called after the machine state is
/// loaded without the extension block.
pub fn rearm(card: &mut dyn Card, ctx: &mut CardCtx) {
    if let Some(c) = card
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<MockingboardCard>())
    {
        c.rearm_events(ctx);
    }
}
