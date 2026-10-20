#![allow(dead_code)]

use apple2_core::text_screen::text_screen;
use apple2_core::Machine;
use apple2_savestate::{ImageRequest, LoadHost};
use apple2_storage::image::DiskImage;

pub fn disk(name: &str) -> Vec<u8> {
    let dir = std::env::var("APPLE2_DISKS").unwrap_or_else(|_| format!("{}/../../assets/disks", env!("CARGO_MANIFEST_DIR")));
    std::fs::read(format!("{dir}/{name}")).unwrap_or_else(|e| panic!("{dir}/{name}: {e}"))
}

pub const DOS33: &str = "DOS 3.3 System Master - 680-0210-A.dsk";

pub fn screen(m: &Machine) -> String {
    text_screen(m).iter().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

/// Resolves images from the shared `assets/disks` folder, recording what was asked for.
#[derive(Default)]
pub struct TestHost {
    pub opened: Vec<String>,
}

impl LoadHost for TestHost {
    fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
        self.opened.push(req.filename.to_string());
        let name = std::path::Path::new(req.filename).file_name().unwrap().to_string_lossy().to_string();
        DiskImage::from_bytes(&name, disk(&name)).map_err(|e| e.to_string())
    }
}
