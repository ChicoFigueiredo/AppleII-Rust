//! Memory commands (Debug.cpp:4059-5009, 5940-6355) and the disassembler data directives
//! (Debugger_DisassemblerData.cpp).

use apple2_core::Machine;

use crate::args::*;
use crate::bp::Prefix;
use crate::cmdtable::Cmd;
use crate::console::*;
use crate::debugger::*;
use crate::disasm::*;
use crate::mem::DebugMem;
use crate::symbols::Table;

/// `ReadByteFromMemoryWithPrefix` (Debugger_Display.cpp:2130) for main/aux banks.
pub fn read_prefixed(m: &Machine, addr: u16, p: &Prefix) -> u8 {
    match p.bank {
        Some(0) if !p.is_rom => m.peek_main(addr),
        Some(b) if b >= 1 && !p.is_rom => m.peek_aux(addr),
        _ => m.peek(addr),
    }
}

/// Parsed `["file"] , [bank :] addr [, len | : end]`: (file, bank, start, length).
type LoadSave = (Option<String>, Option<u16>, u16, Option<u32>);

/// One element of a memory search pattern (`MemorySearch_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pat {
    Exact(u8),
    High(u8),
    Low(u8),
    Wild,
}

impl Debugger {
    // ---- mini dumps ----

    pub(crate) fn cmd_mem_dump(&mut self, m: &mut Machine, a: &Args, which: usize, view: MemView) -> Update {
        if a.n() == 0 {
            let d = self.mem_dump[which];
            if !d.active {
                self.console.print(&format!("Mini memory area-{} not set", which + 1));
            } else {
                self.console.print(&format!("Mini memory area-{}: {}", which + 1, crate::cmd_bp::prefix_addr_text(&d.prefix, d.addr, false)));
            }
            return upd::CONSOLE_DISPLAY;
        }
        let mut i = 1;
        let Ok(prefix) = self.range_prefixes(a, &mut i) else { return self.help_last() };
        if i > a.n() {
            return self.help_last();
        }
        let _ = m;
        self.mem_dump[which] = MemDump { addr: a.value(i), view, active: true, prefix };
        upd::MEM_DUMP
    }

    // ---- enter / fill / move / compare ----

    pub(crate) fn cmd_memory_enter(&mut self, m: &mut Machine, a: &Args, word: bool) -> Update {
        let cmd = if word { Cmd::MemoryEnterWord } else { Cmd::MemoryEnterByte };
        if a.n() < 2 {
            return self.help_for(cmd);
        }
        let addr = a.value(1);
        let mut k = 0u16;
        for i in 2..=a.n() {
            let v = a.value(i);
            if word || v > 0xFF {
                m.write(addr.wrapping_add(k), v as u8);
                m.write(addr.wrapping_add(k + 1), (v >> 8) as u8);
                k += 2;
            } else {
                m.write(addr.wrapping_add(k), v as u8);
                k += 1;
            }
        }
        upd::ALL
    }

    pub(crate) fn cmd_memory_fill(&mut self, m: &mut Machine, a: &Args) -> Update {
        let n = a.n();
        if !(3..=4).contains(&n) {
            return self.help_for(Cmd::MemoryFill);
        }
        let (start, len) = if n == 3 {
            let (s, e) = (a.value(1), a.value(2));
            (s, (e as i32 - s as i32 + 1).clamp(0, 0xFFFF) as u32)
        } else {
            let (a1, a2, rt) = range_get(a, 1);
            match range_end_len(rt, a1, a2) {
                Some((_, l)) => (a1, l),
                None => return self.help_for(Cmd::MemoryMove),
            }
        };
        let v = a.value(n) as u8;
        let mut p = start;
        for _ in 0..len {
            if !(0xC000..=0xC0FF).contains(&p) {
                m.write(p, v);
            }
            p = p.wrapping_add(1);
        }
        upd::ALL
    }

    pub(crate) fn cmd_memory_move(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() < 3 {
            return self.help_for(Cmd::MemoryMove);
        }
        let mut dst = a.value(1);
        let (s, e, rt) = range_get(a, 2);
        let Some((_, len)) = range_end_len(rt, s, e) else { return self.help_for(Cmd::MemoryMove) };
        let mut src = s;
        for _ in 0..len {
            if !(0xC000..=0xC0FF).contains(&dst) {
                let v = m.read(src);
                m.write(dst, v);
            }
            dst = dst.wrapping_add(1);
            src = src.wrapping_add(1);
        }
        upd::ALL
    }

