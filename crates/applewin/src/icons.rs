//! Toolbar icons: AppleWin's bitmaps redrawn as SVG (`assets/ui/toolbar/*.svg`, embedded here),
//! rasterised once per pixel size with `resvg` and cached as egui textures.

use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Help,
    Run,
    RunPravets,
    RunTk3000,
    RunBase64a,
    Drive1,
    Drive2,
    DriveSwap,
    Fullscreen,
    Debug,
    Setup,
    LedOff,
    LedRead,
    LedWrite,
    LedProtected,
    CapsOff,
    CapsOn,
    CapsOnLat,
    CapsOnP8,
}

macro_rules! svg {
    ($f:literal) => {
        include_bytes!(concat!("../../../assets/ui/toolbar/", $f, ".svg")).as_slice()
    };
}

impl Icon {
    pub const ALL: [Icon; 19] = [
        Icon::Help,
        Icon::Run,
        Icon::RunPravets,
        Icon::RunTk3000,
        Icon::RunBase64a,
        Icon::Drive1,
        Icon::Drive2,
        Icon::DriveSwap,
        Icon::Fullscreen,
        Icon::Debug,
        Icon::Setup,
        Icon::LedOff,
        Icon::LedRead,
        Icon::LedWrite,
        Icon::LedProtected,
        Icon::CapsOff,
        Icon::CapsOn,
        Icon::CapsOnLat,
        Icon::CapsOnP8,
    ];

    /// File stem under `assets/ui/toolbar/`.
    pub fn name(self) -> &'static str {
        match self {
            Icon::Help => "help",
            Icon::Run => "run",
            Icon::RunPravets => "run-pravets",
            Icon::RunTk3000 => "run-tk3000",
            Icon::RunBase64a => "run-base64a",
            Icon::Drive1 => "drive1",
            Icon::Drive2 => "drive2",
            Icon::DriveSwap => "drive-swap",
            Icon::Fullscreen => "fullscreen",
            Icon::Debug => "debug",
            Icon::Setup => "setup",
            Icon::LedOff => "led-off",
            Icon::LedRead => "led-read",
            Icon::LedWrite => "led-write",
            Icon::LedProtected => "led-protected",
            Icon::CapsOff => "caps-off",
            Icon::CapsOn => "caps-on",
            Icon::CapsOnLat => "caps-on-lat",
            Icon::CapsOnP8 => "caps-on-p8",
        }
    }

    pub fn svg(self) -> &'static [u8] {
        match self {
            Icon::Help => svg!("help"),
            Icon::Run => svg!("run"),
            Icon::RunPravets => svg!("run-pravets"),
            Icon::RunTk3000 => svg!("run-tk3000"),
            Icon::RunBase64a => svg!("run-base64a"),
            Icon::Drive1 => svg!("drive1"),
            Icon::Drive2 => svg!("drive2"),
            Icon::DriveSwap => svg!("drive-swap"),
            Icon::Fullscreen => svg!("fullscreen"),
            Icon::Debug => svg!("debug"),
            Icon::Setup => svg!("setup"),
            Icon::LedOff => svg!("led-off"),
            Icon::LedRead => svg!("led-read"),
            Icon::LedWrite => svg!("led-write"),
            Icon::LedProtected => svg!("led-protected"),
            Icon::CapsOff => svg!("caps-off"),
            Icon::CapsOn => svg!("caps-on"),
            Icon::CapsOnLat => svg!("caps-on-lat"),
            Icon::CapsOnP8 => svg!("caps-on-p8"),
        }
    }

    /// AppleWin's RUN button bitmap per machine model (`Win32Frame::CreateGdiObjects`);
    /// `model` is the `Model` Debug name used by `EmuStatus`.
    pub fn run_for_model(model: &str) -> Icon {
        match model {
            "Pravets82" | "Pravets8M" | "Pravets8A" => Icon::RunPravets,
            "Tk3000e" => Icon::RunTk3000,
            "Base64A" => Icon::RunBase64a,
            _ => Icon::Run,
        }
    }

    /// Caps Lock indicator bitmap (`DrawStatusArea`): Pravets uses the "C" (P8) pair; the other
    /// models the "a"/"A" pair. `None` on the Apple ][ family, which has no Caps Lock.
    pub fn caps_for_model(model: &str, on: bool) -> Option<Icon> {
        if !crate::overlay::model_has_caps_lock(model) {
            return None;
        }
        let p8 = matches!(model, "Pravets82" | "Pravets8M" | "Pravets8A");
        Some(match (p8, on) {
            (true, true) => Icon::CapsOnP8,
            (_, false) => Icon::CapsOff,
            (false, true) => Icon::CapsOn,
        })
    }
}

/// Rasterises `svg` to a `px` x `px` image (premultiplied RGBA8). The viewBox is square for
/// every shipped icon. `None` if the SVG does not parse.
pub fn rasterize(svg: &[u8], px: u32) -> Option<ColorImage> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).ok()?;
    let mut pm = tiny_skia::Pixmap::new(px, px)?;
    let size = tree.size();
    let tf = tiny_skia::Transform::from_scale(px as f32 / size.width(), px as f32 / size.height());
    resvg::render(&tree, tf, &mut pm.as_mut());
    Some(ColorImage::from_rgba_premultiplied([px as usize, px as usize], pm.data()))
}

/// Textures per (icon, pixel size); a new scale (`pixels_per_point`) rasterises afresh.
#[derive(Default)]
pub struct IconCache {
    textures: HashMap<(Icon, u32), TextureHandle>,
}

impl IconCache {
    /// Texture of `icon` for a `points` x `points` square at the context's current scale.
    pub fn get(&mut self, ctx: &Context, icon: Icon, points: f32) -> Option<&TextureHandle> {
        let px = (points * ctx.pixels_per_point()).round().max(8.0) as u32;
        if let std::collections::hash_map::Entry::Vacant(e) = self.textures.entry((icon, px)) {
            let img = rasterize(icon.svg(), px)?;
            e.insert(ctx.load_texture(format!("icon-{}-{px}", icon.name()), img, TextureOptions::LINEAR));
        }
        self.textures.get(&(icon, px))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_and_caps_variants_follow_the_model() {
        assert_eq!(Icon::run_for_model("Apple2eEnhanced"), Icon::Run);
        assert_eq!(Icon::run_for_model("Pravets8M"), Icon::RunPravets);
        assert_eq!(Icon::run_for_model("Tk3000e"), Icon::RunTk3000);
        assert_eq!(Icon::run_for_model("Base64A"), Icon::RunBase64a);
        assert_eq!(Icon::caps_for_model("Apple2Plus", true), None);
        assert_eq!(Icon::caps_for_model("Apple2e", true), Some(Icon::CapsOn));
        assert_eq!(Icon::caps_for_model("Apple2e", false), Some(Icon::CapsOff));
        assert_eq!(Icon::caps_for_model("Pravets82", true), Some(Icon::CapsOnP8));
    }
}
