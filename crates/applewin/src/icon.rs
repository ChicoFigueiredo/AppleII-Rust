//! Window icon (title bar / taskbar on Windows and X11; ignored by Wayland and macOS, where the
//! icon comes from the .desktop file / .app bundle).
use winit::window::Icon;

const PNG: &[u8] = include_bytes!("../../../packaging/icons/applewin-64.png");

/// Decodes the embedded 64x64 PNG; `None` if anything goes wrong (the icon is cosmetic).
pub fn window_icon() -> Option<Icon> {
    let img = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_icon_decodes() {
        assert!(super::window_icon().is_some());
    }
}