    /// `MC dst src:end` (AppleWin's `CmdMemoryCompare` is a stub): lists differing bytes.
    pub(crate) fn cmd_memory_compare(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() < 3 {
            return self.help_for(Cmd::MemoryCompare);
        }
        let dst = a.value(1);
        let (s, e, rt) = range_get(a, 2);
        let Some((_, len)) = range_end_len(rt, s, e) else { return self.help_for(Cmd::MemoryCompare) };
        let mut diffs = 0;
        for k in 0..len as u16 {
            let (x, y) = (m.read(s.wrapping_add(k)), m.read(dst.wrapping_add(k)));
            if x != y {
                diffs += 1;
                if diffs <= 32 {
                    self.console.print(&format!("{CHC_ADDRESS}{:04X}{CHC_DEFAULT}:{CHC_NUM_HEX}{x:02X} {CHC_ADDRESS}{:04X}{CHC_DEFAULT}:{CHC_NUM_HEX}{y:02X}", s.wrapping_add(k), dst.wrapping_add(k)));
                }
            }
        }
        self.console.print(&format!("{CHC_USAGE}Total{CHC_DEFAULT}: {CHC_NUM_DEC}{diffs}{CHC_DEFAULT} differences"));
        upd::CONSOLE_DISPLAY
    }

    // ---- search ----

    fn parse_search(&mut self, a: &Args, first: usize) -> Option<Vec<Pat>> {
        let mut pats: Vec<Pat> = Vec::new();
        for i in first..=a.n() {
            let arg = &a.cooked[i];
            if arg.kind == Kind::Quoted1 {
                pats.extend(arg.text.bytes().map(|b| Pat::Exact(b | 0x80)));
            } else if arg.kind == Kind::Quoted2 {
                pats.extend(arg.text.bytes().map(|b| Pat::Exact(b & 0x7F)));
            } else if arg.value > 0xFF {
                pats.push(Pat::Exact(arg.value as u8));
                pats.push(Pat::Exact((arg.value >> 8) as u8));
            } else {
                let t: Vec<char> = arg.text.chars().collect();
                if t.len() > 2 {
                    return None;
                }
                let nib = |c: char| c.to_digit(16).unwrap_or(0) as u8;
                let p = match t.as_slice() {
                    ['?'] | ['?', '?'] => Pat::Wild,
                    ['?', c] => Pat::Low(nib(*c)),
                    [c, '?'] => Pat::High(nib(*c) << 4),
                    _ => Pat::Exact(arg.value as u8),
                };
                if p == Pat::Wild && pats.last() == Some(&Pat::Wild) && t.len() == 2 {
                    continue;
                }
                pats.push(p);
            }
        }
        Some(pats)
    }

    pub(crate) fn cmd_memory_search(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() < 4 {
            return self.help_last();
        }
        let (s, e, rt) = range_get(a, 1);
        let Some((end, _)) = range_end_len(rt, s, e) else {
            self.console.error("Error: Missing address separator (comma or colon)");
            return upd::CONSOLE_DISPLAY;
        };
        let Some(pats) = self.parse_search(a, 4) else { return self.help_last() };
        self.search_results.clear();
        self.search_results.push(0xFFFF);
        for addr in s as u32..=end as u32 {
            let mut ok = true;
            for (k, p) in pats.iter().enumerate() {
                let b = m.peek((addr + k as u32) as u16);
                let hit = match p {
                    Pat::Exact(v) => b == *v,
                    Pat::High(v) => b & 0xF0 == *v,
                    Pat::Low(v) => b & 0x0F == *v,
                    Pat::Wild => true,
                };
                if !hit {
                    ok = false;
                    break;
                }
            }
            if ok && !pats.is_empty() {
                self.search_results.push(addr as u16);
            }
        }
        self.search_display()
    }

