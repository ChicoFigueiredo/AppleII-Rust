//! Super Serial Card: a 6551 ACIA behind the SSC firmware (SerialComms.cpp).
//!
//! Register map at `$C080+slot*16` (SerialComms.cpp:400-454): `+1`/`+2` DIP switches, `+8`
//! transmit/receive data, `+9` status (write = programmed reset), `+A` command, `+B` control.
//! The 2K firmware is both the slot ROM (its last page, `$C700`-style) and the `$C800` expansion
//! ROM (:955-975).
//!
//! Differences forced by the headless core (ADR 0001): AppleWin's COM thread and window-message
//! socket events become a 1000-cycle poll event ([`POLL_CYCLES`]) plus [`Card::update`]; and
//! transmission takes the baud-rate-dependent frame time instead of completing instantly, which
//! the real ACIA does and AppleWin only gets for COM ports (`EV_TXEMPTY`).

use crate::serial::{LineConfig, Parity, SerialBackend, StopBits};
use apple2_core::card::{CardCtx, EXPANSION_ROM_SIZE, SLOT_ROM_SIZE};
use apple2_core::{Card, IrqSource};
use std::collections::VecDeque;

/// Cycles between backend polls while the card is open.
pub const POLL_CYCLES: i32 = 1000;
/// NTSC CPU clock, used to turn baud rates into cycles.
const CPU_HZ: u64 = 1_020_484;

const TOKEN_TX_DONE: u16 = 0;
const TOKEN_POLL: u16 = 1;

// 6551 command register (SerialComms.cpp:460-474).
const CMD_PARITY_MASK: u8 = 3 << 6;
const CMD_PARITY_ENA: u8 = 1 << 5;
const CMD_TX_MASK: u8 = 3 << 2;
const CMD_TX_IRQ_ENA_RTS_LOW: u8 = 1 << 2;
const CMD_TX_IRQ_DIS_RTS_HIGH: u8 = 0;
const CMD_RX_IRQ_DIS: u8 = 1 << 1;
const CMD_DTR: u8 = 1;

// 6551 status register (:806-814).
pub const ST_IRQ: u8 = 1 << 7;
pub const ST_DSR: u8 = 1 << 6;
pub const ST_DCD: u8 = 1 << 5;
pub const ST_TX_EMPTY: u8 = 1 << 4;
pub const ST_RX_FULL: u8 = 1 << 3;

/// `eFWMODE` (SerialComms.h:6): value of DIPSW1 bits 0-3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareMode {
    /// Communications mode ("CIC", the SSC proper).
    Cic = 0,
    SicP8 = 1,
    Ppc = 2,
    SicP8a = 3,
}

/// The two DIP switch banks (`SSC_DIPSW`, SerialComms.cpp:50-62). Read by the firmware at
/// `$C0s1`/`$C0s2`; only applied by AppleWin after a reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DipSwitches {
    pub baud: u32,
    pub firmware_mode: FirmwareMode,
    pub stop_bits: StopBits,
    pub data_bits: u8,
    pub parity: Parity,
    /// SW2-5: the SSC discards an LF right after a CR.
    pub linefeed: bool,
    /// SW2-6: ACIA interrupts reach the Apple II (cannot be read by software).
    pub interrupts: bool,
}

impl Default for DipSwitches {
    /// 9600-8-N-1, communications mode (:50-62).
    fn default() -> Self {
        DipSwitches {
            baud: 9600,
            firmware_mode: FirmwareMode::Cic,
            stop_bits: StopBits::One,
            data_bits: 8,
            parity: Parity::None,
            linefeed: false,
            interrupts: true,
        }
    }
}

/// `BaudRateToIndex` (:165-183).
fn baud_to_index(baud: u32) -> u8 {
    match baud {
        110 => 0x05,
        300 => 0x06,
        600 => 0x07,
        1200 => 0x08,
        2400 => 0x0A,
        4800 => 0x0C,
        9600 => 0x0E,
        19200 => 0x0F,
        115200 => 0x00,
        _ => 0x0E,
    }
}

/// The state AppleWin saves for the SSC (`CSuperSerialCard::SaveSnapshot`, SerialComms.cpp:1448).
///
/// AppleWin's `DIPSW Default` is a process-wide static (9600-8-N-1 unless the config changes
/// it) and is not kept by the card; `Support DCD` and the serial port name are host settings of
/// the back end. They are written with AppleWin's defaults and ignored on load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SscSnapshot {
    /// `m_DIPSWCurrent`.
    pub dip: DipSwitches,
    pub command: u8,
    pub control: u8,
    pub tx_irq_pending: bool,
    pub rx_irq_pending: bool,
    /// `m_vbTxEmpty` ("Written TX").
    pub tx_empty: bool,
}

