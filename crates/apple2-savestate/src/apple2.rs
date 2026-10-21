//! `Unit: Apple2` and `Unit: Auxiliary Slot` (SaveState.cpp:Snapshot_SaveState, CPU.cpp:932,
//! Joystick.cpp:1063, Keyboard.cpp:471, Speaker.cpp:924, Video.cpp:305, Memory.cpp:2757-3030)
//! and `Unit: Slots`' RAM cards (LanguageCard.cpp:275, :492).

use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::language_card::{LcInfo, LcKind};
use apple2_core::memory::mf;
use apple2_core::slots::ExpansionRom;
use apple2_core::{AuxCardKind, MachineState, Model, VideoStandard};

/// `UNIT_APPLE2_VER` (SaveState.cpp:67).
pub const UNIT_APPLE2_VER: u32 = 9;
/// `kUNIT_AUXSLOT_VER` / `kUNIT_CARD_VER` (Memory.cpp:2757-2763).
pub const UNIT_AUXSLOT_VER: u32 = 2;
pub const AUX_CARD_VER: u32 = 4;
pub const LC_CARD_VER: u32 = 1;
pub const SATURN_CARD_VER: u32 = 1;

const LC_SIZE: usize = 0x4000;
const LC_MASK: u32 = mf::LANGCARD_MASK;

// Video.h:54-60.
const VF_80COL: u32 = 0x01;
const VF_DHIRES: u32 = 0x02;
const VF_HIRES: u32 = 0x04;
const VF_80STORE: u32 = 0x08;
const VF_MIXED: u32 = 0x10;
const VF_PAGE2: u32 = 0x20;
const VF_TEXT: u32 = 0x40;
const VF_80COL_AUX_EMPTY: u32 = 0x100;

/// SaveState.cpp `SS_YAML_VALUE_*`.
pub fn model_name(m: Model) -> &'static str {
    match m {
        Model::Apple2 => "Apple][",
        Model::Apple2Plus => "Apple][+",
        Model::Apple2JPlus => "Apple][ J-Plus",
        Model::Apple2e => "Apple//e",
        Model::Apple2eEnhanced => "Enhanced Apple//e",
        Model::Apple2c => "Apple2c",
        // AppleWin cannot save the "bad" tags; they are the same machines.
        Model::Pravets82 | Model::BadPravets82 => "Pravets82",
        Model::Pravets8M | Model::BadPravets8M => "Pravets8M",
        Model::Pravets8A => "Pravets8A",
        Model::Tk3000e => "TK3000//e",
        Model::Base64A => "Base 64A",
    }
}

pub fn parse_model(s: &str) -> Result<Model, Error> {
    Ok(match s {
        "Apple][" => Model::Apple2,
        "Apple][+" => Model::Apple2Plus,
        "Apple][ J-Plus" => Model::Apple2JPlus,
        "Apple//e" => Model::Apple2e,
        "Enhanced Apple//e" => Model::Apple2eEnhanced,
        "Apple2c" => Model::Apple2c,
        "Pravets82" => Model::Pravets82,
        "Pravets8M" => Model::Pravets8M,
        "Pravets8A" => Model::Pravets8A,
        "TK3000//e" => Model::Tk3000e,
        "Base 64A" => Model::Base64A,
        _ => return Err(Error::Format("Load: Unknown Apple2 type".into())),
    })
}

fn video_flags(st: &MachineState) -> u32 {
    let (v, mm) = (&st.video, st.memory.memmode);
    let mut f = 0;
    let mut set = |on: bool, bit: u32| {
        if on {
            f |= bit
        }
    };
    set(v.col80, VF_80COL);
    set(v.dhires, VF_DHIRES);
    set(mm & mf::HIRES != 0, VF_HIRES);
    set(mm & mf::STORE80 != 0, VF_80STORE);
    set(v.mixed, VF_MIXED);
    set(mm & mf::PAGE2 != 0, VF_PAGE2);
    set(v.text, VF_TEXT);
    set(
        st.config.aux_card == AuxCardKind::Empty && !st.config.model.is_apple2() && v.col80,
        VF_80COL_AUX_EMPTY,
    );
    f
}

