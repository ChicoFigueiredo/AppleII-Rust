//! Optional bus-access observer (debugger memory/IO breakpoints, traces). Absent by default; the
//! only cost then is one `Option` test per CPU bus access. Not part of the save-state.

/// Direction of a CPU bus access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    Read,
    Write,
}

/// Called for every CPU read/write the [`crate::machine::Board`] services, including
/// `$C000-$CFFF` I/O and opcode/operand fetches (`value` is what the CPU read or wrote).
pub trait AccessObserver {
    fn access(&mut self, addr: u16, value: u8, kind: AccessKind);
}
