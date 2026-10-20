//! Apple II video rendering: a port of AppleWin's NTSC.cpp, NTSC_CharSet.cpp, RGBMonitor.cpp
//! and the rendering parts of Video.cpp. See [`Renderer`] for the design.
//!
//! ```
//! use apple2_core::{Machine, MachineConfig, Model};
//! use apple2_video::{RenderConfig, Renderer, VideoType};
//!
//! let mut machine = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
//! let mut video = Renderer::new(RenderConfig { video_type: VideoType::ColorMonitorNtsc, ..Default::default() });
//! video.run_frame(&mut machine); // runs one frame, rendering after every instruction
//! let rgba = video.rgba8(); // 560x384 RGBA8, ready for a texture
//! assert_eq!(rgba.len(), 560 * 384 * 4);
//! // On Machine::reset()/power_cycle(), also call video.reset().
//! ```

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