pub struct SuperSerialCard {
    backend: Box<dyn SerialBackend>,
    dip: DipSwitches,
    rom: Box<[u8; EXPANSION_ROM_SIZE]>,
    command: u8,
    control: u8,
    baud: u32,
    byte_size: u8,
    parity: Parity,
    stop_bits: StopBits,
    tx_irq_enabled: bool,
    rx_irq_enabled: bool,
    tx_irq_pending: bool,
    rx_irq_pending: bool,
    tx_empty: bool,
    dtr: bool,
    rts: bool,
    rx: VecDeque<u8>,
}

impl SuperSerialCard {
    pub fn new(backend: Box<dyn SerialBackend>) -> Self {
        Self::with_dip_switches(backend, DipSwitches::default())
    }

    pub fn with_dip_switches(backend: Box<dyn SerialBackend>, dip: DipSwitches) -> Self {
        let mut rom = Box::new([0u8; EXPANSION_ROM_SIZE]);
        rom.copy_from_slice(apple2_roms::SSC);
        let mut c = SuperSerialCard {
            backend,
            dip,
            rom,
            command: 0,
            control: 0,
            baud: 115200,
            byte_size: 8,
            parity: Parity::None,
            stop_bits: StopBits::One,
            tx_irq_enabled: false,
            rx_irq_enabled: false,
            tx_irq_pending: false,
            rx_irq_pending: false,
            tx_empty: true,
            dtr: false,
            rts: false,
            rx: VecDeque::new(),
        };
        c.internal_reset();
        c
    }

    /// `InternalReset` (:103-126): hardware reset zeroes command and control.
    fn internal_reset(&mut self) {
        self.update_command_reg(0);
        self.update_control_reg(0);
        self.tx_irq_pending = false;
        self.rx_irq_pending = false;
        self.tx_empty = true;
        self.rx.clear();
        self.dtr = false;
        self.rts = false;
    }

    /// `UpdateCommandReg` (:496-557). The echo mode and transmit-break settings are ignored.
    fn update_command_reg(&mut self, command: u8) {
        self.command = command;
        self.parity = if command & CMD_PARITY_ENA != 0 {
            match command & CMD_PARITY_MASK {
                0x00 => Parity::Odd,
                0x40 => Parity::Even,
                0x80 => Parity::Mark,
                _ => Parity::Space,
            }
        } else {
            Parity::None
        };
        self.rts = (command & CMD_TX_MASK) != CMD_TX_IRQ_DIS_RTS_HIGH;
        if self.dip.interrupts && command & CMD_DTR != 0 {
            self.tx_irq_enabled = command & CMD_TX_MASK == CMD_TX_IRQ_ENA_RTS_LOW;
            self.rx_irq_enabled = command & CMD_RX_IRQ_DIS == 0;
        } else {
            self.tx_irq_enabled = false;
            self.rx_irq_enabled = false;
        }
        self.dtr = command & CMD_DTR != 0;
    }

    /// `UpdateControlReg` (:575-634).
    fn update_control_reg(&mut self, control: u8) {
        self.control = control;
        self.baud = match control & 0x0F {
            0x00 => 115200,
            0x01..=0x05 => 110,
            0x06 => 300,
            0x07 => 600,
            0x08 => 1200,
            0x09 | 0x0A => 2400,
            0x0B | 0x0C => 4800,
            0x0D | 0x0E => 9600,
            _ => 19200,
        };
        self.byte_size = match control & 0x60 {
            0x00 => 8,
            0x20 => 7,
            0x40 => 6,
            _ => 5,
        };
        self.stop_bits = if control & 0x80 != 0 {
            if self.byte_size == 8 && self.parity != Parity::None {
                StopBits::One
            } else if self.byte_size == 5 && self.parity == Parity::None {
                StopBits::OnePointFive
            } else {
                StopBits::Two
            }
        } else {
            StopBits::One
        };
    }

    fn line_config(&self) -> LineConfig {
        LineConfig {
            baud: self.baud,
            data_bits: self.byte_size,
            parity: self.parity,
            stop_bits: self.stop_bits,
            dtr: self.dtr,
            rts: self.rts,
        }
    }

