//! Palette-index lookup images and palettes of the "RGB" renderers (AppleWin RGBMonitor.cpp):
//! lo-res, hi-res half-pixel and double hi-res source images, the 1.25 hi-res colour mixing
//! map, and the NTSC and Le Chat Mauve Feline palettes.

use crate::chroma::{chroma, ALPHA};
use std::sync::OnceLock;

pub const HIRES_COLUMN_SUBUNIT_SIZE: usize = 16;
const HIRES_COLUMN_UNIT_SIZE: usize = HIRES_COLUMN_SUBUNIT_SIZE * 2;
const HIRES_NUMBER_COLUMNS: usize = 1 << 5;

pub const SRCOFFS_LORES: usize = 0;
pub const SRCOFFS_HIRES: usize = SRCOFFS_LORES + 16;
pub const SRCOFFS_DHIRES: usize = SRCOFFS_HIRES + HIRES_NUMBER_COLUMNS * HIRES_COLUMN_UNIT_SIZE;
pub const SRCOFFS_TOTAL: usize = SRCOFFS_DHIRES + 2560;
const MAX_SOURCE_Y: usize = 256;

/// `Color_Palette_Index_e` (RGBMonitor.cpp:36).
pub mod pal {
    pub const HGR_BLACK: u8 = 0;
    pub const HGR_WHITE: u8 = 1;
    pub const HGR_BLUE: u8 = 2;
    pub const HGR_ORANGE: u8 = 3;
    pub const HGR_GREEN: u8 = 4;
    pub const HGR_VIOLET: u8 = 5;
    pub const HGR_GREY1: u8 = 6;
    pub const HGR_GREY2: u8 = 7;
    pub const HGR_YELLOW: u8 = 8;
    pub const HGR_AQUA: u8 = 9;
    pub const HGR_PURPLE: u8 = 10;
    pub const HGR_PINK: u8 = 11;
    /// First of the 16 lo-res / double hi-res colours (`BLACK`).
    pub const BLACK: u8 = 12;
    pub const DEEP_RED: u8 = 13;
    pub const DARK_BLUE: u8 = 14;
    pub const MAGENTA: u8 = 15;
    pub const DARK_GREEN: u8 = 16;
    pub const DARK_GRAY: u8 = 17;
    pub const BLUE: u8 = 18;
    pub const LIGHT_BLUE: u8 = 19;
    pub const BROWN: u8 = 20;
    pub const ORANGE: u8 = 21;
    pub const LIGHT_GRAY: u8 = 22;
    pub const PINK: u8 = 23;
    pub const GREEN: u8 = 24;
    pub const YELLOW: u8 = 25;
    pub const AQUA: u8 = 26;
    pub const WHITE: u8 = 27;
}
use pal::*;

pub const PALETTE_SIZE: usize = 28;

/// Lo-res colour number to palette index (`LoresResColors`).
pub const LORES_RES_COLORS: [u8; 16] = [
    BLACK, DEEP_RED, DARK_BLUE, MAGENTA, DARK_GREEN, DARK_GRAY, BLUE, LIGHT_BLUE, BROWN, ORANGE,
    LIGHT_GRAY, PINK, GREEN, YELLOW, AQUA, WHITE,
];

/// Double hi-res 4-bit pixel (as the bits sit in memory) to palette index (`DoubleHiresPalIndex`).
pub const DOUBLE_HIRES_PAL_INDEX: [u8; 16] = [
    BLACK, DARK_BLUE, DARK_GREEN, BLUE, BROWN, LIGHT_GRAY, GREEN, AQUA, DEEP_RED, MAGENTA,
    DARK_GRAY, LIGHT_BLUE, ORANGE, PINK, YELLOW, WHITE,
];

/// `HiresToPalIndex` indexed by `ColorMapping` (violet, blue, green, orange, black, white).
const HIRES_TO_PAL: [u8; 6] = [
    HGR_VIOLET, HGR_BLUE, HGR_GREEN, HGR_ORANGE, HGR_BLACK, HGR_WHITE,
];
const CM_VIOLET: usize = 0;
const CM_BLACK: usize = 4;
const CM_WHITE: usize = 5;

const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ALPHA | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

