//! Game port: pushbuttons, paddle timers and the Open/Closed Apple keys (AppleWin Joystick.cpp).
//!
//! Host-side concerns (mapping a physical joystick, mouse or keys to buttons/paddles, trim,
//! "square the circle", autofire, button swap, Joyport, copy-protection dongles) stay out of the
//! core: the frontend calls `set_paddle`/`set_button` with final values.

use serde::{Deserialize, Serialize};

/// `PDL_CNTR_INTERVAL` (Joystick.cpp:686): cycles per paddle count.
const PDL_CNTR_INTERVAL: f64 = 2816.0 / 255.0;
/// Paddle value that reads as `287` counts (from KEGS, Joystick.cpp:768).
const PDL_FULL: u8 = 255;
const PDL_FULL_COUNTS: f64 = 287.0;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamePort {
    /// Joystick 0 (paddles 0-1, PB0/PB1) and joystick 1 (paddles 2-3, PB2) present; AppleWin's
    /// default is `{ J0C_JOYSTICK1, J1C_DISABLED }` (Joystick.h:11).
    pub attached: [bool; 2],
    pub buttons: [bool; 3],
    pub paddles: [u8; 4],
    pub open_apple: bool,
    pub closed_apple: bool,
    /// `g_paddleInactiveCycle + 1`: each timer reads active while the cycle is below it (stored
    /// exclusive so that a strobe at cycle 0 works).
    inactive_cycle: [u64; 4],
}

impl Default for GamePort {
    /// Joystick 0 only, paddles centred (`PDL_CENTRAL`, Joystick.cpp:77).
    fn default() -> Self {
        GamePort {
            attached: [true, false],
            buttons: [false; 3],
            paddles: [127; 4],
            open_apple: false,
            closed_apple: false,
            inactive_cycle: [0; 4],
        }
    }
}

impl GamePort {
    /// `JoyReadButton` bit 7 for $C061-$C063 (`n` = 0..=2). `ii_family`: no //e Apple keys, and
    /// PB0/PB1 float high with no joystick (GH#1234).
    pub fn button(&self, n: usize, ii_family: bool) -> bool {
        match n {
            0 | 1 => {
                let apple = if n == 0 { self.open_apple } else { self.closed_apple };
                (ii_family && !self.attached[0]) || self.buttons[n] || apple
            }
            _ => !self.attached[1] || self.buttons[2], // PB2 floats high with no joystick 1
        }
    }

    /// `JoyReadPosition` bit 7 for $C064-$C067 at absolute cycle `now`.
    pub fn paddle_active(&self, n: usize, now: u64) -> bool {
        self.paddle_active_with(n, now, false)
    }

    /// As [`GamePort::paddle_active`]; `forced`: a dongle drives this paddle, so it times out even
    /// with no joystick (Joystick.cpp:696).
    pub fn paddle_active_with(&self, n: usize, now: u64, forced: bool) -> bool {
        (!forced && !self.attached[(n & 2) >> 1]) || now < self.inactive_cycle[n & 3]
    }

    /// AppleWin's `g_paddleInactiveCycle[n]` (the last cycle the timer reads active), for
    /// save-states.
    pub fn inactive_cycles(&self) -> [u64; 4] {
        self.inactive_cycle.map(|c| c.saturating_sub(1))
    }

    /// Restores [`GamePort::inactive_cycles`].
    pub fn set_inactive_cycles(&mut self, cycles: [u64; 4]) {
        self.inactive_cycle = cycles.map(|c| c.saturating_add(1));
    }

    /// `JoyResetPosition` ($C070-$C07F access): arms every timer that is not running.
    pub fn trigger(&mut self, now: u64) {
        self.trigger_with(now, [None; 4]);
    }

    /// [`GamePort::trigger`] where a game-connector device may force a paddle's count
    /// (Joystick.cpp:777-783; it applies even to a running timer).
    pub fn trigger_with(&mut self, now: u64, forced: [Option<u32>; 4]) {
        for (n, end) in self.inactive_cycle.iter_mut().enumerate() {
            if let Some(p) = forced[n] {
                *end = now + (p as f64 * PDL_CNTR_INTERVAL) as u64 + 1;
                continue;
            }
            if now < *end {
                continue; // a running timer ignores the strobe (GH#985)
            }
            let counts = match self.paddles[n] {
                PDL_FULL => PDL_FULL_COUNTS,
                p => p as f64,
            };
            *end = now + (counts * PDL_CNTR_INTERVAL) as u64 + 1;
        }
    }

    /// Power-on (`JoyReset` plus the cycle counter restarting).
    pub fn reset(&mut self) {
        self.open_apple = false;
        self.closed_apple = false;
        self.inactive_cycle = [0; 4];
    }
}
