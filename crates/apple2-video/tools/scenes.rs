//! Deterministic video scenes for the AppleWin differential (HU-0201/HU-0202).
//!
//! A scene is a text file (`tests/conformance/inputs/video/<name>.scene`, "scene v1") with RAM
//! segments and an entry point; the oracle (`audetto-oracle --scene`) and the Rust harness
//! (`conformance scene-fb`) load the same bytes right after power-on and start the CPU at `entry`
//! at cycle 0. The 6502 code is hand-assembled here (no ROM code runs).
//!
//! Included by `examples/gen_scenes.rs` (writes the files) and `tests/scene_files.rs` (checks the
//! committed files are what this module generates).

#![allow(dead_code)]

pub struct SceneDef {
    pub name: String,
    /// `apple2ee`, `apple2e` or `apple2p` (oracle `--model` names).
    pub model: &'static str,
    pub notes: String,
    pub main: Vec<(u16, Vec<u8>)>,
    pub aux: Vec<(u16, Vec<u8>)>,
    pub code: Vec<u8>,
    /// Slot of the VidHD the scene needs (framebuffer 640x400, Super Hi-Res).
    pub vidhd: Option<u8>,
}

pub const ORG: u16 = 0x0300;

// ---- Soft switches ($C0xx) ----
pub const STORE80_OFF: u16 = 0xC000;
pub const STORE80_ON: u16 = 0xC001;
pub const COL80_OFF: u16 = 0xC00C;
pub const COL80_ON: u16 = 0xC00D;
pub const ALT_OFF: u16 = 0xC00E;
pub const ALT_ON: u16 = 0xC00F;
pub const GR: u16 = 0xC050;
pub const TEXT: u16 = 0xC051;
pub const FULL: u16 = 0xC052;
pub const MIXED: u16 = 0xC053;
pub const PAGE1: u16 = 0xC054;
pub const PAGE2: u16 = 0xC055;
pub const LORES: u16 = 0xC056;
pub const HIRES: u16 = 0xC057;
pub const AN3_ON: u16 = 0xC05E; // DHIRES on
pub const AN3_OFF: u16 = 0xC05F;

/// Tiny assembler: only what the scenes need.
#[derive(Default)]
pub struct Asm {
    pub bytes: Vec<u8>,
}

impl Asm {
    pub fn sta(&mut self, addr: u16) -> &mut Self {
        self.bytes.extend([0x8D, addr as u8, (addr >> 8) as u8]);
        self
    }
    /// `n` NOPs (2 cycles each).
    pub fn nops(&mut self, n: usize) -> &mut Self {
        self.bytes.extend(std::iter::repeat_n(0xEA, n));
        self
    }
    pub fn jmp(&mut self, addr: u16) -> &mut Self {
        self.bytes.extend([0x4C, addr as u8, (addr >> 8) as u8]);
        self
    }
    /// `JMP *`.
    pub fn halt(&mut self) -> &mut Self {
        let here = ORG + self.bytes.len() as u16;
        self.jmp(here)
    }
    pub fn here(&self) -> u16 {
        ORG + self.bytes.len() as u16
    }
    /// Sets each soft switch (STA, 4 cycles each).
    pub fn set(&mut self, switches: &[u16]) -> &mut Self {
        for &s in switches {
            self.sta(s);
        }
        self
    }
    /// `LDX #n; L: DEX; BNE L` : 2 + 5n - 1 cycles.
    pub fn delay_loop(&mut self, n: u8) -> &mut Self {
        self.bytes.extend([0xA2, n, 0xCA, 0xD0, 0xFD]);
        self
    }
}

// ---- Data helpers ----

struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 24) as u8
    }
}

fn noise(seed: u32, n: usize) -> Vec<u8> {
    let mut g = Lcg(seed);
    (0..n).map(|_| g.next()).collect()
}

fn text_addr(base: u16, row: usize) -> u16 {
    base + ((row % 8) * 0x80 + (row / 8) * 0x28) as u16
}

fn hgr_addr(base: u16, y: usize) -> u16 {
    base + ((y % 8) * 0x400 + ((y / 8) % 8) * 0x80 + (y / 64) * 0x28) as u16
}

