//! Mockingboard family as a [`Card`]: Mockingboard C, Phasor, MEGA Audio and SD Music
//! (AppleWin Mockingboard.cpp, MockingboardCard Manager mixing in MockingboardCardManager.cpp).
//!
//! Two 6522s at `$Cs00` (left) and `$Cs80` (right), each wired to an AY-3-8913 (Phasor: two) and
//! an SSI263/SC-01 speech pair. The 6522 timers raise IRQ through scheduler events; audio is
//! synthesized lazily: before every register access the chips are rendered up to that Cycle, so
//! a register change lands on the right output sample (AppleWin stamps AY writes with a Cycle
//! offset and replays them, `_AYWriteReg`, cpp:1114).

use crate::ay8910::{Ay, AyType};
use crate::bus::{lock, AudioBus, CardVolume, Source};
use crate::clock::SampleClock;
use crate::ssi263::{Ssi263, SsiOut, SsiType, PH_ECHO_PLUS, PH_MOCKINGBOARD, PH_PHASOR};
use crate::via6522::{reg, TimerSched, Via};
use apple2_core::{Card, CardCtx, IrqSource};

/// Which card (Card.h `CT_MockingboardC`, `CT_Phasor`, `CT_MegaAudio`, `CT_SDMusic`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    MockingboardC,
    Phasor,
    MegaAudio,
    SdMusic,
}

/// Socket population (AppleWin reads it from the registry: Mockingboard.cpp:101-119).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sockets {
    /// SSI263 in each 6522 unit's speech socket (defaults: unit 0 empty, unit 1 SSI263AP).
    pub ssi263: [SsiType; 2],
    /// Votrax SC-01 beside unit 0 (`kSC01_Default`).
    pub sc01: bool,
    pub ay: AyType,
}

impl Default for Sockets {
    fn default() -> Self {
        Sockets { ssi263: [SsiType::Empty, SsiType::Ap], sc01: true, ay: AyType::Ay3_8913 }
    }
}

// AY8913_Write function codes `(BDIR << 2) | (BC2 << 1) | BC1` with BC2 hard-wired to 1.
const AY_INACTIVE: u8 = 2;
const AY_READ: u8 = 3;
const AY_WRITE: u8 = 6;
const AY_LATCH: u8 = 7;
/// Scheduler tokens: 0..=3 timers (`unit*2 + timer`), 4..=5 speech chips.
const TOKEN_SSI: u16 = 4;
const SAVE_VERSION: u8 = 1;

struct Sub {
    via: Via,
    ay: [Ay; 2],
    ssi: Ssi263,
    cur_reg: [u8; 2],
    state: [u8; 2],
    latch_valid: [bool; 2],
    chip_sel: [bool; 2],
    bus_driven: bool,
    speech_irq: bool,
}

impl Sub {
    fn set_bus(&mut self, driven: bool) {
        self.bus_driven = driven;
        self.via.bus_driven = driven;
    }

    /// `MB_SUBUNIT::Reset`.
    fn reset_unit(&mut self, variant: Variant) {
        self.cur_reg = [0; 2];
        self.state = [AY_INACTIVE; 2];
        self.latch_valid = [false; 2];
        self.chip_sel = [variant != Variant::Phasor, false];
        self.set_bus(false);
    }
}

pub struct MockingboardCard {
    variant: Variant,
    bus: AudioBus,
    src: Source,
    subs: [Sub; 2],
    phasor_mode: u8,
    slot: u8,
    // Audio generation: frame `k` is sampled at `clock.cycle_of_frame(k)`.
    clock: SampleClock,
    produced: u64,
    pending_ssi: Option<(usize, SsiOut)>,
}

impl MockingboardCard {
    pub fn new(variant: Variant, bus: &AudioBus) -> Self {
        Self::with_sockets(variant, bus, Sockets::default())
    }

