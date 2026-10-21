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

/// Prefix when no floppy image has been inserted (`FrameBase::Util_MakeScreenShotFileName`).
const DEFAULT_PREFIX: &str = "AppleWin_ScreenShot";

/// `GetImageTitle` (DiskImage.cpp:274): the file name without folder and extension; an
/// all-uppercase name longer than two characters is shown with its tail in lowercase.
pub fn image_title(file_name: &str) -> String {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let mut t = base.to_string();
    if t.len() > 2 && !t.chars().any(char::is_lowercase) {
        let mut c = t.chars();
        t = c
            .next()
            .into_iter()
            .chain(c.flat_map(char::to_lowercase))
            .collect();
    }
    match t.rfind('.') {
        Some(i) if i > 0 => t.truncate(i),
        _ => {}
    }
    t
}

/// Screenshot names as AppleWin makes them (`FrameBase::Video_TakeScreenShot`):
/// `<last floppy image title>_<9-digit counter>.png`, or `AppleWin_ScreenShot_...` when the last
/// drive operation left no image; inserting or ejecting a floppy resets the counter
/// (`Video_ResetScreenshotCounter`, Disk.cpp:438,888,895). First free number wins.
#[derive(Default)]
pub struct ScreenshotNamer {
    counter: u32,
    prefix: String,
}

impl ScreenshotNamer {
    /// A floppy was inserted (`Some(file name)`) or ejected / rejected (`None`).
    pub fn reset(&mut self, floppy_file: Option<&str>) {
        self.counter = 0;
        self.prefix = floppy_file.map(image_title).unwrap_or_default();
    }

    pub fn next_path(&mut self, dir: &Path) -> PathBuf {
        let prefix = if self.prefix.is_empty() {
            DEFAULT_PREFIX
        } else {
            &self.prefix
        };
        loop {
            let p = dir.join(format!("{prefix}_{:09}.png", self.counter));
            self.counter += 1;
            if !p.exists() || self.counter > 999_999_999 {
                return p;
            }
        }
    }
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
        let mut n = ScreenshotNamer::default();
        let p = n.next_path(&dir);
        assert!(p.ends_with("AppleWin_ScreenShot_000000000.png"));
        save_png(&p, &f).unwrap();
        let img = image::open(&p).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0).0, [1, 2, 3, 255]);
        n.reset(None);
        assert_eq!(
            n.next_path(&dir).file_name().unwrap(),
            "AppleWin_ScreenShot_000000001.png"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