/// Writes `Unit: Apple2`. `extra` runs last inside `State` (the Rust extension block).
pub fn save_apple2(
    w: &mut Writer,
    st: &MachineState,
    lcs: &[LcInfo],
    extra: impl FnOnce(&mut Writer),
) {
    let model = st.config.model;
    let is2 = model.is_apple2();
    w.unit("Apple2", UNIT_APPLE2_VER, |w| {
        w.line(&format!("Model: {}", model_name(model)));
        // CPU.cpp:932
        w.map("CPU", |w| {
            w.string(
                "Type",
                if model.cpu_variant() == apple2_cpu_variant_cmos() {
                    "65C02"
                } else {
                    "6502"
                },
            );
            w.hex8("A", st.cpu.a as u32);
            w.hex8("X", st.cpu.x as u32);
            w.hex8("Y", st.cpu.y as u32);
            w.hex8("P", (st.cpu.p | 0x30) as u32);
            w.hex8("S", st.cpu.sp as u32);
            w.hex16("PC", st.cpu.pc as u32);
            w.hex64("Cumulative Cycles", st.cycles);
            w.bool("Defer IRQ By 1 Opcode", st.irq_defer_1_opcode);
        });
        // Joystick.cpp:1063
        w.map("Joystick", |w| {
            w.int("Joystick0 TrimX", 0);
            w.int("Joystick0 TrimY", 0);
            w.line("Joystick1 TrimX: 0 # not implemented yet");
            w.line("Joystick1 TrimY: 0 # not implemented yet");
            for (n, c) in st.game.inactive_cycles().iter().enumerate() {
                w.hex64(&format!("Paddle{n} Inactive Cycle"), *c);
            }
        });
        // Keyboard.cpp:471
        w.map("Keyboard", |w| {
            let (key, waiting) = st.keyboard.latch();
            w.hex8("Last Key", key as u32);
            w.bool("Key Waiting", waiting);
        });
        // Speaker.cpp:924
        w.map("Speaker", |w| {
            w.hex64("Last Cycle", st.speaker.last_toggle().unwrap_or(st.cycles));
        });
        // Video.cpp:305
        w.map("Video", |w| {
            w.bool("Alt Char Set", st.video.altcharset);
            w.hex32("Video Mode", video_flags(st));
            w.uint("Cycles This Frame", st.cycles_in_frame as u64);
            w.uint(
                "Video Refresh Rate",
                if st.config.video_standard == VideoStandard::Pal {
                    1
                } else {
                    2
                },
            );
        });
        // Memory.cpp:2840
        w.map("Memory", |w| {
            let mm = st.memory.memmode;
            w.hex32("Memory Mode", mm & !LC_MASK);
            if !is2 {
                w.hex32("MMU LC Mode", mm & LC_MASK);
                let lrw = lcs
                    .iter()
                    .find(|c| c.slot == 0)
                    .is_some_and(|c| c.last_ram_write);
                w.uint("Last RAM Write", lrw as u64);
            }
            w.hex8("IO_SELECT", st.slots.io_select as u32);
            w.hex8("IO_SELECT_InternalROM", st.slots.intc8rom as u32);
            let (ty, slot) = match st.slots.expansion {
                ExpansionRom::Null => (0, 0),
                ExpansionRom::Internal => (1, 0),
                ExpansionRom::Slot(s) => (2, s as u32),
            };
            w.uint("Expansion ROM Type", ty);
            w.uint("Peripheral ROM Slot", slot as u64);
            // LanguageCardManager::GetLastSlotToSetMainMemLC
            w.uint("Last Slot to Set Main Mem LC", st.lc.last_slot() as u64);
            for (i, a) in st.annunciators.iter().enumerate() {
                w.bool(&format!("Annunciator{i}"), *a);
            }
        });
        w.map("Main Memory", |w| {
            let m = &st.memory;
            if is2 {
                w.memory(&m.main[..0xC000]);
            } else {
                let mut img = m.main[..0xC000].to_vec();
                img.extend_from_slice(&m.lc[..LC_SIZE]);
                w.memory(&img);
            }
        });
        extra(w);
    });
}

