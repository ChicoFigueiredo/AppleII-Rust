//! Symbol tables (Debugger_Symbols.cpp): nine tables searched last-to-first, `.SYM` parsing in
//! both AppleWin (`ADDR NAME`) and ACME (`NAME = $ADDR ; comment`) syntax.

use std::collections::BTreeMap;

pub const MAX_SYMBOLS_LEN: usize = 51;

/// `SymbolTable_Index_e`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Table {
    Main = 0,
    Basic,
    Asm,
    User1,
    User2,
    Src1,
    Src2,
    Dos33,
    ProDos,
}

pub const NUM_TABLES: usize = 9;

impl Table {
    pub const ALL: [Table; NUM_TABLES] = [
        Table::Main,
        Table::Basic,
        Table::Asm,
        Table::User1,
        Table::User2,
        Table::Src1,
        Table::Src2,
        Table::Dos33,
        Table::ProDos,
    ];

    /// `g_aSymbolTableNames` (Debugger_Symbols.cpp:58).
    pub fn name(self) -> &'static str {
        [
            "Main", "Basic", "Asm", "User1", "User2", "Src1", "Src2", "DOS33", "ProDOS",
        ][self as usize]
    }

    /// `g_sFileNameSymbols` (:45).
    pub fn file_name(self) -> &'static str {
        [
            "APPLE2E.SYM",
            "A2_BASIC.SYM",
            "A2_ASM.SYM",
            "A2_USER1.SYM",
            "A2_USER2.SYM",
            "A2_SRC1.SYM",
            "A2_SRC2.SYM",
            "A2_DOS33.SYM2",
            "A2_PRODOS.SYM",
        ][self as usize]
    }

    fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// `APPLE2E.SYM`, `A2_BASIC.SYM`, `A2_DOS33.SYM2` of AppleWin's `bin/`.
pub const BUILTIN_MAIN: &str = include_str!("../symbols/APPLE2E.SYM");
pub const BUILTIN_BASIC: &str = include_str!("../symbols/A2_BASIC.SYM");
pub const BUILTIN_DOS33: &str = include_str!("../symbols/A2_DOS33.SYM2");

/// Result of [`Symbols::parse_table`]: symbols read, and one message per symbol whose address
/// already had another name ("aliases") or whose name already existed ("duplicate").
#[derive(Debug, Default)]
pub struct ParseReport {
    pub loaded: usize,
    pub aliases: Vec<String>,
    pub duplicates: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Symbols {
    tables: [BTreeMap<u16, String>; NUM_TABLES],
    /// `g_bDisplaySymbolTables`: all but ProDOS by default.
    pub display: u32,
}

impl Default for Symbols {
    fn default() -> Self {
        Symbols {
            tables: Default::default(),
            display: ((1 << NUM_TABLES) - 1) & !Table::ProDos.bit(),
        }
    }
}

impl Symbols {
    /// Symbol for `addr`; user tables first (highest table index wins), skipping disabled tables
    /// (`FindSymbolFromAddress`, :140).
    pub fn find_symbol(&self, addr: u16) -> Option<(&str, Table)> {
        for t in Table::ALL.iter().rev() {
            if self.display & t.bit() == 0 {
                continue;
            }
            if let Some(s) = self.tables[*t as usize].get(&addr) {
                return Some((s, *t));
            }
        }
        None
    }

    /// Address of the symbol `name` (case-insensitive, `FindAddressFromSymbol`, :171).
    pub fn find_address(&self, name: &str) -> Option<(u16, Table)> {
        for t in Table::ALL.iter().rev() {
            if self.display & t.bit() == 0 {
                continue;
            }
            if let Some((a, _)) = self.tables[*t as usize]
                .iter()
                .find(|(_, s)| s.eq_ignore_ascii_case(name))
            {
                return Some((*a, *t));
            }
        }
        None
    }

    pub fn table(&self, t: Table) -> &BTreeMap<u16, String> {
        &self.tables[t as usize]
    }

    pub fn len(&self, t: Table) -> usize {
        self.tables[t as usize].len()
    }

    pub fn is_empty(&self) -> bool {
        self.tables.iter().all(BTreeMap::is_empty)
    }

    pub fn clear(&mut self, t: Table) {
        self.tables[t as usize].clear();
    }

    pub fn clear_all(&mut self) {
        for t in &mut self.tables {
            t.clear();
        }
    }

    pub fn is_displayed(&self, t: Table) -> bool {
        self.display & t.bit() != 0
    }

    pub fn set_displayed(&mut self, t: Table, on: bool) {
        if on {
            self.display |= t.bit();
        } else {
            self.display &= !t.bit();
        }
    }

    /// `SymbolUpdate` add/rename: removes a previous entry of the same name in this table, then
    /// stores `name` at `addr`. Returns the previous address of the name if it existed here.
    pub fn set(&mut self, t: Table, name: &str, addr: u16) -> Option<u16> {
        let name: String = name.chars().take(MAX_SYMBOLS_LEN).collect();
        let prev = self.find_in_table(t, &name);
        if let Some(p) = prev {
            self.tables[t as usize].remove(&p);
        }
        self.tables[t as usize].insert(addr, name);
        prev
    }