/// `PaletteRGB_NTSC` entries that `VideoInitializeOriginal` does not overwrite (TV emulation
/// colours of the 1.25 hi-res renderer).
const TV_EMU: [u32; 6] = [
    rgb(0x80, 0x80, 0x80),
    rgb(0x80, 0x80, 0x80),
    rgb(0x9E, 0x9E, 0x00),
    rgb(0x00, 0xCD, 0x4A),
    rgb(0x61, 0x61, 0xFF),
    rgb(0xFF, 0x32, 0xB5),
];

/// `PaletteRGB_Feline` (RGBMonitor.cpp:169): Le Chat Mauve Feline, from a white-balanced capture.
pub const PALETTE_FELINE: [u32; PALETTE_SIZE] = [
    rgb(0x00, 0x00, 0x00),
    rgb(0xFF, 0xFF, 0xFF),
    rgb(0x00, 0x8A, 0xB5),
    rgb(0xFF, 0x72, 0x47),
    rgb(0x6F, 0xE6, 0x2C),
    rgb(0xAA, 0x1A, 0xD1),
    TV_EMU[0],
    TV_EMU[1],
    TV_EMU[2],
    TV_EMU[3],
    TV_EMU[4],
    TV_EMU[5],
    rgb(0x00, 0x00, 0x00),
    rgb(0xAC, 0x12, 0x4C),
    rgb(0x00, 0x07, 0x83),
    rgb(0xAA, 0x1A, 0xD1),
    rgb(0x00, 0x83, 0x2F),
    rgb(0x9F, 0x97, 0x7E),
    rgb(0x00, 0x8A, 0xB5),
    rgb(0x9F, 0x9E, 0xFF),
    rgb(0x7A, 0x5F, 0x00),
    rgb(0xFF, 0x72, 0x47),
    rgb(0x78, 0x68, 0x7F),
    rgb(0xFF, 0x7A, 0xCF),
    rgb(0x6F, 0xE6, 0x2C),
    rgb(0xFF, 0xF6, 0x7B),
    rgb(0x6C, 0xEE, 0xB2),
    rgb(0xFF, 0xFF, 0xFF),
];

pub struct Lookup {
    /// `[y][x]` palette indices, `SRCOFFS_TOTAL` wide (the order of rows is irrelevant here).
    src: Vec<u8>,
    /// `colormixmap[top][mid][bottom]` = `(mixTop << 8) | mixBot`.
    pub colormix: [[[u16; 6]; 6]; 6],
    /// `PaletteRGB_NTSC` after `VideoInitializeOriginal`.
    pub palette_ntsc: [u32; PALETTE_SIZE],
}

impl Lookup {
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> u8 {
        self.src[y * SRCOFFS_TOTAL + x]
    }

    fn set(&mut self, x: usize, y: usize, c: u8) {
        self.src[y * SRCOFFS_TOTAL + x] = c;
    }
}

/// `V_CreateLookup_DoubleHires` (:207). The range conditions are kept as written in the C++.
#[allow(clippy::manual_range_contains, clippy::int_plus_one)]
fn create_double_hires(l: &mut Lookup) {
    const OFFSET: i32 = 3;
    const SIZE: i32 = 10;
    for column in 0..256usize {
        let coloffs = SIZE as usize * column;
        for byteval in 0..256usize {
            let mut color = [0usize; SIZE as usize];
            let pattern = (byteval | (column << 8)) as u32;
            for pixel in 1..15i32 {
                if pattern & (1 << pixel) == 0 {
                    continue;
                }
                let pixelcolor = 1usize << ((pixel - OFFSET) & 3);
                if pixel >= OFFSET + 2
                    && pixel < SIZE + OFFSET + 2
                    && pattern & (0x7 << (pixel - 4)) != 0
                {
                    color[(pixel - (OFFSET + 2)) as usize] |= pixelcolor;
                }
                if pixel >= OFFSET + 1
                    && pixel < SIZE + OFFSET + 1
                    && pattern & (0xF << (pixel - 4)) != 0
                {
                    color[(pixel - (OFFSET + 1)) as usize] |= pixelcolor;
                }
                if pixel >= OFFSET && pixel < SIZE + OFFSET {
                    color[(pixel - OFFSET) as usize] |= pixelcolor;
                }
                if pixel >= OFFSET - 1
                    && pixel < SIZE + OFFSET - 1
                    && pattern & (0xF << (pixel + 1)) != 0
                {
                    color[(pixel - (OFFSET - 1)) as usize] |= pixelcolor;
                }
                if pixel >= OFFSET - 2
                    && pixel < SIZE + OFFSET - 2
                    && pattern & (0x7 << (pixel + 2)) != 0
                {
                    color[(pixel - (OFFSET - 2)) as usize] |= pixelcolor;
                }
            }
            for (x, &c) in color.iter().enumerate() {
                l.set(
                    SRCOFFS_DHIRES + coloffs + x,
                    byteval,
                    DOUBLE_HIRES_PAL_INDEX[c],
                );
            }
        }
    }
}

