//! The [`Machine`]: CPU plus board, advanced cycle by cycle through `Bus::tick`.

use crate::card::{card_event_id, CardCtx, CardOp};
use crate::game_port::GamePort;
use crate::irq::{Interrupts, IrqSource};
use crate::keyboard::{Keyboard, NavKey};
use crate::speaker::Speaker;
use crate::language_card::LanguageCards;
use crate::memory::Memory;
use crate::video_mode::VideoMode;
use crate::video_scanner::VideoScanner;
use crate::scheduler::{EventId, Scheduler, SyncEvent};
use crate::slots::Slots;
use crate::Card;
use crate::{CardKind, MachineConfig};
use crate::Model;
use apple2_cpu::{Bus, Cpu, FLAG_I};

mod io;

/// What a synchronous event does when it expires. Later slices add device variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventAction {
    /// Assert the IRQ line for `source` (level; cleared with `Interrupts::set_irq`).
    RaiseIrq(IrqSource),
    /// [`Card::on_event`]`(token)` of the card in `slot`; the handler's return value re-arms it.
    Card { slot: u8, token: u16 },
    Noop,
}

/// Everything except the CPU, so `cpu.step(&mut board)` borrows cleanly.
pub struct Board {
    pub memory: Memory,
    /// Language Card / Saturn 128K cards (spec item 10): [`Slots`] defers to it for those kinds.
    pub lc: LanguageCards,
    /// Every other card, plus the `$C800-$CFFF` expansion-ROM selection.
    pub slots: Slots,
    pub video: VideoMode,
    /// AN0-AN3 (Memory.cpp `g_Annunciator`).
    pub annunciators: [bool; 4],
    pub keyboard: Keyboard,
    pub speaker: Speaker,
    pub game: GamePort,
    model: Model,
    /// `IsAppleIIeOrAbove`: the //e family and the //c.
    is_e: bool,
    pub scheduler: Scheduler,
    pub interrupts: Interrupts,
    actions: Vec<(EventId, EventAction)>,
    cycles: u64,
    cycles_in_frame: u32,
    cycles_per_frame: u32,
    scanner: VideoScanner,
    /// Cycles ticked by the instruction in execution. AppleWin advances the video scanner after
    /// each opcode (cpu6502.h:385), so I/O during it sees the scanner at the opcode's start.
    opcode_ticks: u32,
    frames: u64,
    /// Cycles ticked since the scheduler was last updated (it advances per opcode, as in AppleWin).
    pending: u32,
    opcode_pc: u16,
    opcode: u8,
}

impl Board {
    /// Opcode address and byte of the instruction in execution (for slice 3, AppleWin GH#700).
    pub fn current_opcode(&self) -> (u16, u8) {
        (self.opcode_pc, self.opcode)
    }

    /// Advances the scheduler by the cycles ticked since the last call (CPU.cpp
    /// `CheckSynchronousInterruptSources`). True if an IRQ-capable event expired exactly on the
    /// opcode's last cycle.
    fn update_events(&mut self) -> bool {
        let n = std::mem::take(&mut self.pending) as i32;
        let floating_bus = self.floating_bus();
        let Board { scheduler, interrupts, actions, slots, cycles, .. } = self;
        let irq_on_last_cycle = scheduler.update(n, |id, _cycles| {
            match actions.iter().find(|(i, _)| *i == id).map(|(_, a)| *a) {
                Some(EventAction::RaiseIrq(src)) => interrupts.set_irq(src, true),
                Some(EventAction::Card { slot, token }) => {
                    if let Some(card) = slots.cards.get_mut(slot as usize).and_then(Option::as_deref_mut) {
                        let mut ctx = CardCtx { slot, cycles: *cycles, floating_bus, interrupts, ops: &mut slots.ops };
                        return card.on_event(token, &mut ctx);
                    }
                }
                Some(EventAction::Noop) | None => {}
            }
            0
        });
        self.flush_card_ops();
        irq_on_last_cycle
    }

