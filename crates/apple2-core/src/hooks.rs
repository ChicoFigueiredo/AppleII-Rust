//! Small generic hooks for devices that are not slot cards (phase 6): the No-Slot Clock sitting
//! under a ROM and game-connector dongles. Implementations live in `apple2-cards`.

use std::any::Any;

/// A device between the CPU and the system ROM (AppleWin `g_NoSlotClock`, Memory.cpp:1035/1091).
/// Called for reads and writes of the internal `$C300-$C3FF`/`$C800-$CFFF` ROM on the //e and of
/// `$F800-$FFFF` on the ][/][+ while the ROM (not Language Card RAM) is visible.
pub trait RomTap {
    /// `data` is the value being written, or 0 on a read (AppleWin passes 0). Returns true if the
    /// device replaced `data` (read only).
    fn access(&mut self, addr: u16, write: bool, data: &mut u8) -> bool;
    /// Power-cycle (not Ctrl-Reset), AppleWin `MemReset`.
    fn reset(&mut self) {}
    fn save(&self) -> Vec<u8> {
        Vec::new()
    }
    fn load(&mut self, _state: &[u8]) -> bool {
        true
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }
}

/// A device on the game I/O connector that overrides pushbuttons and paddles from the
/// annunciators (AppleWin CopyProtectionDongles.cpp, hooked in Joystick.cpp and Memory.cpp).
pub trait GameIoDevice {
    /// Annunciator `n` (0-3) is about to become `on`; `old` is the state before.
    fn annunciator(&mut self, _n: usize, _on: bool, _old: &[bool; 4]) {}
    /// Overrides pushbutton `n` (0-2); `None` leaves it to the joystick.
    fn button(&self, _n: usize, _ann: &[bool; 4]) -> Option<bool> {
        None
    }
    /// Overrides the count of paddle `n` (0-3) timer, in paddle units (not clamped to 255).
    fn paddle(&self, _n: usize, _ann: &[bool; 4]) -> Option<u32> {
        None
    }
    fn save(&self) -> Vec<u8> {
        Vec::new()
    }
    fn load(&mut self, _state: &[u8]) -> bool {
        true
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }
}
