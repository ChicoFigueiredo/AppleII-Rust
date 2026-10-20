//! NTSC chroma/luma lookup tables, ported from AppleWin NTSC.cpp `initChromaPhaseTables`
//! (:919). A pixel colour is looked up by the last 12 signal bits (one bit per 14M dot) and the
//! current colour phase. Colours are `0xAARRGGBB` like AppleWin's `bgra_t` read as a `uint32_t`.

// The filter constants are copied verbatim from NTSC.cpp, digits and all.
#![allow(clippy::excessive_precision)]

use std::sync::OnceLock;

pub const NUM_SEQUENCES: usize = 4096;
pub const NUM_PHASES: usize = 4;
pub const ALPHA: u32 = 0xFF00_0000;

// Filter constants are `float` literals in the C++ and widen to double when used, so they are
// rounded through f32 here too.
const CHROMA_GAIN: f64 = 7.438011255f32 as f64;
const CHROMA_0: f64 = -0.7318893645f32 as f64;
const CHROMA_1: f64 = 1.2336442711f32 as f64;
const LUMA_GAIN: f64 = 13.71331570f32 as f64;
const LUMA_0: f64 = -0.3961075449f32 as f64;
const LUMA_1: f64 = 1.1044202472f32 as f64;
const SIGNAL_GAIN: f64 = 7.614490548f32 as f64;
const SIGNAL_0: f64 = -0.2718798058f32 as f64;
const SIGNAL_1: f64 = 0.7465656072f32 as f64;

const I_TO_R: f64 = 0.956f32 as f64;
const I_TO_G: f64 = -0.272f32 as f64;
const I_TO_B: f64 = -1.105f32 as f64;
const Q_TO_R: f64 = 0.621f32 as f64;
const Q_TO_G: f64 = -0.647f32 as f64;
const Q_TO_B: f64 = 1.702f32 as f64;

use std::f32::consts::PI; // NTSC.cpp's `3.1415926535898f` is this float

/// Second-order IIR section with the state AppleWin keeps in function-local statics, which
/// persists across the whole table generation.
#[derive(Default)]
struct Iir {
    x: [f64; 3],
    y: [f64; 3],
}

impl Iir {
    /// `initFilterChroma`: note the inverted `x[0]`.
    fn chroma(&mut self, z: f64) -> f64 {
        self.x = [self.x[1], self.x[2], z / CHROMA_GAIN];
        self.y = [self.y[1], self.y[2], 0.0];
        self.y[2] = -self.x[0] + self.x[2] + (CHROMA_0 * self.y[0]) + (CHROMA_1 * self.y[1]);
        self.y[2]
    }

    /// `initFilterLuma0/1` and `initFilterSignal`: Butterworth low-pass with the given constants.
    fn lowpass(&mut self, z: f64, gain: f64, c0: f64, c1: f64) -> f64 {
        self.x = [self.x[1], self.x[2], z / gain];
        self.y = [self.y[1], self.y[2], 0.0];
        self.y[2] = self.x[0] + self.x[2] + (2.0 * self.x[1]) + (c0 * self.y[0]) + (c1 * self.y[1]);
        self.y[2]
    }
}

pub struct Chroma {
    pub bnw_monitor: Vec<u32>,
    pub bnw_tv: Vec<u32>,
    pub hue_monitor: [Vec<u32>; NUM_PHASES],
    pub hue_tv: [Vec<u32>; NUM_PHASES],
    /// `GenerateBaseColors` (NTSC.cpp:2697): the 16 lo-res colours as the TV table renders them.
    pub base_colors: [u32; 16],
}

fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn gray(v: u32) -> f32 {
    v as f32 / 255.0
}

fn pack(r: f32, g: f32, b: f32) -> u32 {
    ALPHA | ((((r * 255.0) as u8) as u32) << 16) | ((((g * 255.0) as u8) as u32) << 8) | (((b * 255.0) as u8) as u32)
}

