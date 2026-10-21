//! Breakpoints, watches, zero-page pointers and bookmarks (`Breakpoint_t`, Debugger_Types.h:229;
//! checks in Debug.cpp:1131-1553).

use apple2_core::memory::mf;

pub const MAX_BREAKPOINTS: usize = 16;
pub const MAX_WATCHES: usize = 6;
pub const MAX_ZEROPAGE_POINTERS: usize = 8;
pub const MAX_BOOKMARKS: usize = 10;

/// `BreakpointSource_t`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    RegA,
    RegX,
    RegY,
    RegPc,
    RegS,
    RegP,
    FlagC,
    FlagZ,
    FlagI,
    FlagD,
    FlagB,
    FlagR,
    FlagV,
    FlagN,
    Opcode,
    MemRw,
    MemRead,
    MemWrite,
    Video,
}

impl Source {
    /// `g_aBreakpointSource`
    pub fn name(self) -> &'static str {
        match self {
            Source::RegA => "A",
            Source::RegX => "X",
            Source::RegY => "Y",
            Source::RegPc => "PC",
            Source::RegS => "S",
            Source::RegP => "P",
            Source::FlagC => "C",
            Source::FlagZ => "Z",
            Source::FlagI => "I",
            Source::FlagD => "D",
            Source::FlagB => "B",
            Source::FlagR => "R",
            Source::FlagV => "V",
            Source::FlagN => "N",
            Source::Opcode => "OP",
            Source::MemRw | Source::MemRead | Source::MemWrite => "M",
            Source::Video => "V",
        }
    }

    pub fn is_mem(self) -> bool {
        matches!(self, Source::MemRw | Source::MemRead | Source::MemWrite)
    }
}

/// `BreakpointOperator_t`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    LessEq,
    Less,
    Eq,
    NotEq,
    Greater,
    GreaterEq,
    Read,
    Write,
    ReadWrite,
}

impl Op {
    /// `g_aBreakpointSymbols`: always two characters.
    pub fn symbol(self) -> &'static str {
        match self {
            Op::LessEq => "<=",
            Op::Less => "< ",
            Op::Eq => "= ",
            Op::NotEq => "!=",
            Op::Greater => "> ",
            Op::GreaterEq => ">=",
            Op::Read => "? ",
            Op::Write => "@ ",
            Op::ReadWrite => "* ",
        }
    }

    pub fn parse(s: &str) -> Option<Op> {
        Some(match s {
            "<=" => Op::LessEq,
            "<" => Op::Less,
            "=" => Op::Eq,
            "!=" | "!" => Op::NotEq,
            ">" => Op::Greater,
            ">=" => Op::GreaterEq,
            _ => return None,
        })
    }
}

/// `AddressPrefix_t`: `[sN/][nn/][L<1|2>/][ROM/]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prefix {
    pub slot: Option<u8>,
    pub bank: Option<u16>,
    pub lang_card: Option<u8>,
    pub is_rom: bool,
}

impl Prefix {
    pub fn is_none(&self) -> bool {
        *self == Prefix::default()
    }

    /// `GetFullPrefixAddrForBreakpoint` prefix part.
    pub fn text(&self) -> String {
        let mut s = String::new();
        if let Some(sl) = self.slot {
            s += &format!("s{sl}/");
        }
        if let Some(b) = self.bank {
            s += &if b < 0x100 {
                format!("{b:02X}/")
            } else {
                format!("{b:03X}/")
            };
        }
        if let Some(l) = self.lang_card {
            s += &format!("l{l}/");
        }
        if self.is_rom {
            s += "ROM/";
        }
        s
    }
}

#[derive(Clone, Debug)]
pub struct Breakpoint {
    pub address: u16,
    pub length: u32,
    pub source: Source,
    pub op: Op,
    pub set: bool,
    pub enabled: bool,
    pub temp: bool,
    pub hit: bool,
    pub stop: bool,
    pub hit_count: u32,
    pub prefix: Prefix,
}

