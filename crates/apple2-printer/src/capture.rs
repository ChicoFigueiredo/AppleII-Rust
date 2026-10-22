//! What the printer card hands to the virtual printer (fase A). Produced on the emulation thread,
//! consumed by the spool service off the frame loop; events of one job are ordered by `seq`.

use serde::{Deserialize, Serialize};

/// Transporte (GLOSSARY): what the card does to the byte before the printer sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transport {
    /// Like the 8-bit cards of the time (Grappler): a byte the card's own firmware writes (PR#1,
    /// PRINT, LIST: Apple II text has bit 7 set) loses bit 7; a byte a program writes straight to
    /// the port (graphics drivers such as Print Shop's) arrives whole. Default (ADR 0008).
    #[default]
    Auto,
    /// The whole byte from the bus.
    EightBit,
    /// AppleWin's Generic Printer rule: bit 7 cleared (`value & 0x7F`).
    SevenBitAppleWin,
}

/// Why a job ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseReason {
    /// AppleWin's idle limit elapsed.
    Idle,
    /// Machine reset / power cycle.
    Reset,
    /// A save-state was loaded.
    StateLoaded,
    /// The card was removed or the machine rebuilt.
    CardRemoved,
    /// The user ended the job in the Central de Impressoras.
    User,
    /// The emulator is shutting down.
    Shutdown,
}

/// Where the job came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobOrigin {
    /// Apple II model name as shown to the user (e.g. "Apple //e Enhanced").
    pub machine: String,
    pub slot: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureEvent {
    /// First byte written after the job was closed (a read alone never opens content, CA02).
    Open {
        job: u64,
        origin: JobOrigin,
        transport: Transport,
    },
    /// Bytes in bus order, already through the Transport; `offset` = bytes of this job before them.
    Bytes {
        job: u64,
        offset: u64,
        data: Vec<u8>,
    },
    Close {
        job: u64,
        reason: CloseReason,
    },
}
