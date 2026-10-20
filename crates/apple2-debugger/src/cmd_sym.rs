//! Symbol commands (Debugger_Symbols.cpp:261-1133) and loading of `.SYM` / `.SYM2` files.

use apple2_core::Machine;

use crate::args::*;
use crate::cmdtable::Cmd;
use crate::console::*;
use crate::debugger::*;
use crate::symbols::*;

impl Debugger {
    /// `_CmdSymbolsInfoHeader`
    fn symbols_info_header(&self, t: Table, display_size: Option<usize>) -> String {
        let n = display_size.unwrap_or_else(|| self.symbols.len(t));
        let color = if self.symbols.is_displayed(t) { CHC_NUM_DEC } else { CHC_WARNING };
        format!("{CHC_USAGE}{}{CHC_ARG_SEP}:{color}{n} ", t.name())
    }

    /// `CmdSymbolsInfo`
    pub(crate) fn cmd_symbols_info(&mut self, _a: &Args, only: Option<Table>) -> Update {
        let tables: Vec<Table> = match only {
            Some(t) => vec![t],
            None => Table::ALL.to_vec(),
        };
        let mut text = String::from("  ");
        let mut plain = 2usize;
        for t in tables {
            let hdr = self.symbols_info_header(t, None);
            let len = t.name().len() + 1 + self.symbols.len(t).to_string().len() + 1;
            if plain + len > CONSOLE_WIDTH {
                self.console.print(&text);
                text = String::from("  ");
                plain = 2;
            }
            text += &hdr;
            plain += len;
        }
        self.console.print(&text);
        upd::CONSOLE_DISPLAY
    }

    fn print_symbol(&mut self, name: &str, addr: u16, t: Table) {
        self.console.print(&format!("  {CHC_USAGE}{}{CHC_ARG_SEP}: ${CHC_ADDRESS}{addr:04X} {CHC_SYMBOL}{name}", t.name()));
    }