/// `MemSaveSnapshotAux` (Memory.cpp:2930). Nothing for the ][ family.
pub fn save_aux(w: &mut Writer, st: &MachineState) {
    if st.config.model.is_apple2() {
        return;
    }
    w.unit("Auxiliary Slot", UNIT_AUXSLOT_VER, |w| {
        let (name, _) = match st.config.aux_card {
            AuxCardKind::Empty => ("Empty", 0),
            AuxCardKind::Col80_1K => ("80 Column", 0),
            AuxCardKind::Extended80Col => ("Extended 80 Column", 0),
            AuxCardKind::RamWorksIII { .. } => ("RamWorks III", 0),
        };
        w.line(&format!("Card: {name}"));
        w.line(&format!("Version: {AUX_CARD_VER}"));
        let aux = &st.memory.aux;
        match st.config.aux_card {
            AuxCardKind::Empty => {}
            AuxCardKind::Col80_1K => w.map("State", |w| {
                w.map("Auxiliary Memory Bank", |w| w.memory(&aux[..0x400]))
            }),
            kind => w.map("State", |w| {
                let banks = aux.len() / 0x10000;
                w.line(&format!(
                    "Num Aux Banks: 0x{banks:03X}  # [0,1..100] 0=no aux mem, 1=128K system, etc"
                ));
                let active = if matches!(kind, AuxCardKind::RamWorksIII { .. }) {
                    st.memory.aux_bank
                } else {
                    0
                };
                w.line(&format!(
                    "Active Aux Bank: 0x{active:02X} # [  0..FF] 0=memaux"
                ));
                for (b, bank) in aux.chunks(0x10000).enumerate() {
                    w.map(&format!("Auxiliary Memory Bank{b:02X}"), |w| w.memory(bank));
                }
                // RGBMonitor.cpp:1311 (state of the AppleColor RGB adaptor; not modelled)
                w.map("AppleColor RGB Adaptor", |w| {
                    w.hex8("RGB mode flags", 0);
                    w.hex8("RGB mode", 0);
                    w.hex8("Previous AN3", 0);
                    w.bool("80COL changed", false);
                    w.bool("Invert bit7", false);
                });
            }),
        }
    });
}

/// Language Card / Saturn entries of `Unit: Slots` for one card.
pub fn save_lc_card(w: &mut Writer, st: &MachineState, c: &LcInfo) {
    let lrw = c.last_ram_write as u64;
    let mem = &st.memory.lc;
    match c.kind {
        LcKind::Builtin => {} // "CT_LanguageCardIIe" is not a card
        LcKind::Card16K => w.slot(c.slot, "Language Card", LC_CARD_VER, |w| {
            w.hex32("Memory Mode", c.memmode);
            w.uint("Last RAM Write", lrw);
            w.map("Memory Bank", |w| {
                w.memory(&mem[c.region * LC_SIZE..][..LC_SIZE])
            });
        }),
        LcKind::Saturn { banks, active } => w.slot(c.slot, "Saturn 128", SATURN_CARD_VER, |w| {
            w.hex32("Memory Mode", c.memmode);
            w.uint("Last RAM Write", lrw);
            w.line(&format!(
                "Num Saturn Banks: 0x{banks:02X}   # [1..8] 4=64K, 8=128K card"
            ));
            w.line(&format!("Active Saturn Bank: 0x{active:02X} # [0..7]"));
            let base = c.region - active as usize;
            for b in 0..banks as usize {
                w.map(&format!("Memory Bank{b:02X}"), |w| {
                    w.memory(&mem[(base + b) * LC_SIZE..][..LC_SIZE])
                });
            }
        }),
    }
}

fn apple2_cpu_variant_cmos() -> apple2_cpu::Variant {
    apple2_cpu::Variant::Cmos65C02
}

// ---------------------------------------------------------------------------------------------
// Loading

/// What the aux unit says (`MemLoadSnapshotAuxCommon`, Memory.cpp:2985).
pub struct AuxUnit<'a> {
    pub kind: AuxCardKind,
    pub active_bank: u16,
    pub state: Option<Map<'a>>,
}

