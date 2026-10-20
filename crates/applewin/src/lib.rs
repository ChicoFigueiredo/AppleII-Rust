//! AppleII-Rust desktop shell. See docs/adr/0003-*.md for the thread design.
#![forbid(unsafe_code)]

pub mod actions;
pub mod app;
pub mod audio;
pub mod backend;
pub mod debug_view;
pub mod disk_ui;
pub mod emu;
pub mod factory;
pub mod gfx;
pub mod help;
pub mod headless;
pub mod host;
pub mod icon;
pub mod joy;
pub mod keymap;
pub mod overlay;
pub mod pacing;
pub mod screenshot;
pub mod state_io;
pub mod settings;
pub mod toolbar;
pub mod ui;
pub mod video;
