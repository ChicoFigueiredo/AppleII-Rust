//! IRQ/NMI line aggregation: the line is active while any source is asserting it.

use serde::{Deserialize, Serialize};

/// Devices that can hold the IRQ (or NMI) line (AppleWin CPU.h `eIRQSRC`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrqSource {
    Mockingboard6522 = 1 << 0,
    Speech = 1 << 1,
    Ssc = 1 << 2,
    Mouse = 1 << 3,
    BreakpointCard = 1 << 4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interrupts {
    irq: u32,
    nmi: u32,
}

impl Interrupts {
    pub fn set_irq(&mut self, src: IrqSource, asserted: bool) {
        Self::set(&mut self.irq, src, asserted);
    }

    pub fn set_nmi(&mut self, src: IrqSource, asserted: bool) {
        Self::set(&mut self.nmi, src, asserted);
    }

    fn set(mask: &mut u32, src: IrqSource, on: bool) {
        if on {
            *mask |= src as u32;
        } else {
            *mask &= !(src as u32);
        }
    }

    pub fn irq_line(&self) -> bool {
        self.irq != 0
    }

    pub fn nmi_line(&self) -> bool {
        self.nmi != 0
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_is_union_of_sources() {
        let mut i = Interrupts::default();
        assert!(!i.irq_line());
        i.set_irq(IrqSource::Ssc, true);
        i.set_irq(IrqSource::Mouse, true);
        i.set_irq(IrqSource::Ssc, false);
        assert!(i.irq_line());
        i.set_irq(IrqSource::Mouse, false);
        assert!(!i.irq_line());
    }

    #[test]
    fn nmi_and_irq_are_independent() {
        let mut i = Interrupts::default();
        i.set_nmi(IrqSource::Speech, true);
        assert!(i.nmi_line() && !i.irq_line());
        i.clear();
        assert!(!i.nmi_line());
    }
}