    pub fn with_sockets(variant: Variant, bus: &AudioBus, sockets: Sockets) -> Self {
        let mega = variant == Variant::MegaAudio;
        let has_speech = matches!(variant, Variant::MockingboardC | Variant::Phasor);
        let mk = |i: usize| {
            let ssi = if has_speech {
                Ssi263::new(sockets.ssi263[i], i == 0 && sockets.sc01)
            } else {
                Ssi263::new(SsiType::Empty, false)
            };
            let mut sub = Sub {
                via: Via::new(mega),
                ay: [Ay::new(sockets.ay), Ay::new(sockets.ay)],
                ssi,
                cur_reg: [0; 2],
                state: [AY_INACTIVE; 2],
                latch_valid: [false; 2],
                chip_sel: [true, false],
                bus_driven: false,
                speech_irq: false,
            };
            sub.reset_unit(variant);
            sub
        };
        let mut card = MockingboardCard {
            variant,
            bus: bus.clone(),
            src: bus.new_source(),
            subs: [mk(0), mk(1)],
            phasor_mode: PH_MOCKINGBOARD,
            slot: 0,
            clock: bus.clock(),
            produced: 0,
            pending_ssi: None,
        };
        card.set_ay_clocks();
        for s in &mut card.subs {
            for ay in &mut s.ay {
                ay.reset();
            }
        }
        card
    }

    /// Volume/mute handle for the frontend.
    pub fn volume(&self) -> CardVolume {
        CardVolume(self.src.clone())
    }

    pub fn variant(&self) -> Variant {
        self.variant
    }

    pub fn phasor_mode(&self) -> u8 {
        self.phasor_mode
    }

    /// Debug view of a unit's 6522 (`GetSnapshotForDebugger`).
    pub fn via(&self, unit: usize) -> &Via {
        &self.subs[unit].via
    }

    /// Debug view of an AY's registers.
    pub fn ay_regs(&self, unit: usize, ay: usize) -> [u8; 16] {
        *self.subs[unit].ay[ay].regs()
    }

    fn is_phasor(&self) -> bool {
        self.variant == Variant::Phasor
    }

    fn has_speech(&self) -> bool {
        matches!(self.variant, Variant::MockingboardC | Variant::Phasor)
    }

    // ---- audio ----

    fn set_ay_clocks(&mut self) {
        // Phasor native mode clocks the AYs at twice the base clock (SetPhasorMode, cpp:893).
        let scale = if self.phasor_mode == PH_PHASOR { 2.0 } else { 1.0 };
        let (clk, rate) = (self.clock.clk_hz() * scale, self.clock.rate_hz);
        for s in &mut self.subs {
            for ay in &mut s.ay {
                ay.set_clock(clk, rate);
            }
        }
    }

    /// Renders every frame whose sample instant is before `now`.
    fn advance_audio(&mut self, now: u64) {
        let clk = self.bus.clock();
        let src_arc = self.src.clone();
        let mut src = lock(&src_arc);
        if clk != self.clock {
            self.clock = clk;
            self.set_ay_clocks();
            self.produced = clk.frames_at(now);
            src.restart_at(self.produced);
        }
        let target = self.clock.frames_at(now);
        if target < self.produced {
            // Time went backwards (save-state load): drop the future.
            self.produced = target;
            src.restart_at(target);
        }
        if target - self.produced > crate::bus::MAX_PENDING as u64 {
            self.produced = target - crate::bus::MAX_PENDING as u64; // long gap: nobody wants it
            src.restart_at(self.produced);
        }
        if target == self.produced {
            return;
        }
        // 2/3 attenuation of the summed voices (MixAllAndCopyToRingBuffer, cpp:376).
        let mix = |a: [i32; 3], b: [i32; 3]| -> f32 {
            let sum: i32 = a.iter().chain(b.iter()).map(|&v| (v as f64 * (2.0 / 3.0)) as i32).sum();
            sum.clamp(-32768, 32767) as f32 / 32768.0
        };
        let echo = self.phasor_mode == PH_ECHO_PLUS && self.is_phasor();
        let speech = self.has_speech();
        let clock = self.clock;
        for k in self.produced..target {
            let c = self.clock.cycle_of_frame(k);
            let [s0, s1] = &mut self.subs;
            let (l0, l1) = (s0.ay[0].sample(), s0.ay[1].sample());
            let (r0, r1) = (s1.ay[0].sample(), s1.ay[1].sample());
            // Echo+: the right speaker's AYs are also output to the left.
            let (mut l, mut r) = if echo { (mix(r0, r1), mix(r0, r1)) } else { (mix(l0, l1), mix(r0, r1)) };
            if speech {
                let sp = s0.ssi.sample_at(c, &clock) + s1.ssi.sample_at(c, &clock);
                l += sp;
                r += sp;
            }
            src.push([l, r]);
        }
        self.produced = target;
    }

