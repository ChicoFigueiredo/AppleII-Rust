//! AppleMouse II interface card (AppleWin MouseInterface.cpp, based on Apple in PC's mousecard).
//!
//! The firmware talks to a MC6821 PIA whose port B clocks bytes in and out of the "mouse chip"
//! (a 6805 in the real card): commands arrive in `buff`, the chip answers through port A. The
//! card also pages its 2K firmware into `$Cs00` through three bits of port B. Host input comes in
//! through [`MouseCard::move_by`] / [`MouseCard::set_button`] (see [`mouse_move`]).

use crate::pia6821::{Pia6821, PiaOut};
use crate::state::{Dec, Enc};
use apple2_core::card::SLOT_ROM_SIZE;
use apple2_core::{Card, CardCtx, IrqSource, Machine};
use std::any::Any;

// Commands (MouseInterface.cpp:62-81).
const MOUSE_SET: u8 = 0x00;
const MOUSE_READ: u8 = 0x10;
const MOUSE_SERV: u8 = 0x20;
const MOUSE_CLEAR: u8 = 0x30;
const MOUSE_POS: u8 = 0x40;
const MOUSE_INIT: u8 = 0x50;
const MOUSE_CLAMP: u8 = 0x60;
const MOUSE_HOME: u8 = 0x70;
const MOUSE_TIME: u8 = 0x90;

// Interrupt status byte (:95-102).
pub const STAT_PREV_BUTTON1: u8 = 1 << 0;
pub const STAT_INT_MOVEMENT: u8 = 1 << 1;
pub const STAT_INT_BUTTON: u8 = 1 << 2;
pub const STAT_INT_VBL: u8 = 1 << 3;
pub const STAT_CURR_BUTTON1: u8 = 1 << 4;
pub const STAT_MOVEMENT_SINCE_READMOUSE: u8 = 1 << 5;
pub const STAT_PREV_BUTTON0: u8 = 1 << 6;
pub const STAT_CURR_BUTTON0: u8 = 1 << 7;
pub const STAT_INT_ALL: u8 = STAT_INT_VBL | STAT_INT_BUTTON | STAT_INT_MOVEMENT;

// Mode byte (:109-112).
pub const MODE_MOUSE_ON: u8 = 1 << 0;
pub const MODE_INT_MOVEMENT: u8 = 1 << 1;
pub const MODE_INT_BUTTON: u8 = 1 << 2;
pub const MODE_INT_VBL: u8 = 1 << 3;

const VBL_TOKEN: u16 = 0;
const FIRMWARE_SIZE: usize = 2048;

/// Everything `CMouseInterface::SaveSnapshot` keeps (MouseInterface.cpp:636), as the card holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MouseSnapshot {
    pub pia: Pia6821,
    /// `m_by6821A`, `m_by6821B`.
    pub by_a: u8,
    pub by_b: u8,
    /// `m_byBuff`.
    pub buff: [u8; 8],
    pub buf_pos: usize,
    pub data_len: usize,
    pub mode: u8,
    pub state: u8,
    /// `m_nX`, `m_nY`: position reported to the firmware.
    pub n_x: i32,
    pub n_y: i32,
    /// `m_iX`, `m_iMinX`, `m_iMaxX`, and the Y trio.
    pub i_x: i32,
    pub min_x: i32,
    pub max_x: i32,
    pub i_y: i32,
    pub min_y: i32,
    pub max_y: i32,
    /// `m_bBtn0`, `m_bBtn1`: buttons as of the last READMOUSE.
    pub btn0: bool,
    pub btn1: bool,
    /// `m_bButtons`: live host buttons.
    pub buttons: [bool; 2],
}

pub struct MouseCard {
    pia: Pia6821,
    firmware: [u8; FIRMWARE_SIZE],
    /// `m_by6821A`/`m_by6821B`: last values latched from the PIA output ports.
    by_a: u8,
    by_b: u8,
    buff: [u8; 8],
    buf_pos: usize,
    data_len: usize,
    mode: u8,
    state: u8,
    // Position reported to the firmware (`m_nX`) vs live host position (`m_iX`).
    n_x: i32,
    n_y: i32,
    i_x: i32,
    i_y: i32,
    clamp_x: (i32, i32),
    clamp_y: (i32, i32),
    btn0: bool,
    btn1: bool,
    buttons: [bool; 2],
}

