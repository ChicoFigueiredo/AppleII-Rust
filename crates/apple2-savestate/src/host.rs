//! What the loader asks of its caller: disk images and cards that need host objects.

use apple2_core::{Card, Model};
use apple2_storage::image::DiskImage;

/// A disk image the save-state refers to (`Disk2InterfaceCard::LoadSnapshotFloppy`,
/// `HarddiskInterfaceCard::LoadSnapshotHDDUnit`). AppleWin first tries `filename` as given
/// (relative to the current directory), then `absolute_path`, then asks the user; the resolver is
/// free to do the same.
#[derive(Clone, Copy, Debug)]
pub struct ImageRequest<'a> {
    /// "Filename": the image's file name (older states: the full path).
    pub filename: &'a str,
    /// "Absolute Path" (unit version 9 / HDD version 6; empty before).
    pub absolute_path: &'a str,
    /// "Write Protected" as saved (AppleWin itself ignores it and uses the file attribute).
    pub write_protected: bool,
    /// True for a hard disk image, false for a floppy.
    pub hard_disk: bool,
}

/// Supplies images and cards to [`load_aws_yaml`](crate::load_aws_yaml).
pub trait LoadHost {
    /// Re-opens the image of a drive. An `Err` leaves the drive empty (AppleWin's
    /// `bImageError` path) and is reported in [`Loaded::warnings`](crate::Loaded).
    fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String>;

    /// Creates the card named `name` ([`Card::name`] of our cards) for `slot`. The default builds
    /// the cards that need nothing from the host; override for the ones that do (serial and
    /// network back ends, audio bus, printer sink, clocks) and call
    /// [`default_card`] for the rest. `None` skips the card with a warning.
    fn make_card(&mut self, name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
        default_card(name, slot, model)
    }

    /// Creates the No-Slot Clock (`MemInsertNoSlotClock`). The default reads a fixed time;
    /// override to supply the host clock. `None` leaves the machine without one.
    fn make_no_slot_clock(&mut self) -> Option<Box<dyn apple2_core::hooks::RomTap>> {
        let clock = apple2_cards::FixedClock(apple2_cards::WallTime::SAMPLE);
        Some(Box::new(apple2_cards::NoSlotClock::new(Box::new(clock))))
    }

    /// Creates the copy-protection dongle of the game I/O connector.
    fn make_dongle(&mut self, kind: apple2_cards::DongleKind) -> Option<Box<dyn apple2_core::hooks::GameIoDevice>> {
        Some(Box::new(apple2_cards::Dongle::new(kind)))
    }
}

/// Cards that can be built without host objects.
pub fn default_card(name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
    use apple2_storage::hdd::{HardDiskCard, HdcMode};
    Some(match name {
        "Disk II" => Box::new(apple2_storage::disk2::Disk2Card::new()),
        "Hard Disk Controller" => Box::new(HardDiskCard::new(slot, model, HdcMode::Default)),
        "4Play" => Box::new(apple2_cards::FourPlay::new()),
        "SNES MAX" => Box::new(apple2_cards::SnesMax::new()),
        "VidHD" => Box::new(apple2_cards::VidHd::new()),
        "Mouse Card" => Box::new(apple2_cards::MouseCard::new()),
        "Z80 SoftCard" => Box::new(apple2_cards::SoftCard::new()),
        "Breakpoint" => Box::new(apple2_cards::BreakpointCard::new()),
        _ => return None,
    })
}
