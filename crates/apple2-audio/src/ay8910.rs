//! AY-3-8910/8912/8913 sound generator (AppleWin AY8910.cpp, itself FUSE's `sound.c`).
//!
//! AppleWin queues register writes with a Cycle stamp and replays them inside
//! `sound_ay_overlay` per output sample. Here the caller renders up to the write's Cycle first
//! and then calls [`Ay::write`], which applies the register side effects at once: same result,
//! no queue.

/// `AMPL_AY_TONE` (cpp:66): 42*256 per voice, three voices fill 16 bits.
const AMPL_AY_TONE: i32 = 42 * 256;
/// Envelope bits (cpp:467).
const ENV_CONT: u8 = 8;
const ENV_ATTACK: u8 = 4;
const ENV_ALT: u8 = 2;
const ENV_HOLD: u8 = 1;

/// AY chip variants (AY8910.h `AY891xType`); they differ in what unimplemented bits read back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AyType {
    Empty,
    Ay3_8910,
    Ay3_8912,
    Ay3_8913,
    Ym2149f,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ay {
    pub ty: AyType,
    regs: [u8; 16],
    tone_levels: [i32; 16],
    tone_tick: [u32; 3],
    tone_high: [bool; 3],
    tone_period: [u32; 3],
    noise_tick: u32,
    noise_period: u32,
    env_internal_tick: u32,
    env_tick: u32,
    env_period: u32,
    tone_subcycles: u32,
    env_subcycles: u32,
    rng: u32,
    noise_toggle: bool,
    env_first: bool,
    env_rev: bool,
    env_counter: i32,
    /// `ay_tick_incr`: 65536 * AY clock / sample rate.
    tick_incr: u32,
}

impl Ay {
    pub fn new(ty: AyType) -> Self {
        let mut ay = Ay {
            ty,
            regs: [0; 16],
            tone_levels: [0; 16],
            tone_tick: [0; 3],
            tone_high: [false; 3],
            tone_period: [1; 3],
            noise_tick: 0,
            noise_period: 0,
            env_internal_tick: 0,
            env_tick: 0,
            env_period: 0,
            tone_subcycles: 0,
            env_subcycles: 0,
            rng: 1,
            noise_toggle: false,
            env_first: true,
            env_rev: false,
            env_counter: 15,
            tick_incr: 0,
        };
        ay.init_levels();
        ay
    }

    /// `sound_ay_init` levels: measurements by Matthew Westcott scaled to 16 bits (cpp:138).
    fn init_levels(&mut self) {
        const LEVELS: [i32; 16] = [
            0x0000, 0x0385, 0x053D, 0x0770, 0x0AD7, 0x0FD5, 0x15B0, 0x230C, 0x2B4C, 0x43C1, 0x5A4B,
            0x732F, 0x9204, 0xAFF1, 0xD921, 0xFFFF,
        ];
        for (o, l) in self.tone_levels.iter_mut().zip(LEVELS) {
            *o = (l * AMPL_AY_TONE + 0x8000) / 0xFFFF;
        }
    }

    /// `sound_init`/`AY8910_InitClock`: `ay_tick_incr` for AY clock `clk_hz` and host `rate_hz`.
    pub fn set_clock(&mut self, clk_hz: f64, rate_hz: u32) {
        self.tick_incr = (65536.0 * clk_hz / rate_hz as f64) as u32;
    }

    /// `sound_ay_reset` (cpp:815): registers cleared through the normal write path.
    pub fn reset(&mut self) {
        let (ty, tick_incr) = (self.ty, self.tick_incr);
        *self = Ay::new(ty);
        self.tick_incr = tick_incr;
        for r in 0..16 {
            self.write(r, 0);
        }
        self.tone_high = [false; 3];
        self.tone_subcycles = 0;
        self.env_subcycles = 0;
    }

    /// Register write with the side effects of cpp:532-569.
    pub fn write(&mut self, reg: u8, val: u8) {
        if self.ty == AyType::Empty {
            return;
        }
        let reg = (reg & 15) as usize;
        self.regs[reg] = val;
        match reg {
            0..=5 => {
                let r = reg >> 1;
                // A zero-length period is the same as 1.
                let p = self.regs[reg & !1] as u32 | ((self.regs[reg | 1] & 15) as u32) << 8;
                self.tone_period[r] = p.max(1);
                // Needed so e.g. Ghouls 'n' Ghosts vibrato isn't scratchy.
                if self.tone_tick[r] >= self.tone_period[r] * 2 {
                    self.tone_tick[r] %= self.tone_period[r] * 2;
                }
            }
            6 => {
                self.noise_tick = 0;
                self.noise_period = (val & 31) as u32;
            }
            11 | 12 => self.env_period = self.regs[11] as u32 | (self.regs[12] as u32) << 8,
            13 => {
                self.env_internal_tick = 0;
                self.env_tick = 0;
                self.env_subcycles = 0;
                self.env_first = true;
                self.env_rev = false;
                self.env_counter = if val & ENV_ATTACK != 0 { 0 } else { 15 };
            }
            _ => {}
        }
    }

