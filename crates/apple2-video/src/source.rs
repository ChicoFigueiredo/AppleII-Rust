//! The read-only view of a machine that the renderer reads, so rendering works headless and
//! against fakes in tests.

use apple2_core::memory::mf;
use apple2_core::{AuxCardKind, Machine, Model, VideoMode, VideoStandard};

/// What the renderer needs from a VidHD card (AppleWin `VidHDCard`). `apple2-video` does not
/// depend on `apple2-cards`: the cards crate implements this trait for its `VidHd` (so the
/// IIgs `$0RGB` palette decode lives in one place), and a caller hands the view to the
/// renderer with [`WithVidHd`] or its own [`VideoSource::vidhd`].
pub trait VidHdView {
    /// `IsSHR` (VidHD.h:30): NEWVIDEO b7 and b6, i.e. `VF_SHR`.
    fn is_shr(&self) -> bool;
    /// `UpdateSHRCell` (VidHD.cpp:130): the 16 pixels of four SHR bytes `a` (first byte in the low
    /// bits) given the scanline's 16 IIgs palette entries; `prev` is the pixel to the left (colour
    /// fill).
    fn shr_cell(
        &self,
        is_640: bool,
        color_fill: bool,
        palette: &[u16; 16],
        a: u32,
        prev: [u8; 3],
    ) -> [[u8; 3]; 16];
}

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
    /// The VidHD card, if one is installed: the framebuffer is then 640x400 (Video.cpp:96-103)
    /// with the older modes centred, and Super Hi-Res is drawn while [`VidHdView::is_shr`].
    fn vidhd(&self) -> Option<&dyn VidHdView> {
        None
    }
    /// Debugger pseudo hires page (`VF_PAGE0/3/4/5/6/7/8`, NTSC.cpp:2039-2069): the page index into
    /// `aPageAddr` = `$0000 $2000 $4000 $6000 $8000 $A000 $C000 $D000 $E000`. `None` = the
    /// page the switches select.
    fn hgr_pseudo_page(&self) -> Option<u8> {
        None
    }
}

/// A [`VideoSource`] plus a VidHD view (the machine itself knows nothing of cards' concrete types).
pub struct WithVidHd<'a> {
    pub source: &'a dyn VideoSource,
    pub vidhd: &'a dyn VidHdView,
}

macro_rules! delegate {
    ($field:ident) => {
        fn main(&self, addr: u16) -> u8 {
            self.$field.main(addr)
        }
        fn aux(&self, addr: u16) -> u8 {
            self.$field.aux(addr)
        }
        fn model(&self) -> Model {
            self.$field.model()
        }
        fn standard(&self) -> VideoStandard {
            self.$field.standard()
        }
        fn aux_card(&self) -> AuxCardKind {
            self.$field.aux_card()
        }
        fn floating_bus(&self) -> u8 {
            self.$field.floating_bus()
        }
        fn cycles_in_frame(&self) -> u32 {
            self.$field.cycles_in_frame()
        }
    };
}

impl VideoSource for WithVidHd<'_> {
    delegate!(source);
    fn mode(&self) -> VideoMode {
        self.source.mode()
    }
    fn memmode(&self) -> u32 {
        self.source.memmode()
    }
    fn vidhd(&self) -> Option<&dyn VidHdView> {
        Some(self.vidhd)
    }
    fn hgr_pseudo_page(&self) -> Option<u8> {
        self.source.hgr_pseudo_page()
    }
}

/// The debugger's "view" commands (Debug.cpp `_ViewOutput`, VF_PAGE0..8): shows `source`'s memory
/// with explicit mode `flags` instead of its soft switches, optionally on a pseudo hires page.
/// Render it with [`Renderer::redraw`](crate::Renderer::redraw).
pub struct DebugView<'a> {
    source: &'a dyn VideoSource,
    mode: VideoMode,
    memmode: u32,
    page: Option<u8>,
    hd: Option<NoShr<'a>>,
}

/// A VidHD seen by the debugger: present (same framebuffer) but never in SHR.
struct NoShr<'a>(&'a dyn VidHdView);

impl VidHdView for NoShr<'_> {
    fn is_shr(&self) -> bool {
        false
    }
    fn shr_cell(
        &self,
        is_640: bool,
        fill: bool,
        palette: &[u16; 16],
        a: u32,
        prev: [u8; 3],
    ) -> [[u8; 3]; 16] {
        self.0.shr_cell(is_640, fill, palette, a, prev)
    }
}

impl<'a> DebugView<'a> {
    /// `flags`: text, mixed, hires, dhires, col80 and page2 are taken from it; the rest from the
    /// source. `pseudo_page`: `Some(n)` with `n` in `0, 3..=8` selects that hires page (pages 1
    /// and 2 are `page2` false/true).
    pub fn new(source: &'a dyn VideoSource, flags: VideoFlags, pseudo_page: Option<u8>) -> Self {
        let mut mode = source.mode();
        (mode.text, mode.mixed, mode.dhires, mode.col80) =
            (flags.text, flags.mixed, flags.dhires, flags.col80);
        let mut memmode = source.memmode() & !(mf::HIRES | mf::PAGE2 | mf::STORE80);
        if flags.hires {
            memmode |= mf::HIRES;
        }
        if flags.page2 {
            memmode |= mf::PAGE2;
        }
        DebugView {
            source,
            mode,
            memmode,
            page: pseudo_page,
            hd: source.vidhd().map(NoShr),
        }
    }
}

impl VideoSource for DebugView<'_> {
    delegate!(source);
    fn mode(&self) -> VideoMode {
        self.mode
    }
    fn memmode(&self) -> u32 {
        self.memmode
    }
    fn vidhd(&self) -> Option<&dyn VidHdView> {
        self.hd.as_ref().map(|h| h as &dyn VidHdView)
    }
    fn hgr_pseudo_page(&self) -> Option<u8> {
        self.page
    }
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