/// `V_CreateLookup_Lores` (:261).
fn create_lores(l: &mut Lookup) {
    for (color, &c) in LORES_RES_COLORS.iter().enumerate() {
        for x in 0..16 {
            for y in 0..16 {
                l.set(SRCOFFS_LORES + x, (color << 4) + y, c);
            }
        }
    }
}

/// `V_CreateLookup_HiResHalfPixel_Authentic(VT_COLOR_IDEALIZED)` (:280).
fn create_hires(l: &mut Lookup) {
    for column in 0..HIRES_NUMBER_COLUMNS {
        let offsetx = column * HIRES_COLUMN_UNIT_SIZE;
        let prev_high_bit = column >= 16;
        let mut p = [false; 11]; // c2 c3 b6 b5 b4 b3 b2 b1 b0 c0 c1
        p[0] = column & 4 != 0;
        p[1] = column & 8 != 0;
        p[9] = column & 1 != 0;
        p[10] = column & 2 != 0;
        for byte in 0..256usize {
            for (i, px) in p[2..9].iter_mut().enumerate() {
                *px = byte & (1 << i) != 0;
            }
            let high = (byte >> 7) & 1;
            let y = byte;
            // Half-pixel fix-ups that cross the video byte boundary.
            if high != 0 {
                if p[1] {
                    if p[2] || p[0] {
                        l.set(SRCOFFS_HIRES + offsetx, y, HGR_WHITE);
                        l.set(
                            SRCOFFS_HIRES + offsetx + HIRES_COLUMN_SUBUNIT_SIZE,
                            y,
                            HGR_WHITE,
                        );
                    } else {
                        l.set(
                            SRCOFFS_HIRES + offsetx,
                            y,
                            if !prev_high_bit {
                                HGR_BLACK
                            } else {
                                HGR_ORANGE
                            },
                        );
                        l.set(
                            SRCOFFS_HIRES + offsetx + HIRES_COLUMN_SUBUNIT_SIZE,
                            y,
                            HGR_BLUE,
                        );
                    }
                } else if p[0] && p[2] {
                    // VT_COLOR_IDEALIZED: always.
                    l.set(SRCOFFS_HIRES + offsetx, y, HGR_BLUE);
                    l.set(
                        SRCOFFS_HIRES + offsetx + HIRES_COLUMN_SUBUNIT_SIZE,
                        y,
                        HGR_ORANGE,
                    );
                }
            }
            let mut x = high;
            for odd in 0..2usize {
                if odd != 0 {
                    x = HIRES_COLUMN_SUBUNIT_SIZE + high;
                }
                for ip in 2..9usize {
                    let mut color = CM_BLACK;
                    if p[ip] {
                        color = CM_WHITE;
                        if !(p[ip - 1] || p[ip + 1]) {
                            color = ((odd ^ (ip & 1)) << 1) | high;
                        }
                    } else if p[ip - 1] && p[ip + 1] {
                        // VT_COLOR_IDEALIZED fills in colours between white pixels.
                        color = ((odd ^ ((ip & 1 == 0) as usize)) << 1) | high;
                    }
                    let c = HIRES_TO_PAL[color];
                    l.set(SRCOFFS_HIRES + offsetx + x, y, c);
                    l.set(SRCOFFS_HIRES + offsetx + x + 1, y, c);
                    x += 2;
                }
            }
        }
    }
    let _ = CM_VIOLET;
}