    /// `sound_ay_read` (cpp:730): what the chip puts on the bus.
    pub fn read(&self, reg: u8) -> u8 {
        let reg = reg & 15;
        let mut val = self.regs[reg as usize];
        match reg {
            1 | 3 | 5 | 13 => val &= 15,
            6 | 8 | 9 | 10 => val &= 31,
            7 => match self.ty {
                AyType::Ay3_8912 => val &= 0x7F,
                AyType::Ay3_8913 => val &= 0x3F,
                _ => {}
            },
            14 if self.ty == AyType::Ay3_8913 => val = 14, // no such register: bus returns its number
            15 if matches!(self.ty, AyType::Ay3_8912 | AyType::Ay3_8913) => val = 15,
            _ => {}
        }
        if self.ty == AyType::Empty {
            val = 0xFF;
        }
        val
    }

    pub fn regs(&self) -> &[u8; 16] {
        &self.regs
    }

    /// One output sample of the three voices (each `-10752..=10752`), the loop body of
    /// `sound_ay_overlay` (cpp:572-727).
    pub fn sample(&mut self) -> [i32; 3] {
        let mut tone_level = [0i32; 3];
        for (g, t) in tone_level.iter_mut().enumerate() {
            *t = self.tone_levels[(self.regs[8 + g] & 15) as usize];
        }
        let envshape = self.regs[13];
        let env_level = self.tone_levels[self.env_counter as usize];
        for (g, t) in tone_level.iter_mut().enumerate() {
            if self.regs[8 + g] & 16 != 0 {
                *t = env_level;
            }
        }

        // The envelope counter advances every 16 AY cycles (sub-sample resolution).
        self.env_subcycles += self.tick_incr;
        let mut noise_count = 0u32;
        while self.env_subcycles >= 16 << 16 {
            self.env_subcycles -= 16 << 16;
            noise_count += 1;
            self.env_tick += 1;
            while self.env_tick >= self.env_period {
                self.env_tick -= self.env_period;
                // A 1/16th-of-period step if needed.
                if self.env_first || (envshape & ENV_CONT != 0 && envshape & ENV_HOLD == 0) {
                    let up = (envshape & ENV_ATTACK != 0) != self.env_rev;
                    self.env_counter += if up { 1 } else { -1 };
                    self.env_counter = self.env_counter.clamp(0, 15);
                }
                self.env_internal_tick += 1;
                while self.env_internal_tick >= 16 {
                    self.env_internal_tick -= 16;
                    // End of cycle.
                    if envshape & ENV_CONT == 0 {
                        self.env_counter = 0;
                    } else if envshape & ENV_HOLD != 0 {
                        if self.env_first && envshape & ENV_ALT != 0 {
                            self.env_counter = if self.env_counter != 0 { 0 } else { 15 };
                        }
                    } else if envshape & ENV_ALT != 0 {
                        self.env_rev = !self.env_rev;
                    } else {
                        self.env_counter = if envshape & ENV_ATTACK != 0 { 0 } else { 15 };
                    }
                    self.env_first = false;
                }
                // Don't keep trying if the period is zero.
                if self.env_period == 0 {
                    break;
                }
            }
        }

        let mixer = self.regs[7];
        self.tone_subcycles += self.tick_incr;
        let tone_count = self.tone_subcycles >> (3 + 16);
        self.tone_subcycles &= (8 << 16) - 1;

        let mut out = tone_level;
        for (c, o) in out.iter_mut().enumerate() {
            if mixer & (1 << c) == 0 {
                *o = self.do_tone(c, tone_level[c], tone_count);
            }
            if mixer & (8 << c) == 0 && self.noise_toggle {
                *o = 0;
            }
        }

        // Noise: 17-bit LFSR, bit 0 is output, input is bit 0 xor bit 2.
        self.noise_tick += noise_count;
        while self.noise_tick >= self.noise_period {
            self.noise_tick -= self.noise_period;
            if (self.rng & 1) ^ ((self.rng & 2) >> 1) != 0 {
                self.noise_toggle = !self.noise_toggle;
            }
            if (self.rng & 1) ^ ((self.rng & 4) >> 2) != 0 {
                self.rng |= 0x20000;
            }
            self.rng >>= 1;
            if self.noise_period == 0 {
                break;
            }
        }
        out
    }