    /// `_SearchMemoryDisplay`
    pub(crate) fn search_display(&mut self) -> Update {
        let found = self.search_results.len().saturating_sub(1);
        if found > 0 {
            let mut line = String::new();
            let mut plain = 0usize;
            for i in 1..=found {
                let item = format!("{CHC_NUM_DEC}{i:02X}{CHC_DEFAULT}:{CHC_ARG_SEP}${CHC_ADDRESS}{:04X} ", self.search_results[i]);
                let w = 2 + 1 + 1 + 4 + 1;
                if plain + w > CONSOLE_WIDTH - 1 {
                    self.console.print(&line);
                    line.clear();
                    plain = 0;
                }
                line += &item;
                plain += w;
            }
            self.console.print(&line);
        }
        self.console.print(&format!("{CHC_USAGE}Total{CHC_DEFAULT}: {CHC_NUM_DEC}{found}  {CHC_ARG_SEP}({CHC_ARG_SEP}#${CHC_NUM_HEX}{found:04X}{CHC_ARG_SEP})"));
        upd::CONSOLE_DISPLAY
    }

    // ---- BLOAD / BSAVE / TSAVE ----

    /// Parses `["file"] , addr [, len | : end]` or the bank form `["file"] , bank : addr [...]`.
    fn parse_load_save(&self, a: &Args) -> Option<LoadSave> {
        let n = a.n();
        let has_name = a.get(1).map(|x| x.kind == Kind::Quoted2).unwrap_or(false);
        let name = has_name.then(|| a.text(1).to_string());
        let mut i = if has_name { 2 } else { 1 };
        if !a.get(i)?.is_op(",") {
            return None;
        }
        i += 1;
        let mut bank = None;
        if a.get(i + 1).map(|t| t.is_op(":")).unwrap_or(false) && n >= i + 4 && a.get(i + 3).map(|t| t.is_op(",") || t.is_op(":")).unwrap_or(false) {
            bank = Some(a.value(i));
            i += 2;
        }
        let (s, e, rt) = range_get(a, i);
        let len = range_end_len(rt, s, e).map(|(_, l)| l);
        Some((name, bank, s, len))
    }