    // ---- IRQ ----

    fn sync_irq(&mut self, ctx: &mut CardCtx) {
        let via = self.subs.iter().any(|s| s.via.irq());
        let speech = self.subs.iter().any(|s| s.speech_irq);
        let slot = ctx.slot as u32 & 7;
        let m = self.bus.irq_bits(1 << slot, via);
        ctx.set_irq(IrqSource::Mockingboard6522, m & 0xFF != 0);
        let m = self.bus.irq_bits(1 << (8 + slot), speech);
        ctx.set_irq(IrqSource::Speech, m & 0xFF00 != 0);
    }

    fn apply_ssi(&mut self, unit: usize, out: SsiOut, ctx: &mut CardCtx) {
        let s = &mut self.subs[unit];
        if out.deassert_irq {
            s.speech_irq = false;
        }
        if out.assert_irq {
            s.speech_irq = true;
        }
        if out.ifr_clear | out.ifr_set != 0 {
            s.via.update_ifr(out.ifr_clear, out.ifr_set);
        }
        if let Some(c) = out.schedule {
            ctx.schedule(TOKEN_SSI + unit as u16, c);
        }
    }

    fn schedule_timer(&self, unit: usize, t: Option<TimerSched>, ctx: &mut CardCtx) {
        if let Some(t) = t {
            ctx.schedule_gated((unit * 2) as u16 + t.timer as u16, t.cycles, t.can_assert_irq);
        }
    }

    // ---- AY bus (AY8913_Write etc., cpp:265-443) ----

    fn ay_reset(&mut self, unit: usize) {
        let phasor = self.is_phasor();
        let s = &mut self.subs[unit];
        s.ay[0].reset();
        if phasor {
            s.ay[1].reset(); // GH#1197: both AYs regardless of mode / chip-select
        }
        s.reset_unit(self.variant);
        if self.variant == Variant::SdMusic {
            self.subs[0].set_bus(false);
        }
    }

    fn ay_write(&mut self, unit: usize, ay: usize, value: u8) {
        let sd = self.variant == Variant::SdMusic;
        let via_i = if sd { 0 } else { unit };
        let phasor_native = self.is_phasor() && self.phasor_mode == PH_PHASOR;
        let func = ((value & 2) << 1) | 2 | (value & 1);
        let mut bus_state = false;
        if self.subs[unit].state[ay] == AY_INACTIVE {
            let (ora, ddra) = (self.subs[via_i].via.ora, self.subs[via_i].via.ddra);
            let sub = &mut self.subs[unit];
            match func {
                AY_READ => {
                    let mut v = None;
                    if sub.chip_sel[ay] && sub.latch_valid[ay] {
                        v = Some(sub.ay[ay].read(sub.cur_reg[ay]) & (ddra ^ 0xFF));
                        bus_state = true;
                    }
                    if phasor_native && ay == 0 && sub.chip_sel[1] && sub.latch_valid[1] {
                        let b = sub.ay[1].read(sub.cur_reg[1]) & (ddra ^ 0xFF);
                        v = Some(v.unwrap_or(ora) | b);
                    }
                    if let Some(v) = v {
                        self.subs[via_i].via.ora = v;
                    }
                }
                AY_WRITE => {
                    if sub.chip_sel[ay] && sub.latch_valid[ay] {
                        sub.ay[ay].write(sub.cur_reg[ay], ora);
                    }
                    if phasor_native && ay == 0 && sub.chip_sel[1] && sub.latch_valid[1] {
                        sub.ay[1].write(sub.cur_reg[1], ora);
                    }
                }
                // Addresses above $0F put the AY in a state that ignores the bus.
                AY_LATCH if ora <= 0x0F => {
                    sub.cur_reg[ay] = ora & 0x0F;
                    sub.chip_sel[ay] = true;
                    sub.latch_valid[ay] = true;
                    if phasor_native {
                        if ay == 0 {
                            sub.chip_sel[1] = false;
                        } else {
                            sub.chip_sel[0] = true;
                            sub.cur_reg[0] = sub.cur_reg[1];
                            sub.latch_valid[0] = true;
                        }
                    }
                }
                _ => {}
            }
        }
        self.subs[unit].state[ay] = func;
        if sd {
            self.subs[0].set_bus(bus_state);
        } else {
            self.subs[unit].set_bus(bus_state);
        }
    }