    /// `AY_DO_TONE` (cpp:415): square wave with a sub-sample edge position.
    fn do_tone(&mut self, c: usize, level: i32, tone_count: u32) -> i32 {
        let mut var = 0;
        let mut is_low = false;
        if level != 0 {
            if self.tone_high[c] {
                var = level;
            } else {
                var = -level;
                is_low = true;
            }
        }
        self.tone_tick[c] += tone_count;
        let mut count = 0;
        while self.tone_tick[c] >= self.tone_period[c] {
            count += 1;
            self.tone_tick[c] -= self.tone_period[c];
            self.tone_high[c] = !self.tone_high[c];
            if count == 1 && level != 0 && self.tone_tick[c] < tone_count {
                let sub = (level as i64 * 2 * self.tone_tick[c] as i64 / tone_count as i64) as i32;
                if is_low {
                    var += sub;
                } else {
                    var -= sub;
                }
            }
        }
        // Changed more than once during the sample: can't be represented; just hope it's a sample.
        if count > 1 {
            var = -level;
        }
        var
    }

    /// Serialized chip state: registers and the type (derived state is rebuilt from them).
    pub fn type_code(&self) -> u8 {
        match self.ty {
            AyType::Empty => 0,
            AyType::Ay3_8910 => 1,
            AyType::Ay3_8912 => 2,
            AyType::Ay3_8913 => 3,
            AyType::Ym2149f => 4,
        }
    }

    pub fn type_from_code(c: u8) -> AyType {
        match c {
            0 => AyType::Empty,
            1 => AyType::Ay3_8910,
            2 => AyType::Ay3_8912,
            4 => AyType::Ym2149f,
            _ => AyType::Ay3_8913,
        }
    }
}

impl Ay {
    /// The chip state in AppleWin's `SaveSnapshot` terms (AY8910.cpp:1098).
    pub fn snapshot(&self) -> crate::snapshot::AySnapshot {
        crate::snapshot::AySnapshot {
            ty: self.ty,
            tone_tick: self.tone_tick,
            tone_high: self.tone_high,
            noise_tick: self.noise_tick,
            tone_subcycles: self.tone_subcycles,
            env_subcycles: self.env_subcycles,
            env_internal_tick: self.env_internal_tick,
            env_tick: self.env_tick,
            tick_incr: self.tick_incr,
            tone_period: self.tone_period,
            noise_period: self.noise_period,
            rng: self.rng,
            noise_toggle: self.noise_toggle,
            env_first: self.env_first,
            env_rev: self.env_rev,
            env_counter: self.env_counter,
            regs: self.regs,
        }
    }