/// `MixColors` (:398).
fn mix_colors(c1: u8, c2: u8) -> u8 {
    let pair = |a: u8, b: u8| (c1 == a && c2 == b) || (c1 == b && c2 == a);
    if c1 == c2 {
        c1
    } else if pair(HGR_BLUE, HGR_ORANGE) {
        HGR_GREY1
    } else if pair(HGR_GREEN, HGR_VIOLET) {
        HGR_GREY2
    } else if pair(HGR_ORANGE, HGR_GREEN) {
        HGR_YELLOW
    } else if pair(HGR_BLUE, HGR_GREEN) {
        HGR_AQUA
    } else if pair(HGR_BLUE, HGR_VIOLET) {
        HGR_PURPLE
    } else if pair(HGR_ORANGE, HGR_VIOLET) {
        HGR_PINK
    } else {
        WHITE // visible failure indicator
    }
}

/// `CreateColorMixMap` (:422).
fn create_color_mix_map() -> [[[u16; 6]; 6]; 6] {
    const FROM_NEIGHBOUR: u16 = 0;
    const MIX_THRESHOLD: u8 = HGR_BLUE;
    let mut map = [[[0u16; 6]; 6]; 6];
    for (t, plane) in map.iter_mut().enumerate() {
        for (m, row) in plane.iter_mut().enumerate() {
            for (b, cell) in row.iter_mut().enumerate() {
                let (c_top, c_mid, c_bot) = (t as u8, m as u8, b as u8);
                let (mut mix_top, mut mix_bot);
                if c_mid < MIX_THRESHOLD {
                    mix_top = c_mid as u16;
                    mix_bot = c_mid as u16;
                } else {
                    mix_top = if c_top < MIX_THRESHOLD {
                        FROM_NEIGHBOUR
                    } else {
                        mix_colors(c_mid, c_top) as u16
                    };
                    mix_bot = if c_bot < MIX_THRESHOLD {
                        FROM_NEIGHBOUR
                    } else {
                        mix_colors(c_mid, c_bot) as u16
                    };
                    if mix_top == FROM_NEIGHBOUR && mix_bot != FROM_NEIGHBOUR {
                        mix_top = mix_bot;
                    } else if mix_bot == FROM_NEIGHBOUR && mix_top != FROM_NEIGHBOUR {
                        mix_bot = mix_top;
                    } else if mix_bot == FROM_NEIGHBOUR && mix_top == FROM_NEIGHBOUR {
                        mix_top = c_mid as u16;
                        mix_bot = c_mid as u16;
                    }
                }
                *cell = (mix_top << 8) | mix_bot;
            }
        }
    }
    map
}

fn build() -> Lookup {
    let base = chroma().base_colors;
    let mut palette = [0u32; PALETTE_SIZE];
    // HGR_BLACK, HGR_WHITE; the four hi-res colours are copies of the generated lo-res ones.
    palette[HGR_BLACK as usize] = rgb(0, 0, 0);
    palette[HGR_WHITE as usize] = rgb(0xFF, 0xFF, 0xFF);
    palette[6..12].copy_from_slice(&TV_EMU);
    palette[BLACK as usize..].copy_from_slice(&base);
    palette[HGR_BLUE as usize] = palette[BLUE as usize];
    palette[HGR_ORANGE as usize] = palette[ORANGE as usize];
    palette[HGR_GREEN as usize] = palette[GREEN as usize];
    palette[HGR_VIOLET as usize] = palette[MAGENTA as usize];

    let mut l = Lookup {
        src: vec![0; SRCOFFS_TOTAL * MAX_SOURCE_Y],
        colormix: create_color_mix_map(),
        palette_ntsc: palette,
    };
    create_lores(&mut l);
    create_hires(&mut l);
    create_double_hires(&mut l);
    l
}

static LOOKUP: OnceLock<Lookup> = OnceLock::new();

