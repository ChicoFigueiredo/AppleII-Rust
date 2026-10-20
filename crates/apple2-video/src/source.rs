//! The read-only view of a machine that the renderer reads, so rendering works headless and
//! against fakes in tests.

use apple2_core::memory::mf;
use apple2_core::{AuxCardKind, Machine, Model, VideoMode, VideoStandard};

pub trait VideoSource {
    /// Main RAM, ignoring paging (`MemGetMainPtr`).
    fn main(&self, addr: u16) -> u8;
    /// Auxiliary RAM of the current bank, ignoring paging (`MemGetAuxPtr`).
    fn aux(&self, addr: u16) -> u8;
    fn model(&self) -> Model;
    fn standard(&self) -> VideoStandard;
    fn aux_card(&self) -> AuxCardKind;
    /// TEXT, MIXED, 80COL, ALTCHARSET, DHIRES and the J-Plus AN2 switch.
    fn mode(&self) -> VideoMode;
    /// `MF_*` paging flags; the renderer reads 80STORE, PAGE2 and HIRES.
    fn memmode(&self) -> u32;
    /// The byte the video generator is fetching (for 80-column text with an empty aux slot).
    fn floating_bus(&self) -> u8;
    /// Cycle of the video frame, for [`crate::Renderer::resync`].
    fn cycles_in_frame(&self) -> u32;
}

impl VideoSource for Machine {
    fn main(&self, addr: u16) -> u8 {
        self.peek_main(addr)
    }
    fn aux(&self, addr: u16) -> u8 {
        self.peek_aux(addr)
    }
    fn model(&self) -> Model {
        self.config().model
    }
    fn standard(&self) -> VideoStandard {
        self.config().video_standard
    }
    fn aux_card(&self) -> AuxCardKind {
        self.config().aux_card
    }
    fn mode(&self) -> VideoMode {
        self.board().video
    }
    fn memmode(&self) -> u32 {
        Machine::memmode(self)
    }
    fn floating_bus(&self) -> u8 {
        self.peek(0xC000)
    }
    fn cycles_in_frame(&self) -> u32 {
        Machine::cycles_in_frame(self)
    }
}

/// AppleWin's `VF_*` video mode flags, as `NTSC_SetVideoMode` sees them (Video.h:54).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VideoFlags {
    pub text: bool,
    pub mixed: bool,
    pub hires: bool,
    pub dhires: bool,
    pub col80: bool,
    pub page2: bool,
    pub store80: bool,
    /// `VF_80COL_AUX_EMPTY`: 80COL on with no aux card, so the aux bytes are the floating bus.
    pub aux_empty: bool,
    pub altcharset: bool,
    /// J-Plus AN2 (selects the ASCII half of its character ROM).
    pub jplus_ascii: bool,
}

/// Raw switches and the flags passed on to the NTSC renderer (`Video::VideoSetMode`, Video.cpp:179).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeSnapshot {
    /// `g_uVideoMode` after the aux-card adjustments.
    pub raw: VideoFlags,
    /// What `NTSC_SetVideoMode` receives.
    pub ntsc: VideoFlags,
}

pub fn mode_snapshot(s: &dyn VideoSource) -> ModeSnapshot {
    let m = s.mode();
    let mm = s.memmode();
    let model = s.model();
    let mut raw = VideoFlags {
        text: m.text,
        mixed: m.mixed,
        hires: mm & mf::HIRES != 0,
        dhires: m.dhires,
        col80: m.col80,
        page2: mm & mf::PAGE2 != 0,
        store80: mm & mf::STORE80 != 0,
        aux_empty: false,
        altcharset: m.altcharset,
        jplus_ascii: m.jplus_ascii,
    };
    let mut ntsc = raw;
    let aux = s.aux_card();
    if !model.is_apple2() && matches!(aux, AuxCardKind::Empty | AuxCardKind::Col80_1K) {
        raw.dhires = false;
        if aux == AuxCardKind::Empty {
            raw.aux_empty = raw.col80;
        }
        ntsc = raw;
        if !ntsc.text {
            ntsc.col80 = false; // aux empty or 80col card && not TEXT: the 80COL switch is ignored
        }
    }
    ModeSnapshot { raw, ntsc }
}
