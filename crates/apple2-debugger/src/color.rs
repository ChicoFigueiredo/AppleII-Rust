//! Debugger colors (Debugger_Color.h/.cpp): the 8-level palette ramps, the default index of every
//! `DebugColors_e` entry and the three schemes (color, monochrome, black & white).

/// Palette index: `K0` is 0, then 8 levels (8 brightest) of R, G, Y, B, M, C, W, then 16 customs.
const fn p(ramp: u8, level: u8) -> u8 {
    1 + ramp * 8 + (8 - level)
}
const K0: u8 = 0;
const R: u8 = 0;
const G: u8 = 1;
const Y: u8 = 2;
const B: u8 = 3;
const M: u8 = 4;
const C: u8 = 5;
const W: u8 = 6;
/// `COLOR_CUSTOM_nn` (1-based).
const fn cu(n: u8) -> u8 {
    57 + n - 1
}

pub const NUM_PALETTE: usize = 57 + 16;

/// `g_aColorPalette` (Debugger_Color.cpp:36).
pub fn palette(i: usize) -> (u8, u8, u8) {
    if i == 0 {
        return (0, 0, 0);
    }
    if i < 57 {
        let ramp = (i - 1) / 8;
        let v = 255 - 32 * ((i - 1) % 8) as u8;
        let (r, g, b) = match ramp {
            0 => (v, 0, 0),
            1 => (0, v, 0),
            2 => (v, v, 0),
            3 => (0, 0, v),
            4 => (v, 0, v),
            5 => (0, v, v),
            _ => (v, v, v),
        };
        return (r, g, b);
    }
    match i - 57 {
        0 => (80, 192, 255),
        1 => (0, 128, 192),
        2 => (0, 64, 128),
        3 => (255, 128, 0),
        4 => (128, 64, 0),
        _ => (0, 0, 0),
    }
}

macro_rules! colors {
    ($( $name:ident = $idx:expr ),* $(,)?) => {
        /// `DebugColors_e` (Debugger_Color.h:51).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(usize)]
        #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
        pub enum DebugColor { $( $name ),* }
        impl DebugColor {
            pub const ALL: &'static [DebugColor] = &[ $( DebugColor::$name ),* ];
            pub const COUNT: usize = Self::ALL.len();
            pub fn name(self) -> &'static str { match self { $( DebugColor::$name => stringify!($name) ),* } }
        }
        /// `g_aColorIndex` (Debugger_Color.cpp:69): default palette index per color.
        const DEFAULT_INDEX: &[u8] = &[ $( $idx ),* ];
    };
}

