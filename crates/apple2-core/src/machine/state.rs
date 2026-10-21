//! Save-state: [`Machine::save_state`] / [`Machine::load_state`] (spec item 14).
//!
//! The state is a plain `serde` value (no file format here; the AppleWin YAML snapshot is phase
//! 7). It holds the [`MachineConfig`] (including a custom ROM) so a state can be checked against
//! the machine it is loaded into; the system ROM itself is not stored, it comes from the config.
//! Boxed cards travel as their own [`Card::save`](crate::Card::save) bytes; the Language Card /
//! Saturn state is serialized directly. Cards inserted with [`Machine::insert_card`] are not in
//! the config, so the receiving machine must have the same cards inserted in the same slots.

use super::*;
use crate::game_port::GamePort;
use crate::keyboard::Keyboard;
use crate::language_card::LanguageCards;
use crate::memory::MemoryState;
use crate::slots::SlotsState;
use crate::speaker::Speaker;
use serde::{Deserialize, Serialize};

/// CPU registers and latches ([`Cpu`]; the variant comes from the config).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuState {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub p: u8,
    pub cycles: u64,
    pub irq: bool,
    pub nmi_pending: bool,
    pub stopped: bool,
    pub waiting: bool,
    pub opcode: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineState {
    pub config: MachineConfig,
    pub cpu: CpuState,
    pub memory: MemoryState,
    pub lc: LanguageCards,
    pub slots: SlotsState,
    pub video: VideoMode,
    pub annunciators: [bool; 4],
    pub keyboard: Keyboard,
    pub speaker: Speaker,
    pub game: GamePort,
    pub events: Vec<SyncEvent>,
    pub actions: Vec<(EventId, EventAction)>,
    pub interrupts: Interrupts,
    pub cycles: u64,
    pub cycles_in_frame: u32,
    pub frames: u64,
    pub pending: u32,
    pub opcode_ticks: u32,
    pub opcode_pc: u16,
    pub opcode: u8,
    pub nmi_line: bool,
    pub irq_on_last_opcode_cycle: bool,
    pub irq_defer_1_opcode: bool,
    /// Slot whose card owns the CPU (Z80 SoftCard active).
    #[serde(default)]
    pub cpu_owner: Option<u8>,
    /// [`RomTap`](crate::hooks::RomTap) / [`GameIoDevice`](crate::hooks::GameIoDevice) bytes.
    #[serde(default)]
    pub rom_tap: Option<Vec<u8>>,
    #[serde(default)]
    pub game_io: Option<Vec<u8>>,
}

/// Why [`Machine::load_state`] refused a state (the machine is left untouched).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    /// The state was taken from a machine with another [`MachineConfig`].
    ConfigMismatch,
    /// RAM or Language Card sizes, or the inserted cards, do not match.
    Incompatible,
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            StateError::ConfigMismatch => "state was saved with a different machine configuration",
            StateError::Incompatible => {
                "state does not fit this machine (memory sizes or cards differ)"
            }
        })
    }
}

impl std::error::Error for StateError {}

impl Machine {
    /// Snapshot of everything that determines future behaviour. Two machines with equal states
    /// continue identically.
    pub fn save_state(&self) -> MachineState {
        let (c, b) = (&self.cpu, &self.board);
        MachineState {
            config: self.config.clone(),
            cpu: CpuState {
                a: c.a,
                x: c.x,
                y: c.y,
                sp: c.sp,
                pc: c.pc,
                p: c.p,
                cycles: c.cycles,
                irq: c.irq,
                nmi_pending: c.nmi_pending(),
                stopped: c.stopped,
                waiting: c.waiting,
                opcode: c.opcode,
            },
            memory: b.memory.state(),
            lc: b.lc.clone(),
            slots: b.slots.save(),
            video: b.video,
            annunciators: b.annunciators,
            keyboard: b.keyboard.clone(),
            speaker: b.speaker.clone(),
            game: b.game.clone(),
            events: b.scheduler.events().to_vec(),
            actions: b.actions.clone(),
            interrupts: b.interrupts,
            cycles: b.cycles,
            cycles_in_frame: b.cycles_in_frame,
            frames: b.frames,
            pending: b.pending,
            opcode_ticks: b.opcode_ticks,
            opcode_pc: b.opcode_pc,
            opcode: b.opcode,
            nmi_line: self.nmi_line,
            irq_on_last_opcode_cycle: self.irq_on_last_opcode_cycle,
            irq_defer_1_opcode: self.irq_defer_1_opcode,
            cpu_owner: b.cpu_owner,
            rom_tap: b.rom_tap.as_ref().map(|t| t.save()),
            game_io: b.game_io.as_ref().map(|t| t.save()),
        }
    }

    /// Restores [`Machine::save_state`] output. Fails, leaving the machine as it was, if the
    /// state comes from another configuration or a machine with other cards.
    pub fn load_state(&mut self, s: &MachineState) -> Result<(), StateError> {
        if s.config != self.config {
            return Err(StateError::ConfigMismatch);
        }
        let b = &mut self.board;
        // Memory and Language Card sizes are checked first so a mismatch changes nothing; a card
        // that rejects its own bytes can still leave earlier slots loaded.
        if !b.memory.fits(&s.memory)
            || s.lc.regions() != b.lc.regions()
            || s.rom_tap.is_some() != b.rom_tap.is_some()
            || s.game_io.is_some() != b.game_io.is_some()
            || !b.slots.load(&s.slots)
        {
            return Err(StateError::Incompatible);
        }
        if let (Some(t), Some(st)) = (b.rom_tap.as_mut(), &s.rom_tap) {
            t.load(st);
        }
        if let (Some(t), Some(st)) = (b.game_io.as_mut(), &s.game_io) {
            t.load(st);
        }
        b.cpu_owner = s.cpu_owner;
        b.memory.restore(&s.memory);
        b.lc = s.lc.clone();
        b.video = s.video;
        b.annunciators = s.annunciators;
        b.keyboard = s.keyboard.clone();
        b.speaker = s.speaker.clone();
        b.game = s.game.clone();
        b.scheduler.set_events(s.events.clone());
        b.actions = s.actions.clone();
        b.interrupts = s.interrupts;
        (b.cycles, b.cycles_in_frame, b.frames, b.pending) =
            (s.cycles, s.cycles_in_frame, s.frames, s.pending);
        (b.opcode_ticks, b.opcode_pc, b.opcode) = (s.opcode_ticks, s.opcode_pc, s.opcode);
        let c = &mut self.cpu;
        let cs = &s.cpu;
        (c.a, c.x, c.y, c.sp, c.pc, c.p, c.cycles) =
            (cs.a, cs.x, cs.y, cs.sp, cs.pc, cs.p, cs.cycles);
        (c.irq, c.stopped, c.waiting, c.opcode) = (cs.irq, cs.stopped, cs.waiting, cs.opcode);
        c.set_nmi_pending(cs.nmi_pending);
        self.nmi_line = s.nmi_line;
        self.irq_on_last_opcode_cycle = s.irq_on_last_opcode_cycle;
        self.irq_defer_1_opcode = s.irq_defer_1_opcode;
        Ok(())
    }
}