/// A text/lores page as 24 rows of 40 bytes, `f(row, col)`.
fn text_page(base: u16, f: impl Fn(usize, usize) -> u8) -> Vec<(u16, Vec<u8>)> {
    (0..24)
        .map(|r| (text_addr(base, r), (0..40).map(|c| f(r, c)).collect()))
        .collect()
}

/// A hi-res page as 192 lines of 40 bytes, `f(y, col)`.
fn hgr_page(base: u16, mut f: impl FnMut(usize, usize) -> u8) -> Vec<(u16, Vec<u8>)> {
    let mut out = Vec::new();
    for y in 0..192 {
        let row: Vec<u8> = (0..40).map(|c| f(y, c)).collect();
        out.push((hgr_addr(base, y), row));
    }
    out
}

/// 16 bands of 12 lines, each a colour-artefact pattern; bit 7 and the byte phase both vary.
fn bars(y: usize, col: usize) -> u8 {
    let even = col.is_multiple_of(2);
    match (y / 12) % 16 {
        0 => 0x55,
        1 => 0xAA,
        2 => 0xD5,
        3 => 0xAA | 0x80,
        4 => 0x2A,
        5 => 0x7F,
        6 => 0xFF,
        7 => 0x80,
        8 => {
            if even {
                0x55
            } else {
                0xD5
            }
        }
        9 => {
            if even {
                0x2A
            } else {
                0xAA
            }
        }
        10 => {
            if even {
                0x7F
            } else {
                0x00
            }
        }
        11 => 0x01 << (col % 7),
        12 => 0x03 << (col % 6),
        13 => 0x81 << (col % 6),
        14 => {
            if (col / 5).is_multiple_of(2) {
                0xF0
            } else {
                0x0F
            }
        }
        _ => (col as u8).wrapping_mul(7) ^ (y as u8),
    }
}

fn sequence(r: usize, c: usize) -> u8 {
    ((r * 40 + c) & 0xFF) as u8
}

fn flatten(parts: Vec<Vec<(u16, Vec<u8>)>>) -> Vec<(u16, Vec<u8>)> {
    parts.into_iter().flatten().collect()
}

fn scene(
    name: &str,
    model: &'static str,
    notes: &str,
    main: Vec<(u16, Vec<u8>)>,
    aux: Vec<(u16, Vec<u8>)>,
    code: Asm,
) -> SceneDef {
    SceneDef {
        name: name.into(),
        model,
        notes: notes.into(),
        main,
        aux,
        code: code.bytes,
        vidhd: None,
    }
}

/// A Super Hi-Res picture in aux memory (VidHD, NEWVIDEO = $C1): 200 lines of 160 bytes at
/// $2000, scan-line control bytes at $9D00 and sixteen palettes of sixteen 0RGB words at $9E00.
fn shr_scene(
    name: &str,
    notes: &str,
    seed: u32,
    ctrl: impl Fn(usize) -> u8,
    fill_pixels: bool,
) -> SceneDef {
    let mut g = Lcg(seed);
    let mut aux = Vec::new();
    for line in 0..200usize {
        let row: Vec<u8> = (0..160)
            .map(|i| {
                if fill_pixels && i % 8 != 0 {
                    0
                } else {
                    g.next()
                }
            })
            .collect();
        aux.push((0x2000 + (line * 160) as u16, row));
    }
    aux.push((0x9D00, (0..200).map(&ctrl).collect()));
    for pal in 0..16u16 {
        let mut bytes = Vec::new();
        for _ in 0..16 {
            let w = u16::from(g.next()) << 8 | u16::from(g.next());
            let w = w & 0x0FFF;
            bytes.extend([w as u8, (w >> 8) as u8]);
        }
        aux.push((0x9E00 + pal * 32, bytes));
    }
    let mut a = Asm::default();
    a.bytes.extend([0xA9, 0xC1]); // LDA #$C1
    a.sta(0xC029).halt(); // NEWVIDEO: SHR on, linear
    let mut sc = scene(name, "apple2ee", notes, vec![], aux, a);
    sc.vidhd = Some(2);
    sc
}

fn static_code(switches: &[u16]) -> Asm {
    let mut a = Asm::default();
    a.set(switches).halt();
    a
}

/// A loop of exactly 65 cycles (one scan line): `first`, `d1` cycles of NOPs, `second`, the rest.
/// The switches therefore toggle at a fixed horizontal position of every line.
fn line_loop(setup: &[u16], first: u16, second: u16, d1: usize) -> Asm {
    assert!(d1.is_multiple_of(2) && d1 <= 54);
    let mut a = Asm::default();
    a.set(setup);
    let top = a.here();
    a.sta(first)
        .nops(d1 / 2)
        .sta(second)
        .nops((54 - d1) / 2)
        .jmp(top);
    a
}

