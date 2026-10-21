//! Toolbar SVG icons: every file parses, stays within the plain-shapes subset egui / the design
//! canvas accept, and renders at 24, 48 and 128 px. Set `ICONS_OUT=<dir>` (and optionally
//! `ICONS_REF=<dir with the original BMPs as PNG>`) to also save the PNGs and a contact sheet.

use applewin_shell::icons::{rasterize, Icon};
use egui::ColorImage;
use image::{imageops, Rgba, RgbaImage};
use std::path::{Path, PathBuf};

const FORBIDDEN: [&str; 14] = [
    "<style",
    "class=",
    "<animate",
    "<set ",
    "<foreignobject",
    "<image",
    "<text",
    "<tspan",
    "href",
    "@import",
    "url(http",
    "<script",
    "<filter",
    "<mask",
];

fn to_rgba(img: &ColorImage, bg: [u8; 3]) -> RgbaImage {
    let mut out = RgbaImage::new(img.size[0] as u32, img.size[1] as u32);
    for (i, p) in img.pixels.iter().enumerate() {
        // premultiplied over an opaque background
        let inv = 255 - p.a() as u32;
        let c = |s: u8, b: u8| (s as u32 + b as u32 * inv / 255).min(255) as u8;
        out.put_pixel(
            (i % img.size[0]) as u32,
            (i / img.size[0]) as u32,
            Rgba([c(p.r(), bg[0]), c(p.g(), bg[1]), c(p.b(), bg[2]), 255]),
        );
    }
    out
}

fn ref_name(i: Icon) -> &'static str {
    match i {
        Icon::Help => "HELP",
        Icon::Run => "RUN",
        Icon::RunPravets => "RUNP",
        Icon::RunTk3000 => "RUN3000E",
        Icon::RunBase64a => "RUNBASE64A",
        Icon::Drive1 => "DRIVE1",
        Icon::Drive2 => "DRIVE2",
        Icon::DriveSwap => "DriveSwap",
        Icon::Fullscreen => "FULLSCR",
        Icon::Debug => "DEBUG",
        Icon::Setup => "SETUP",
        Icon::LedOff => "DISKOFF",
        Icon::LedRead => "DISKREAD",
        Icon::LedWrite => "DISKWRIT",
        Icon::LedProtected => "DISKPROT",
        Icon::CapsOff => "LED_CAPS_OFF",
        Icon::CapsOn => "LED_CAPS_ON",
        Icon::CapsOnLat => "LED_CAPS_ON_LAT",
        Icon::CapsOnP8 => "LED_CAPS_ON_P8",
    }
}

#[test]
fn every_icon_is_plain_svg_and_renders() {
    let out: Option<PathBuf> = std::env::var_os("ICONS_OUT").map(PathBuf::from);
    let refd: Option<PathBuf> = std::env::var_os("ICONS_REF").map(PathBuf::from);
    if let Some(o) = &out {
        std::fs::create_dir_all(o).unwrap();
    }
    let light = [244, 244, 246];
    let dark = [32, 33, 38];
    let (cw, ch) = (128 + 8, 128 + 8);
    let cols = 2usize;
    let rows = Icon::ALL.len().div_ceil(cols);
    let block_w = 5 * cw as u32 + 24;
    let mut sheet = RgbaImage::from_pixel(
        block_w * cols as u32,
        rows as u32 * ch as u32,
        Rgba([128, 128, 128, 255]),
    );

    for (n, icon) in Icon::ALL.iter().copied().enumerate() {
        let bytes = icon.svg();
        assert!(
            bytes.len() < 8 * 1024,
            "{} is {} bytes",
            icon.name(),
            bytes.len()
        );
        let text = String::from_utf8(bytes.to_vec()).unwrap().to_lowercase();
        for f in FORBIDDEN {
            assert!(!text.contains(f), "{} uses forbidden `{f}`", icon.name());
        }
        for px in [24u32, 48, 128] {
            let img =
                rasterize(bytes, px).unwrap_or_else(|| panic!("{} does not parse", icon.name()));
            assert_eq!(img.size, [px as usize, px as usize]);
            assert!(
                img.pixels.iter().any(|p| p.a() > 0),
                "{} renders empty at {px}",
                icon.name()
            );
            if px == 128 {
                if let Some(o) = &out {
                    to_rgba(&img, [0, 0, 0])
                        .save(o.join(format!("{}.png", icon.name())))
                        .ok();
                    let mut t = RgbaImage::new(128, 128);
                    for (i, p) in img.pixels.iter().enumerate() {
                        let a = p.a();
                        let un = |c: u8| {
                            if a == 0 {
                                0
                            } else {
                                (c as u32 * 255 / a as u32).min(255) as u8
                            }
                        };
                        t.put_pixel(
                            (i % 128) as u32,
                            (i / 128) as u32,
                            Rgba([un(p.r()), un(p.g()), un(p.b()), a]),
                        );
                    }
                    t.save(o.join(format!("{}-128.png", icon.name()))).ok();
                }
            }
        }
        if out.is_some() {
            let (bx, by) = ((n % cols) as u32 * block_w, (n / cols) as u32 * ch as u32);
            let mut x = bx + 4;
            let y = by + 4;
            if let Some(r) = refd.as_deref().map(Path::new) {
                if let Ok(im) = image::open(r.join(format!("{}.png", ref_name(icon)))) {
                    let im =
                        imageops::resize(&im.to_rgba8(), 128, 128, imageops::FilterType::Nearest);
                    imageops::overlay(&mut sheet, &im, x as i64, y as i64);
                }
            }
            x += cw as u32;
            for bg in [light, dark] {
                let big = to_rgba(&rasterize(bytes, 128).unwrap(), bg);
                imageops::overlay(&mut sheet, &big, x as i64, y as i64);
                x += cw as u32;
            }
            for bg in [light, dark] {
                let mut cell = RgbaImage::from_pixel(128, 128, Rgba([bg[0], bg[1], bg[2], 255]));
                let small = to_rgba(&rasterize(bytes, 24).unwrap(), bg);
                imageops::overlay(&mut cell, &small, 8, 8);
                let mid = to_rgba(&rasterize(bytes, 48).unwrap(), bg);
                imageops::overlay(&mut cell, &mid, 40, 40);
                imageops::overlay(&mut sheet, &cell, x as i64, y as i64);
                x += cw as u32;
            }
        }
    }
    if let Some(o) = &out {
        sheet.save(o.join("contact-sheet.png")).unwrap();
    }
}
