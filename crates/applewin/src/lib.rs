//! AppleII-Rust desktop shell. See docs/adr/0003-*.md for the thread design.
#![forbid(unsafe_code)]

pub mod actions;
pub mod audio;
pub mod backend;
pub mod emu;
pub mod headless;
pub mod joy;
pub mod keymap;
pub mod pacing;
pub mod screenshot;
pub mod toolbar;
pub mod video;