    /// `String2Address`: `$hex` or `0hex`/`0xhex`.
    fn string_to_address(s: &str) -> Option<u16> {
        let hexp = |h: &str| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit());
        if let Some(h) = s.strip_prefix('$') {
            return hexp(h).then(|| hex_prefix(h));
        }
        if s.starts_with('0') {
            if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                return hexp(h).then(|| hex_prefix(h));
            }
            if hexp(s) {
                return Some(hex_prefix(s));
            }
        }
        None
    }

    /// `_CmdSymbolsListTables`
    fn symbols_list_tables(&mut self, a: &Args, tables: &[Table]) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::SymbolsList);
        }
        let sym2addr = |me: &mut Debugger, name: &str| -> bool {
            if let Some((addr, t)) = me.symbols.find_address(name) {
                if tables.contains(&t) {
                    me.print_symbol(name, addr, t);
                }
                true
            } else {
                false
            }
        };
        let addr2sym = |me: &mut Debugger, addr: u16| -> bool {
            if let Some((name, t)) = me.symbols.find_symbol(addr) {
                let name = name.to_string();
                if tables.contains(&t) {
                    me.print_symbol(&name, addr, t);
                }
                true
            } else {
                false
            }
        };
        for i in 1..=a.n() {
            let text = a.text(i).to_string();
            let val = a.value(i);
            if a.raw.get(i).map(|r| r.is_op("*")).unwrap_or(false) {
                for t in tables.iter().copied() {
                    let entries: Vec<(u16, String)> = self.symbols.table(t).iter().map(|(k, v)| (*k, v.clone())).collect();
                    if !entries.is_empty() {
                        for (ad, n) in entries {
                            self.print_symbol(&n, ad, t);
                        }
                        let h = self.symbols_info_header(t, None);
                        self.console.print(&h);
                    }
                }
            } else if val != 0 {
                if !sym2addr(self, &text) && !addr2sym(self, val) {
                    self.console.print(&format!(" Address not found: {CHC_ARG_SEP}${CHC_ADDRESS}{val:04X}{CHC_DEFAULT}"));
                }
            } else if !sym2addr(self, &text) {
                match Self::string_to_address(&text) {
                    Some(ad) => {
                        if !addr2sym(self, ad) {
                            self.console.print(&format!(" {CHC_ERROR}Symbol not found: {CHC_SYMBOL}{text}{CHC_DEFAULT}"));
                        }
                    }
                    None => self.console.print(&format!(" {CHC_ERROR}Symbol not found: {CHC_SYMBOL}{text}{CHC_DEFAULT}")),
                }
            }
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_symbols_list(&mut self, _m: &mut Machine, a: &Args) -> Update {
        self.symbols_list_tables(a, &Table::ALL)
    }

    /// `_CmdSymbolsUpdate`: `SYM ! name`, `SYM name = addr`, `SYM @ = addr`. None if not an update.
    fn symbols_update_forms(&mut self, a: &Args, table: Table) -> Option<Update> {
        let n = a.n();
        let mut remove = false;
        let mut update = false;
        let mut name = a.text(1).to_string();
        let mut addr = a.value(3);
        let mut table = table;
        if n == 2 && (a.cooked[1].is_op("!") || a.cooked[1].is_op("~")) {
            remove = true;
            name = a.text(2).to_string();
        }
        if n == 3 && a.cooked[2].is_op("=") {
            update = true;
        }
        if n == 2 && a.raw.get(1).map(|r| r.is_op("@")).unwrap_or(false) && a.cooked[1].is_op("=") {
            if table == Table::User1 {
                table = Table::User2;
            }
            addr = a.value(2);
            name = format!("_{addr:04X}");
            update = true;
        }
        if remove || update {
            self.symbol_update(table, &name, addr, remove, update);
            return Some(upd::CONSOLE_DISPLAY);
        }
        None
    }

    pub(crate) fn cmd_symbols(&mut self, m: &mut Machine, a: &Args) -> Update {
        let _ = m;
        if a.n() == 0 {
            return self.cmd_symbols_info(a, None);
        }
        if let Some(u) = self.symbols_update_forms(a, Table::User1) {
            return u;
        }
        self.symbols_list_tables(a, &Table::ALL)
    }

    pub(crate) fn cmd_symbols_table(&mut self, m: &mut Machine, a: &Args, cmd: Cmd) -> Update {
        let table = match cmd {
            Cmd::SymbolsRom => Table::Main,
            Cmd::SymbolsApplesoft => Table::Basic,
            Cmd::SymbolsAssembly => Table::Asm,
            Cmd::SymbolsUser1 => Table::User1,
            Cmd::SymbolsUser2 => Table::User2,
            Cmd::SymbolsSrc1 => Table::Src1,
            Cmd::SymbolsSrc2 => Table::Src2,
            Cmd::SymbolsDos33 => Table::Dos33,
            _ => Table::ProDos,
        };
        if a.n() == 0 {
            return self.cmd_symbols_info(a, Some(table));
        }
        if let Some(u) = self.symbols_update_forms(a, table) {
            return u;
        }
        {
            let i = 1;
            match a.text(i).to_ascii_uppercase().as_str() {
                "CLEAR" => {
                    self.symbols.clear(table);
                    self.data.0.retain(|d| table != Table::Asm || d.removed);
                    self.console.print(&format!(" Cleared symbol table: {CHC_STRING}{}", table.name()));
                    return upd::CONSOLE_DISPLAY | upd::SYMBOLS;
                }
                "LOAD" => {
                    // SYMxxx LOAD "file" [, offset]
                    let file = a.get(i + 1).filter(|x| x.quoted()).map(|x| x.text.clone());
                    let mut offset = 0u32;
                    if a.get(i + 2).map(|t| t.is_op(",")).unwrap_or(false) && a.get(i + 3).is_some() {
                        let o = a.value(i + 3) as u32;
                        offset = o;
                    }
                    let (loaded, u) = self.load_symbol_file(m, table, file.as_deref(), offset);
                    if loaded > 0 {
                        let h = self.symbols_info_header(table, Some(loaded));
                        self.console.print(&h);
                    }
                    return u | upd::CONSOLE_DISPLAY;
                }
                "SAVE" => return upd::CONSOLE_DISPLAY,
                "ON" => {
                    self.symbols.set_displayed(table, true);
                    let h = format!("{}{CHC_ARG_SEP}({CHC_COMMAND}ON{CHC_ARG_SEP})", self.symbols_info_header(table, None));
                    self.console.print(&h);
                    return upd::CONSOLE_DISPLAY | upd::DISASM;
                }
                "OFF" => {
                    self.symbols.set_displayed(table, false);
                    let h = format!("{}{CHC_ARG_SEP}({CHC_COMMAND}OFF{CHC_ARG_SEP})", self.symbols_info_header(table, None));
                    self.console.print(&h);
                    return upd::CONSOLE_DISPLAY | upd::DISASM;
                }
                _ => return self.symbols_list_tables(a, &[table]),
            }
        }
    }

    /// `CmdSymbolsLoad`: from the host file `file` (or the table's default file name), falling back
    /// to the files shipped with AppleWin that are built in. `.SYM2` files are scripts. Returns the
    /// number of symbols read.
    pub fn load_symbol_file(&mut self, m: &mut Machine, table: Table, file: Option<&str>, offset: u32) -> (usize, Update) {
        let name = file.unwrap_or(table.file_name()).to_string();
        let text: Option<String> = match self.host.read(&name) {
            Ok(b) => Some(String::from_utf8_lossy(&b).into_owned()),
            Err(_) if file.is_none() => match table {
                Table::Main => Some(BUILTIN_MAIN.to_string()),
                Table::Basic => Some(BUILTIN_BASIC.to_string()),
                Table::Dos33 => Some(BUILTIN_DOS33.to_string()),
                _ => None,
            },
            Err(_) => None,
        };
        let Some(text) = text else {
            if file.is_some() {
                self.console.error("Symbol File not found:");
                self.console.print(&format!("{CHC_PATH}{}", self.host.dir()));
            }
            return (0, upd::DISASM);
        };
        let n;
        if name.to_ascii_uppercase().ends_with(".SYM2") {
            let before = self.symbols.len(table);
            let lvl = self.console.level;
            self.console.level = OutputLevel::Error;
            self.run_script_text(m, &text);
            self.console.level = lvl;
            n = self.symbols.len(table).saturating_sub(before);
        } else {
            let rep = self.symbols.parse_table(&text, table, offset);
            n = rep.loaded;
            for s in rep.aliases.iter().take(3) {
                self.console.print_level(OutputLevel::All, &format!(" {CHC_INFO}Info.: {CHC_SYMBOL}{s}"));
            }
            for s in rep.duplicates.iter().take(3) {
                self.console.print_level(OutputLevel::All, &format!("{CHC_ERROR} {s}"));
            }
        }
        (n, upd::DISASM | if n > 0 { upd::SYMBOLS } else { 0 })
    }

    /// `CmdDebugStartup`: Main, Applesoft, DOS 3.3 and User 1 symbols.
    pub fn startup_symbols(&mut self, m: &mut Machine) {
        self.symbols.clear_all();
        for t in [Table::Main, Table::Basic, Table::Dos33, Table::User1] {
            self.load_symbol_file(m, t, None, 0);
        }
    }

    pub(crate) fn cmd_startup(&mut self, m: &mut Machine) -> Update {
        self.startup_symbols(m);
        upd::NOTHING
    }
}
