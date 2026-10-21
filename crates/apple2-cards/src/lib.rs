//! Non-network peripheral cards of AppleWin (phase 6). Every card implements
//! [`apple2_core::Card`]; devices that are not slot cards use the hooks of
//! [`apple2_core::hooks`]. Host input (mouse, controllers) and host time enter through setters
//! and the [`time::HostClock`] trait: nothing here touches the host.

#![forbid(unsafe_code)]

pub mod breakpoint;
pub mod dongles;
pub mod fourplay;
pub mod mouse;
pub mod nsc;
pub mod pia6821;
pub mod snesmax;
pub mod softcard;
pub mod state;
pub mod thunderclock;
pub mod time;
pub mod vidhd;
pub mod z80;

pub use breakpoint::BreakpointCard;
pub use dongles::{Dongle, DongleKind};
pub use fourplay::FourPlay;
pub use mouse::{mouse_button, mouse_move, MouseCard, MouseSnapshot};
pub use nsc::{NoSlotClock, NscSnapshot};
pub use snesmax::SnesMax;
pub use softcard::SoftCard;
pub use thunderclock::{ClockModel, ThunderClock};
pub use time::{FixedClock, HostClock, WallTime};
pub use vidhd::{VidHd, VidHdSnapshot};