pub fn parse_aux<'a>(unit_version: u32, st: Map<'a>) -> Result<AuxUnit<'a>, Error> {
    if !(1..=UNIT_AUXSLOT_VER).contains(&unit_version) {
        return Err(Error::Format("Unit: AuxSlot: Version mismatch".into()));
    }
    let card = st.string("Card")?;
    // v2 has the card version here and a nested State (absent for an empty slot).
    let inner = if unit_version == 1 {
        st
    } else if card != "Empty" {
        st.need_sub("State")?
    } else {
        st
    };
    Ok(match card.as_str() {
        "Empty" => AuxUnit {
            kind: AuxCardKind::Empty,
            active_bank: 0,
            state: None,
        },
        "80 Column" => AuxUnit {
            kind: AuxCardKind::Col80_1K,
            active_bank: 0,
            state: Some(inner),
        },
        "Extended 80 Column" | "RamWorks III" | "RamWorksIII" => {
            let banks = inner.u32("Num Aux Banks")?;
            let active = inner.u32("Active Aux Bank")?;
            let ext = card == "Extended 80 Column";
            if ext && (banks != 1 || active != 0)
                || !ext && (!(2..=256).contains(&banks) || active + 1 > banks)
            {
                return Err(Error::Format(
                    "Unit: AuxSlot: Bad aux slot card state".into(),
                ));
            }
            let kind = if ext {
                AuxCardKind::Extended80Col
            } else {
                AuxCardKind::RamWorksIII {
                    banks: banks as u16,
                }
            };
            AuxUnit {
                kind,
                active_bank: active as u16,
                state: Some(inner),
            }
        }
        other => {
            return Err(Error::Format(format!(
                "Unit: AuxSlot: Unknown card: {other}"
            )))
        }
    })
}

/// Fills the aux RAM of `st` from the unit.
pub fn load_aux_memory(st: &mut MachineState, aux: &AuxUnit) -> Result<(), Error> {
    let Some(s) = aux.state else { return Ok(()) };
    let mem = &mut st.memory;
    match aux.kind {
        AuxCardKind::Empty => {}
        AuxCardKind::Col80_1K => {
            let sub = s.need_sub("Auxiliary Memory Bank")?;
            sub.load_memory(&mut mem.aux[..0x400])?;
        }
        AuxCardKind::Extended80Col | AuxCardKind::RamWorksIII { .. } => {
            for (b, bank) in mem.aux.chunks_mut(0x10000).enumerate() {
                let name = format!("Auxiliary Memory Bank{b:02X}");
                let sub = s
                    .sub(&name)
                    .ok_or_else(|| Error::Format(format!("Memory: Missing map name: {name}")))?;
                sub.load_memory(bank)?;
            }
            mem.aux_bank = aux.active_bank;
        }
    }
    Ok(())
}

/// Result of reading `Unit: Apple2`'s registers; the rest is applied straight onto the state.
pub struct Apple2Unit<'a> {
    pub version: u32,
    pub model: Model,
    pub state: Map<'a>,
    pub video_standard: VideoStandard,
}

pub fn parse_apple2<'a>(version: u32, state: Map<'a>) -> Result<Apple2Unit<'a>, Error> {
    if version == 0 || version > UNIT_APPLE2_VER {
        return Err(Error::Format("Unit: Apple2: Version mismatch".into()));
    }
    let model = parse_model(&state.string("Model")?)?;
    let mut video_standard = VideoStandard::Ntsc;
    if let Some(v) = state.sub("Video") {
        if version >= 4 && v.u32("Video Refresh Rate")? == 1 {
            video_standard = VideoStandard::Pal;
        }
    }
    Ok(Apple2Unit {
        version,
        model,
        state,
        video_standard,
    })
}

/// LC cards the Slots unit declared, with their parsed state (applied by [`apply_apple2`]).
pub struct LcLoad {
    pub slot: u8,
    pub memmode: u32,
    pub last_ram_write: bool,
    pub active_bank: Option<u8>,
    /// 16K regions in bank order.
    pub banks: Vec<Vec<u8>>,
}