impl Default for MouseCard {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseCard {
    pub fn new() -> Self {
        let mut firmware = [0u8; FIRMWARE_SIZE];
        firmware.copy_from_slice(&apple2_roms::MOUSEINTERFACE[..FIRMWARE_SIZE]);
        let mut c = MouseCard {
            pia: Pia6821::default(),
            firmware,
            by_a: 0,
            by_b: 0,
            buff: [0; 8],
            buf_pos: 0,
            data_len: 1,
            mode: 0,
            state: 0,
            n_x: 0,
            n_y: 0,
            i_x: 0,
            i_y: 0,
            clamp_x: (0, 1023),
            clamp_y: (0, 1023),
            btn0: false,
            btn1: false,
            buttons: [false; 2],
        };
        c.reset_state();
        c
    }

    /// `CMouseInterface::Reset` (:197) without the scheduler part.
    fn reset_state(&mut self) {
        self.pia.reset();
        self.by_a = 0;
        self.by_b = 0x40; // PB6
        self.pia.ib = self.by_b;
        self.mode = 0;
        self.n_x = 0;
        self.n_y = 0;
        self.i_x = 0;
        self.clamp_x = (0, 1023);
        self.i_y = 0;
        self.clamp_y = (0, 1023);
        self.buttons = [false; 2];
        self.clear();
        self.buff = [0; 8];
    }

    pub fn snapshot(&self) -> MouseSnapshot {
        MouseSnapshot {
            pia: self.pia,
            by_a: self.by_a,
            by_b: self.by_b,
            buff: self.buff,
            buf_pos: self.buf_pos,
            data_len: self.data_len,
            mode: self.mode,
            state: self.state,
            n_x: self.n_x,
            n_y: self.n_y,
            i_x: self.i_x,
            min_x: self.clamp_x.0,
            max_x: self.clamp_x.1,
            i_y: self.i_y,
            min_y: self.clamp_y.0,
            max_y: self.clamp_y.1,
            btn0: self.btn0,
            btn1: self.btn1,
            buttons: self.buttons,
        }
    }

    /// `CMouseInterface::LoadSnapshot` (MouseInterface.cpp:722). `buf_pos` and `data_len` are
    /// kept inside the buffer, as [`Card::load`] does.
    pub fn apply_snapshot(&mut self, s: &MouseSnapshot) {
        self.pia = s.pia;
        (self.by_a, self.by_b, self.buff) = (s.by_a, s.by_b, s.buff);
        self.buf_pos = s.buf_pos.min(7);
        self.data_len = s.data_len.clamp(1, 8);
        (self.mode, self.state) = (s.mode, s.state);
        (self.n_x, self.n_y, self.i_x, self.i_y) = (s.n_x, s.n_y, s.i_x, s.i_y);
        self.clamp_x = (s.min_x, s.max_x);
        self.clamp_y = (s.min_y, s.max_y);
        (self.btn0, self.btn1, self.buttons) = (s.btn0, s.btn1, s.buttons);
    }

    /// Current position as the host sees it (after clamping).
    pub fn position(&self) -> (i32, i32) {
        (self.i_x, self.i_y)
    }

    pub fn clamp(&self) -> ((i32, i32), (i32, i32)) {
        (self.clamp_x, self.clamp_y)
    }

    /// Mode byte set by the firmware's SETMOUSE.
    pub fn mode(&self) -> u8 {
        self.mode
    }

    /// Interrupt status byte (`m_byState`).
    pub fn status(&self) -> u8 {
        self.state
    }