impl Default for Breakpoint {
    fn default() -> Self {
        Breakpoint {
            address: 0,
            length: 0,
            source: Source::RegPc,
            op: Op::Eq,
            set: false,
            enabled: false,
            temp: false,
            hit: false,
            stop: false,
            hit_count: 0,
            prefix: Prefix::default(),
        }
    }
}

/// Memory-paging facts the address-prefix filter needs.
#[derive(Clone, Copy, Debug, Default)]
pub struct PagingView {
    pub memmode: u32,
    /// RamWorks active bank + 1 (`[0x01..0x100]`).
    pub aux_bank_plus1: u32,
}

impl Breakpoint {
    /// `_BreakpointValid`
    pub fn valid(&self) -> bool {
        self.enabled && self.length != 0
    }

    /// `_CheckBreakpointValueWithPrefix` (Debug.cpp:1175). The Saturn slot check is not made (the
    /// debugger has no card registry), a slot prefix is accepted for any `D000-FFFF` address.
    fn prefix_ok(&self, v: u32, pg: &PagingView) -> bool {
        if self.prefix.is_none() || self.op != Op::Eq {
            return true;
        }
        let m = pg.memmode;
        let is_read = matches!(self.source, Source::MemRw | Source::MemRead | Source::RegPc);
        let is_write = matches!(self.source, Source::MemRw | Source::MemWrite);
        let bank = self.prefix.bank;
        let want_main = bank == Some(0);
        let want_aux = bank == Some(pg.aux_bank_plus1 as u16);
        let sel = |aux_flag: bool| (want_main && !aux_flag) || (want_aux && aux_flag);
        if v <= 0x1FF {
            sel(m & mf::ALTZP != 0)
        } else if ((0x400..=0x7FF).contains(&v) && m & mf::STORE80 != 0)
            || ((0x2000..=0x3FFF).contains(&v)
                && m & (mf::STORE80 | mf::HIRES) == (mf::STORE80 | mf::HIRES))
        {
            sel(m & mf::PAGE2 != 0)
        } else if v <= 0xBFFF {
            (is_read && sel(m & mf::AUXREAD != 0)) || (is_write && sel(m & mf::AUXWRITE != 0))
        } else if v <= 0xCFFF {
            true
        } else {
            let no_bank = self.prefix.slot.is_none() && bank.is_none();
            if m & mf::HIGHRAM != 0 {
                let ok = no_bank || (self.prefix.slot.is_none() && sel(m & mf::ALTZP != 0));
                let lc_ok = match self.prefix.lang_card {
                    None => true,
                    Some(1) => m & mf::BANK2 == 0,
                    Some(2) => m & mf::BANK2 != 0,
                    _ => false,
                } || v >= 0xE000;
                ok && lc_ok
                    && !self.prefix.is_rom
                    && (is_read || (is_write && m & mf::WRITERAM != 0))
            } else if is_write && m & mf::WRITERAM != 0 {
                true
            } else {
                self.prefix.slot.is_none() && bank.is_none() && self.prefix.lang_card.is_none()
            }
        }
    }

    /// `_CheckBreakpointValue` (:1301): does `v` satisfy the comparison.
    pub fn check_value(&self, v: u32, pg: &PagingView) -> bool {
        let a = self.address as u32;
        let ok = match self.op {
            Op::LessEq => v <= a,
            Op::Less => v < a,
            Op::Eq => v >= a && v < a + self.length,
            Op::NotEq => v < a || v >= a + self.length,
            Op::Greater => v > a,
            Op::GreaterEq => v >= a,
            _ => false,
        };
        ok && self.prefix_ok(v, pg)
    }
}

/// A fixed table of slots (`g_aBreakpoints[]`, `g_aWatches[]`, ...).
#[derive(Clone, Debug)]
pub struct BpTable {
    pub slots: Vec<Breakpoint>,
    pub count: usize,
}

impl BpTable {
    pub fn new(n: usize) -> Self {
        BpTable {
            slots: vec![Breakpoint::default(); n],
            count: 0,
        }
    }