pub fn lookup() -> &'static Lookup {
    LOOKUP.get_or_init(build)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_hires_color_mapping_matches_applewin() {
        // DoubleHiresPalIndex: memory nibble (aux bit0 first) -> colour. Spot checks of the table
        // in RGBMonitor.cpp:110, with the standard DHGR colour names.
        let names = [
            BLACK, DARK_BLUE, DARK_GREEN, BLUE, BROWN, LIGHT_GRAY, GREEN, AQUA, DEEP_RED, MAGENTA,
            DARK_GRAY, LIGHT_BLUE, ORANGE, PINK, YELLOW, WHITE,
        ];
        assert_eq!(DOUBLE_HIRES_PAL_INDEX, names);
        // A solid line of nibble 0xF is white in the source image; nibble 0 black.
        let l = lookup();
        let col = |hi: usize, lo: usize, x: usize| l.at(SRCOFFS_DHIRES + 10 * hi + x, lo);
        // The 15-bit window is split into HIBYTE ($7F) and LOBYTE ($FF).
        assert_eq!(col(0x7F, 0xFF, 5), WHITE);
        assert_eq!(col(0, 0, 4), BLACK);
    }

    #[test]
    fn lores_source_image_has_one_colour_per_nibble() {
        let l = lookup();
        for (c, &want) in LORES_RES_COLORS.iter().enumerate() {
            assert_eq!(l.at(SRCOFFS_LORES + 5, (c << 4) + 7), want);
        }
    }

    #[test]
    fn ntsc_palette_hi_res_colours_alias_the_generated_lores_ones() {
        let p = lookup().palette_ntsc;
        assert_eq!(p[HGR_BLACK as usize], 0xFF000000);
        assert_eq!(p[HGR_WHITE as usize], 0xFFFFFFFF);
        assert_eq!(p[HGR_VIOLET as usize], p[MAGENTA as usize]);
        assert_eq!(p[HGR_BLUE as usize], p[BLUE as usize]);
        assert_eq!(p[HGR_ORANGE as usize], p[ORANGE as usize]);
        assert_eq!(p[HGR_GREEN as usize], p[GREEN as usize]);
        assert_eq!(p[WHITE as usize], 0xFFFFFFFF);
    }

    #[test]
    fn feline_palette_values() {
        assert_eq!(PALETTE_FELINE[ORANGE as usize] & 0xFFFFFF, 0xFF7247);
        assert_eq!(PALETTE_FELINE[DEEP_RED as usize] & 0xFFFFFF, 0xAC124C);
        assert_eq!(PALETTE_FELINE.len(), PALETTE_SIZE);
    }

    #[test]
    fn hires_isolated_pixels_get_the_artifact_colours() {
        // One lit dot at an even column, bit7 clear -> violet; odd -> green. bit7 set: blue/orange.
        let l = lookup();
        let y = 0b0000_0001usize; // pixel 0 on, high bit 0
        assert_eq!(l.at(SRCOFFS_HIRES, y), HGR_VIOLET); // even sub-unit, pixel 0 (a 14M dot pair)
        assert_eq!(l.at(SRCOFFS_HIRES + 1, y), HGR_VIOLET);
        assert_eq!(l.at(SRCOFFS_HIRES + 2, y), HGR_BLACK);
        // Same dot with bit 7 set is blue, shifted half a pixel right.
        let y = 0b1000_0001usize;
        assert_eq!(l.at(SRCOFFS_HIRES + 1, y), HGR_BLUE);
        // Adjacent lit dots are white.
        assert_eq!(l.at(SRCOFFS_HIRES, 0b0000_0011), HGR_WHITE);
        assert_eq!(l.at(SRCOFFS_HIRES + 2, 0b0000_0011), HGR_WHITE);
    }

    #[test]
    fn color_mix_blends_complementary_neighbours() {
        let m = create_color_mix_map();
        // Blue over orange with orange below: top half grey, bottom half orange stays orange.
        let v = m[HGR_BLUE as usize][HGR_ORANGE as usize][HGR_ORANGE as usize];
        assert_eq!((v >> 8) as u8, HGR_GREY1);
        assert_eq!(v as u8, HGR_ORANGE);
        // Black/white pass through unchanged.
        assert_eq!(
            m[3][HGR_WHITE as usize][2],
            ((HGR_WHITE as u16) << 8) | HGR_WHITE as u16
        );
    }
}