pub fn parse_lc_card(slot: u8, saturn: bool, version: u32, st: Map) -> Result<LcLoad, Error> {
    let want = if saturn { SATURN_CARD_VER } else { LC_CARD_VER };
    if version != want {
        return Err(Error::Format(format!(
            "Version {version} is not supported for card '{}'.",
            if saturn {
                "Saturn 128"
            } else {
                "Language Card"
            }
        )));
    }
    let memmode = st.u32("Memory Mode")? & LC_MASK;
    let last_ram_write = st.u32("Last RAM Write")? != 0;
    let mut out = LcLoad {
        slot,
        memmode,
        last_ram_write,
        active_bank: None,
        banks: Vec::new(),
    };
    let read = |name: &str| -> Result<Vec<u8>, Error> {
        let sub = st
            .sub(name)
            .ok_or_else(|| Error::Format(format!("Memory: Missing map name: {name}")))?;
        let mut b = vec![0u8; LC_SIZE];
        sub.load_memory(&mut b)?;
        Ok(b)
    };
    if saturn {
        let n = st.u32("Num Saturn Banks")?;
        let active = st.u32("Active Saturn Bank")?;
        if !(1..=8).contains(&n) || active >= n {
            return Err(Error::Format("Unit: Bad Saturn card state".into()));
        }
        out.active_bank = Some(active as u8);
        for b in 0..n {
            out.banks.push(read(&format!("Memory Bank{b:02X}"))?);
        }
    } else {
        out.banks.push(read("Memory Bank")?);
    }
    Ok(out)
}