    /// Runs `f` on the card in `slot` (None if empty or a RAM card) and applies the scheduling
    /// requests it made. No allocation unless the card schedules something.
    pub(crate) fn card_call<R>(&mut self, slot: u8, f: impl FnOnce(&mut dyn Card, &mut CardCtx) -> R) -> Option<R> {
        let floating_bus = self.floating_bus();
        let Board { slots, interrupts, cycles, .. } = self;
        let card = slots.cards.get_mut(slot as usize)?.as_deref_mut()?;
        let mut ctx = CardCtx { slot, cycles: *cycles, floating_bus, interrupts, ops: &mut slots.ops };
        let r = f(card, &mut ctx);
        self.flush_card_ops();
        Some(r)
    }

    /// Applies [`CardCtx::schedule`]/[`CardCtx::cancel`]: times are relative to the current cycle,
    /// i.e. including the cycles of the running opcode the scheduler has not seen yet.
    fn flush_card_ops(&mut self) {
        if self.slots.ops.is_empty() {
            return;
        }
        let ops = std::mem::take(&mut self.slots.ops);
        for op in &ops {
            match *op {
                CardOp::Schedule { slot, token, cycles } => {
                    let id = card_event_id(slot, token);
                    self.scheduler.remove(id);
                    self.actions.retain(|(i, _)| *i != id);
                    self.actions.push((id, EventAction::Card { slot, token }));
                    self.scheduler.insert(SyncEvent::new(id, cycles + self.pending as i32));
                }
                CardOp::Cancel { slot, token } => {
                    let id = card_event_id(slot, token);
                    self.scheduler.remove(id);
                    self.actions.retain(|(i, _)| *i != id);
                }
            }
        }
        self.slots.ops = ops;
        self.slots.ops.clear();
    }

    /// Calls `f` on every card with a context (reset, per-run update).
    fn for_each_card(&mut self, mut f: impl FnMut(&mut dyn Card, &mut CardCtx)) {
        for slot in 0..8 {
            self.card_call(slot, &mut f);
        }
    }
}

impl Bus for Board {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        match addr >> 8 {
            0xC0 => self.io_read(addr),
            0xC1..=0xCF => self.cx_access(addr, None),
            _ => match self.memory.read(addr) {
                Some(v) => v,
                None => self.floating_bus(),
            },
        }
    }

    #[inline]
    fn write(&mut self, addr: u16, value: u8) {
        match addr >> 8 {
            0xC0 => self.io_write(addr, value),
            0xC1..=0xCF => {
                self.cx_access(addr, Some(value));
            }
            _ => self.memory.write(addr, value),
        }
    }

    fn tick(&mut self) {
        self.cycles += 1;
        self.pending += 1;
        self.opcode_ticks += 1;
        self.cycles_in_frame += 1;
        if self.cycles_in_frame == self.cycles_per_frame {
            self.cycles_in_frame = 0;
            self.frames += 1;
        }
    }

    fn opcode_fetched(&mut self, pc: u16, opcode: u8) {
        self.opcode_pc = pc;
        self.opcode = opcode;
    }
}

pub struct Machine {
    config: MachineConfig,
    cpu: Cpu,
    board: Board,
    nmi_line: bool,
    /// CPU.cpp g_irqOnLastOpcodeCycle / g_irqDefer1Opcode.
    irq_on_last_opcode_cycle: bool,
    irq_defer_1_opcode: bool,
}

impl Machine {
    pub fn new(config: MachineConfig) -> Self {
        let rom = Self::rom_image(&config);
        let is_e = !config.model.is_apple2();
        let mut memory = Memory::new(config.model, &rom, config.aux_card, config.slots[0].is_some());
        let mut lc = LanguageCards::new(is_e, &config.slots);
        lc.attach(&mut memory);
        // Kinds backed by `LanguageCards`; a new `CardKind` forces a decision here.
        for kind in config.slots.iter().flatten() {
            match kind {
                CardKind::LanguageCard16K | CardKind::Saturn128K => {}
            }
        }
        let board = Board {
            memory,
            lc,
            slots: Slots::default(),
            video: VideoMode::default(),
            annunciators: [false; 4],
            keyboard: Keyboard::default(),
            speaker: Speaker::default(),
            game: GamePort::default(),
            model: config.model,
            is_e,
            scheduler: Scheduler::new(),
            interrupts: Interrupts::default(),
            actions: Vec::new(),
            cycles: 0,
            cycles_in_frame: 0,
            cycles_per_frame: config.video_standard.cycles_per_frame(),
            scanner: VideoScanner::new(config.video_standard, config.model.value() & 0xFFFE == 0),
            opcode_ticks: 0,
            frames: 0,
            pending: 0,
            opcode_pc: 0,
            opcode: 0,
        };
        let mut m = Machine {
            cpu: Cpu::new(config.model.cpu_variant()),
            config,
            board,
            nmi_line: false,
            irq_on_last_opcode_cycle: false,
            irq_defer_1_opcode: false,
        };
        m.reset();
        m
    }