    pub fn first_free(&self) -> Option<usize> {
        self.slots.iter().position(|b| !b.set)
    }

    /// `_BWZ_RemoveOne`
    pub fn remove(&mut self, i: usize) {
        if let Some(b) = self.slots.get_mut(i) {
            if b.set {
                b.set = false;
                b.enabled = false;
                b.length = 0;
                self.count -= 1;
            }
        }
    }

    pub fn remove_all(&mut self) {
        for i in 0..self.slots.len() {
            self.remove(i);
        }
    }

    pub fn set_enabled(&mut self, i: usize, on: bool) {
        if let Some(b) = self.slots.get_mut(i) {
            b.enabled = on;
        }
    }

    /// Fills slot `i` (`_CmdBreakpointAddReg`).
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        i: usize,
        source: Source,
        op: Op,
        address: u16,
        length: u32,
        temp: bool,
        prefix: Prefix,
    ) {
        let b = &mut self.slots[i];
        if !b.set {
            self.count += 1;
        }
        *b = Breakpoint {
            address,
            length,
            source,
            op,
            set: true,
            enabled: true,
            temp,
            hit: false,
            stop: true,
            hit_count: 0,
            prefix,
        };
    }

    /// `ClearTempBreakpoints`: temporaries that were hit go away; hit flags reset.
    pub fn clear_temp(&mut self) {
        for i in 0..self.slots.len() {
            if self.slots[i].valid() {
                if self.slots[i].hit && self.slots[i].temp {
                    self.remove(i);
                } else {
                    self.slots[i].hit = false;
                }
            }
        }
    }

    pub fn iter_set(&self) -> impl Iterator<Item = (usize, &Breakpoint)> {
        self.slots.iter().enumerate().filter(|(_, b)| b.set)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bp(op: Op, addr: u16, len: u32) -> Breakpoint {
        Breakpoint {
            address: addr,
            length: len,
            op,
            set: true,
            enabled: true,
            stop: true,
            ..Default::default()
        }
    }

    #[test]
    fn comparison_operators_follow_applewin() {
        let pg = PagingView::default();
        let b = bp(Op::Eq, 0x300, 0x10);
        assert!(
            b.check_value(0x300, &pg) && b.check_value(0x30F, &pg) && !b.check_value(0x310, &pg)
        );
        assert!(bp(Op::NotEq, 0x300, 1).check_value(0x301, &pg));
        assert!(
            !bp(Op::Less, 5, 1).check_value(5, &pg) && bp(Op::LessEq, 5, 1).check_value(5, &pg)
        );
        assert!(
            bp(Op::Greater, 5, 1).check_value(6, &pg)
                && bp(Op::GreaterEq, 5, 1).check_value(5, &pg)
        );
    }

    #[test]
    fn rom_and_bank_prefixes_filter_by_paging() {
        let mut b = bp(Op::Eq, 0xD000, 1);
        b.prefix.bank = Some(0);
        let ram = PagingView {
            memmode: mf::HIGHRAM | mf::WRITERAM,
            aux_bank_plus1: 1,
        };
        assert!(b.check_value(0xD000, &ram));
        let rom = PagingView {
            memmode: 0,
            aux_bank_plus1: 1,
        };
        assert!(!b.check_value(0xD000, &rom));
    }

    #[test]
    fn table_bookkeeping() {
        let mut t = BpTable::new(MAX_BREAKPOINTS);
        t.add(0, Source::RegPc, Op::Eq, 0x300, 1, true, Prefix::default());
        t.add(
            1,
            Source::MemWrite,
            Op::Eq,
            0x400,
            0x400,
            false,
            Prefix::default(),
        );
        assert_eq!((t.count, t.first_free()), (2, Some(2)));
        t.slots[0].hit = true;
        t.clear_temp();
        assert_eq!(t.count, 1);
        t.remove_all();
        assert_eq!(t.count, 0);
        assert_eq!(
            Prefix {
                bank: Some(1),
                lang_card: Some(2),
                ..Default::default()
            }
            .text(),
            "01/l2/"
        );
    }
}