pub fn all() -> Vec<SceneDef> {
    let mut v = Vec::new();

    // ---- Text ----
    let t40 = text_page(0x400, sequence);
    let t40_p2 = text_page(0x800, |r, c| sequence(r, c).wrapping_add(0x55));
    let t40_all = flatten(vec![t40.clone(), t40_p2.clone()]);
    v.push(scene(
        "text40",
        "apple2ee",
        "40 columns, all 256 character codes (inverse, flash, normal), primary character set",
        t40.clone(),
        vec![],
        static_code(&[TEXT]),
    ));
    v.push(scene(
        "text40-alt",
        "apple2ee",
        "40 columns, alternate character set (MouseText), ALTCHARSET on",
        t40.clone(),
        vec![],
        static_code(&[ALT_ON, TEXT]),
    ));
    v.push(scene(
        "text40-page2",
        "apple2ee",
        "40 columns, page 2 selected",
        t40_all.clone(),
        vec![],
        static_code(&[TEXT, PAGE2]),
    ));
    let aux_t = text_page(0x400, |r, c| sequence(r, c).wrapping_add(97));
    v.push(scene(
        "text80",
        "apple2ee",
        "80 columns (aux then main), primary character set",
        t40.clone(),
        aux_t.clone(),
        static_code(&[COL80_ON, TEXT]),
    ));
    v.push(scene(
        "text80-alt",
        "apple2ee",
        "80 columns, alternate character set",
        t40.clone(),
        aux_t.clone(),
        static_code(&[ALT_ON, COL80_ON, TEXT]),
    ));
    v.push(scene(
        "text80-page2",
        "apple2ee",
        "80 columns, page 2",
        t40_all.clone(),
        text_page(0x800, |r, c| sequence(r, c).wrapping_add(33)),
        static_code(&[COL80_ON, TEXT, PAGE2]),
    ));
    v.push(scene(
        "text40-ii-plus",
        "apple2p",
        "Apple II+ 40 columns (flash/inverse/normal/low-case-less ROM)",
        t40.clone(),
        vec![],
        static_code(&[TEXT]),
    ));
    v.push(scene(
        "text40-iie",
        "apple2e",
        "unenhanced Apple IIe 40 columns, primary set",
        t40.clone(),
        vec![],
        static_code(&[TEXT]),
    ));
    v.push(scene(
        "text40-iie-alt",
        "apple2e",
        "unenhanced Apple IIe 40 columns, alternate set",
        t40.clone(),
        vec![],
        static_code(&[ALT_ON, TEXT]),
    ));

    // ---- Lo-res ----
    let lores = text_page(0x400, |r, c| ((r * 7 + c * 3) & 0xFF) as u8);
    let lores_all = flatten(vec![
        lores.clone(),
        text_page(0x800, |r, c| ((r * 5 + c * 11 + 1) & 0xFF) as u8),
    ]);
    v.push(scene(
        "lores",
        "apple2ee",
        "lo-res full screen",
        lores_all.clone(),
        vec![],
        static_code(&[GR, FULL, LORES]),
    ));
    v.push(scene(
        "lores-mixed",
        "apple2ee",
        "lo-res with 4 text lines (mixed mode)",
        lores_all.clone(),
        vec![],
        static_code(&[GR, MIXED, LORES]),
    ));
    v.push(scene(
        "lores-page2",
        "apple2ee",
        "lo-res page 2",
        lores_all.clone(),
        vec![],
        static_code(&[GR, FULL, LORES, PAGE2]),
    ));
    v.push(scene(
        "lores-ii-plus",
        "apple2p",
        "Apple II+ lo-res mixed",
        lores_all.clone(),
        vec![],
        static_code(&[GR, MIXED, LORES]),
    ));
    let dl_aux = text_page(0x400, |r, c| ((r * 13 + c * 5 + 3) & 0xFF) as u8);
    v.push(scene(
        "dlores",
        "apple2ee",
        "double lo-res (80COL, AN3 off, lo-res)",
        lores_all.clone(),
        dl_aux.clone(),
        static_code(&[GR, FULL, LORES, COL80_ON, AN3_ON]),
    ));
    v.push(scene(
        "dlores-mixed",
        "apple2ee",
        "double lo-res with 4 text lines (80 columns)",
        lores_all.clone(),
        dl_aux.clone(),
        static_code(&[GR, MIXED, LORES, COL80_ON, AN3_ON]),
    ));

    // ---- Hi-res ----
    let hgr_bars = flatten(vec![
        hgr_page(0x2000, bars),
        hgr_page(0x4000, |y, c| bars(y + 12 * 3, c ^ 1)),
        t40.clone(),
    ]);
    let hgr_noise = {
        let mut g = Lcg(12345);
        let p1 = hgr_page(0x2000, |_, _| g.next());
        let p2 = hgr_page(0x4000, |_, _| g.next());
        flatten(vec![p1, p2, t40.clone()])
    };
    v.push(scene(
        "hires-bars",
        "apple2ee",
        "hi-res colour-artefact bands (bit 7, byte phase)",
        hgr_bars.clone(),
        vec![],
        static_code(&[GR, FULL, HIRES]),
    ));
    v.push(scene(
        "hires-noise",
        "apple2ee",
        "hi-res pseudo-random bytes",
        hgr_noise.clone(),
        vec![],
        static_code(&[GR, FULL, HIRES]),
    ));
    v.push(scene(
        "hires-mixed",
        "apple2ee",
        "hi-res with 4 text lines",
        hgr_noise.clone(),
        vec![],
        static_code(&[GR, MIXED, HIRES]),
    ));
    v.push(scene(
        "hires-page2",
        "apple2ee",
        "hi-res page 2",
        hgr_bars.clone(),
        vec![],
        static_code(&[GR, FULL, HIRES, PAGE2]),
    ));
    v.push(scene(
        "hires-bars-ii-plus",
        "apple2p",
        "Apple II+ hi-res bands",
        hgr_bars.clone(),
        vec![],
        static_code(&[GR, FULL, HIRES]),
    ));
    v.push(scene(
        "hires-mixed-ii-plus",
        "apple2p",
        "Apple II+ hi-res mixed",
        hgr_noise.clone(),
        vec![],
        static_code(&[GR, MIXED, HIRES]),
    ));

    // ---- Double hi-res ----
    let dhgr_main = hgr_page(0x2000, bars);
    let dhgr_aux = {
        let mut g = Lcg(777);
        hgr_page(0x2000, |_, _| g.next())
    };
    let dhgr_main_noise = {
        let mut g = Lcg(4242);
        hgr_page(0x2000, |_, _| g.next())
    };
    let dh_regs = [GR, FULL, HIRES, COL80_ON, AN3_ON];
    v.push(scene(
        "dhires-bars",
        "apple2ee",
        "double hi-res, main bands over aux noise",
        flatten(vec![dhgr_main.clone(), t40.clone()]),
        dhgr_aux.clone(),
        static_code(&dh_regs),
    ));
    v.push(scene(
        "dhires-noise",
        "apple2ee",
        "double hi-res, noise in main and aux",
        flatten(vec![dhgr_main_noise.clone(), t40.clone()]),
        dhgr_aux.clone(),
        static_code(&dh_regs),
    ));
    v.push(scene(
        "dhires-mixed",
        "apple2ee",
        "double hi-res with 4 text lines (80 columns)",
        flatten(vec![dhgr_main_noise.clone(), t40.clone()]),
        flatten(vec![dhgr_aux.clone(), aux_t.clone()]),
        static_code(&[GR, MIXED, HIRES, COL80_ON, AN3_ON]),
    ));
    v.push(scene(
        "dhires-80store-page2",
        "apple2ee",
        "double hi-res with 80STORE+PAGE2 (aux/main swap on the visible page)",
        flatten(vec![dhgr_main_noise.clone(), t40.clone()]),
        dhgr_aux.clone(),
        static_code(&[STORE80_ON, GR, FULL, HIRES, COL80_ON, AN3_ON, PAGE2]),
    ));

    // ---- RGB card modes (AN3 toggled twice with 80COL on/off: F2 F1 = !80COL at each 5E->5F) ----
    // mode 0 (140): 80COL on twice; mode 1 (160): on then off; mode 2 (mixed): off then on; mode 3 (560): off twice.
    for (mode, (first, second)) in [
        (COL80_ON, COL80_ON),
        (COL80_ON, COL80_OFF),
        (COL80_OFF, COL80_ON),
        (COL80_OFF, COL80_OFF),
    ]
    .into_iter()
    .enumerate()
    {
        let mut a = Asm::default();
        a.set(&[
            GR, FULL, HIRES, first, AN3_ON, AN3_OFF, second, AN3_ON, AN3_OFF, COL80_ON, AN3_ON,
        ]);
        a.halt();
        v.push(scene(
            &format!("rgb-mode{mode}-dhires"),
            "apple2ee",
            "RGB card mode latched by two AN3 toggles (mode 0=140, 1=160, 2=mixed, 3=560), then double hi-res",
            flatten(vec![dhgr_main_noise.clone(), t40.clone()]),
            dhgr_aux.clone(),
            a,
        ));
        let mut a = Asm::default();
        a.set(&[
            GR, MIXED, HIRES, first, AN3_ON, AN3_OFF, second, AN3_ON, AN3_OFF, COL80_ON, AN3_ON,
        ]);
        a.halt();
        v.push(scene(
            &format!("rgb-mode{mode}-dhires-mixed"),
            "apple2ee",
            "as rgb-modeN-dhires but with 4 text lines",
            flatten(vec![dhgr_main_noise.clone(), t40.clone()]),
            flatten(vec![dhgr_aux.clone(), aux_t.clone()]),
            a,
        ));
        let mut a = Asm::default();
        a.set(&[
            GR, FULL, HIRES, first, AN3_ON, AN3_OFF, second, AN3_ON, AN3_OFF, COL80_OFF,
        ]);
        a.halt();
        v.push(scene(
            &format!("rgb-mode{mode}-hires"),
            "apple2ee",
            "RGB card mode N with plain (single) hi-res",
            hgr_noise.clone(),
            vec![],
            a,
        ));
    }
    {
        let mut a = Asm::default();
        a.set(&[COL80_ON, TEXT]);
        a.halt();
        v.push(scene(
            "rgb-text80",
            "apple2ee",
            "RGB card, 80-column text",
            t40.clone(),
            aux_t.clone(),
            a,
        ));
    }

    // ---- Timed scenes (mode changes inside the frame) ----
    // Hi-res graphics with text switched in and out at horizontal position ~ d1 of every line.
    for d1 in [0usize, 10, 20, 24, 28, 40, 54] {
        v.push(scene(
            &format!("midline-hgr-text-{d1}"),
            "apple2ee",
            "every 65-cycle line: TEXT on, d1 cycles later GR on (#1443/#1131: mode change mid-line)",
            hgr_noise.clone(),
            vec![],
            line_loop(&[GR, FULL, HIRES], TEXT, GR, d1),
        ));
    }
    // #1443, "Homer choking Homer": HGR then text later in the line, and text then HGR.
    v.push(scene(
        "midline-1443-hgr-then-text",
        "apple2ee",
        "AppleWin GH#1443: line starts in HGR, switches to TEXT at hpos ~23 (HGR side black&white afterwards in Color TV)",
        hgr_bars.clone(),
        vec![],
        line_loop(&[GR, FULL, HIRES], GR, TEXT, 46),
    ));
    v.push(scene(
        "midline-1443-text-then-hgr",
        "apple2ee",
        "AppleWin GH#1443: line starts in TEXT, switches to HGR at hpos ~23 (HGR coloured)",
        hgr_bars.clone(),
        vec![],
        line_loop(&[GR, FULL, HIRES], TEXT, GR, 46),
    ));
    for (n, (a, b)) in [
        ("lores-hires", (LORES, HIRES)),
        ("page1-page2", (PAGE1, PAGE2)),
        ("col40-col80", (COL80_OFF, COL80_ON)),
        ("full-mixed", (FULL, MIXED)),
        ("dhires-off-on", (AN3_OFF, AN3_ON)),
    ] {
        let mut data_main = hgr_noise.clone();
        data_main.extend(lores_all.clone());
        v.push(scene(
            &format!("midline-{n}"),
            "apple2ee",
            "a soft-switch pair toggled twice per scan line at fixed horizontal positions",
            data_main,
            flatten(vec![dhgr_aux.clone(), aux_t.clone()]),
            line_loop(&[GR, FULL, HIRES, COL80_ON], a, b, 24),
        ));
    }
    // Flash with a mode change in the middle of the frame (AUD-P-003): text/graphics once per frame.
    {
        let mut a = Asm::default();
        a.set(&[TEXT]);
        let top = a.here();
        a.delay_loop(255).delay_loop(255).delay_loop(255); // ~3 * 1275 cycles
        for _ in 0..4 {
            a.delay_loop(255).delay_loop(255).delay_loop(255);
        }
        a.sta(GR).sta(FULL).sta(HIRES);
        a.delay_loop(255).delay_loop(255).delay_loop(255);
        a.set(&[TEXT]);
        a.jmp(top);
        v.push(scene(
            "flash-mode-change",
            "apple2ee",
            "flash text with TEXT <-> HIRES changing in the middle of frames",
            hgr_noise.clone(),
            vec![],
            a,
        ));
    }
    // Page flip in the middle of the frame, every ~ 5000 cycles.
    {
        let mut a = Asm::default();
        a.set(&[GR, FULL, HIRES]);
        let top = a.here();
        a.sta(PAGE2);
        a.delay_loop(255)
            .delay_loop(255)
            .delay_loop(255)
            .delay_loop(255);
        a.sta(PAGE1);
        a.delay_loop(255).delay_loop(255).delay_loop(200);
        a.jmp(top);
        v.push(scene(
            "pageflip-midframe",
            "apple2ee",
            "HIRES page 1/2 switched several times per frame",
            hgr_bars.clone(),
            vec![],
            a,
        ));
    }
    // 80-column text with the mode switched to graphics for part of the frame (borders / junction).
    {
        let mut a = Asm::default();
        a.set(&[COL80_ON, TEXT]);
        let top = a.here();
        a.delay_loop(255)
            .delay_loop(255)
            .delay_loop(255)
            .delay_loop(255);
        a.set(&[GR, MIXED, LORES]);
        a.delay_loop(255).delay_loop(255).delay_loop(255);
        a.set(&[TEXT]);
        a.jmp(top);
        v.push(scene(
            "text80-lores-midframe",
            "apple2ee",
            "80-column text with a lo-res mixed interval in the middle of the frame",
            flatten(vec![lores_all.clone(), t40.clone()]),
            aux_t.clone(),
            a,
        ));
    }

    // ---- Super Hi-Res (VidHD, HU-0202) ----
    v.push(shr_scene(
        "shr-320-noise",
        "SHR 320x200, a palette per line, random pixels and palettes",
        99,
        |l| (l % 16) as u8,
        false,
    ));
    v.push(shr_scene(
        "shr-640-noise",
        "SHR 640x200 (dithered four-colour columns), random pixels",
        100,
        |l| 0x80 | (l % 16) as u8,
        false,
    ));
    v.push(shr_scene(
        "shr-mixed-control",
        "SHR with 320/640 and colour-fill flags changing line by line",
        101,
        |l| ((l * 37) as u8 & 0xAF) | (l % 16) as u8,
        false,
    ));
    v.push(shr_scene(
        "shr-fill",
        "SHR 320 colour-fill mode (index 0 repeats the previous colour)",
        102,
        |l| 0x20 | (l % 16) as u8,
        true,
    ));

    v
}

fn hex_lines(kind: &str, segs: &[(u16, Vec<u8>)], out: &mut String) {
    for (addr, bytes) in segs {
        for (i, chunk) in bytes.chunks(40).enumerate() {
            out.push_str(&format!("{kind} {:04X}", *addr as usize + i * 40));
            for b in chunk {
                out.push_str(&format!(" {b:02X}"));
            }
            out.push('\n');
        }
    }
}

impl SceneDef {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s.push_str(
            "# scene v1 (generated by crates/apple2-video/examples/gen_scenes.rs; do not edit)\n",
        );
        s.push_str(&format!("name {}\n", self.name));
        s.push_str(&format!("model {}\n", self.model));
        if let Some(slot) = self.vidhd {
            s.push_str(&format!("vidhd {slot}\n"));
        }
        s.push_str(&format!("# {}\n", self.notes));
        hex_lines("main", &self.main, &mut s);
        hex_lines("aux", &self.aux, &mut s);
        hex_lines("main", &[(ORG, self.code.clone())], &mut s);
        s.push_str(&format!("entry {ORG:04X}\n"));
        s
    }
}
