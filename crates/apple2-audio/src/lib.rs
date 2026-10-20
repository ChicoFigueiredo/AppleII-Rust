//! Audio for AppleII-Rust: speaker, Mockingboard family, SSI263 speech and SAM (ADR 0001: no
//! host audio I/O; the frontend plays the PCM from [`Mixer::produce`]).
//!
//! Usage: one [`AudioBus`] per machine; build cards with it
//! (`MockingboardCard::new(Variant::MockingboardC, &bus)`, `SamCard::new(&bus)`), insert them
//! with `Machine::insert_card`, and each video frame call
//! `Mixer::produce_machine(&mut machine)` to get interleaved stereo `f32` samples at the bus's
//! host rate.

#![forbid(unsafe_code)]

pub mod ay8910;
pub mod bus;
pub mod clock;
pub mod mixer;
pub mod mockingboard;
pub mod sam;
pub mod speaker;
pub mod ssi263;
pub mod via6522;

pub use bus::{AudioBus, CardVolume};
pub use clock::{volume_gain, SampleClock};
pub use mixer::Mixer;
pub use mockingboard::{MockingboardCard, Sockets, Variant};
pub use sam::SamCard;
