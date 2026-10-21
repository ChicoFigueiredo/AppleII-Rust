//! PNG screenshots of the Apple screen (AppleWin's Print Screen saves the frame buffer as BMP/PNG).

use crate::video::Frame;
use std::path::{Path, PathBuf};

pub fn save_png(path: &Path, frame: &Frame) -> Result<(), String> {
    if frame.width == 0 || frame.height == 0 {
        return Err("no frame to save".into());
    }
    image::save_buffer(
        path,
        &frame.rgba,
        frame.width,
        frame.height,
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())
}

/// First free `AppleWin_NNNN.png` in `dir` (AppleWin numbers its screenshots the same way).
pub fn next_screenshot_path(dir: &Path) -> PathBuf {
    (0..10_000)
        .map(|n| dir.join(format!("AppleWin_{n:04}.png")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join("AppleWin_9999.png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip() {
        let mut f = Frame::new(2, 2);
        f.rgba[..4].copy_from_slice(&[1, 2, 3, 255]);
        let dir = std::env::temp_dir().join(format!("applewin-shot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = next_screenshot_path(&dir);
        assert!(p.ends_with("AppleWin_0000.png"));
        save_png(&p, &f).unwrap();
        let img = image::open(&p).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0).0, [1, 2, 3, 255]);
        assert_eq!(
            next_screenshot_path(&dir).file_name().unwrap(),
            "AppleWin_0001.png"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
