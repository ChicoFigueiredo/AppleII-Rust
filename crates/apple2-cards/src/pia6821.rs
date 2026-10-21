//! MC6821 PIA as AppleWin trimmed it from VICE (6821.cpp): registers plus host-driven input
//! lines. Writes report the output-port value to the owner instead of calling a listener.

/// Output-port values produced by a register write (`pra & ddra`, `prb & ddrb`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PiaOut {
    pub a: Option<u8>,
    pub b: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pia6821 {
    pub pra: u8,
    pub ddra: u8,
    pub cra: u8,
    pub prb: u8,
    pub ddrb: u8,
    pub crb: u8,
    /// Input lines driven by the peripheral (`m_byIA`, `m_byIB`).
    pub ia: u8,
    pub ib: u8,
}

impl Pia6821 {
    /// `mc6821_reset_internal` (6821.cpp:221): the input lines are untouched.
    pub fn reset(&mut self) {
        *self = Pia6821 {
            ia: self.ia,
            ib: self.ib,
            ..Default::default()
        };
    }

    /// `mc6821_read_internal` (6821.cpp:188), register select `rs & 3`.
    pub fn read(&mut self, rs: u8) -> u8 {
        match rs & 3 {
            0 if self.cra & 4 != 0 => {
                self.cra &= 0x3F; // clear IRQ1/IRQ2 flags (6821.cpp:75)
                (self.pra & self.ddra) | (self.ia & !self.ddra)
            }
            0 => self.ddra,
            1 => self.cra,
            2 if self.crb & 4 != 0 => {
                self.crb &= 0x3F;
                (self.prb & self.ddrb) | (self.ib & !self.ddrb)
            }
            2 => self.ddrb,
            _ => self.crb,
        }
    }

    /// `mc6821_store_internal` (6821.cpp:159).
    pub fn write(&mut self, rs: u8, byte: u8) -> PiaOut {
        let mut out = PiaOut::default();
        match rs & 3 {
            0 if self.cra & 4 != 0 => {
                self.pra = byte;
                if self.ddra != 0 {
                    out.a = Some(self.pra & self.ddra);
                }
            }
            0 => {
                if self.ddra != byte {
                    self.ddra = byte;
                    if self.ddra != 0 {
                        out.a = Some(self.pra & self.ddra);
                    }
                }
            }
            1 => self.cra = byte,
            2 if self.crb & 4 != 0 => {
                self.prb = byte;
                if self.ddrb != 0 {
                    out.b = Some(self.prb & self.ddrb);
                }
            }
            2 => {
                if self.ddrb != byte {
                    self.ddrb = byte;
                    if self.ddrb != 0 {
                        out.b = Some(self.prb & self.ddrb);
                    }
                }
            }
            _ => self.crb = byte,
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cr_bit2_selects_data_direction_or_port() {
        let mut p = Pia6821::default();
        assert_eq!(p.write(0, 0x0F).a, Some(0), "DDRA write drives pra & ddra");
        p.write(1, 4); // CRA: select the port register
        assert_eq!(p.write(0, 0xFF).a, Some(0x0F));
        p.ia = 0xA0;
        assert_eq!(
            p.read(0),
            0xAF,
            "output bits from pra, input bits from the line"
        );
        p.write(1, 0);
        assert_eq!(p.read(0), 0x0F, "CRA bit 2 clear reads DDRA");
    }

    #[test]
    fn port_read_clears_interrupt_flags() {
        let mut p = Pia6821 {
            cra: 0xC4,
            crb: 0xC4,
            ..Default::default()
        };
        p.read(0);
        p.read(2);
        assert_eq!((p.cra, p.crb), (0x04, 0x04));
    }

    #[test]
    fn rewriting_the_same_ddr_is_silent() {
        let mut p = Pia6821::default();
        assert!(p.write(2, 0x3E).b.is_some());
        assert_eq!(p.write(2, 0x3E).b, None);
    }
}
