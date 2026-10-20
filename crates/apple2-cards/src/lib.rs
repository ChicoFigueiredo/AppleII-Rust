//! Non-network peripheral cards of AppleWin (phase 6). Every card implements
//! [`apple2_core::Card`]; devices that are not slot cards use the hooks of
//! [`apple2_core::hooks`]. Host input (mouse, controllers) and host time enter through setters
//! and the [`time::HostClock`] trait: nothing here touches the host.

#![forbid(unsafe_code)]

pub mod mouse;
pub mod nsc;
pub mod pia6821;
pub mod softcard;
pub mod state;
pub mod thunderclock;
pub mod time;
pub mod z80;

pub use nsc::NoSlotClock;
pub use softcard::SoftCard;
pub use thunderclock::{ClockModel, ThunderClock};
pub use time::{FixedClock, HostClock, WallTime};
pub use mouse::{mouse_button, mouse_move, MouseCard};