    /// Why the transport could not be opened (busy or missing port, no permission), if so.
    pub fn last_error(&self) -> Option<&str> {
        self.backend.last_error()
    }

    /// `CheckComm` (:212): opens the transport on first use.
    fn check_comm(&mut self) -> bool {
        if self.backend.is_active() {
            return true;
        }
        if self.backend.open() {
            let line = self.line_config();
            self.backend.configure(&line);
            true
        } else {
            false
        }
    }

    /// Cycles one character occupies on the wire at the current format.
    pub fn frame_cycles(&self) -> i32 {
        let stop2 = match self.stop_bits {
            StopBits::One => 2,
            StopBits::OnePointFive => 3,
            StopBits::Two => 4,
        };
        let bits2 = 2 * (1 + self.byte_size as u64 + (self.parity != Parity::None) as u64) + stop2;
        ((bits2 * CPU_HZ) / (2 * self.baud as u64)).max(1) as i32
    }

    /// Pulls bytes from the backend; a new byte raises the receive IRQ if enabled
    /// (`CommTcpSerialReceive`, :376-396).
    fn poll(&mut self, ctx: &mut CardCtx) {
        if !self.backend.is_active() {
            return;
        }
        let before = self.rx.len();
        self.backend.poll_rx(&mut self.rx);
        if self.rx.len() > before && self.rx_irq_enabled {
            ctx.set_irq(IrqSource::Ssc, true);
            self.rx_irq_pending = true;
        }
    }

    fn comm_receive(&mut self, ctx: &mut CardCtx) -> u8 {
        if !self.check_comm() {
            return 0;
        }
        self.poll(ctx);
        if self.rx.is_empty() || self.command & CMD_DTR == 0 {
            return 0; // receiver disabled: the sender must not send (:665)
        }
        let result = self.rx.pop_front().unwrap_or(0);
        if self.rx_irq_enabled && !self.rx.is_empty() {
            ctx.set_irq(IrqSource::Ssc, true);
            self.rx_irq_pending = true;
        }
        result
    }

    fn comm_status(&mut self, ctx: &mut CardCtx) -> u8 {
        if !self.check_comm() {
            return ST_DSR | ST_DCD | ST_TX_EMPTY;
        }
        self.poll(ctx);
        let modem = self.backend.modem_status();
        let mut irq = 0;
        if self.tx_irq_enabled {
            if self.tx_irq_pending {
                irq |= ST_IRQ;
            }
            self.tx_irq_pending = false; // only the first read reports it
        }
        if self.rx_irq_enabled {
            if self.rx_irq_pending {
                irq |= ST_IRQ;
            }
            self.rx_irq_pending = false;
        }
        let status = irq
            | if modem.dsr { 0 } else { ST_DSR } // active low (GH#386)
            | if modem.dcd { 0 } else { ST_DCD }
            | if self.tx_empty { ST_TX_EMPTY } else { 0 }
            | if self.rx.is_empty() { 0 } else { ST_RX_FULL };
        ctx.set_irq(IrqSource::Ssc, false); // reading status always clears the IRQ
        status
    }

    fn comm_transmit(&mut self, value: u8, ctx: &mut CardCtx) {
        if !self.check_comm() {
            return;
        }
        if self.command & CMD_TX_MASK == CMD_TX_IRQ_DIS_RTS_HIGH {
            return; // transmitter disabled: discard (:742)
        }
        if self.backend.can_send() {
            let mut data = value;
            if self.byte_size < 8 {
                data &= !(1 << self.byte_size);
            }
            if self.backend.write(data) {
                self.tx_empty = false;
                ctx.schedule(TOKEN_TX_DONE, self.frame_cycles());
            }
        }
    }

    /// `CommDipSw` (:896-951).
    fn dip_switch(&mut self, reg: u8) -> u8 {
        match reg {
            1 => (baud_to_index(self.dip.baud) << 4) | self.dip.firmware_mode as u8,
            _ => {
                let sw2_1 = (self.dip.stop_bits == StopBits::Two) as u8;
                let sw2_2 = (self.dip.data_bits == 7) as u8;
                let (sw2_3, sw2_4) = match self.dip.parity {
                    Parity::Odd => (0, 1),
                    Parity::Even => (1, 1),
                    _ => (0, 0),
                };
                let sw2_5 = !self.dip.linefeed as u8;
                let cts = if self.check_comm() && self.backend.modem_status().cts {
                    0
                } else {
                    1
                };
                sw2_1 << 7 | sw2_2 << 5 | sw2_3 << 3 | sw2_4 << 2 | sw2_5 << 1 | cts
            }
        }
    }