    /// Host mouse delta (`SetPositionRel`, :587); returns whether each axis hit its clamp
    /// (-1/0/1). Raises a movement interrupt when the mode asks for one.
    pub fn move_by(&mut self, dx: i32, dy: i32, ctx: &mut CardCtx) -> (i32, i32) {
        self.i_x += dx;
        let ox = clamp(&mut self.i_x, self.clamp_x);
        self.i_y += dy;
        let oy = clamp(&mut self.i_y, self.clamp_y);
        self.on_mouse_event(false, ctx);
        (ox, oy)
    }

    /// Host button 0 or 1 (`SetButton`, :598).
    pub fn set_button(&mut self, button: usize, down: bool, ctx: &mut CardCtx) {
        self.buttons[button & 1] = down;
        self.on_mouse_event(false, ctx);
    }

    fn set_position_abs(&mut self, x: i32, y: i32) {
        (self.i_x, self.i_y) = (x, y);
    }

    fn clear(&mut self) {
        self.buf_pos = 0;
        self.data_len = 1;
        self.state = 0;
        self.n_x = 0;
        self.n_y = 0;
        self.btn0 = false;
        self.btn1 = false;
        self.set_position_abs(0, 0);
    }

    fn set_clamp(range: &mut (i32, i32), min: i32, max: i32) {
        if !(0..=0xFFFF).contains(&min) || !(0..=0xFFFF).contains(&max) {
            return;
        }
        *range = if min > max { (0, (min + max) & 0xFFFF) } else { (min, max) }; // Blazing Paddles
    }

    /// `OnMouseEvent` (:451).
    fn on_mouse_event(&mut self, vbl: bool, ctx: &mut CardCtx) {
        let mut st = 0u8;
        if self.mode & MODE_INT_VBL != 0 && vbl {
            st |= STAT_INT_VBL;
        }
        if self.mode & MODE_MOUSE_ON != 0 {
            if self.n_x != self.i_x || self.n_y != self.i_y {
                st |= STAT_INT_MOVEMENT | STAT_MOVEMENT_SINCE_READMOUSE;
                self.state |= STAT_MOVEMENT_SINCE_READMOUSE; // CopyII+ 9.1, ProTERM 3.1
            }
            if self.btn0 != self.buttons[0] || self.btn1 != self.buttons[1] {
                st |= STAT_INT_BUTTON;
            }
            st &= (self.mode & STAT_INT_ALL) | STAT_MOVEMENT_SINCE_READMOUSE;
        } else {
            st &= STAT_INT_VBL; // GH#1138
        }
        if st & STAT_INT_ALL != 0 {
            self.state |= st;
            ctx.set_irq(IrqSource::Mouse, true);
        }
    }

    /// Firmware offset of the page mapped at `$Cs00` (`SetSlotRom`, :238).
    fn rom_page(&self) -> usize {
        ((self.by_b as usize) << 7) & 0x700
    }

    /// `On6821_B` (:277): port B drives the chip handshake and the ROM page bits.
    fn on_6821_b(&mut self, data: u8, ctx: &mut CardCtx) {
        let diff = (self.by_b ^ data) & 0x3E;
        if diff == 0 {
            return;
        }
        self.by_b = (self.by_b & !0x3E) | (data & 0x3E);
        if diff & 0x20 != 0 {
            // Write to the chip.
            if data & 0x20 != 0 {
                self.by_b |= 0x80; // ready to read from the 6821
            } else {
                // Clock activate for read.
                self.buff[self.buf_pos] = self.by_a;
                self.buf_pos += 1;
                if self.buf_pos == 1 {
                    self.on_command(ctx);
                }
                if self.buf_pos == self.data_len || self.buf_pos > 7 {
                    self.on_write();
                    self.buf_pos = 0;
                }
                self.by_b &= !0x80;
                self.pia.ib = self.by_b;
            }
        }
        if diff & 0x10 != 0 {
            // Read from the chip.
            if data & 0x10 != 0 {
                self.by_b &= !0x40; // preparing the next value
            } else {
                if self.buf_pos != 0 {
                    self.buf_pos += 1;
                }
                if self.buf_pos == self.data_len || self.buf_pos > 7 {
                    self.buf_pos = 0;
                } else {
                    self.pia.ia = self.buff[self.buf_pos];
                }
                self.by_b |= 0x40;
            }
        }
        self.pia.ib = self.by_b;
    }