    /// System ROM with the F8 overrides of `MemInitializeCustomF8ROM` (Memory.cpp:2120-2171), applied
    /// after a custom ROM: an original ][ with a 16K Language Card gets the ][+ F8 (autostart)
    /// ROM, and "Freeze's F8" replaces F8 on the ][ family.
    fn rom_image(config: &MachineConfig) -> Vec<u8> {
        const F8_SIZE: usize = 0x800;
        const ROM_SIZE: usize = 0x3000;
        let mut rom = match (&config.custom_rom, config.model.system_rom()) {
            (Some(r), _) => r.clone(),
            (None, Some(r)) => r.to_vec(),
            (None, None) => Vec::new(),
        };
        if rom.len() >= ROM_SIZE {
            let f8 = ROM_SIZE - F8_SIZE..ROM_SIZE;
            if config.model == Model::Apple2 && config.slots[0] == Some(CardKind::LanguageCard16K) {
                rom[f8.clone()].copy_from_slice(&apple2_roms::APPLE2_PLUS[f8.clone()]);
            }
            if config.use_freeze_f8 && config.model.is_apple2() {
                rom[f8].copy_from_slice(apple2_roms::FREEZES_NON_AUTOSTART_F8_ROM);
            }
        }
        rom
    }

    pub fn config(&self) -> &MachineConfig {
        &self.config
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    /// Absolute cycle counter since power-on.
    pub fn cycles(&self) -> u64 {
        self.board.cycles
    }

    pub fn cycles_in_frame(&self) -> u32 {
        self.board.cycles_in_frame
    }

    /// Completed frames since power-on.
    pub fn frames(&self) -> u64 {
        self.board.frames
    }

    /// Current `MF_*` paging flags (see [`crate::memory::mf`]).
    pub fn memmode(&self) -> u32 {
        self.board.memory.memmode()
    }

    /// Reads as the CPU would under the current paging, without touching soft switches
    /// ($C000-$C0FF reads the floating bus) or the cycle count (test/debug helper).
    pub fn peek(&self, addr: u16) -> u8 {
        self.board.memory.read(addr).unwrap_or_else(|| self.board.floating_bus())
    }

    /// Writes as the CPU would under the current paging (ROM and I/O ignore it).
    pub fn poke(&mut self, addr: u16, value: u8) {
        if !(0xC000..0xD000).contains(&addr) {
            self.board.memory.write(addr, value);
        }
    }

    /// Main RAM, ignoring paging.
    pub fn peek_main(&self, addr: u16) -> u8 {
        self.board.memory.peek_main(addr)
    }

    pub fn poke_main(&mut self, addr: u16, value: u8) {
        self.board.memory.poke_main(addr, value);
    }

    /// Aux RAM of the current RamWorks bank, ignoring paging.
    pub fn peek_aux(&self, addr: u16) -> u8 {
        self.board.memory.peek_aux(addr)
    }

    pub fn poke_aux(&mut self, addr: u16, value: u8) {
        self.board.memory.poke_aux(addr, value);
    }

    /// Schedules `action` to run `cycles` from now.
    pub fn schedule(&mut self, id: EventId, cycles: i32, action: EventAction) {
        self.board.actions.retain(|(i, _)| *i != id);
        self.board.actions.push((id, action));
        self.board.scheduler.insert(SyncEvent::new(id, cycles));
    }

    /// Inserts a card in an empty slot 0-7 not used by a Language Card/Saturn; false if refused.
    /// The card gets a power-on [`Card::reset`].
    pub fn insert_card(&mut self, slot: u8, card: Box<dyn Card>) -> bool {
        if self.board.lc.has_slot(slot) || !self.board.slots.insert(slot, card) {
            return false;
        }
        self.board.card_call(slot, |c, ctx| c.reset(true, ctx));
        true
    }

    // ---- Input and sound (slice 6). Host-key translation is the frontend's job; see
    // [`crate::keyboard`]. ----

    /// Types a character as AppleWin's `WM_CHAR` path sees it (ANSI/Latin-1). Applies the model's
    /// case and remap rules; false if the model has no such key.
    pub fn key_press(&mut self, key: u8) -> bool {
        self.board.keyboard.press(self.config.model, key)
    }

    /// Cursor/edit keys; false if the model lacks the key.
    pub fn key_press_nav(&mut self, key: NavKey) -> bool {
        self.board.keyboard.press_nav(self.config.model, key)
    }

    /// Queues `text` for pasting, one character per strobe clear; false if one is pending.
    pub fn paste(&mut self, text: &str) -> bool {
        self.board.keyboard.paste(text)
    }

    /// Whether any Apple key is held (AKD, $C010 bit 7 on the //e).
    pub fn set_any_key_down(&mut self, down: bool) {
        self.board.keyboard.set_any_key_down(down);
    }

    /// Caps lock (Latin/Cyrillic lock on the Pravets). Has no effect on the ][/][+ itself.
    pub fn set_caps_lock(&mut self, on: bool) {
        self.board.keyboard.set_caps_lock(on);
    }

    /// TK3000 //e |Mode| key (accented-character mode).
    pub fn set_tk3000_mode(&mut self, on: bool) {
        self.board.keyboard.set_tk3000_mode(on);
    }

    pub fn set_open_apple(&mut self, down: bool) {
        self.board.game.open_apple = down;
    }

    pub fn set_closed_apple(&mut self, down: bool) {
        self.board.game.closed_apple = down;
    }

    /// Pushbutton `n` (0-2).
    pub fn set_button(&mut self, n: usize, down: bool) {
        self.board.game.buttons[n] = down;
    }

    /// Paddle `n` (0-3) position, 0-255; read when a $C07x strobe starts its timer.
    pub fn set_paddle(&mut self, n: u8, pos: u8) {
        self.board.game.paddles[usize::from(n)] = pos;
    }

    /// Whether joystick `n` (0 or 1) is plugged in; the default is joystick 0 only. Without
    /// joystick 0 the ][ family reads PB0/PB1 pressed and paddles 0-1 never time out; without
    /// joystick 1 PB2 reads pressed and paddles 2-3 never time out.
    pub fn set_joystick_attached(&mut self, n: usize, attached: bool) {
        self.board.game.attached[n] = attached;
    }

    /// Removes the speaker toggles recorded so far (absolute cycles, oldest first). If the
    /// frontend never drains, only the newest [`crate::speaker::CAPACITY`] are kept.
    pub fn drain_speaker_toggles(&mut self) -> impl Iterator<Item = u64> + '_ {
        self.board.speaker.drain()
    }