    /// `WriteToORB` (cpp:265).
    fn write_orb(&mut self, mut unit: usize, unit_for_ay: usize, now: u64) {
        let value = self.subs[unit].via.orb_bus();
        let speech_cards = matches!(self.variant, Variant::MockingboardC | Variant::Phasor);
        if speech_cards && unit == 0 && self.subs[0].via.pcr == 0xB0 {
            // Votrax speech data; DDRB's zero bits are high-impedance, so output as 1 (GH#952).
            let ddrb = self.subs[0].via.ddrb;
            let clk = self.clock;
            let out = self.subs[0].ssi.votrax_write((value & ddrb) | (ddrb ^ 0xFF), now, &clk);
            self.pending_ssi = Some((0, out));
            return;
        }
        if self.is_phasor() {
            const KAY1: u8 = 2; // bit4=0 (active low) selects the 1st AY
            const KAY2: u8 = 1; // bit3=0 selects the 2nd AY (unavailable in Mockingboard mode)
            let cs = if self.phasor_mode == PH_PHASOR || self.phasor_mode == PH_ECHO_PLUS {
                !(value >> 3) & 3
            } else {
                KAY1
            };
            if self.phasor_mode == PH_ECHO_PLUS {
                unit = 1;
            }
            if value & 4 == 0 {
                self.ay_reset(unit);
                return;
            }
            // Selecting both AYs: order matters (AY1 latch deselects AY2, AY2 latch selects both).
            if cs & KAY1 != 0 {
                self.ay_write(unit, 0, value);
            }
            if cs & KAY2 != 0 {
                self.ay_write(unit, 1, value);
            }
            if cs == 0 {
                self.subs[unit].set_bus(false);
            }
        } else {
            let u = if self.variant == Variant::SdMusic { unit_for_ay } else { unit };
            if value & 4 == 0 {
                self.ay_reset(u);
                return;
            }
            self.ay_write(u, 0, value);
        }
    }

    // ---- bus access ----

    fn pre(&mut self, ctx: &CardCtx) {
        self.slot = ctx.slot;
        self.advance_audio(ctx.cycles);
    }

    /// `IOReadInternal` (cpp:671). `a` = low byte of the address.
    fn read_io(&mut self, a: u8, ctx: &mut CardCtx) -> u8 {
        self.pre(ctx);
        let now = ctx.cycles;
        let r = a & 0xF;
        if self.is_phasor() {
            let cs = match self.phasor_mode {
                PH_MOCKINGBOARD => ((a & 0x80) >> 7) + 1,
                PH_PHASOR => ((a & 0x80) >> 6) | ((a & 0x10) >> 4),
                PH_ECHO_PLUS => 2,
                _ => 0,
            };
            let mut res = 0;
            if cs & 1 != 0 {
                res |= self.subs[0].via.read(r, now);
            }
            if cs & 2 != 0 {
                res |= self.subs[1].via.read(r, now);
            }
            let mut accessed = cs & 3 != 0;
            // SSI263 D7 at $Cs2x/$Cs4x (Phasor native only; not readable in Mockingboard mode).
            let cs_ssi = a & 0x10 == 0 && a & 0x60 != 0 && a & 0x80 == 0;
            if self.phasor_mode == PH_PHASOR && cs_ssi {
                if a & 0x40 != 0 {
                    res = self.subs[1].ssi.read(ctx.floating_bus);
                }
                if a & 0x20 != 0 {
                    res = self.subs[0].ssi.read(ctx.floating_bus);
                }
                accessed = true;
            }
            self.sync_irq(ctx);
            return if accessed { res } else { ctx.floating_bus };
        }
        let unit = if self.variant == Variant::SdMusic || a & 0x80 == 0 { 0 } else { 1 };
        let v = self.subs[unit].via.read(r, now);
        self.sync_irq(ctx);
        v
    }