    /// Removes `name` from table `t`; true if it was there.
    pub fn remove(&mut self, t: Table, name: &str) -> bool {
        match self.find_in_table(t, name) {
            Some(a) => self.tables[t as usize].remove(&a).is_some(),
            None => false,
        }
    }

    fn find_in_table(&self, t: Table, name: &str) -> Option<u16> {
        self.tables[t as usize]
            .iter()
            .find(|(_, s)| s.eq_ignore_ascii_case(name))
            .map(|(a, _)| *a)
    }

    /// `ParseSymbolTable` (:555): reads `.SYM` text into table `t`, adding `offset` to every address.
    /// Lines without `$` are `ADDR NAME` (hex); lines with `$` are ACME `NAME = $ADDR ; comment`.
    pub fn parse_table(&mut self, text: &str, t: Table, offset: u32) -> ParseReport {
        let mut rep = ParseReport::default();
        for raw in text.lines() {
            let parsed = if !raw.contains('$') {
                let mut it = raw.split_whitespace();
                match (
                    it.next()
                        .and_then(|a| u32::from_str_radix(a.trim_start_matches("0x"), 16).ok()),
                    it.next(),
                ) {
                    (Some(a), Some(n)) => Some((a, n.to_string())),
                    _ => None,
                }
            } else {
                let line = raw.split(';').next().unwrap_or("");
                let line = line.replacen('=', " ", 1).replacen('$', " ", 1);
                let mut it = line.split_whitespace();
                match (
                    it.next(),
                    it.next().and_then(|a| u32::from_str_radix(a, 16).ok()),
                ) {
                    (Some(n), Some(a)) => Some((a, n.to_string())),
                    _ => None,
                }
            };
            let Some((addr, name)) = parsed else { continue };
            let name = name.as_str();
            let addr = addr.wrapping_add(offset);
            if addr > 0xFFFF || name.is_empty() {
                continue;
            }
            let name: String = name.chars().take(MAX_SYMBOLS_LEN).collect();
            if let Some((prev, pt)) = self.find_symbol(addr as u16) {
                rep.aliases
                    .push(format!("{name} aliases ${addr:04X} {prev} ({})", pt.name()));
            }
            if let Some((_, pt)) = self.find_address(&name) {
                rep.duplicates.push(format!(
                    "Dup Symbol Name ({}) ${addr:04X} {name}",
                    pt.name()
                ));
            }
            self.tables[t as usize].insert(addr as u16, name);
            rep.loaded += 1;
        }
        rep
    }

    /// Loads the three symbol files AppleWin ships (Main, Basic from `.SYM`; DOS33 is a script, see
    /// [`crate::Debugger::load_builtin_symbols`]).
    pub fn load_builtin_sym(&mut self) {
        self.parse_table(BUILTIN_MAIN, Table::Main, 0);
        self.parse_table(BUILTIN_BASIC, Table::Basic, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_main_and_basic_tables_parse() {
        let mut s = Symbols::default();
        let main = s.parse_table(BUILTIN_MAIN, Table::Main, 0);
        let basic = s.parse_table(BUILTIN_BASIC, Table::Basic, 0);
        // 798 / 539 column-0 `XXXX NAME` lines + 110 indented zero-page `XX NAME` lines.
        assert_eq!((main.loaded, basic.loaded), (798, 649));
        assert_eq!(s.find_symbol(0xFDED).map(|x| x.0), Some("COUT"));
        assert_eq!(s.find_address("cout"), Some((0xFDED, Table::Main)));
        assert_eq!(s.find_address("HOME").map(|x| x.0), Some(0xFC58));
        assert_eq!(s.find_address("PRINT.FAC").map(|x| x.0), Some(0xED2E));
    }

    #[test]
    fn acme_syntax_and_offset() {
        let mut s = Symbols::default();
        let r = s.parse_table(
            "FOO = $1234 ; comment\nBAR=$0010\n; ignored\n",
            Table::User1,
            0x100,
        );
        assert_eq!(r.loaded, 2);
        assert_eq!(s.find_address("foo"), Some((0x1334, Table::User1)));
        assert_eq!(s.find_address("BAR"), Some((0x0110, Table::User1)));
    }

    #[test]
    fn later_tables_win_and_disabled_are_skipped() {
        let mut s = Symbols::default();
        s.set(Table::Main, "A", 0x300);
        s.set(Table::User1, "B", 0x300);
        assert_eq!(s.find_symbol(0x300), Some(("B", Table::User1)));
        s.set_displayed(Table::User1, false);
        assert_eq!(s.find_symbol(0x300), Some(("A", Table::Main)));
    }

    #[test]
    fn set_renames_within_table() {
        let mut s = Symbols::default();
        s.set(Table::User1, "X", 0x300);
        assert_eq!(s.set(Table::User1, "x", 0x400), Some(0x300));
        assert_eq!(s.find_symbol(0x300), None);
        assert!(s.remove(Table::User1, "X"));
    }

    #[test]
    fn out_of_range_and_aliases() {
        let mut s = Symbols::default();
        let r = s.parse_table("FFFF TOP\n10000 BAD\n0 A\n0 B\n", Table::Main, 0);
        assert_eq!(r.loaded, 3);
        assert_eq!(r.aliases.len(), 1);
    }
}