colors! {
    BG_CONSOLE_OUTPUT = K0, FG_CONSOLE_OUTPUT = p(W, 8), BG_CONSOLE_INPUT = p(B, 2), FG_CONSOLE_INPUT = cu(1),
    BG_DISASM_1 = p(B, 2), BG_DISASM_2 = p(B, 3),
    BG_DISASM_BP_S_C = p(R, 8), FG_DISASM_BP_S_C = p(W, 8),
    BG_DISASM_BP_0_C = p(R, 6), FG_DISASM_BP_0_C = p(W, 5),
    FG_DISASM_BP_S_X = p(R, 7), FG_DISASM_BP_0_X = p(W, 5),
    BG_DISASM_C = p(W, 8), FG_DISASM_C = K0,
    BG_DISASM_PC_C = p(Y, 8), FG_DISASM_PC_C = K0,
    BG_DISASM_PC_X = p(Y, 4), FG_DISASM_PC_X = p(W, 8),
    BG_DISASM_BOOKMARK = p(C, 4), FG_DISASM_BOOKMARK = p(W, 8),
    FG_DISASM_ADDRESS = p(W, 8), FG_DISASM_OPERATOR = p(W, 6), FG_DISASM_OPCODE = p(Y, 8),
    FG_DISASM_MNEMONIC = p(W, 8), FG_DISASM_DIRECTIVE = p(M, 8), FG_DISASM_TARGET = cu(4),
    FG_DISASM_SYMBOL = p(G, 8), FG_DISASM_CHAR = p(C, 8), FG_DISASM_BRANCH = p(G, 8), FG_DISASM_SINT8 = cu(1),
    BG_INFO = p(C, 3), BG_INFO_WATCH = p(C, 3), BG_INFO_ZEROPAGE = p(C, 3),
    FG_INFO_TITLE = p(W, 8), FG_INFO_BULLET = p(Y, 7), FG_INFO_OPERATOR = p(W, 6),
    FG_INFO_ADDRESS = cu(4), FG_INFO_OPCODE = p(Y, 8), FG_INFO_REG = cu(1),
    BG_INFO_INVERSE = p(W, 8), FG_INFO_INVERSE = p(C, 3),
    BG_INFO_CHAR = p(C, 5), FG_INFO_CHAR_HI = p(W, 8), FG_INFO_CHAR_LO = p(Y, 8),
    BG_INFO_IO_BYTE = cu(4), FG_INFO_IO_BYTE = cu(4),
    FG_INFO_BP_MEM_READ = p(G, 8), FG_INFO_BP_MEM_WRITE = cu(4),
    FG_INFO_MEM_SLOT = p(Y, 8), BG_INFO_MEM_BANK = p(C, 3), FG_INFO_MEM_BANK = p(W, 8), FG_INFO_MEM_LC = p(C, 4),
    BG_DATA_1 = p(C, 1), BG_DATA_2 = p(C, 2), FG_DATA_BYTE = p(Y, 8), FG_DATA_TEXT = p(W, 8),
    BG_SYMBOLS_1 = p(G, 4), BG_SYMBOLS_2 = p(G, 3), FG_SYMBOLS_ADDRESS = p(W, 8), FG_SYMBOLS_NAME = p(M, 8),
    BG_SOURCE_TITLE = K0, FG_SOURCE_TITLE = p(W, 8), BG_SOURCE_1 = p(W, 2), BG_SOURCE_2 = p(W, 3), FG_SOURCE = p(W, 8),
    BG_VIDEOSCANNER_TITLE = p(C, 3), FG_VIDEOSCANNER_TITLE = p(W, 8),
    FG_VIDEOSCANNER_INVISIBLE = p(Y, 8), FG_VIDEOSCANNER_VISIBLE = p(G, 8),
    BG_IRQ_TITLE = p(C, 3), FG_IRQ_TITLE = p(R, 8),
    FG_SY6522_EVEN = cu(4), FG_SY6522_ODD = p(Y, 8), FG_AY8913_EVEN = cu(1), FG_AY8913_ODD = p(Y, 8),
    FG_AY8913_FUNCTION = p(R, 8), FG_INFO_ADDRESS_SY6522_AY8913_BAD = p(R, 8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Color,
    Mono,
    Bw,
}

pub type Rgb = (u8, u8, u8);

/// The three color tables (`g_aColors`) and the scheme in use.
#[derive(Clone, Debug)]
pub struct ColorTable {
    pub scheme: Scheme,
    colors: [Vec<Rgb>; 3],
}

impl Default for ColorTable {
    fn default() -> Self {
        let mut t = ColorTable { scheme: Scheme::Color, colors: [vec![], vec![], vec![]] };
        t.reset();
        t
    }
}

impl ColorTable {
    /// `ConfigColorsReset` (Debugger_Color.cpp:230): mono = average, B&W = threshold 64, with the
    /// GH#1231 contrast fix for `FG_DISASM_PC_X`.
    pub fn reset(&mut self) {
        let mut col = Vec::new();
        let mut mono = Vec::new();
        let mut bw: Vec<Rgb> = Vec::new();
        for (i, idx) in DEFAULT_INDEX.iter().enumerate() {
            let (r, g, b) = palette(*idx as usize);
            let m = ((r as u32 + g as u32 + b as u32) / 3) as u8;
            let mut v: u8 = if m < 64 { 0 } else { 255 };
            if i == DebugColor::FG_DISASM_PC_X as usize {
                let prev = bw[i - 1].0;
                let contrast = 0xFF - prev;
                if (v as i32 - prev as i32).abs() < 0x80 {
                    v = contrast;
                }
            }
            col.push((r, g, b));
            mono.push((m, m, m));
            bw.push((v, v, v));
        }
        self.colors = [col, mono, bw];
    }

    pub fn get(&self, c: DebugColor) -> Rgb {
        self.colors[self.scheme as usize][c as usize]
    }

    /// `DebuggerSetColor`.
    pub fn set(&mut self, scheme: Scheme, c: DebugColor, rgb: Rgb) {
        self.colors[scheme as usize][c as usize] = rgb;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_table_matches_applewin_values() {
        let t = ColorTable::default();
        assert_eq!(t.get(DebugColor::BG_DISASM_PC_C), (255, 255, 0)); // Y8
        assert_eq!(t.get(DebugColor::BG_INFO), (0, 95, 95)); // C3
        assert_eq!(t.get(DebugColor::FG_DISASM_TARGET), (255, 128, 0));
        assert_eq!(t.get(DebugColor::FG_DISASM_OPERATOR), (191, 191, 191)); // G192 = W6
        assert_eq!(DebugColor::ALL.len(), DEFAULT_INDEX.len());
    }

    #[test]
    fn mono_and_bw_schemes() {
        let mut t = ColorTable::default();
        t.scheme = Scheme::Mono;
        let (r, g, b) = t.get(DebugColor::FG_DISASM_TARGET);
        assert!(r == g && g == b);
        t.scheme = Scheme::Bw;
        let (r, g, b) = t.get(DebugColor::FG_CONSOLE_OUTPUT);
        assert_eq!((r, g, b), (255, 255, 255));
    }
}
