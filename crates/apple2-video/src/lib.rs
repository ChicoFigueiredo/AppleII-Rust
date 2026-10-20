//! Apple II video rendering: a port of AppleWin's NTSC.cpp, NTSC_CharSet.cpp, RGBMonitor.cpp
//! and the rendering parts of Video.cpp. See [`Renderer`] for the design.

#![forbid(unsafe_code)]

pub mod charset;
pub mod chroma;
pub mod config;
pub mod lookup;
mod renderer;
mod rgb;
pub mod screen_text;
pub mod source;

pub use config::{RenderConfig, RgbCard, VideoType};
pub use renderer::{Renderer, BORDER_H, BORDER_W, FB_H, FB_W, VISIBLE_H, VISIBLE_W};
pub use source::VideoSource;
