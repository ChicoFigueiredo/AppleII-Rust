//! AppleII-Rust desktop shell. See docs/adr/0003-*.md for the thread design.
#![forbid(unsafe_code)]

pub mod actions;
pub mod app;
pub mod audio;
pub mod backend;
pub mod benchmark;
pub mod debug_view;
pub mod disk_ui;
pub mod emu;
pub mod factory;
pub mod gfx;
pub mod headless;
pub mod help;
pub mod host;
pub mod hotswap;
pub mod icon;
pub mod icons;
pub mod input;
pub mod joy;
pub mod keymap;
pub mod overlay;
pub mod pacing;
pub mod printer_hub;
pub mod printer_model;
pub mod printer_panels;
pub mod printer_window;
pub mod screenshot;
pub mod settings;
pub mod state_io;
pub mod toolbar;
pub mod ui;
pub mod video;
