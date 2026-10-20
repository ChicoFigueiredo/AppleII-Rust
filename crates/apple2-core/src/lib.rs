//! Headless, deterministic Apple II machine core (ADR 0001). No host I/O, threads or wall clock.

#![forbid(unsafe_code)]

pub mod card;
pub mod config;
pub mod irq;
pub mod language_card;
pub mod machine;
pub mod memory;
pub mod model;
pub mod scheduler;
pub mod slots;
pub mod video_mode;
pub mod video_scanner;
pub mod video_standard;

pub use card::{Card, CardCtx};
pub use config::{AuxCardKind, CardKind, MachineConfig};
pub use irq::{Interrupts, IrqSource};
pub use machine::Machine;
pub use model::Model;
pub use scheduler::{EventId, Scheduler, SyncEvent};
pub use video_mode::VideoMode;
pub use video_standard::VideoStandard;
