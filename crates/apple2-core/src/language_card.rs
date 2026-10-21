//! Language Card state machines (AppleWin LanguageCard.cpp): the //e built-in card, the ][/][+
//! 16K card in slot 0 and the Saturn 128K in any slot.
//!
//! Each card keeps its own `BANK2|HIGHRAM|WRITERAM` flags and "last access was an odd read"
//! latch. [`LanguageCards`] owns all of them (AppleWin's `LanguageCardManager`) and merges the
//! flags of the card that can write into the global memmode. The 16K RAM behind $D000-$FFFF is a
//! "region" of [`Memory`]: region 0 for the built-in / slot-0 card, then the Saturn banks.
//!
//! [`LanguageCards::io`] has the shape of a slot card's $C080+s*16 handler; the board tries it before the boxed cards (see `slots`).

use crate::memory::{mf, Memory};
use crate::CardKind;
use serde::{Deserialize, Serialize};

/// Banks of a full Saturn 128K (`kMaxSaturnBanks`).
const SATURN_BANKS: u8 = 8;

/// Public view of a card's kind (see [`LanguageCards::infos`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LcKind {
    Builtin,
    Card16K,
    Saturn { banks: u8, active: u8 },
}

/// Public view of one Language Card / Saturn (see [`LanguageCards::infos`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LcInfo {
    pub slot: u8,
    pub kind: LcKind,
    /// `BANK2|HIGHRAM|WRITERAM` bits.
    pub memmode: u32,
    pub last_ram_write: bool,
    /// [`Memory`] LC region this card currently exposes (the active Saturn bank's).
    pub region: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Kind {
    /// //e built-in card (`CT_LanguageCardIIe`).
    Builtin,
    /// ][/][+ 16K card, slot 0 only (`CT_LanguageCard`).
    Card16K,
    Saturn {
        banks: u8,
        active: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Card {
    slot: u8,
    kind: Kind,
    /// Only the `LANGCARD_MASK` bits are used.
    memmode: u32,
    /// `m_bLastRamWrite`.
    last_ram_write: bool,
    /// First `Memory` LC region owned by a Saturn.
    region_base: usize,
}

impl Card {
    fn region(&self) -> usize {
        match self.kind {
            Kind::Saturn { active, .. } => self.region_base + active as usize,
            _ => 0,
        }
    }

    /// `LanguageCardUnit::Reset` (LanguageCard.cpp:50): the ][/][+ card only on power-cycle.
    fn reset(&mut self, power_cycle: bool, is_e: bool) {
        if power_cycle || is_e {
            self.memmode = mf::INITIAL;
            if matches!(self.kind, Kind::Saturn { .. }) {
                self.memmode &= !mf::WRITERAM; // "Hack: force Saturn to be write-protected"
            }
            self.last_ram_write = false;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageCards {
    /// Ascending slot order.
    cards: Vec<Card>,
    /// `m_lastSlotToSetMainMemLC`.
    last_slot: u8,
}

/// `LanguageCardUnit::IsOpcodeRMWabs` (LanguageCard.cpp:179, GH#404/GH#700): the instruction at
/// the current opcode, with absolute operand `operand`, is a read-modify-write that touches
/// `addr`. AppleWin also tests X==0 for `INC abs,X`; `operand == addr` implies it.
pub fn is_rmw_abs(opcode: u8, operand: u16, addr: u16, cmos: bool) -> bool {
    if operand != addr {
        return false;
    }
    match opcode {
        0xFE => true, // INC abs,X (any CPU)
        // INC/DEC/ROR/LSR/ROL/ASL abs, TRB/TSB abs: 65C02 only
        0xEE | 0xCE | 0x6E | 0x4E | 0x2E | 0x0E | 0x1C | 0x0C => cmos,
        _ => false,
    }
}

impl LanguageCards {
    /// Cards for a configuration (CardManager/Memory.cpp:320-400): the //e family always has the
    /// built-in card in slot 0; the ][ family takes `CardKind::LanguageCard16K` or a Saturn in
    /// slot 0; a Saturn may sit in slots 1-7. A 16K card anywhere but slot 0 is ignored.
    pub fn new(is_e: bool, slots: &[Option<CardKind>; 8]) -> Self {
        let mut cards = Vec::new();
        let mut next_region = 1;
        for (s, kind) in slots.iter().enumerate() {
            let kind = match (s, kind, is_e) {
                (0, _, true) => Some(Kind::Builtin), // "No slot0 for //e": a configured card is ignored
                (0, Some(CardKind::LanguageCard16K), false) => Some(Kind::Card16K),
                (_, Some(CardKind::Saturn128K), _) => Some(Kind::Saturn {
                    banks: SATURN_BANKS,
                    active: 0,
                }),
                _ => None,
            };
            if let Some(kind) = kind {
                let mut card = Card {
                    slot: s as u8,
                    kind,
                    memmode: 0,
                    last_ram_write: false,
                    region_base: next_region,
                };
                if matches!(kind, Kind::Saturn { .. }) {
                    next_region += SATURN_BANKS as usize;
                }
                card.reset(true, is_e);
                cards.push(card);
            }
        }
        LanguageCards {
            cards,
            last_slot: 0,
        }
    }

    /// 16K RAM regions the memory must provide.
    pub fn regions(&self) -> usize {
        self.cards
            .iter()
            .map(|c| {
                c.region_base
                    + if matches!(c.kind, Kind::Saturn { .. }) {
                        SATURN_BANKS as usize
                    } else {
                        1
                    }
            })
            .max()
            .unwrap_or(1)
    }

    /// Makes the memory use the region the first card (the one in the lowest slot) owns.
    pub fn attach(&mut self, mem: &mut Memory) {
        mem.alloc_lc_regions(self.regions());
        self.last_slot = 0;
        let region = self
            .cards
            .first()
            .filter(|c| c.slot == 0)
            .map_or(0, Card::region);
        mem.set_lc_region(region);
    }

    /// Ctrl-Reset; `is_e` is `IsAppleIIeOrAbove`. The memory resets the global flags itself.
    pub fn reset(&mut self, is_e: bool) {
        for c in &mut self.cards {
            c.reset(false, is_e);
        }
    }

    /// Power off and on: all cards back to their initial state (the memory was already cleared).
    pub fn power_cycle(&mut self, is_e: bool, mem: &mut Memory) {
        for c in &mut self.cards {
            c.reset(true, is_e);
            if let Kind::Saturn { active, .. } = &mut c.kind {
                *active = 0;
            }
        }
        self.attach(mem);
    }

    /// Per-card state in ascending slot order, for save-states.
    pub fn infos(&self) -> Vec<LcInfo> {
        self.cards
            .iter()
            .map(|c| LcInfo {
                slot: c.slot,
                kind: match c.kind {
                    Kind::Builtin => LcKind::Builtin,
                    Kind::Card16K => LcKind::Card16K,
                    Kind::Saturn { banks, active } => LcKind::Saturn { banks, active },
                },
                memmode: c.memmode,
                last_ram_write: c.last_ram_write,
                region: c.region(),
            })
            .collect()
    }

    /// Sets the flags, "last access was odd" latch and (Saturn) active bank of the card in
    /// `slot`; false if there is none. The memory is not touched.
    pub fn set_info(
        &mut self,
        slot: u8,
        memmode: u32,
        last_ram_write: bool,
        active_bank: Option<u8>,
    ) -> bool {
        let Some(c) = self.cards.iter_mut().find(|c| c.slot == slot) else {
            return false;
        };
        c.memmode = memmode & mf::LANGCARD_MASK;
        c.last_ram_write = last_ram_write;
        if let (Kind::Saturn { banks, active }, Some(a)) = (&mut c.kind, active_bank) {
            *active = a.min(*banks - 1);
        }
        true
    }

    /// Slot of the card that last set the main LC RAM region (`m_lastSlotToSetMainMemLC`).
    pub fn last_slot(&self) -> u8 {
        self.last_slot
    }

    pub fn set_last_slot(&mut self, slot: u8) {
        self.last_slot = slot;
    }

    /// True if a Language Card or Saturn lives in `slot`.
    pub fn has_slot(&self, slot: u8) -> bool {
        self.cards.iter().any(|c| c.slot == slot)
    }

    /// Flags of the card in `slot`, for tests and debuggers.
    pub fn flags(&self, slot: u8) -> Option<u32> {
        self.cards
            .iter()
            .find(|c| c.slot == slot)
            .map(|c| c.memmode)
    }

    /// Active Saturn bank (0-7) of the card in `slot`.
    pub fn saturn_bank(&self, slot: u8) -> Option<u8> {
        self.cards
            .iter()
            .find(|c| c.slot == slot)
            .and_then(|c| match c.kind {
                Kind::Saturn { active, .. } => Some(active),
                _ => None,
            })
    }

    /// Access to $C080+`slot`*16+`lo` (`lo` 0-15); `rmw` is `is_rmw_abs` of the executing
    /// opcode. Returns false if no Language Card lives in `slot` (the caller then owns the access).
    /// Reads of a handled access return the floating bus. AppleWin's `MemOptimizeForModeChanging`
    /// shortcut (skipping the paging update before an `STA $C002-5`) is not ported.
    pub fn io(&mut self, slot: u8, lo: u8, write: bool, rmw: bool, mem: &mut Memory) -> bool {
        let Some(i) = self.cards.iter().position(|c| c.slot == slot) else {
            return false;
        };
        let lo = lo & 0xF;
        let before = self.cards[i].memmode;
        let mut moved = false;
        let c = &mut self.cards[i];
        let mut m = before;
        match c.kind {
            Kind::Saturn {
                banks,
                ref mut active,
            } if lo & 4 != 0 => {
                // Bank select, LanguageCard.cpp:410: banks beyond the card's size clamp.
                *active = (((lo >> 1) & 4) | (lo & 3)).min(banks - 1);
                self.select_region(i, mem);
                moved = true;
            }
            kind => {
                m &= !(mf::BANK2 | mf::HIGHRAM);
                if lo & 8 == 0 {
                    m |= mf::BANK2;
                }
                if (lo >> 1) & 1 == lo & 1 {
                    m |= mf::HIGHRAM;
                }
                if matches!(kind, Kind::Saturn { .. }) {
                    // Any two consecutive odd accesses, read or write (LanguageCard.cpp:432).
                    m = if lo & 1 != 0 && c.last_ram_write {
                        m | mf::WRITERAM
                    } else {
                        m & !mf::WRITERAM
                    };
                    c.last_ram_write = lo & 1 != 0;
                } else {
                    if lo & 1 == 0 {
                        m &= !mf::WRITERAM; // UTAIIe:5-23
                    } else if c.last_ram_write && (!write || rmw) {
                        m |= mf::WRITERAM; // GH#392 / GH#700; otherwise unchanged
                    }
                    c.last_ram_write = lo & 1 != 0 && !write;
                }
                c.memmode = m;
                if self.last_slot != slot {
                    self.select_region(i, mem);
                    moved = true;
                }
            }
        }
        if m != before || moved {
            self.set_mem_mode(slot, mem);
        }
        true
    }

    /// `SetMemMainLanguageCard` for card `i`.
    fn select_region(&mut self, i: usize, mem: &mut Memory) {
        self.last_slot = self.cards[i].slot;
        mem.set_lc_region(self.cards[i].region());
    }

    /// `LanguageCardManager::SetMemMode` (LanguageCard.cpp:~640): the global flags come from the
    /// first card with WRITERAM (favouring `slot`, the last accessed), else from `slot` itself.
    fn set_mem_mode(&mut self, slot: u8, mem: &mut Memory) {
        let mut writable = None;
        for (i, c) in self.cards.iter().enumerate() {
            if c.memmode & mf::WRITERAM != 0 {
                writable = Some(i);
                if c.slot == slot {
                    break;
                }
            }
        }
        match writable {
            Some(i) => {
                mem.set_lc_flags(self.cards[i].memmode);
                if self.cards[i].slot != slot {
                    self.select_region(i, mem);
                }
            }
            None => {
                if let Some(c) = self.cards.iter().find(|c| c.slot == slot) {
                    mem.set_lc_flags(c.memmode);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_is_serde() {
        fn assert_serde<T: Serialize + for<'a> Deserialize<'a>>() {}
        assert_serde::<LanguageCards>();
    }

    #[test]
    fn rmw_detection_needs_matching_operand() {
        assert!(is_rmw_abs(0xFE, 0xC083, 0xC083, false));
        assert!(!is_rmw_abs(0xFE, 0xC083, 0xC084, false));
        assert!(is_rmw_abs(0xEE, 0xC08B, 0xC08B, true));
        assert!(!is_rmw_abs(0xEE, 0xC08B, 0xC08B, false));
        assert!(!is_rmw_abs(0x8D, 0xC083, 0xC083, true));
    }
}
