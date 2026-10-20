//! AppleII-Rust desktop shell. See docs/adr/0003-*.md for the thread design.
#![forbid(unsafe_code)]

pub mod actions;
pub mod app;
pub mod audio;
pub mod backend;
pub mod debug_view;
pub mod emu;
pub mod gfx;
pub mod help;
pub mod headless;
pub mod host;
pub mod icon;
pub mod joy;
pub mod keymap;
pub mod pacing;
pub mod screenshot;
pub mod toolbar;
pub mod ui;
pub mod video;
