//! User settings for AppleII-Rust: the settings model, TOML persistence, importers for AppleWin
//! configurations, the AppleWin-compatible command line and conversion to the core's
//! `MachineConfig`.
#![forbid(unsafe_code)]

pub mod card;
pub mod cmdline;
pub mod import;
pub mod machine;
pub mod registry_map;
pub mod roms;
pub mod settings;
pub mod store;

pub use card::{CardType, SlotCardSpec};
pub use cmdline::{CmdLine, CmdLineError};
pub use settings::Settings;
pub use store::StoreError;