    /// `LoadSnapshot` (AY8910.cpp:1157): the raw state goes in without the register-write side
    /// effects. `tick_incr` is kept (it follows the clocks, AppleWin re-inits it after loading).
    pub fn apply_snapshot(&mut self, s: &crate::snapshot::AySnapshot) {
        self.ty = s.ty;
        if s.ty == AyType::Empty {
            return;
        }
        self.tone_tick = s.tone_tick;
        self.tone_high = s.tone_high;
        self.noise_tick = s.noise_tick;
        self.tone_subcycles = s.tone_subcycles;
        self.env_subcycles = s.env_subcycles;
        self.env_internal_tick = s.env_internal_tick;
        self.env_tick = s.env_tick;
        self.tone_period = s.tone_period;
        self.noise_period = s.noise_period;
        self.rng = s.rng;
        self.noise_toggle = s.noise_toggle;
        self.env_first = s.env_first;
        self.env_rev = s.env_rev;
        self.env_counter = s.env_counter;
        self.regs = s.regs;
        for r in [1, 3, 5] {
            self.regs[r] &= 0xF; // the period high bytes keep 4 bits (cpp:1202)
        }
        self.env_period = self.regs[11] as u32 | (self.regs[12] as u32) << 8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLK: f64 = 1_020_484.45;
    const RATE: u32 = 44_100;

    fn chip() -> Ay {
        let mut a = Ay::new(AyType::Ay3_8913);
        a.set_clock(CLK, RATE);
        a.reset();
        a
    }

    fn tone_a(period: u16, vol: u8) -> Ay {
        let mut a = chip();
        a.write(0, period as u8);
        a.write(1, (period >> 8) as u8);
        a.write(7, 0x3E); // tone A on, everything else off
        a.write(8, vol);
        a
    }

    fn zero_crossings(a: &mut Ay, n: usize) -> usize {
        let s: Vec<i32> = (0..n).map(|_| a.sample()[0]).collect();
        s.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count()
    }

    #[test]
    fn silent_until_programmed() {
        let mut a = chip();
        assert!((0..2000).all(|_| a.sample() == [0, 0, 0]));
    }

    #[test]
    fn tone_frequency_is_clock_over_16_times_period() {
        // Period 100: 1020484.45 / (16 * 100) = 637.8 Hz => 1275.6 edges per second.
        let mut a = tone_a(100, 15);
        let edges = zero_crossings(&mut a, RATE as usize);
        assert!((1272..=1280).contains(&edges), "{edges}");
        // A longer period (1000) = 63.8 Hz.
        let mut a = tone_a(1000, 15);
        let edges = zero_crossings(&mut a, RATE as usize);
        assert!((125..=130).contains(&edges), "{edges}");
    }

    #[test]
    fn volume_register_selects_the_log_level() {
        let peak = |vol| {
            let mut a = tone_a(200, vol);
            (0..4000).map(|_| a.sample()[0]).max().unwrap()
        };
        assert_eq!(peak(0), 0);
        assert_eq!(peak(15), 10752);
        assert!(peak(8) > peak(7) && peak(8) < peak(9));
    }

    fn envelope(shape: u8, period: u16, n: usize) -> Vec<i32> {
        let mut a = chip();
        a.write(7, 0x3F); // tone and noise off: the level is output unmodified
        a.write(8, 0x10); // channel A follows the envelope
        a.write(11, period as u8);
        a.write(12, (period >> 8) as u8);
        a.write(13, shape);
        (0..n).map(|_| a.sample()[0]).collect()
    }

    #[test]
    fn envelope_shapes() {
        let top = 10752;
        // $00: \___ decays once, then stays at 0.
        let s = envelope(0x00, 2, 3000);
        assert_eq!(s[0], top);
        assert!(s.windows(2).all(|w| w[1] <= w[0]));
        assert_eq!(*s.last().unwrap(), 0);
        // $0D: /¯¯¯ rises once and holds at max.
        let s = envelope(0x0D, 2, 3000);
        assert_eq!(s[0], 0);
        assert!(s.windows(2).all(|w| w[1] >= w[0]));
        assert_eq!(*s.last().unwrap(), top);
        // $0E: /\/\ triangle: goes up and back down repeatedly.
        let s = envelope(0x0E, 2, 6000);
        let ups = s.windows(2).filter(|w| w[1] > w[0]).count();
        let downs = s.windows(2).filter(|w| w[1] < w[0]).count();
        assert!(ups > 20 && downs > 20);
        assert_eq!(s.iter().copied().max(), Some(top));
        // $0C: //// sawtooth restarts from 0 each cycle.
        let s = envelope(0x0C, 2, 6000);
        assert!(s.windows(2).filter(|w| w[1] < w[0]).count() >= 3);
        // $08: \\\\ decaying sawtooth.
        let s = envelope(0x08, 2, 6000);
        assert!(s.windows(2).filter(|w| w[1] > w[0]).count() >= 3);
    }

    #[test]
    fn noise_flips_levels_pseudo_randomly() {
        let mut a = chip();
        a.write(7, 0x37); // tone off, noise on for A
        a.write(6, 1);
        a.write(8, 15);
        let s: Vec<i32> = (0..4000).map(|_| a.sample()[0]).collect();
        let zeros = s.iter().filter(|&&x| x == 0).count();
        assert!(zeros > 1000 && zeros < 3000, "{zeros}");
    }

    #[test]
    fn unimplemented_bits_read_back_as_the_8913_bus_does() {
        let mut a = chip();
        a.write(7, 0xFF);
        assert_eq!(a.read(7), 0x3F);
        assert_eq!(a.read(14), 14);
        assert_eq!(a.read(15), 15);
        a.write(1, 0xFF);
        assert_eq!(a.read(1), 0x0F);
        assert_eq!(Ay::new(AyType::Empty).read(3), 0xFF);
    }
}