    /// `OnCommand` (:327).
    fn on_command(&mut self, ctx: &mut CardCtx) {
        match self.buff[0] & 0xF0 {
            MOUSE_SET => {
                self.data_len = 1;
                self.mode = self.buff[0] & 0x0F;
            }
            MOUSE_READ => {
                self.data_len = 6;
                self.state &= STAT_MOVEMENT_SINCE_READMOUSE;
                self.n_x = self.i_x;
                self.n_y = self.i_y;
                if self.btn0 {
                    self.state |= STAT_PREV_BUTTON0;
                }
                if self.btn1 {
                    self.state |= STAT_PREV_BUTTON1;
                }
                self.btn0 = self.buttons[0];
                self.btn1 = self.buttons[1];
                if self.btn0 {
                    self.state |= STAT_CURR_BUTTON0;
                }
                if self.btn1 {
                    self.state |= STAT_CURR_BUTTON1;
                }
                self.buff[1] = self.n_x as u8;
                self.buff[2] = (self.n_x >> 8) as u8;
                self.buff[3] = self.n_y as u8;
                self.buff[4] = (self.n_y >> 8) as u8;
                self.buff[5] = self.state;
                self.state &= !STAT_MOVEMENT_SINCE_READMOUSE;
            }
            MOUSE_SERV => {
                self.data_len = 2;
                self.buff[1] = self.state & !STAT_MOVEMENT_SINCE_READMOUSE;
                ctx.set_irq(IrqSource::Mouse, false);
            }
            MOUSE_CLEAR => {
                self.clear(); // NB. clamps are kept (Fantavision)
                self.data_len = 1;
            }
            MOUSE_POS | MOUSE_CLAMP => self.data_len = 5,
            MOUSE_INIT => {
                self.data_len = 3;
                self.buff[1] = 0xFF;
            }
            MOUSE_HOME => {
                self.data_len = 1;
                self.set_position_abs(0, 0);
            }
            MOUSE_TIME => {
                self.data_len = match self.buff[0] & 0x0C {
                    0x00 => 1,
                    0x04 => 3,
                    0x08 => 2,
                    _ => 4,
                }
            }
            0xA0 => self.data_len = 2,
            _ => self.data_len = 1,
        }
        self.pia.ia = self.buff[1];
    }

    /// `OnWrite` (:420): the command's data bytes are complete.
    fn on_write(&mut self) {
        match self.buff[0] & 0xF0 {
            MOUSE_CLAMP => {
                let min = (i32::from(self.buff[3]) << 8) | i32::from(self.buff[1]);
                let max = (i32::from(self.buff[4]) << 8) | i32::from(self.buff[2]);
                if self.buff[0] & 1 != 0 {
                    Self::set_clamp(&mut self.clamp_y, min, max);
                    clamp(&mut self.i_y, self.clamp_y);
                } else {
                    Self::set_clamp(&mut self.clamp_x, min, max);
                    clamp(&mut self.i_x, self.clamp_x);
                }
            }
            MOUSE_POS => {
                self.n_x = (i32::from(self.buff[2]) << 8) | i32::from(self.buff[1]);
                self.n_y = (i32::from(self.buff[4]) << 8) | i32::from(self.buff[3]);
                self.set_position_abs(self.n_x, self.n_y);
            }
            MOUSE_INIT => {
                self.n_x = 0;
                self.n_y = 0;
                self.clamp_x = (0, 1023);
                self.clamp_y = (0, 1023);
                self.set_position_abs(0, 0);
            }
            _ => {}
        }
    }

    fn apply(&mut self, out: PiaOut, ctx: &mut CardCtx) {
        if let Some(a) = out.a {
            self.by_a = a;
        }
        if let Some(b) = out.b {
            self.on_6821_b(b, ctx);
        }
    }
}

fn clamp(v: &mut i32, (min, max): (i32, i32)) -> i32 {
    if *v > max {
        *v = max;
        1
    } else if *v < min {
        *v = min;
        -1
    } else {
        0
    }
}

impl Card for MouseCard {
    fn name(&self) -> &'static str {
        "Mouse Card"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn io_read(&mut self, lo: u8, _ctx: &mut CardCtx) -> u8 {
        self.pia.read(lo & 3)
    }