    /// `IOWriteInternal` (cpp:738).
    fn write_io(&mut self, a: u8, v: u8, ctx: &mut CardCtx) {
        self.pre(ctx);
        self.false_read(a, ctx);
        let now = ctx.cycles;
        let r = a & 0xF;
        if self.is_phasor() {
            let cs = match self.phasor_mode {
                PH_MOCKINGBOARD => ((a & 0x80) >> 7) + 1,
                PH_PHASOR => ((a & 0x80) >> 6) | ((a & 0x10) >> 4),
                PH_ECHO_PLUS => 2,
                _ => 0,
            };
            for unit in 0..2 {
                if cs & (1 << unit) != 0 {
                    let t = self.subs[unit].via.write(r, v, now);
                    self.schedule_timer(unit, t, ctx);
                    self.sync_gates(unit, r, ctx);
                    if r == reg::ORB {
                        self.write_orb(unit, 0, now);
                    }
                }
            }
            if self.phasor_mode == PH_MOCKINGBOARD || self.phasor_mode == PH_PHASOR {
                self.write_ssi(a, v, now, ctx);
            }
        } else {
            let (unit, for_ay) = if self.variant == Variant::SdMusic {
                (0, (a & 0x80 != 0) as usize)
            } else {
                ((a & 0x80 != 0) as usize, 0)
            };
            let t = self.subs[unit].via.write(r, v, now);
            self.schedule_timer(unit, t, ctx);
            self.sync_gates(unit, r, ctx);
            if r == reg::ORB {
                self.write_orb(unit, for_ay, now);
            }
            if self.has_speech() {
                self.write_ssi(a, v, now, ctx);
            }
        }
        if let Some((u, out)) = self.pending_ssi.take() {
            self.apply_ssi(u, out, ctx);
        }
        self.sync_irq(ctx);
    }

    /// 6502 false read of `STA abs,X/Y` and `STA (zp),Y` (GH#52, cpp:750-778): without a page
    /// cross the CPU first reads the target; only T1CL/T2CL reads have an effect (they clear
    /// IFR). `STA (zp),Y` only does it on the NMOS 6502 (UTAIIe:4-23, 4-27).
    fn false_read(&mut self, a: u8, ctx: &mut CardCtx) {
        let o = ctx.opcode;
        let hit = (o.opcode == 0x91 && !o.cmos) || o.opcode == 0x99 || o.opcode == 0x9D;
        let addr16 = 0xC000 | (ctx.slot as u16) << 8 | a as u16;
        if hit && (o.indexed_base ^ addr16) >> 8 == 0 && matches!(a & 0xF, reg::T1CL | reg::T2CL) {
            self.read_io(a, ctx);
        }
    }

    /// After an IER write, the pending timer events take the new `m_canAssertIRQ` (cpp:217-220).
    fn sync_gates(&self, unit: usize, r: u8, ctx: &mut CardCtx) {
        if r == reg::IER {
            let v = &self.subs[unit].via;
            ctx.set_can_assert_irq((unit * 2) as u16, !v.t1_gated);
            ctx.set_can_assert_irq((unit * 2) as u16 + 1, !v.t2_gated);
        }
    }

