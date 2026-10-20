//! Test fixtures: a fake [`VideoSource`] with plain 64K main and aux RAM.

#![allow(dead_code)]

use apple2_core::memory::mf;
use apple2_core::{AuxCardKind, Model, VideoMode, VideoStandard};
use apple2_video::{RenderConfig, Renderer, VideoSource, VideoType};

pub struct Fake {
    pub main: Vec<u8>,
    pub aux: Vec<u8>,
    pub model: Model,
    pub standard: VideoStandard,
    pub aux_card: AuxCardKind,
    pub mode: VideoMode,
    pub memmode: u32,
    pub cycle: u32,
}

impl Fake {
    pub fn new(model: Model) -> Self {
        Fake {
            main: vec![0; 0x10000],
            aux: vec![0; 0x10000],
            model,
            standard: VideoStandard::Ntsc,
            aux_card: if model.is_apple2e_family() { AuxCardKind::Extended80Col } else { AuxCardKind::Empty },
            mode: VideoMode::default(),
            memmode: mf::INITIAL,
            cycle: 0,
        }
    }

    pub fn text(mut self) -> Self {
        self.mode.text = true;
        self
    }

    pub fn hires(mut self) -> Self {
        self.mode.text = false;
        self.memmode |= mf::HIRES;
        self
    }

    pub fn lores(mut self) -> Self {
        self.mode.text = false;
        self
    }

    /// Writes `bytes` at `addr` of main RAM.
    pub fn poke(&mut self, addr: u16, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.main[addr as usize + i] = *b;
        }
    }

    pub fn poke_aux(&mut self, addr: u16, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.aux[addr as usize + i] = *b;
        }
    }
}

impl VideoSource for Fake {
    fn main(&self, addr: u16) -> u8 {
        self.main[addr as usize]
    }
    fn aux(&self, addr: u16) -> u8 {
        self.aux[addr as usize]
    }
    fn model(&self) -> Model {
        self.model
    }
    fn standard(&self) -> VideoStandard {
        self.standard
    }
    fn aux_card(&self) -> AuxCardKind {
        self.aux_card
    }
    fn mode(&self) -> VideoMode {
        self.mode
    }
    fn memmode(&self) -> u32 {
        self.memmode
    }
    fn floating_bus(&self) -> u8 {
        0
    }
    fn cycles_in_frame(&self) -> u32 {
        self.cycle
    }
}

pub fn renderer(video_type: VideoType) -> Renderer {
    Renderer::new(RenderConfig { video_type, ..Default::default() })
}

/// Renders `frames` whole frames of the unchanging `s`.
pub fn render(r: &mut Renderer, s: &Fake, frames: u32) {
    for _ in 0..frames {
        r.redraw(s);
    }
}

/// Is the pixel not (nearly) black? The TV luma filter leaves a residue of 1/255 in "black".
pub fn lit(r: &Renderer, x: usize, y: usize) -> bool {
    let p = r.pixel(x, y);
    p[0] > 8 || p[1] > 8 || p[2] > 8
}

/// Rows `y` of the visible area as a string of '#' (lit) and '.' for pixels `x0..x0+w`.
pub fn art(r: &Renderer, x0: usize, y: usize, w: usize) -> String {
    (x0..x0 + w).map(|x| if lit(r, x, y) { '#' } else { '.' }).collect()
}

pub fn rgb(r: &Renderer, x: usize, y: usize) -> u32 {
    let p = r.pixel(x, y);
    (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32
}