    /// Toggles discarded because the log was full.
    pub fn speaker_toggles_dropped(&self) -> u64 {
        self.board.speaker.dropped()
    }

    /// CPU reset (the Reset key): pending events and cycle counters are kept.
    pub fn reset(&mut self) {
        self.reset_cpu_and_board(false);
    }

    fn reset_cpu_and_board(&mut self, power_cycle: bool) {
        // Utilities.cpp CtrlReset(): the //e resets paging and the video mode; the annunciators
        // are reset on the //e and Base64A only (the Base64A ROM page is reset by the memory).
        let b = &mut self.board;
        b.memory.reset();
        b.lc.reset(b.is_e);
        if b.is_e {
            b.video = VideoMode::default();
        }
        if b.is_e || b.model == Model::Base64A {
            b.annunciators = [false; 4];
        }
        b.keyboard.reset();
        b.for_each_card(|c, ctx| c.reset(power_cycle, ctx));
        self.cpu.reset(&mut self.board);
        self.irq_on_last_opcode_cycle = false;
        self.irq_defer_1_opcode = false;
        self.board.opcode_ticks = 0;
    }

    /// Power off and on: RAM cleared, counters, events and interrupt lines reset.
    pub fn power_cycle(&mut self) {
        let b = &mut self.board;
        b.memory.power_cycle();
        b.lc.power_cycle(b.is_e, &mut b.memory);
        b.slots.power_cycle();
        b.video = VideoMode::default();
        b.annunciators = [false; 4];
        b.speaker.reset();
        b.game.reset();
        b.scheduler.reset();
        b.actions.clear();
        b.interrupts.clear();
        b.cycles = 0;
        b.cycles_in_frame = 0;
        b.frames = 0;
        b.pending = 0;
        b.opcode_ticks = 0;
        self.cpu.cycles = 0;
        self.nmi_line = false;
        self.reset_cpu_and_board(true);
    }