    pub(crate) fn cmd_memory_save(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            self.console.print("Last saved: none");
            return upd::CONSOLE_DISPLAY;
        }
        let Some((name, bank, start, Some(len))) = self.parse_load_save(a) else { return self.help_for(Cmd::MemorySave) };
        if len == 0 || start as u32 + len > 0x10000 {
            self.console.print("Error: Length = 0.");
            return upd::CONSOLE_DISPLAY;
        }
        let name = name.unwrap_or_else(|| match bank {
            Some(b) => format!("{start:04X}.{len:04X}.bank{b:02X}.bin"),
            None => format!("{start:04X}.{len:04X}.bin"),
        });
        let bytes: Vec<u8> = (0..len as u16)
            .map(|k| {
                let ad = start.wrapping_add(k);
                match bank {
                    Some(0) => m.peek_main(ad),
                    Some(_) => m.peek_aux(ad),
                    None => m.peek(ad),
                }
            })
            .collect();
        match self.host.write(&name, &bytes) {
            Ok(()) => self.console.print("Saved."),
            Err(_) => self.console.print("Error opening file."),
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_memory_load(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() < 1 {
            return self.help_for(Cmd::MemoryLoad);
        }
        let name = if a.get(1).map(|x| x.kind == Kind::Quoted2).unwrap_or(false) { a.text(1).to_string() } else { String::new() };
        // Known extensions load at fixed addresses.
        let lower = name.to_ascii_lowercase();
        let known: Option<(u16, u32)> = if lower.ends_with(".hgr2") {
            Some((0x4000, 0x2000))
        } else if lower.ends_with(".hgr") {
            Some((0x2000, 0x2000))
        } else {
            None
        };
        let parsed = self.parse_load_save(a);
        let (bank, start, len) = match (parsed, known) {
            (Some((_, bank, s, l)), _) => (bank, s, l),
            (None, Some((s, l))) => (None, s, Some(l)),
            _ => return self.help_for(Cmd::MemoryLoad),
        };
        let bytes = match self.host.read(&name) {
            Ok(b) => b,
            Err(_) => {
                self.console.print("ERROR: Bad filename");
                self.console.print(&format!("Path: {}", self.host.dir()));
                self.console.print(&format!("File: {name}"));
                return upd::CONSOLE_DISPLAY;
            }
        };
        let file_bytes = bytes.len().min(0x10000) as u32;
        let n = match len {
            Some(l) if l > 0 => l.min(file_bytes),
            _ => file_bytes,
        };
        self.console.print(&format!("Loaded @ A${start:04X},L${n:04X}"));
        for k in 0..n as u16 {
            let ad = start.wrapping_add(k);
            let v = bytes[k as usize];
            match bank {
                Some(0) => m.poke_main(ad, v),
                Some(_) => m.poke_aux(ad, v),
                None => m.write(ad, v),
            }
        }
        upd::ALL
    }

    pub(crate) fn cmd_text_save(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() > 1 {
            return self.help_for(Cmd::TextSave);
        }
        let name = if a.n() == 1 && a.cooked[1].quoted() {
            a.text(1).to_string()
        } else if m.board().video.col80 {
            "AppleWin_Text80.txt".into()
        } else {
            "AppleWin_Text40.txt".into()
        };
        let text = apple2_core::text_screen::text_screen(m).join("\n") + "\n";
        match self.host.write(&name, text.as_bytes()) {
            Ok(()) => self.console.print(&format!("Saved: {name}")),
            Err(_) => self.console.print("Error opening file."),
        }
        upd::CONSOLE_DISPLAY
    }

    // ---- disassembler data ----

    /// `SymbolUpdate` (Debugger_Symbols.cpp:892).
    pub(crate) fn symbol_update(&mut self, t: Table, name: &str, addr: u16, remove: bool, update: bool) {
        if name.len() >= crate::symbols::MAX_SYMBOLS_LEN {
            self.console.print(&format!("{CHC_ERROR}Error: {CHC_DEFAULT}Symbol length {CHC_NUM_DEC}{} {CHC_ARG_SEP}> {CHC_NUM_DEC}{} ", name.len(), crate::symbols::MAX_SYMBOLS_LEN));
            return;
        }
        if remove {
            if self.symbols.remove(t, name) {
                self.console.print(" Removing symbol.");
            } else {
                self.console.print(" Symbol not in table.");
            }
        }
        if update {
            let prev = self.symbols.set(t, name, addr);
            if let Some(p) = prev {
                self.console.print_level(
                    OutputLevel::Info,
                    &format!(" Updating {CHC_SYMBOL}{name}{CHC_DEFAULT} from {CHC_ARG_SEP}${CHC_ADDRESS}{p:04X}{CHC_DEFAULT} to {CHC_ARG_SEP}${CHC_ADDRESS}{addr:04X}{CHC_DEFAULT}"),
                );
            }
            self.console.print_level(OutputLevel::Info, &format!(" Added: {CHC_ARG_SEP}${CHC_ADDRESS}{addr:04X}{CHC_DEFAULT} {CHC_SYMBOL}{name}"));
        }
    }

    fn auto_symbol_name(kind: Nop, start: u16) -> String {
        match kind {
            Nop::Address => format!("A_{start:04X}"),
            Nop::Fac => format!("F_{start:04X}"),
            Nop::StringAscii | Nop::StringApple => format!("T_{start:04X}"),
            Nop::Word1 | Nop::Word2 | Nop::Word4 => format!("W_{start:04X}"),
            _ => format!("B_{start:04X}"),
        }
    }

    /// `DB DW DA ASC DF` and the DB2/4/8, DW2/4 variants (`_CmdDisasmDataDefByteX`...).
    pub(crate) fn cmd_define_data(&mut self, m: &mut Machine, a: &Args, kind: Nop) -> Update {
        let n = a.n();
        let cmd_for_help = self.last_cmd.unwrap_or(Cmd::DefineDataByte1);
        let max = match kind {
            Nop::Byte1 | Nop::Byte2 | Nop::Byte4 | Nop::Byte8 | Nop::Fac => 5,
            _ => 4,
        };
        if n > max {
            return self.help_for(cmd_for_help);
        }
        let mut i_arg = 2usize;
        if n == 3 && a.get(2).map(|t| t.is_op(":")).unwrap_or(false) {
            i_arg = 1;
        }
        // _GetDataRange
        let mut nlen: i32 = 0;
        let address: u16;
        if n < 1 {
            address = self.disasm_cur;
        } else {
            if a.get(i_arg).map(|t| t.is_op("=")).unwrap_or(false) {
                i_arg += 1;
            }
            let (a1, a2, rt) = range_get(a, i_arg);
            if let Some((_, l)) = range_end_len(rt, a1, a2) {
                address = a1;
                nlen = l as i32 - 1;
            } else if n > 1 {
                address = a.value(i_arg);
            } else {
                address = a.value(1);
            }
        }
        if kind == Nop::Word1 && !nlen & 1 != 0 {
            nlen += 1;
        }
        let start = address;
        let end = address.wrapping_add(nlen as u16);
        // Symbol name: the first argument unless it is a bare range.
        let name = if n > 1 && !a.get(2).map(|t| t.is_op(":")).unwrap_or(false) {
            a.text(1).chars().take(crate::symbols::MAX_SYMBOLS_LEN - 1).collect::<String>()
        } else {
            Self::auto_symbol_name(kind, start)
        };
        self.symbol_update(Table::Asm, &name, address, false, true);
        let (directive, lookup) = match kind {
            Nop::Fac => ("df", false),
            Nop::Address => ("da", true),
            Nop::Word1 | Nop::Word2 | Nop::Word4 => ("dw", false),
            Nop::StringApple | Nop::StringAscii => ("ASC", false),
            _ => ("db", false),
        };
        let d = DisasmData { symbol: name, start, end, target: 0, directive, kind, symbol_lookup: lookup, removed: false };
        match self.data.find_mut(address) {
            Some(existing) => *existing = d,
            None => self.data.add(d),
        }
        let _ = m;
        upd::DISASM | upd::CONSOLE_DISPLAY
    }

    /// `X [addr[:end]]`: back to code (`CmdDisasmDataDefCode`).
    pub(crate) fn cmd_disasm_code(&mut self, m: &mut Machine, a: &Args) -> Update {
        let n = a.n();
        if !(n <= 2 || n == 4 || n == 3) {
            return self.help_for(Cmd::DisasmCode);
        }
        let i_arg = if n == 3 { 1 } else { 2 };
        // _GetDataRange with n < 1 uses the cursor
        let (start, end) = if n < 1 {
            (self.disasm_cur, self.disasm_cur)
        } else {
            let mut ia = i_arg;
            if a.get(ia).map(|t| t.is_op("=")).unwrap_or(false) {
                ia += 1;
            }
            let (a1, a2, rt) = range_get(a, ia);
            match range_end_len(rt, a1, a2) {
                Some((e, _)) => (a1, e),
                None if n > 1 => (a.value(ia), a.value(ia)),
                None => (a.value(1), a.value(1)),
            }
        };
        let mut addr = start as u32;
        while addr <= end as u32 {
            let ad = addr as u16;
            if let Some(idx) = self.data.0.iter().position(|d| !d.removed && ad >= d.start && ad <= d.end) {
                let (ds, de) = (self.data.0[idx].start, self.data.0[idx].end);
                if ad <= ds && end >= de {
                    self.data.0[idx].removed = true;
                } else if ad <= ds && start < de {
                    self.data.0[idx].start = ad.wrapping_add(1);
                } else if ad > ds && end >= de {
                    self.data.0[idx].end = ad.wrapping_sub(1);
                } else {
                    let mut split = self.data.0[idx].clone();
                    self.data.0[idx].end = ad.wrapping_sub(1);
                    split.start = end.wrapping_add(1);
                    split.symbol = Self::auto_symbol_name(split.kind, split.start);
                    self.symbol_update(Table::Asm, &split.symbol.clone(), split.start, false, true);
                    self.data.add(split);
                }
            }
            addr += 1;
        }
        self.goto(m, self.disasm_cur);
        upd::DISASM | upd::CONSOLE_DISPLAY
    }

    /// `B`: list data blocks.
    pub(crate) fn cmd_disasm_list(&mut self) -> Update {
        let lines: Vec<String> = self
            .data
            .iter()
            .map(|d| {
                let sym = if d.symbol.is_empty() { "???".to_string() } else { d.symbol.clone() };
                format!("{CHC_CATEGORY}{} {CHC_SYMBOL}{sym:>51} {CHC_ADDRESS}{:04X}{CHC_ARG_SEP}:{CHC_ADDRESS}{:04X}", d.kind.name(), d.start, d.end)
            })
            .collect();
        for l in lines {
            self.console.print(&l);
        }
        upd::DISASM | upd::CONSOLE_DISPLAY
    }
}
