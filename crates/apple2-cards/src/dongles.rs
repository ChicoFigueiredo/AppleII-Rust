//! Copy-protection dongles on the game I/O connector (AppleWin CopyProtectionDongles.cpp). They
//! hang off the annunciators and answer on pushbutton 1/2 or paddle 3, so they plug into
//! the core's [`GameIoDevice`] hook (`Machine::set_game_io`).
//!
//! Supported (as in AppleWin): SDS SpeedStar datakey, Cortechs CodeWriter key, Robocom
//! Robo 500/1000/1500 interface modules and the Hayden Applesoft Compiler key.

use crate::state::{Dec, Enc};
use apple2_core::hooks::GameIoDevice;
use std::any::Any;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DongleKind {
    /// Software Dongle Systems' datakey for the SpeedStar Applesoft compiler.
    SdsSpeedStar,
    /// Dynatech/Cortechs "CodeWriter" key: a 7-bit LFSR on PB2.
    CodeWriter,
    Robocom500,
    Robocom1000,
    Robocom1500,
    /// Hayden Book Company "Applesoft Compiler" key.
    HaydenCompiler,
}

const CODEWRITER_INITIAL_LFSR: u8 = 0x6B;

pub struct Dongle {
    kind: DongleKind,
    lfsr: u8,
}

impl Dongle {
    pub fn new(kind: DongleKind) -> Self {
        Dongle {
            kind,
            lfsr: CODEWRITER_INITIAL_LFSR,
        }
    }

    pub fn kind(&self) -> DongleKind {
        self.kind
    }

    /// Sets CodeWriter's shift register (`codewriterLFSR` on load; 7 bits).
    pub fn set_lfsr(&mut self, lfsr: u8) {
        self.lfsr = lfsr & 0x7F;
    }

    /// CodeWriter's shift register (7 bits).
    pub fn lfsr(&self) -> u8 {
        self.lfsr
    }
}

impl GameIoDevice for Dongle {
    /// `DongleControl` (:52), called before the annunciator changes.
    fn annunciator(&mut self, n: usize, on: bool, old: &[bool; 4]) {
        if self.kind != DongleKind::CodeWriter {
            return;
        }
        if (n == 3 && on) || old[3] {
            self.lfsr = CODEWRITER_INITIAL_LFSR; // reset takes precedence over AN2
        } else if n == 2 && !on && old[2] {
            let bit = ((self.lfsr >> 1) ^ self.lfsr) & 1;
            self.lfsr = (self.lfsr >> 1) | (bit << 6);
        }
    }

    /// `CopyProtectionDonglePB1/PB2` (:98-124); PB0 is never used.
    fn button(&self, n: usize, ann: &[bool; 4]) -> Option<bool> {
        match (n, self.kind) {
            (1, DongleKind::HaydenCompiler) => Some(false), // connected to GND
            // A NAND of AN1 and AN2, powered by AN0.
            (2, DongleKind::SdsSpeedStar) => Some(!ann[0] || !(ann[1] && ann[2])),
            (2, DongleKind::CodeWriter) => Some(self.lfsr & 1 != 0),
            _ => None,
        }
    }

    /// `CopyProtectionDonglePDL` (:126).
    fn paddle(&self, n: usize, ann: &[bool; 4]) -> Option<u32> {
        if self.kind == DongleKind::HaydenCompiler && n == 3 {
            const HAYDEN: [u32; 4] = [0xFF, 0x96, 0x96, 0x50]; // derived from reverse-engineered code
            return Some(HAYDEN[usize::from(ann[2]) << 1 | usize::from(ann[0])]);
        }
        let robocom = matches!(
            self.kind,
            DongleKind::Robocom500 | DongleKind::Robocom1000 | DongleKind::Robocom1500
        );
        if !robocom || ann[3] || n != 3 {
            return None; // AN3 powers the module down
        }
        let mode = usize::from(ann[2]) << 2 | usize::from(ann[1]) << 1 | usize::from(ann[0]);
        Some(match self.kind {
            DongleKind::Robocom500 => {
                // PDL3 lower/upper bounds, GH#1247; the mean matches the other modules' values.
                const LO: [u32; 8] = [0x3F, 0x2E, 0x54, 0x54, 0x2E, 0x22, 0x72, 0x17];
                const HI: [u32; 8] = [0x6F, 0x54, 0x94, 0x94, 0x54, 0x40, 0xC4, 0x2E];
                (LO[mode] + HI[mode] - 1) / 2
            }
            DongleKind::Robocom1000 => [34, 151, 48, 64, 113, 113, 64, 85][mode],
            _ => [153, 34, 64, 34, 48, 86, 114, 48][mode],
        })
    }

    fn save(&self) -> Vec<u8> {
        let mut e = Enc::default();
        e.u8(self.lfsr);
        e.0
    }

    fn load(&mut self, state: &[u8]) -> bool {
        let mut d = Dec::new(state);
        match d.u8() {
            Some(l) if d.done() && l < 0x80 => {
                self.lfsr = l;
                true
            }
            _ => false,
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speedstar_is_a_nand_gated_by_an0() {
        let d = Dongle::new(DongleKind::SdsSpeedStar);
        assert_eq!(
            d.button(2, &[false, true, true, false]),
            Some(true),
            "unpowered: high"
        );
        assert_eq!(
            d.button(2, &[true, true, true, false]),
            Some(false),
            "AN1&AN2 pull it low"
        );
        assert_eq!(d.button(2, &[true, true, false, false]), Some(true));
        assert_eq!(d.button(1, &[true; 4]), None);
    }

    #[test]
    fn codewriter_lfsr_clocks_on_an2_falling_edge_and_resets_on_an3() {
        let mut d = Dongle::new(DongleKind::CodeWriter);
        assert_eq!(d.lfsr(), 0x6B);
        d.annunciator(2, false, &[false, false, true, false]); // AN2 true -> false
        assert_eq!(d.lfsr(), 0x35);
        d.annunciator(2, false, &[false, false, false, false]); // no edge
        let l = d.lfsr();
        d.annunciator(2, false, &[false, false, false, false]);
        assert_eq!(d.lfsr(), l);
        d.annunciator(3, true, &[false; 4]);
        assert_eq!(d.lfsr(), 0x6B);
        // While AN3 was already set, AN2 edges reset instead of clocking.
        d.annunciator(2, false, &[false, false, true, true]);
        assert_eq!(d.lfsr(), 0x6B);
    }

    #[test]
    fn hayden_ties_pb1_low_and_reports_pdl3_by_an0_an2() {
        let d = Dongle::new(DongleKind::HaydenCompiler);
        assert_eq!(d.button(1, &[false; 4]), Some(false));
        assert_eq!(d.paddle(3, &[false; 4]), Some(0xFF));
        assert_eq!(d.paddle(3, &[true, false, true, false]), Some(0x50));
        assert_eq!(d.paddle(2, &[false; 4]), None);
    }

    #[test]
    fn robocom_modules_answer_on_pdl3_only_when_powered() {
        let d = Dongle::new(DongleKind::Robocom1000);
        assert_eq!(d.paddle(3, &[false, false, false, false]), Some(34));
        assert_eq!(d.paddle(3, &[true, true, false, false]), Some(64));
        assert_eq!(
            d.paddle(3, &[false, false, false, true]),
            None,
            "AN3 cuts the power"
        );
        assert_eq!(d.paddle(0, &[false; 4]), None);
        let d = Dongle::new(DongleKind::Robocom500);
        assert_eq!(d.paddle(3, &[false; 4]), Some((0x3F + 0x6F - 1) / 2));
    }
}
