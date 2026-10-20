//! Cards that talk to the host: Super Serial Card, parallel printer, Uthernet I (CS8900A) and
//! Uthernet II (W5100). `apple2-core` stays free of host I/O (ADR 0001); every card reaches the
//! outside world only through a backend trait defined here, with `std`-based implementations and
//! in-memory ones for tests. Heavy backends sit behind cargo features (`serialport`, `pcap`).

#![forbid(unsafe_code)]

pub mod serial;
pub mod ssc;

pub use serial::{LoopbackHandle, LoopbackSerial, SerialBackend, TcpServerSerial};
pub use ssc::SuperSerialCard;