    /// Executes one instruction (or interrupt entry); returns the cycles it took.
    pub fn step(&mut self) -> u32 {
        self.board.opcode_ticks = 0;
        let nmi = self.board.interrupts.nmi_line();
        if nmi && !self.nmi_line {
            self.cpu.nmi();
        }
        self.nmi_line = nmi;

        // CPU.cpp IRQ(): an IRQ that appeared on the previous opcode's last cycle waits one opcode.
        let mut irq = self.board.interrupts.irq_line();
        if irq && self.cpu.p & FLAG_I == 0 {
            if self.irq_on_last_opcode_cycle && !self.irq_defer_1_opcode {
                self.irq_defer_1_opcode = true;
                irq = false;
            } else {
                self.irq_defer_1_opcode = false;
            }
        }
        self.irq_on_last_opcode_cycle = false;
        self.cpu.irq = irq;

        let cycles = self.cpu.step(&mut self.board);
        // SetIrqOnLastOpcodeCycle only matters while interrupts are enabled.
        if self.board.update_events() && self.cpu.p & FLAG_I == 0 {
            self.irq_on_last_opcode_cycle = true;
        }
        self.board.opcode_ticks = 0; // the scanner catches up (CPU.cpp after CheckSynchronousInterruptSources)
        cycles
    }

    /// Runs whole instructions until at least `n` cycles passed; returns the cycles executed.
    pub fn run_cycles(&mut self, n: u64) -> u64 {
        let mut done = 0;
        while done < n {
            done += self.step() as u64;
        }
        self.board.for_each_card(|c, ctx| c.update(done, ctx));
        done
    }

    /// Runs until the next frame boundary; returns the cycles executed.
    pub fn run_frame(&mut self) -> u64 {
        let target = self.board.frames + 1;
        let mut done = 0;
        while self.board.frames < target {
            done += self.step() as u64;
        }
        self.board.for_each_card(|c, ctx| c.update(done, ctx));
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Model, VideoStandard};

    const SRC: IrqSource = IrqSource::Mouse;

    /// Machine with a 12K custom ROM whose vectors are RESET=$0300, IRQ=$0400, NMI=$0500.
    fn machine(model: Model, std: VideoStandard) -> Machine {
        let mut rom = vec![0xEAu8; 0x3000];
        for (vec, target) in [(0xFFFAu16, 0x0500u16), (0xFFFC, 0x0300), (0xFFFE, 0x0400)] {
            let o = vec as usize - 0xD000;
            rom[o] = target as u8;
            rom[o + 1] = (target >> 8) as u8;
        }
        let mut cfg = MachineConfig::for_model(model);
        cfg.video_standard = std;
        cfg.custom_rom = Some(rom);
        let mut m = Machine::new(cfg);
        // $0300: CLI, then NOPs (RAM is zeroed = BRK, so fill).
        m.poke(0x300, 0x58);
        for a in 0x301..0x320 {
            m.poke(a, 0xEA);
        }
        m.poke(0x400, 0xE6); // handler: INC $10 ; JMP $0402
        m.poke(0x401, 0x10);
        m.poke(0x402, 0x4C);
        m.poke(0x403, 0x02);
        m.poke(0x404, 0x04);
        m.poke(0x500, 0xE6); // NMI handler: INC $11 ; JMP $0502
        m.poke(0x501, 0x11);
        m.poke(0x502, 0x4C);
        m.poke(0x503, 0x02);
        m.poke(0x504, 0x05);
        m
    }

    #[test]
    fn reset_loads_vector_and_uses_model_cpu() {
        let m = machine(Model::Apple2Plus, VideoStandard::Ntsc);
        assert_eq!(m.cpu().pc, 0x0300);
        assert_eq!(m.cpu().variant, apple2_cpu::Variant::Nmos6502);
        let m = machine(Model::Apple2eEnhanced, VideoStandard::Ntsc);
        assert_eq!(m.cpu().variant, apple2_cpu::Variant::Cmos65C02);
    }

