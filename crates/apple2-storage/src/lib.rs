//! Disk II, hard disk controller and disk images: ports of AppleWin `Disk.cpp`,
//! `Disk2CardManager.cpp`, `DiskImage.cpp`, `DiskImageHelper.cpp`, `DiskFormatTrack.cpp`,
//! `Harddisk.cpp` and `ProDOS_Utils.cpp`. Each module cites the AppleWin file:line it follows.
//!
//! # Using it from a frontend
//! ```ignore
//! let image = DiskImage::open_path(path, Class::Floppy, false)?;      // or DiskImage::from_bytes(name, bytes)
//! machine.insert_card(6, Box::new(Disk2Card::new()));                  // once
//! machine.card_mut::<Disk2Card>(6).unwrap().insert_disk(0, image);     // drive 1; returns the previous image
//! let card = machine.card::<Disk2Card>(6).unwrap();
//! card.light_status(0); card.track(0); card.is_motor_on();             // LEDs and status line
//! machine.card_mut::<Disk2Card>(6).unwrap().drain_events();            // motor / head-step events for sounds
//! machine.card_mut::<Disk2Card>(6).unwrap().eject_disk(0);             // flushes the dirty track first
//! ```
//! Images are held in memory; writes go through an optional [`store::Store`] (a file by
//! [`image::DiskImage::open_path`], or a [`store::MemStore`] the frontend keeps a clone of), so
//! the core never does file I/O. The hard disk card is [`hdd::HardDiskCard`]; new and formatted
//! images come from [`new_disk`].
//!
//! Tape: AppleWin implements no cassette beyond `$C020`/`$C060` reading the floating bus, which
//! `apple2-core` already does, so there is nothing to port here.

#![forbid(unsafe_code)]

pub mod disk2;
pub mod format_track;
pub mod hdd;
pub mod image;
pub mod new_disk;
pub mod nibble;
pub mod store;
pub mod woz;