    /// Writes to `$Cs4x-$Cs7x` hit the first speech chip, `$Cs2x-$Cs3x`/`$Cs6x` the second
    /// (2nd 6522 is used for the 1st speech chip, cpp:843).
    fn write_ssi(&mut self, a: u8, v: u8, now: u64, ctx: &mut CardCtx) {
        let clk = self.clock;
        if a & 0x40 != 0 {
            let out = self.subs[1].ssi.write(a & 7, v, now, &clk);
            self.apply_ssi(1, out, ctx);
        }
        if a & 0x20 != 0 {
            let out = self.subs[0].ssi.write(a & 7, v, now, &clk);
            self.apply_ssi(0, out, ctx);
        }
    }

    /// Phasor mode select via `$C0(8+slot)x` (cpp:878).
    fn phasor_io(&mut self, lo: u8, ctx: &mut CardCtx) {
        self.pre(ctx);
        let mut bits = self.phasor_mode;
        if lo & 8 != 0 {
            bits = 0;
        }
        bits |= lo & 7;
        self.set_phasor_mode(bits, ctx);
    }

    fn set_phasor_mode(&mut self, mode: u8, ctx: &mut CardCtx) {
        self.phasor_mode = mode;
        if mode == PH_MOCKINGBOARD {
            for s in &mut self.subs {
                s.chip_sel[0] = true;
            }
        }
        self.set_ay_clocks();
        for u in 0..2 {
            let pcr = self.subs[u].via.pcr;
            let out = self.subs[u].ssi.set_card_mode(mode, pcr);
            self.apply_ssi(u, out, ctx);
        }
        self.sync_irq(ctx);
    }
}

