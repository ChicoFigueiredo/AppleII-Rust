//! Headless, deterministic Apple II machine core (ADR 0001). No host I/O, threads or wall clock.

#![forbid(unsafe_code)]

pub mod card;
pub mod config;
pub mod game_port;
pub mod hooks;
pub mod irq;
pub mod keyboard;
pub mod language_card;
pub mod machine;
pub mod memory;
pub mod model;
pub mod observer;
pub mod ram_init;
pub mod scheduler;
pub mod slots;
pub mod speaker;
pub mod text_screen;
pub mod video_mode;
pub mod video_scanner;
pub mod video_standard;

pub use card::{Card, CardCtx, OpcodeInfo};
pub use config::{AuxCardKind, CardKind, ConfigError, MachineConfig};
pub use irq::{Interrupts, IrqSource};
pub use keyboard::NavKey;
pub use machine::{Machine, MachineState, StateError};
pub use model::Model;
pub use observer::{AccessKind, AccessObserver};
pub use ram_init::{RamInit, RandomSource};
pub use scheduler::{EventId, Scheduler, SyncEvent};
pub use video_mode::VideoMode;
pub use video_standard::VideoStandard;
