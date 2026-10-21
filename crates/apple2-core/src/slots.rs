//! The eight slots and the `$C800-$CFFF` expansion-ROM selection (Memory.cpp `IO_Cxxx`,
//! :946-1086).
//!
//! Language Card and Saturn 128K cards are not stored here: they share state (the memory region
//! and WRITERAM arbitration), so [`LanguageCards`](crate::language_card::LanguageCards) keeps
//! managing them and the board asks it first. Every other kind lives here as a `Box<dyn Card>`.

use crate::card::{Card, CardOp, EXPANSION_ROM_SIZE};
use serde::{Deserialize, Serialize};

/// What currently drives `$C800-$CFFF` (AppleWin `g_eExpansionRomType` +
/// `g_uPeripheralRomSlot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpansionRom {
    /// Nothing: reads return the floating bus.
    Null,
    Internal,
    Slot(u8),
}

/// Serializable slot state: the cards' own bytes ([`Card::save`]) plus the ROM selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotsState {
    pub cards: Vec<Option<Vec<u8>>>,
    pub io_select: u8,
    pub intc8rom: bool,
    pub expansion: ExpansionRom,
}

pub struct Slots {
    pub(crate) cards: [Option<Box<dyn Card>>; 8],
    /// Scheduler requests queued by cards during a callback.
    pub(crate) ops: Vec<CardOp>,
    /// `IO_SELECT`: slots whose `$Cs00` ROM was touched since the last `$CFFF` (sticky).
    io_select: u8,
    /// `INTC8ROM`: internal ROM keeps `$C800-$CFFF` until `$CFFF` (UTAIIe:5-28).
    intc8rom: bool,
    expansion: ExpansionRom,
}

impl Default for Slots {
    fn default() -> Self {
        Slots {
            cards: Default::default(),
            ops: Vec::new(),
            io_select: 0,
            intc8rom: false,
            expansion: ExpansionRom::Null,
        }
    }
}

impl Slots {
    /// Puts `card` in `slot` (0-7); false if the slot is out of range or taken.
    pub fn insert(&mut self, slot: u8, card: Box<dyn Card>) -> bool {
        match self.cards.get_mut(slot as usize) {
            Some(s @ None) => {
                *s = Some(card);
                true
            }
            _ => false,
        }
    }

    pub fn card(&self, slot: u8) -> Option<&dyn Card> {
        self.cards.get(slot as usize)?.as_deref()
    }

    pub fn intc8rom(&self) -> bool {
        self.intc8rom
    }

    pub fn io_select(&self) -> u8 {
        self.io_select
    }

    pub fn expansion(&self) -> ExpansionRom {
        self.expansion
    }

    fn has_expansion(&self, slot: u8) -> bool {
        self.card(slot).is_some_and(|c| c.expansion_rom().is_some())
    }

    /// `MemReset`'s "init the I/O ROM vars" (Memory.cpp:2305). Ctrl-Reset keeps them.
    pub fn power_cycle(&mut self) {
        self.io_select = 0;
        self.intc8rom = false;
        self.expansion = ExpansionRom::Null;
    }

    /// `MemSetPaging` after INTCXROM/SLOTC3ROM changed (Memory.cpp:1838-1880). `intcxrom` is the
    /// new value.
    pub fn cx_switches_changed(&mut self, intcxrom: bool) {
        if !intcxrom {
            if !self.intc8rom {
                self.expansion = ExpansionRom::Null; // GH#423
            }
        } else {
            self.expansion = ExpansionRom::Internal;
        }
    }

    /// The state update of `IO_Cxxx` for an access to `addr` in `$C100-$CFFF`, read or write.
    /// `is_e`: `IsAppleIIeOrAbove` (the ][ family has no INTCXROM/INTC8ROM).
    pub fn select(&mut self, addr: u16, is_e: bool, intcxrom: bool, slotc3rom: bool) {
        if addr == 0xCFFF {
            // Release $C800-$CFFF.
            self.io_select = 0;
            self.intc8rom = false;
            if !intcxrom {
                self.expansion = ExpansionRom::Null;
            }
        }
        let mut strobe = false;
        if !is_e || !intcxrom {
            if addr < 0xC800 {
                let slot = (addr >> 8) as u8 & 7;
                if !is_e || slot != 3 {
                    if self.has_expansion(slot) {
                        self.io_select |= 1 << slot;
                    }
                } else if slotc3rom && self.has_expansion(slot) {
                    self.io_select |= 1 << slot;
                } else if !slotc3rom {
                    self.intc8rom = true; // slot 3 with internal ROM
                }
            } else if !self.intc8rom {
                strobe = true; // GH#423, UTAIIe:5-28
            }
            if self.io_select != 0 && strobe {
                let slot = self.io_select.trailing_zeros() as u8;
                if self.has_expansion(slot) {
                    self.expansion = ExpansionRom::Slot(slot);
                }
            } else if self.intc8rom {
                self.expansion = ExpansionRom::Internal;
            }
        }
        if is_e && intcxrom {
            if (0xC300..0xC400).contains(&addr) && !slotc3rom {
                self.intc8rom = true;
            }
            if self.intc8rom {
                self.expansion = ExpansionRom::Internal;
            }
        }
    }

    pub fn save(&self) -> SlotsState {
        SlotsState {
            cards: self
                .cards
                .iter()
                .map(|c| c.as_ref().map(|c| c.save()))
                .collect(),
            io_select: self.io_select,
            intc8rom: self.intc8rom,
            expansion: self.expansion,
        }
    }

    /// Restores a state taken from a machine with the same cards inserted.
    pub fn load(&mut self, s: &SlotsState) -> bool {
        let fits = s.cards.len() == 8
            && self
                .cards
                .iter()
                .zip(&s.cards)
                .all(|(c, st)| c.is_some() == st.is_some());
        if !fits {
            return false;
        }
        for (c, st) in self.cards.iter_mut().zip(&s.cards) {
            if let (Some(c), Some(st)) = (c, st) {
                if !c.load(st) {
                    return false;
                }
            }
        }
        (self.io_select, self.intc8rom, self.expansion) = (s.io_select, s.intc8rom, s.expansion);
        true
    }
}

/// Offset inside an expansion ROM for an address in `$C800-$CFFF`.
pub(crate) fn expansion_offset(addr: u16) -> usize {
    (addr as usize - 0xC800) & (EXPANSION_ROM_SIZE - 1)
}