impl Card for MockingboardCard {
    fn name(&self) -> &'static str {
        match self.variant {
            Variant::MockingboardC => "Mockingboard C",
            Variant::Phasor => "Phasor",
            Variant::MegaAudio => "MEGA Audio",
            Variant::SdMusic => "SD Music",
        }
    }

    // The 6522s live in the slot's `$Cs00-$CsFF` page (RegisterIoHandler(..., IORead, IOWrite)).
    fn slot_rom_read(&mut self, offset: u8, ctx: &mut CardCtx) -> u8 {
        self.read_io(offset, ctx)
    }

    fn slot_rom_write(&mut self, offset: u8, value: u8, ctx: &mut CardCtx) {
        self.write_io(offset, value, ctx);
    }

    // Only the Phasor has `$C0(8+slot)x` handlers (mode select).
    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        if self.is_phasor() {
            self.phasor_io(lo, ctx);
        }
        ctx.floating_bus
    }

    fn io_write(&mut self, lo: u8, _value: u8, ctx: &mut CardCtx) {
        if self.is_phasor() {
            self.phasor_io(lo, ctx);
        }
    }

    fn reset(&mut self, power_cycle: bool, ctx: &mut CardCtx) {
        self.pre(ctx);
        for t in 0..6 {
            ctx.cancel(t);
        }
        self.phasor_mode = PH_MOCKINGBOARD;
        for s in &mut self.subs {
            s.chip_sel[0] = true;
        }
        self.set_ay_clocks();
        let (variant, phasor) = (self.variant, self.is_phasor());
        for u in 0..2 {
            let s = &mut self.subs[u];
            s.via.reset(power_cycle, ctx.cycles);
            s.ay[0].reset();
            s.ay[1].reset();
            s.reset_unit(variant);
            s.ssi.card_mode = PH_MOCKINGBOARD;
            let out = s.ssi.reset(power_cycle, phasor);
            s.speech_irq = false;
            self.apply_ssi(u, out, ctx);
        }
        self.sync_irq(ctx);
    }

    fn update(&mut self, _cycles: u64, ctx: &mut CardCtx) {
        self.pre(ctx);
        for s in &mut self.subs {
            s.via.update_to(ctx.cycles);
        }
    }

    fn on_event(&mut self, token: u16, ctx: &mut CardCtx) -> i32 {
        self.pre(ctx);
        let now = ctx.cycles;
        let r = match token {
            0..=3 => {
                let via = &mut self.subs[(token / 2) as usize].via;
                if token & 1 == 0 {
                    via.on_timer1_event(now)
                } else {
                    via.on_timer2_event(now)
                }
            }
            4 | 5 => {
                let u = (token - TOKEN_SSI) as usize;
                let (clk, pcr) = (self.clock, self.subs[u].via.pcr);
                let (out, wait) = self.subs[u].ssi.on_event(now, pcr, &clk);
                self.apply_ssi(u, out, ctx);
                wait
            }
            _ => 0,
        };
        self.sync_irq(ctx);
        r
    }

    fn save(&self) -> Vec<u8> {
        let mut w = vec![SAVE_VERSION, self.variant as u8, self.phasor_mode];
        for s in &self.subs {
            let v = &s.via;
            w.extend([v.orb, v.ora, v.ddrb, v.ddra, v.sr, v.acr, v.pcr, v.ifr, v.ier]);
            for x in [v.t1_counter, v.t1_latch, v.t2_counter, v.t2_latch] {
                w.extend(x.to_le_bytes());
            }
            w.push(
                v.t1_irq_delay as u8
                    | (v.t2_irq_delay as u8) << 1
                    | (v.t1_active as u8) << 2
                    | (v.t2_active as u8) << 3
                    | (v.bus_driven as u8) << 4
                    | (s.speech_irq as u8) << 5
                    | (v.t1_gated as u8) << 6
                    | (v.t2_gated as u8) << 7,
            );
            w.extend(v.last.to_le_bytes());
            for ay in &s.ay {
                w.push(ay.type_code());
                w.extend(ay.regs());
            }
            w.extend(s.cur_reg);
            w.extend(s.state);
            w.push(
                s.latch_valid[0] as u8
                    | (s.latch_valid[1] as u8) << 1
                    | (s.chip_sel[0] as u8) << 2
                    | (s.chip_sel[1] as u8) << 3
                    | (s.bus_driven as u8) << 4,
            );
            s.ssi.save(&mut w);
        }
        w
    }

    fn load(&mut self, st: &[u8]) -> bool {
        const PER_SUB: usize = 9 + 8 + 1 + 8 + 2 * 17 + 2 + 2 + 1 + Ssi263::SAVED_LEN;
        if st.len() != 3 + 2 * PER_SUB || st[0] != SAVE_VERSION || st[1] != self.variant as u8 {
            return false;
        }
        self.phasor_mode = st[2];
        let mut p = 3;
        for s in &mut self.subs {
            let b = &st[p..p + PER_SUB];
            let v = &mut s.via;
            (v.orb, v.ora, v.ddrb, v.ddra, v.sr, v.acr, v.pcr, v.ifr, v.ier) =
                (b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8]);
            let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
            (v.t1_counter, v.t1_latch, v.t2_counter, v.t2_latch) = (u16_at(9), u16_at(11), u16_at(13), u16_at(15));
            let f = b[17];
            (v.t1_irq_delay, v.t2_irq_delay, v.t1_active, v.t2_active) =
                (f & 1 != 0, f & 2 != 0, f & 4 != 0, f & 8 != 0);
            v.bus_driven = f & 16 != 0;
            s.speech_irq = f & 32 != 0;
            (v.t1_gated, v.t2_gated) = (f & 64 != 0, f & 128 != 0);
            v.last = u64::from_le_bytes(b[18..26].try_into().unwrap());
            let mut q = 26;
            for ay in &mut s.ay {
                let mut n = Ay::new(Ay::type_from_code(b[q]));
                for r in 0..16 {
                    n.write(r, b[q + 1 + r as usize]);
                }
                *ay = n;
                q += 17;
            }
            s.cur_reg = [b[q], b[q + 1]];
            s.state = [b[q + 2], b[q + 3]];
            let f = b[q + 4];
            s.latch_valid = [f & 1 != 0, f & 2 != 0];
            s.chip_sel = [f & 4 != 0, f & 8 != 0];
            s.bus_driven = f & 16 != 0;
            if !s.ssi.load(&b[q + 5..]) {
                return false;
            }
            p += PER_SUB;
        }
        self.set_ay_clocks();
        true
    }
}