    fn io_write(&mut self, lo: u8, value: u8, ctx: &mut CardCtx) {
        let out = self.pia.write(lo & 3, value);
        self.apply(out, ctx);
    }

    fn slot_rom_read(&mut self, offset: u8, _ctx: &mut CardCtx) -> u8 {
        self.firmware[self.rom_page() + offset as usize]
    }

    fn reset(&mut self, _power_cycle: bool, ctx: &mut CardCtx) {
        self.reset_state();
        // The VBL event stays armed across Ctrl-Reset (:226); re-arming gives the same time.
        ctx.schedule(VBL_TOKEN, ctx.cycles_until_vblank() as i32);
    }

    fn on_event(&mut self, _token: u16, ctx: &mut CardCtx) -> i32 {
        self.on_mouse_event(true, ctx);
        ctx.cycles_until_vblank() as i32
    }

    fn save(&self) -> Vec<u8> {
        let mut e = Enc::default();
        let p = &self.pia;
        e.bytes(&[p.pra, p.ddra, p.cra, p.prb, p.ddrb, p.crb, p.ia, p.ib, self.by_a, self.by_b]);
        e.bytes(&self.buff).u8(self.buf_pos as u8).u8(self.data_len as u8).u8(self.mode).u8(self.state);
        for v in [self.n_x, self.n_y, self.i_x, self.i_y, self.clamp_x.0, self.clamp_x.1, self.clamp_y.0, self.clamp_y.1] {
            e.i32(v);
        }
        e.bool(self.btn0).bool(self.btn1).bool(self.buttons[0]).bool(self.buttons[1]);
        e.0
    }

    fn load(&mut self, state: &[u8]) -> bool {
        let mut d = Dec::new(state);
        let r = (|| {
            let mut regs = [0u8; 10];
            d.bytes(&mut regs)?;
            self.pia = Pia6821 { pra: regs[0], ddra: regs[1], cra: regs[2], prb: regs[3], ddrb: regs[4], crb: regs[5], ia: regs[6], ib: regs[7] };
            (self.by_a, self.by_b) = (regs[8], regs[9]);
            d.bytes(&mut self.buff)?;
            self.buf_pos = usize::from(d.u8()?).min(7);
            self.data_len = usize::from(d.u8()?).clamp(1, 8);
            self.mode = d.u8()?;
            self.state = d.u8()?;
            (self.n_x, self.n_y, self.i_x, self.i_y) = (d.i32()?, d.i32()?, d.i32()?, d.i32()?);
            self.clamp_x = (d.i32()?, d.i32()?);
            self.clamp_y = (d.i32()?, d.i32()?);
            (self.btn0, self.btn1) = (d.bool()?, d.bool()?);
            self.buttons = [d.bool()?, d.bool()?];
            Some(())
        })();
        r.is_some() && d.done()
    }
}

const _: () = assert!(SLOT_ROM_SIZE == 0x100);

/// Feeds a host mouse delta to the mouse card in `slot`; false if there is none.
pub fn mouse_move(m: &mut Machine, slot: u8, dx: i32, dy: i32) -> bool {
    m.with_card(slot, |c, ctx| c.as_any_mut().and_then(|a| a.downcast_mut::<MouseCard>()).map(|mc| mc.move_by(dx, dy, ctx))).flatten().is_some()
}

/// Feeds a host button change (0 or 1) to the mouse card in `slot`; false if there is none.
pub fn mouse_button(m: &mut Machine, slot: u8, button: usize, down: bool) -> bool {
    m.with_card(slot, |c, ctx| c.as_any_mut().and_then(|a| a.downcast_mut::<MouseCard>()).map(|mc| mc.set_button(button, down, ctx))).flatten().is_some()
}