    fn command_access(&mut self, write: Option<u8>) -> u8 {
        if !self.check_comm() {
            return 0;
        }
        if let Some(v) = write {
            if v != self.command {
                self.update_command_reg(v);
                let line = self.line_config();
                self.backend.configure(&line);
            }
        }
        self.command
    }

    fn control_access(&mut self, write: Option<u8>) -> u8 {
        if !self.check_comm() {
            return 0;
        }
        if let Some(v) = write {
            if v != self.control {
                self.update_control_reg(v);
                let line = self.line_config();
                self.backend.configure(&line);
            }
        }
        self.control
    }

    /// The AppleWin view of the card state (see [`SscSnapshot`]).
    pub fn snapshot(&self) -> SscSnapshot {
        SscSnapshot {
            dip: self.dip,
            command: self.command,
            control: self.control,
            tx_irq_pending: self.tx_irq_pending,
            rx_irq_pending: self.rx_irq_pending,
            tx_empty: self.tx_empty,
        }
    }

    /// `LoadSnapshot` (SerialComms.cpp:1487): DIP switches first, then command and control
    /// registers (`UpdateCommandAndControlRegs`), then the flags. The IRQ line itself is the
    /// machine's state, not the card's.
    pub fn apply_snapshot(&mut self, s: &SscSnapshot) {
        self.dip = s.dip;
        self.update_command_reg(s.command);
        self.update_control_reg(s.control);
        self.tx_irq_pending = s.tx_irq_pending;
        self.rx_irq_pending = s.rx_irq_pending;
        self.tx_empty = s.tx_empty;
    }

    pub fn command(&self) -> u8 {
        self.command
    }

    pub fn control(&self) -> u8 {
        self.control
    }
}

impl Card for SuperSerialCard {
    fn name(&self) -> &'static str {
        "Super Serial Card"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        match lo {
            0x1 | 0x2 => self.dip_switch(lo),
            0x8 => self.comm_receive(ctx),
            0x9 => self.comm_status(ctx),
            0xA => self.command_access(None),
            0xB => self.control_access(None),
            _ => ctx.floating_bus, // IO_Null
        }
    }

    fn io_write(&mut self, lo: u8, value: u8, ctx: &mut CardCtx) {
        match lo {
            0x8 => self.comm_transmit(value, ctx),
            // CommProgramReset: parity bits survive, the rest of the command register clears.
            0x9 => self.update_command_reg(self.command & (CMD_PARITY_MASK | CMD_PARITY_ENA)),
            0xA => {
                self.command_access(Some(value));
            }
            0xB => {
                self.control_access(Some(value));
            }
            _ => {}
        }
    }

    fn slot_rom_read(&mut self, offset: u8, _ctx: &mut CardCtx) -> u8 {
        self.rom[7 * SLOT_ROM_SIZE + offset as usize]
    }

    fn expansion_rom(&self) -> Option<&[u8; EXPANSION_ROM_SIZE]> {
        Some(&self.rom)
    }

    fn reset(&mut self, _power_cycle: bool, ctx: &mut CardCtx) {
        self.backend.close();
        self.internal_reset();
        ctx.set_irq(IrqSource::Ssc, false);
        ctx.cancel(TOKEN_TX_DONE);
        ctx.schedule(TOKEN_POLL, POLL_CYCLES);
    }

    fn update(&mut self, _cycles: u64, ctx: &mut CardCtx) {
        self.poll(ctx);
    }

    fn on_event(&mut self, token: u16, ctx: &mut CardCtx) -> i32 {
        match token {
            TOKEN_TX_DONE => {
                self.tx_empty = true;
                if self.tx_irq_enabled {
                    ctx.set_irq(IrqSource::Ssc, true); // GH#522
                    self.tx_irq_pending = true;
                }
                0
            }
            _ => {
                self.poll(ctx);
                POLL_CYCLES
            }
        }
    }

    fn save(&self) -> Vec<u8> {
        vec![self.command, self.control, self.tx_empty as u8]
    }

    fn load(&mut self, state: &[u8]) -> bool {
        let [command, control, tx_empty] = state else {
            return false;
        };
        self.update_command_reg(*command);
        self.update_control_reg(*control);
        self.tx_empty = *tx_empty != 0;
        true
    }
}

impl Drop for SuperSerialCard {
    fn drop(&mut self) {
        self.backend.close();
    }
}