    #[test]
    fn real_rom_reset_vector_is_in_rom() {
        for model in [Model::Apple2, Model::Apple2Plus, Model::Apple2e, Model::Apple2eEnhanced, Model::Base64A, Model::Pravets8A] {
            let m = Machine::new(MachineConfig::for_model(model));
            assert!(m.cpu().pc >= 0xC000, "{model:?} pc={:#x}", m.cpu().pc);
        }
    }

    #[test]
    fn cycle_counter_follows_executed_cycles() {
        let mut m = machine(Model::Apple2, VideoStandard::Ntsc);
        assert_eq!(m.cycles(), 0);
        let done = m.run_cycles(100);
        assert!((100..107).contains(&done));
        assert_eq!(m.cycles(), done);
        assert_eq!(m.cycles_in_frame() as u64, done);
    }

    #[test]
    fn run_frame_stops_on_frame_boundary_ntsc_and_pal() {
        for (std, len) in [(VideoStandard::Ntsc, 17030u64), (VideoStandard::Pal, 20280)] {
            let mut m = machine(Model::Apple2, std);
            let done = m.run_frame();
            assert!(done >= len && done < len + 7, "{done}");
            assert_eq!(m.frames(), 1);
            assert!(m.cycles_in_frame() < 7);
        }
    }

    #[test]
    fn power_cycle_clears_counters_and_ram() {
        let mut m = machine(Model::Apple2, VideoStandard::Ntsc);
        m.run_frame();
        m.poke(0x10, 7);
        m.power_cycle();
        assert_eq!((m.cycles(), m.frames(), m.peek(0x10)), (0, 0, 0));
    }

    #[test]
    fn reset_keeps_cycle_counter() {
        let mut m = machine(Model::Apple2, VideoStandard::Ntsc);
        m.run_cycles(50);
        let c = m.cycles();
        m.reset();
        assert_eq!((m.cycles(), m.cpu().pc), (c, 0x300));
    }

    #[test]
    fn opcode_is_visible_to_the_board() {
        let mut m = machine(Model::Apple2, VideoStandard::Ntsc);
        m.step();
        assert_eq!(m.board().current_opcode(), (0x300, 0x58));
    }

    #[test]
    fn scheduled_irq_is_taken_after_event_expires() {
        let mut m = machine(Model::Apple2, VideoStandard::Ntsc);
        m.schedule(EventId(1), 3, EventAction::RaiseIrq(SRC));
        m.step(); // CLI
        m.step(); // NOP: event expires mid-opcode (rem -1)
        m.step(); // IRQ taken
        assert_eq!(m.cpu().pc, 0x400);
    }

    #[test]
    fn irq_from_event_on_last_cycle_of_opcode_is_deferred_one_opcode() {
        let mut m = machine(Model::Apple2, VideoStandard::Ntsc);
        m.schedule(EventId(1), 4, EventAction::RaiseIrq(SRC));
        m.step(); // CLI (2 cycles)
        m.step(); // NOP (event expires exactly on its last cycle)
        m.step(); // deferred: runs a NOP
        assert_eq!(m.cpu().pc, 0x303);
        m.step(); // now taken
        assert_eq!(m.cpu().pc, 0x400);
    }

    #[test]
    fn irq_masked_by_i_flag_is_not_taken_until_cli() {
        let mut m = machine(Model::Apple2, VideoStandard::Ntsc);
        m.board_mut().interrupts.set_irq(SRC, true);
        m.poke(0x300, 0xEA);
        m.poke(0x301, 0x58);
        m.step();
        m.step(); // CLI
        assert_eq!(m.cpu().pc, 0x302);
        m.step();
        assert_eq!(m.cpu().pc, 0x400);
    }

    #[test]
    fn nmi_is_edge_triggered_through_the_interrupt_lines() {
        let mut m = machine(Model::Apple2, VideoStandard::Ntsc);
        m.board_mut().interrupts.set_nmi(SRC, true);
        m.step();
        assert_eq!(m.cpu().pc, 0x500);
        m.run_cycles(100); // line stays high: no retrigger, handler loops
        assert_eq!(m.peek(0x11), 1, "handler entered once, not re-triggered");
    }
}
