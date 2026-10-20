//! Rendering options (AppleWin Video.h `VideoType_e`, `VideoStyle_e`, RGBMonitor.h
//! `RGB_Videocard_e`).

/// `VideoType_e` (Video.h:18). The discriminants are AppleWin's and are persisted by its
/// configuration, so they must not change.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VideoType {
    /// Monochrome in the user's colour ([`RenderConfig::monochrome_rgb`]).
    MonoCustom = 0,
    /// Colour rendering of AppleWin 1.25 (GH#357): table-driven, no composite artefacts.
    ColorIdealized = 1,
    /// Real RGB card: Apple's Extended 80-Column Text/AppleColor, Video-7 SL7, Le Chat Mauve.
    ColorVideocardRgb = 2,
    /// Composite colour monitor (NTSC or PAL).
    ColorMonitorNtsc = 3,
    /// Colour TV (the default).
    ColorTv = 4,
    MonoTv = 5,
    MonoAmber = 6,
    MonoGreen = 7,
    MonoWhite = 8,
}

impl VideoType {
    pub const ALL: [VideoType; 9] = [
        VideoType::MonoCustom,
        VideoType::ColorIdealized,
        VideoType::ColorVideocardRgb,
        VideoType::ColorMonitorNtsc,
        VideoType::ColorTv,
        VideoType::MonoTv,
        VideoType::MonoAmber,
        VideoType::MonoGreen,
        VideoType::MonoWhite,
    ];

    /// `VT_DEFAULT`.
    pub const DEFAULT: VideoType = VideoType::ColorTv;

    /// Menu text (Video.cpp `g_apVideoModeDesc`, :71).
    pub fn description(self) -> &'static str {
        match self {
            VideoType::MonoCustom => "Monochrome (Custom)",
            VideoType::ColorIdealized => "Color (Composite Idealized)",
            VideoType::ColorVideocardRgb => "Color (RGB Card/Monitor)",
            VideoType::ColorMonitorNtsc => "Color (Composite Monitor)",
            VideoType::ColorTv => "Color TV",
            VideoType::MonoTv => "B&W TV",
            VideoType::MonoAmber => "Monochrome (Amber)",
            VideoType::MonoGreen => "Monochrome (Green)",
            VideoType::MonoWhite => "Monochrome (White)",
        }
    }

    /// `IncVideoType`/`DecVideoType` wrap-around neighbours.
    pub fn next(self) -> VideoType {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> VideoType {
        Self::ALL[(self as usize + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// `RGB_Videocard_e` (RGBMonitor.h:7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum RgbCard {
    /// Video-7 RGB-SL7; Apple's Extended 80-Column Text/AppleColor Adaptor is an alias.
    #[default]
    Video7Sl7,
    LeChatMauveEve,
    LeChatMauveFeline,
}

/// Everything the renderer needs besides the machine state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderConfig {
    pub video_type: VideoType,
    /// `VS_HALF_SCANLINES` (default on): the in-between scan lines are dimmed or black instead of
    /// repeating the line above ("50% scan lines").
    pub half_scanlines: bool,
    /// `VS_COLOR_VERTICAL_BLEND` (GH#616): "TV Emu" hi-res colour blending of AppleWin 1.25, only
    /// used by [`VideoType::ColorIdealized`].
    pub vertical_blend: bool,
    /// Colour of [`VideoType::MonoCustom`] (`GetMonochromeRGB`; default `MONO_COLOR_DEFAULT`).
    pub monochrome_rgb: [u8; 3],
    pub rgb_card: RgbCard,
    /// Foreground and background of regular text on RGB cards (`RGB_SetRegularTextFG/BG`);
    /// lo-res colour numbers 0-15.
    pub rgb_text_fg: u8,
    pub rgb_text_bg: u8,
    /// `RGB_SetInvertBit7`: invert the mixed-mode bit 7 of RGB DHGR (GH#633).
    pub rgb_invert_bit7: bool,
    /// `RGB_SetMacLCCardDLGR`: Apple IIe Card for Macintosh LC double lo-res quirk (GH#1258).
    pub rgb_mac_lc_dlgr: bool,
    /// Base64A video ROM rocker switch (`GetVideoRomRockerSwitch`; AppleWin default off).
    pub video_rom_rocker: bool,
    /// `-videorom`: the user's video ROM (2K for the ][/][+, 4K or 8K for the //e family).
    pub user_video_rom: Option<std::sync::Arc<Vec<u8>>>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        RenderConfig {
            video_type: VideoType::DEFAULT,
            half_scanlines: true, // VS_DEFAULT = VS_HALF_SCANLINES
            vertical_blend: false,
            monochrome_rgb: [0xC0, 0xC0, 0xC0],
            rgb_card: RgbCard::Video7Sl7,
            rgb_text_fg: 15,
            rgb_text_bg: 0,
            rgb_invert_bit7: false,
            rgb_mac_lc_dlgr: false,
            video_rom_rocker: false,
            user_video_rom: None,
        }
    }
}