/// Patches `st` (a fresh state of a machine built for this file) with `Unit: Apple2`, the aux
/// memory and the LC cards, like `ParseUnitApple2` + `MemInitializeFromSnapshot`.
pub fn apply_apple2(
    st: &mut MachineState,
    unit: &Apple2Unit,
    lcs: &[LcLoad],
    warnings: &mut Vec<String>,
) -> Result<(), Error> {
    let s = unit.state;
    let version = unit.version;
    let is2 = st.config.model.is_apple2();

    // CPU (CPU.cpp:948)
    if let Some(c) = s.sub("CPU") {
        let ty = c.string("Type")?;
        let cmos = st.config.model.cpu_variant() == apple2_cpu_variant_cmos();
        match ty.as_str() {
            "6502" | "65C02" => {
                if (ty == "65C02") != cmos {
                    warnings.push(format!(
                        "CPU: file says {ty}, which differs from the {} the model uses",
                        if cmos { "65C02" } else { "6502" }
                    ));
                }
            }
            _ => return Err(Error::Format("Load: Unknown main CPU type".into())),
        }
        st.cpu.a = c.u32("A")? as u8;
        st.cpu.x = c.u32("X")? as u8;
        st.cpu.y = c.u32("Y")? as u8;
        // CPU.cpp:963: `| (AF_RESERVED | AF_BREAK)`, so B reads as 1 like in AppleWin.
        st.cpu.p = (c.u32("P")? as u8) | 0x30;
        st.cpu.sp = c.u32("S")? as u8;
        st.cpu.pc = c.u32("PC")? as u16;
        st.cpu.irq = false;
        st.cpu.nmi_pending = false;
        let cum = c.u64("Cumulative Cycles")?;
        st.cpu.cycles = cum;
        st.cycles = cum;
        if version >= 5 {
            st.irq_defer_1_opcode = c.bool("Defer IRQ By 1 Opcode")?;
        }
    }
    // Joystick (Joystick.cpp:1078)
    if let Some(j) = s.sub("Joystick") {
        j.i32("Joystick0 TrimX")?;
        j.i32("Joystick0 TrimY")?;
        j.i32("Joystick1 TrimX")?;
        j.i32("Joystick1 TrimY")?;
        let mut c = [0u64; 4];
        for (n, slot) in c.iter_mut().enumerate() {
            *slot = if version >= 7 {
                j.u64(&format!("Paddle{n} Inactive Cycle"))?
            } else {
                j.u64("Counter Reset Cycle")?
            };
        }
        st.game.set_inactive_cycles(c);
    }
    // Keyboard (Keyboard.cpp:478)
    if let Some(k) = s.sub("Keyboard") {
        let key = k.u32("Last Key")? as u8;
        let waiting = if version >= 2 {
            k.bool("Key Waiting")?
        } else {
            false
        };
        st.keyboard.set_latch(key, waiting);
    }
    // Speaker: "Last Cycle" has no counterpart (the toggle log starts empty).
    if let Some(sp) = s.sub("Speaker") {
        sp.u64("Last Cycle")?;
    }
    // Video (Video.cpp:314)
    if let Some(v) = s.sub("Video") {
        st.video.altcharset = v.bool("Alt Char Set")?;
        let mode = v.u32("Video Mode")?;
        st.video.text = mode & VF_TEXT != 0;
        st.video.mixed = mode & VF_MIXED != 0;
        st.video.col80 = mode & VF_80COL != 0;
        st.video.dhires = mode & VF_DHIRES != 0;
        st.cycles_in_frame = v.u32("Cycles This Frame")?;
        // HIRES / PAGE2 / 80STORE live in the memory flags, which "Memory Mode" sets below.
    }
    // Memory (Memory.cpp:2850)
    let m = s.need_sub("Memory")?;
    st.slots.io_select = m.u32("IO_SELECT")? as u8;
    st.slots.intc8rom = m.u32("IO_SELECT_InternalROM")? != 0;
    let ty = m.u32("Expansion ROM Type")?;
    let rom_slot = m.u32("Peripheral ROM Slot")? as u8;
    st.slots.expansion = match ty {
        0 => ExpansionRom::Null,
        1 => ExpansionRom::Internal,
        _ => ExpansionRom::Slot(rom_slot.min(7)),
    };
    let mut mode = m.u32("Memory Mode")?;
    if version == 1 {
        mode ^= mf::INTCXROM; // SLOTCXROM -> INTCXROM
    }
    // All flags but the LC bits, which come from the card that last set them (below).
    let mut memmode = mode & !LC_MASK;
    let mut mmu_lc = mode & LC_MASK;
    let mut builtin_lrw = false;
    if !is2 {
        if version >= 9 {
            mmu_lc = m.u32("MMU LC Mode")?;
        }
        builtin_lrw = m.u32("Last RAM Write")? != 0;
    }
    if version >= 3 {
        for i in 0..4 {
            st.annunciators[i] = m.bool(&format!("Annunciator{i}"))?;
        }
    }
    let last_slot = if version >= 9 {
        m.u32("Last Slot to Set Main Mem LC")? as u8
    } else {
        0
    };

    // Main memory (Memory.cpp:2905)
    let mm = s.need_sub("Main Memory")?;
    let mut img = vec![0u8; 0x10000];
    mm.load_memory(&mut img)?;
    st.memory.main[..0xC000].copy_from_slice(&img[..0xC000]);
    let lc_bytes = st.memory.lc.len();
    st.memory.lc.fill(0);
    // The top 16K of the //e image is the built-in card's RAM (region 0).
    if !is2 && lc_bytes >= LC_SIZE {
        st.memory.lc[..LC_SIZE].copy_from_slice(&img[0xC000..]);
    }

    // LC cards: the card entries own their RAM and flags.
    let infos = st.lc.infos();
    let mut last_flags = None;
    for info in &infos {
        let load = lcs.iter().find(|l| l.slot == info.slot);
        let (flags, lrw, active) = match load {
            Some(l) => (l.memmode, l.last_ram_write, l.active_bank),
            None if info.kind == LcKind::Builtin => (mmu_lc, builtin_lrw, None),
            None => (info.memmode, info.last_ram_write, None),
        };
        st.lc.set_info(info.slot, flags, lrw, active);
        if let Some(l) = load {
            let base = st
                .lc
                .infos()
                .iter()
                .find(|i| i.slot == info.slot)
                .map_or(0, |i| i.region - active.unwrap_or(0) as usize);
            for (b, data) in l.banks.iter().enumerate() {
                let off = (base + b) * LC_SIZE;
                if off + LC_SIZE <= st.memory.lc.len() {
                    st.memory.lc[off..off + LC_SIZE].copy_from_slice(data);
                }
            }
        }
        if info.slot == last_slot {
            last_flags = Some(flags);
        }
    }
    st.lc.set_last_slot(last_slot);
    let infos = st.lc.infos();
    let owner = infos
        .iter()
        .find(|i| i.slot == last_slot)
        .or_else(|| infos.first());
    if let Some(o) = owner {
        st.memory.lc_region = o.region;
        memmode |= last_flags.unwrap_or(o.memmode) & LC_MASK;
    }
    st.memory.memmode = memmode;
    Ok(())
}
