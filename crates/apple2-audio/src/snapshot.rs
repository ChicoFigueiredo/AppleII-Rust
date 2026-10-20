//! Plain-data views of the Mockingboard family state in AppleWin's terms, for the `.aws.yaml`
//! reader/writer (Mockingboard.cpp:1174-1520, 6522.cpp:634, AY8910.cpp:1098, SSI263.cpp:1005).
//! [`MockingboardCard::snapshot`](crate::MockingboardCard::snapshot) builds them and
//! [`apply_snapshot`](crate::MockingboardCard::apply_snapshot) restores them; neither changes the
//! `Card::save` byte format.

use crate::ay8910::AyType;
use crate::ssi263::SsiType;

/// `SY6522::SaveSnapshot` registers and timer state (6522.cpp:634).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViaSnapshot {
    pub orb: u8,
    pub ora: u8,
    pub ddrb: u8,
    pub ddra: u8,
    pub t1_counter: u16,
    pub t1_latch: u16,
    pub t1_irq_delay: bool,
    pub t1_active: bool,
    pub t2_counter: u16,
    pub t2_latch: u16,
    pub t2_irq_delay: bool,
    pub t2_active: bool,
    pub serial_shift: u8,
    pub acr: u8,
    pub pcr: u8,
    pub ifr: u8,
    pub ier: u8,
}

/// `AY8913::SaveSnapshot` (AY8910.cpp:1098). The pending register-change queue (`Change`) has no
/// counterpart (writes are applied when they happen) and is neither written nor read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AySnapshot {
    pub ty: AyType,
    pub tone_tick: [u32; 3],
    pub tone_high: [bool; 3],
    pub noise_tick: u32,
    pub tone_subcycles: u32,
    pub env_subcycles: u32,
    pub env_internal_tick: u32,
    pub env_tick: u32,
    /// `ay_tick_incr`: derived from the clocks, ignored on load (AppleWin re-inits it too).
    pub tick_incr: u32,
    pub tone_period: [u32; 3],
    pub noise_period: u32,
    pub rng: u32,
    pub noise_toggle: bool,
    pub env_first: bool,
    pub env_rev: bool,
    pub env_counter: i32,
    pub regs: [u8; 16],
}

/// `SSI263::SaveSnapshot` plus the unit-0 `SC01` entry (SSI263.cpp:1005, :1122).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SsiSnapshot {
    pub ty: SsiType,
    pub duration_phoneme: u8,
    pub inflection: u8,
    pub rate_inflection: u8,
    pub ctrl_art_amp: u8,
    pub filter_freq: u8,
    /// `m_currentMode.mode`: b0 = D7, b5 = enableInts, b7:6 = function.
    pub current_mode: u8,
    pub has_sc01: bool,
    pub votrax_phoneme: u8,
    pub is_votrax_phoneme: bool,
}

/// One `MB_SUBUNIT` (a 6522 with its AYs and speech chips).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitSnapshot {
    pub via: ViaSnapshot,
    pub ay: [AySnapshot; 2],
    pub ssi: SsiSnapshot,
    /// `state[]` (`MockingboardUnitState_e`: 2 inactive, 3 read, 6 write, 7 latch).
    pub state: [u8; 2],
    pub ay_current_register: [u8; 2],
    pub chip_selected: [bool; 2],
    pub latch_valid: [bool; 2],
    pub bus_driven: bool,
}

/// The whole card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockingboardSnapshot {
    /// `m_phasorMode` (0 Mockingboard, 5 Phasor, 7 Echo+).
    pub phasor_mode: u8,
    pub units: [UnitSnapshot; 2],
}
