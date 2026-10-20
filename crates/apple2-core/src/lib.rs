//! Headless, deterministic Apple II machine core (ADR 0001). No host I/O, threads or wall clock.

#![forbid(unsafe_code)]

pub mod config;
pub mod irq;
pub mod machine;
pub mod memory;
pub mod model;
pub mod scheduler;
pub mod video_standard;

pub use config::{AuxCardKind, CardKind, MachineConfig};
pub use irq::{Interrupts, IrqSource};
pub use machine::Machine;
pub use model::Model;
pub use scheduler::{EventId, Scheduler, SyncEvent};
pub use video_standard::VideoStandard;