fn build() -> Chroma {
    let mut bnw_monitor = vec![0; NUM_SEQUENCES];
    let mut bnw_tv = vec![0; NUM_SEQUENCES];
    let mut hue_monitor: [Vec<u32>; NUM_PHASES] = std::array::from_fn(|_| vec![0; NUM_SEQUENCES]);
    let mut hue_tv: [Vec<u32>; NUM_PHASES] = std::array::from_fn(|_| vec![0; NUM_SEQUENCES]);
    let (mut f_signal, mut f_chroma, mut f_luma0, mut f_luma1) =
        (Iir::default(), Iir::default(), Iir::default(), Iir::default());

    let rad_45 = PI * 0.25f32;
    let cycle_start = PI * 45.0f32 / 180.0f32;

    for phase in 0..NUM_PHASES {
        // `phi` is set once per phase and keeps accumulating across the sequences.
        let mut phi = ((phase as f32 * PI * 0.5f32) + cycle_start) as f64;
        for s in 0..NUM_SEQUENCES {
            let mut t = s;
            let (mut y0, mut y1, mut i, mut q) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let mut z = 0.0f64;
            for _ in 0..12 {
                z = if t & 0x800 != 0 { 1.0 } else { 0.0 };
                t <<= 1;
                for _ in 0..2 {
                    let zz = f_signal.lowpass(z, SIGNAL_GAIN, SIGNAL_0, SIGNAL_1);
                    let mut c = f_chroma.chroma(zz);
                    y0 = f_luma0.lowpass(zz, LUMA_GAIN, LUMA_0, LUMA_1);
                    y1 = f_luma1.lowpass(zz - c, LUMA_GAIN, LUMA_0, LUMA_1);
                    c *= 2.0;
                    i += (c * phi.cos() - i) / 8.0;
                    q += (c * phi.sin() - q) / 8.0;
                    phi += rad_45 as f64;
                }
            }

            let b = (clamp01(z as f32) * 255.0) as u8 as u32;
            bnw_monitor[s] = ALPHA | (b << 16) | (b << 8) | b;
            let b = (clamp01(y1 as f32) * 255.0) as u8 as u32;
            bnw_tv[s] = ALPHA | (b << 16) | (b << 8) | b;

            let color = s & 15;
            let fix = |mut r: f32, mut g: f32, mut b: f32, tv: bool| {
                // NTSC_REMOVE_WHITE_RINGING / BLACK_GHOSTING / GRAY_CHROMA
                match color {
                    15 => (r, g, b) = (1.0, 1.0, 1.0),
                    0 => (r, g, b) = (0.0, 0.0, 0.0),
                    5 if !tv => (r, g, b) = (gray(0x83), gray(0x83), gray(0x83)),
                    10 if !tv => (r, g, b) = (gray(0x78), gray(0x78), gray(0x78)),
                    _ => {}
                }
                pack(r, g, b)
            };
            let rgb = |y: f64| {
                (
                    clamp01((y + I_TO_R * i + Q_TO_R * q) as f32),
                    clamp01((y + I_TO_G * i + Q_TO_G * q) as f32),
                    clamp01((y + I_TO_B * i + Q_TO_B * q) as f32),
                )
            };
            let (r, g, b) = rgb(y0);
            hue_monitor[phase][s] = fix(r, g, b, false);
            let (r, g, b) = rgb(y1);
            hue_tv[phase][s] = fix(r, g, b, true);
        }
    }

    let mut base_colors = [0u32; 16];
    for (c, out) in base_colors.iter_mut().enumerate() {
        let (mut phase, mut sig) = (0usize, 0usize);
        let mut bits = (c << 12) | (c << 8) | (c << 4) | c;
        let mut colors = [0u32; 4];
        for j in 0..16 {
            sig = ((sig << 1) | (bits & 1)) & 0xFFF;
            colors[j & 3] = hue_tv[phase][sig];
            bits >>= 1;
            phase = (phase + 1) & 3;
        }
        let avg = |sh: u32| colors.iter().map(|c| (c >> sh) & 0xFF).sum::<u32>() / 4;
        *out = ALPHA | (avg(16) << 16) | (avg(8) << 8) | avg(0);
    }

    Chroma { bnw_monitor, bnw_tv, hue_monitor, hue_tv, base_colors }
}

static CHROMA: OnceLock<Chroma> = OnceLock::new();

pub fn chroma() -> &'static Chroma {
    CHROMA.get_or_init(build)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_colors_are_the_applewin_ntsc_lores_palette() {
        // Index = lo-res colour number. Values are AppleWin's NTSC-generated palette as this port
        // yields them (deep red 930B7C, dark blue 1F35D3, ... white FFFFFF).
        let want = [
            0x000000, 0x930B7C, 0x1F35D3, 0xBB36FF, 0x00760C, 0x7E7E7E, 0x07A8E0, 0x9DACFF, 0x624C00, 0xF9561D,
            0x7E7E7E, 0xFF81EC, 0x43C800, 0xDCCD16, 0x5DF784, 0xFFFFFF,
        ];
        let got: Vec<u32> = chroma().base_colors.iter().map(|c| c & 0xFF_FFFF).collect();
        assert_eq!(got, want);
        assert!(chroma().base_colors.iter().all(|c| c >> 24 == 0xFF));
    }

    #[test]
    fn solid_black_and_white_have_no_chroma_for_every_phase() {
        let c = chroma();
        for p in 0..NUM_PHASES {
            assert_eq!(c.hue_monitor[p][0x000], 0xFF000000);
            assert_eq!(c.hue_monitor[p][0xFFF], 0xFFFFFFFF);
            assert_eq!(c.hue_tv[p][0x000], 0xFF000000);
            assert_eq!(c.hue_tv[p][0xFFF], 0xFFFFFFFF);
        }
    }

    #[test]
    fn gray_patterns_are_chroma_free_on_the_monitor_table() {
        // NTSC_REMOVE_GRAY_CHROMA: alternating dots (0101 / 1010) are flat grays $83 and $78.
        let c = chroma();
        for p in 0..NUM_PHASES {
            assert_eq!(c.hue_monitor[p][0b0101_0101_0101] & 0xFF_FFFF, 0x838383, "{p}");
            assert_eq!(c.hue_monitor[p][0b1010_1010_1010] & 0xFF_FFFF, 0x787878, "{p}");
        }
    }

    #[test]
    fn monochrome_monitor_table_is_the_last_dot() {
        let c = chroma();
        for s in [0usize, 1, 2, 0xFFE, 0xFFF, 0x555, 0xAAA] {
            let v = if s & 1 != 0 { 0xFFFFFFFF } else { 0xFF000000 };
            assert_eq!(c.bnw_monitor[s], v, "{s:x}");
        }
        // The TV luma table is filtered: a lone lit dot is dimmer than a solid line.
        assert!(c.bnw_tv[1] & 0xFF < c.bnw_tv[0xFFF] & 0xFF);
    }
}
