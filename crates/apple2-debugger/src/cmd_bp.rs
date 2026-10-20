//! Breakpoint, watch, zero-page-pointer and bookmark commands (Debug.cpp:602-757, 903-2353,
//! 7334-7620, 7871-8058).

use apple2_core::Machine;

use crate::args::*;
use crate::bp::*;
use crate::cmdtable::Cmd;
use crate::console::*;
use crate::debugger::*;

/// `GetFullPrefixAddrForBreakpoint` (Debug.cpp:2103), without color escapes except the address.
pub fn prefix_addr_text(p: &Prefix, addr: u16, padding: bool) -> String {
    let mut pad = 1usize;
    let mut s = String::new();
    if let Some(sl) = p.slot {
        s += &format!("s{sl}/");
    } else {
        pad += 3;
    }
    if let Some(b) = p.bank {
        if b < 0x100 {
            s += &format!("{b:02X}/");
        } else {
            s += &format!("{b:03X}/");
            pad -= 1;
        }
    } else {
        pad += 3;
    }
    if let Some(l) = p.lang_card {
        s += &format!("l{l}/");
    } else {
        pad += 3;
    }
    if p.is_rom {
        s += "ROM/";
        pad = 6;
    }
    let mut out = String::new();
    if padding {
        out += &" ".repeat(pad);
    }
    out += &format!("{CHC_INFO}{s}{CHC_ADDRESS}{addr:04X}");
    out
}

impl Debugger {
    /// `Range_GetAllPrefixes`: consumes `[sN/][nn/][lN/][ROM/]` starting at argument `i`.
    pub(crate) fn range_prefixes(&mut self, a: &Args, i: &mut usize) -> Result<Prefix, ()> {
        let mut p = Prefix::default();
        while a.get(*i + 1).map(|t| t.is_op("/")).unwrap_or(false) && *i + 1 <= a.n() {
            let t = a.text(*i).to_ascii_lowercase();
            if t.starts_with('s') {
                let n = t.chars().last().and_then(|c| c.to_digit(10)).unwrap_or(99);
                if n > 7 {
                    self.console.error("Address prefix out-of-range. Use slots 0-7.");
                    return Err(());
                }
                p.slot = Some(n as u8);
            } else if t.starts_with('l') {
                let n = t.chars().last().and_then(|c| c.to_digit(10)).unwrap_or(99);
                if !(1..=2).contains(&n) {
                    self.console.error("Address prefix out-of-range. Use lc 1 or 2.");
                    return Err(());
                }
                p.lang_card = Some(n as u8);
            } else if t.starts_with('r') {
                p.is_rom = true;
            } else {
                let v = a.value(*i);
                if v > 0x100 {
                    self.console.error("Address prefix out-of-range. Use bank 0-100 (or 0-7 for Saturn).");
                    return Err(());
                }
                p.bank = Some(v);
            }
            *i += 2;
        }
        if p.is_rom && (p.slot.is_some() || p.bank.is_some() || p.lang_card.is_some()) {
            self.console.error("Address prefix bad: 'r/' not permitted with other prefixes.");
            return Err(());
        }
        Ok(p)
    }

    /// `_CmdBreakpointAddCommonArg`: returns the number of arguments consumed (0 = error).
    fn bp_add_common(&mut self, m: &Machine, a: &Args, i: usize, src: Source, op: Op, temp: bool) -> usize {
        let Some(slot) = self.breakpoints.first_free() else {
            self.console.error("All Breakpoint slots are currently in use.");
            return 0;
        };
        let mut j = i;
        let Ok(prefix) = self.range_prefixes(a, &mut j) else { return 0 };
        let d_prefix = j - i;
        if j > a.n() {
            return 0;
        }
        let (a1, a2, rt) = range_get(a, j);
        let mut d_arg = 1;
        let mut len = 0u32;
        let mut addr = a1;
        if let Some((_, l)) = range_end_len(rt, a1, a2) {
            len = l;
            d_arg = 3;
        }
        if len == 0 {
            len = 1;
        }
        let mut len = len.min(0x10000);
        if src == Source::Video {
            let lines = m.video_lines();
            if addr as u32 >= lines {
                addr = (lines - 1) as u16;
            }
            if addr as u32 + len >= lines {
                len = lines - addr as u32;
            }
        }
        self.breakpoints.add(slot, src, op, addr, len, temp, prefix);
        self.sync_watch();
        d_prefix + d_arg
    }

    pub(crate) fn cmd_bp_add_pc(&mut self, m: &mut Machine, a: &Args) -> Update {
        // `Args` with no operands defaults to the cursor address.
        let mut a = a.clone();
        if a.n() == 0 {
            let mut c = a.cooked[0].clone();
            c.text = "BPX".into();
            let mut v = c.clone();
            v.value = self.disasm_cur;
            v.text = format!("{:04X}", self.disasm_cur);
            v.kind = Kind::Text;
            a.cooked = vec![c, v];
        }
        let mut op = Op::Eq;
        let mut i = 1;
        while i <= a.n() {
            if a.cooked[i].is_operator() {
                if let Some(o) = Op::parse(&a.cooked[i].text) {
                    op = o;
                }
                i += 1;
            } else {
                let d = self.bp_add_common(m, &a, i, Source::RegPc, op, false);
                if d == 0 {
                    return self.help_for(Cmd::BreakpointAddPc);
                }
                i += d;
            }
        }
        upd::BREAKPOINTS | upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_bp_add_reg(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::BreakpointAddReg);
        }
        let mut src = Source::RegPc;
        let mut op = Op::Eq;
        let mut i = 1;
        while i <= a.n() {
            let t = a.text(i).to_ascii_uppercase();
            let s = match t.as_str() {
                "A" => Some(Source::RegA),
                "X" => Some(Source::RegX),
                "Y" => Some(Source::RegY),
                "PC" => Some(Source::RegPc),
                "S" | "SP" => Some(Source::RegS),
                "P" => Some(Source::RegP),
                _ => None,
            };
            let o = if a.cooked[i].is_operator() { Op::parse(&t) } else { None };
            if let Some(s) = s {
                src = s;
                i += 1;
            } else if let Some(o) = o {
                op = o;
                i += 1;
            } else {
                let d = self.bp_add_common(m, a, i, src, op, false);
                if d == 0 {
                    return self.help_for(Cmd::BreakpointAddReg);
                }
                i += d;
            }
        }
        upd::ALL
    }

    pub(crate) fn cmd_bp_add_smart(&mut self, m: &mut Machine, a: &Args) -> Update {
        let mut a = a.clone();
        if a.n() == 0 {
            let mut v = a.cooked[0].clone();
            v.value = self.disasm_cur;
            v.text = format!("{:04X}", self.disasm_cur);
            v.kind = Kind::Text;
            a.cooked.push(v);
        }
        if (0xC000..=0xC0FF).contains(&a.value(1)) {
            return self.cmd_bp_add_mem(m, &a, Source::MemRw);
        }
        self.cmd_bp_add_reg(m, &a);
        self.cmd_bp_add_mem(m, &a, Source::MemRw);
        upd::BREAKPOINTS
    }

    pub(crate) fn cmd_bp_add_mem(&mut self, m: &mut Machine, a: &Args, src: Source) -> Update {
        let cmd = match src {
            Source::MemRead => Cmd::BreakpointAddMemr,
            Source::MemWrite => Cmd::BreakpointAddMemw,
            _ => Cmd::BreakpointAddMem,
        };
        if a.n() == 0 {
            return self.help_for(cmd);
        }
        let mut i = 1;
        while i <= a.n() {
            if a.cooked[i].is_operator() && !a.cooked[i].is_op("/") {
                return self.help_for(cmd);
            }
            let d = self.bp_add_common(m, a, i, src, Op::Eq, false);
            if d == 0 {
                return self.help_for(cmd);
            }
            i += d;
        }
        self.attach(m);
        upd::BREAKPOINTS | upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_bp_add_video(&mut self, m: &mut Machine, a: &Args) -> Update {
        let mut i = 1;
        while i <= a.n() {
            let d = self.bp_add_common(m, a, i, Source::Video, Op::Eq, false);
            if d == 0 {
                return self.help_for(Cmd::BreakpointAddVideo);
            }
            i += d;
        }
        upd::BREAKPOINTS | upd::CONSOLE_DISPLAY
    }

    fn bp_none(&mut self) -> Update {
        self.console.print(&format!("There are no {CHC_ARG_SEP}({CHC_CATEGORY}PC{CHC_ARG_SEP}){CHC_DEFAULT} Breakpoints defined."));
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_bp_clear(&mut self, _m: &mut Machine, a: &Args) -> Update {
        if self.breakpoints.count == 0 {
            return self.bp_none();
        }
        if a.n() == 0 {
            self.breakpoints.remove_all();
        } else {
            bwz_clear_via_args(&mut self.breakpoints, a);
        }
        self.sync_watch();
        upd::DISASM | upd::BREAKPOINTS | upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_bp_enable(&mut self, _m: &mut Machine, a: &Args, on: bool) -> Update {
        if self.breakpoints.count == 0 {
            return self.bp_none();
        }
        if a.n() == 0 {
            return self.help_for(if on { Cmd::BreakpointEnable } else { Cmd::BreakpointDisable });
        }
        bwz_enable_via_args(&mut self.breakpoints, a, on);
        self.sync_watch();
        upd::BREAKPOINTS
    }

    pub(crate) fn cmd_bp_change(&mut self, _m: &mut Machine, a: &Args) -> Update {
        if self.breakpoints.count == 0 {
            return self.bp_none();
        }
        if a.n() < 2 {
            return self.help_for(Cmd::BreakpointChange);
        }
        let slot = a.value(1) as usize;
        if slot < MAX_BREAKPOINTS && self.breakpoints.slots[slot].set {
            for i in 2..=a.n() {
                let b = &mut self.breakpoints.slots[slot];
                match a.text(i) {
                    "E" => b.enabled = true,
                    "e" => b.enabled = false,
                    "T" => b.temp = true,
                    "t" => b.temp = false,
                    "S" => b.stop = true,
                    "s" => b.stop = false,
                    _ => return self.help_for(Cmd::BreakpointChange),
                }
            }
            self.sync_watch();
        }
        upd::BREAKPOINTS
    }

    pub(crate) fn cmd_bp_list(&mut self) -> Update {
        if self.breakpoints.count == 0 {
            self.console.print(&format!("  There are no current breakpoints.  (Max: {MAX_BREAKPOINTS})"));
        } else {
            self.bwz_list_all_bp();
        }
        upd::CONSOLE_DISPLAY
    }

    /// `_BWZ_ListAll`
    fn bwz_list_all_bp(&mut self) {
        self.console.print("  ID On Stop Temp HitCounter    Prefix/Addr: End Mem Symbol");
        let lines: Vec<String> = self.breakpoints.iter_set().map(|(i, b)| self.bwz_line(i, b)).collect();
        for l in lines {
            self.console.print(&l);
        }
    }

    fn bwz_line(&self, i: usize, b: &Breakpoint) -> String {
        let sym = self.symbols.find_symbol(b.address).map(|s| s.0.to_string()).unwrap_or_else(|| format!("${:02X}", b.address));
        let mem = match b.source {
            Source::MemRw => "R/W",
            Source::MemRead => "R  ",
            Source::MemWrite => "W  ",
            Source::Video => "Vid",
            _ => "   ",
        };
        let mut pa = prefix_addr_text(&b.prefix, b.address, true);
        if b.length > 1 {
            pa += &format!(":{CHC_ADDRESS}{:04X}", b.address as u32 + b.length - 1);
        } else {
            pa += "     ";
        }
        format!(
            "  #{i:X} {}  {}    {}  {}   {:08X} {pa} {CHC_INFO}{mem}{CHC_SYMBOL} {sym}",
            if b.enabled { 'E' } else { '-' },
            if b.stop { 'S' } else { '-' },
            if b.temp { 'T' } else { '-' },
            if b.hit { '*' } else { ' ' },
            b.hit_count
        )
    }

    /// Text of the commands that recreate the breakpoints (`CmdBreakpointSave`).
    pub fn breakpoints_script(&self) -> String {
        let mut s = String::from("// Breakpoints\nBPC *\n");
        for (i, b) in self.breakpoints.iter_set() {
            let rng = if b.length > 1 { format!("{:04X}:{:04X}", b.address, b.address as u32 + b.length - 1) } else { format!("{:04X}", b.address) };
            let pfx = b.prefix.text();
            s += &match b.source {
                Source::RegPc => format!("BPX {pfx}{rng}\n"),
                Source::MemRw => format!("BPM {pfx}{rng}\n"),
                Source::MemRead => format!("BPMR {pfx}{rng}\n"),
                Source::MemWrite => format!("BPMW {pfx}{rng}\n"),
                Source::Video => format!("BPV {rng}\n"),
                src => format!("BPR {} {} {rng}\n", src.name(), b.op.symbol().trim()),
            };
            if !b.enabled {
                s += &format!("BPD {i:X}\n");
            }
        }
        s
    }

    pub(crate) fn cmd_bp_save(&mut self, a: &Args) -> Update {
        let text = self.breakpoints_script();
        self.save_script(a, &text, Cmd::BreakpointSave)
    }

    /// `ConfigSave_BufferToDisk` for a quoted file name argument.
    pub(crate) fn save_script(&mut self, a: &Args, text: &str, cmd: Cmd) -> Update {
        if a.n() == 0 {
            for l in text.lines() {
                self.console.print(l);
            }
            return upd::CONSOLE_DISPLAY;
        }
        if !a.cooked[1].quoted() {
            return self.help_for(cmd);
        }
        match self.host.write(a.text(1), text.as_bytes()) {
            Ok(()) => self.console.print("Saved."),
            Err(e) => self.console.error(&format!("Error saving: {e}")),
        }
        upd::CONSOLE_DISPLAY
    }

    // ---- BRK / invalid / opcode / interrupt ----

    pub(crate) fn cmd_break_invalid(&mut self, a: &Args) -> Update {
        let n = a.n();
        if n > 2 {
            return self.help_for(Cmd::BreakInvalid);
        }
        let name_state = |on: bool| if on { "ON" } else { "OFF" };
        let last = a.text(n).to_ascii_uppercase();
        let param = match last.as_str() {
            "ON" => Some(true),
            "OFF" => Some(false),
            _ => None,
        };
        let set = |me: &mut Debugger, t: u32, v: bool| {
            if t <= 3 {
                me.break_invalid = (me.break_invalid & !(1 << t)) | ((v as u8) << t);
            }
        };
        let report = |me: &mut Debugger, t: u32, on: bool| {
            if t == 0 {
                me.console.print(&format!("Enter debugger on BRK opcode: {}", name_state(on)));
            } else {
                me.console.print(&format!("Enter debugger on INVALID {t:1X} opcode: {}", name_state(on)));
            }
        };
        if n <= 1 {
            let t = if n == 0 { 0 } else { a.value(1) as u32 };
            if param.is_none() {
                if t > 3 {
                    return self.help_last();
                }
                let on = self.break_invalid & (1 << t) != 0;
                report(self, t, on);
            } else {
                // `BRK ON|OFF` (n == 1 and the arg is the state): applies to BRK
                set(self, 0, param.unwrap());
                report(self, 0, param.unwrap());
            }
            return upd::CONSOLE_DISPLAY;
        }
        // n == 2
        if a.text(1).eq_ignore_ascii_case("ALL") {
            if let Some(on) = param {
                for t in 0..=3 {
                    set(self, t, on);
                }
                self.console.print(&format!("Enter debugger on BRK opcode and INVALID opcodes: {}", name_state(on)));
                return upd::CONSOLE_DISPLAY;
            }
            return self.help_last();
        }
        match param {
            Some(on) if a.value(1) <= 3 => {
                let t = a.value(1) as u32;
                set(self, t, on);
                report(self, t, on);
            }
            _ => return self.help_last(),
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_break_opcode(&mut self, a: &Args) -> Update {
        if a.n() > 1 {
            return self.help_last();
        }
        let mut action = "Current";
        if a.n() == 1 {
            let v = a.value(1);
            self.break_opcode = if v & 0xFF == 0 { None } else { Some(v as u8) };
            action = "Setting";
            if v >= 256 {
                self.console.print(&format!("Warning: clamping opcode: {:02X}", v & 0xFF));
                return upd::CONSOLE_DISPLAY;
            }
        }
        match self.break_opcode {
            None => self.console.print(&format!("{action} Break on Opcode: None")),
            Some(o) => self.console.print(&format!("{action} Break on Opcode: {o:02X} {}", crate::opcodes::OPCODES_65C02[o as usize].mnemonic)),
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_break_on_interrupt(&mut self, a: &Args) -> Update {
        if a.n() > 1 {
            return self.help_last();
        }
        let mut action = "Current";
        if a.n() == 1 {
            match a.text(1).to_ascii_uppercase().as_str() {
                "ON" => self.break_on_interrupt = true,
                "OFF" => self.break_on_interrupt = false,
                _ => return self.help_last(),
            }
            action = "Setting";
        }
        self.console.print(&format!("{action} Break on Interrupt: {}", if self.break_on_interrupt { "Enabled" } else { "Disabled" }));
        upd::CONSOLE_DISPLAY
    }

    // ---- bookmarks ----

    pub fn bookmark_find(&self, addr: u16) -> Option<usize> {
        self.bookmarks.iter().position(|b| *b == Some(addr)).map(|i| i + 1)
    }

    pub(crate) fn cmd_bookmark_add(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            return self.cmd_bookmark_list(m);
        }
        let (idx, addr) = if a.n() > 1 {
            (a.value(1) as usize, a.value(2))
        } else {
            match self.bookmarks.iter().position(|b| b.is_none()) {
                Some(i) => (i, a.value(1)),
                None => (MAX_BOOKMARKS, a.value(1)),
            }
        };
        if idx >= MAX_BOOKMARKS {
            self.console.print(&format!("All bookmarks are currently in use.  (Max: {MAX_BOOKMARKS})"));
            return upd::CONSOLE_DISPLAY;
        }
        for b in &mut self.bookmarks {
            if *b == Some(addr) {
                *b = None;
            }
        }
        self.bookmarks[idx] = Some(addr);
        upd::DISASM | upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_bookmark_clear(&mut self, a: &Args) -> Update {
        for i in 1..=a.n() {
            if a.text(a.n()) == "*" {
                self.bookmarks = [None; MAX_BOOKMARKS];
                break;
            }
            let k = a.value(i) as usize;
            if k < MAX_BOOKMARKS {
                self.bookmarks[k] = None;
            }
        }
        upd::DISASM
    }

    pub(crate) fn cmd_bookmark_goto(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::BookmarkGoto);
        }
        if let Some(Some(addr)) = self.bookmarks.get(a.value(1) as usize).copied() {
            self.disasm_cur_line = 0;
            self.goto(m, addr);
        }
        upd::DISASM
    }

    pub(crate) fn cmd_bookmark_list(&mut self, _m: &mut Machine) -> Update {
        if self.bookmarks.iter().all(|b| b.is_none()) {
            self.console.print(&format!("  There are no current bookmarks.  (Max: {MAX_BOOKMARKS})"));
        } else {
            self.console.print("  ID  Addr  Symbol");
            for (i, b) in self.bookmarks.iter().enumerate() {
                if let Some(a) = b {
                    let sym = self.symbols.find_symbol(*a).map(|s| s.0.to_string()).unwrap_or_default();
                    self.console.print(&format!("  #{i:X} {CHC_ADDRESS}{a:04X}{CHC_SYMBOL} {sym}"));
                }
            }
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_bookmark_save(&mut self, a: &Args) -> Update {
        let mut s = String::from("// Bookmarks\nBMC *\n");
        for (i, b) in self.bookmarks.iter().enumerate() {
            if let Some(addr) = b {
                s += &format!("BMA {i:x} {addr:04X}\n");
            }
        }
        self.save_script(a, &s, Cmd::BookmarkSave)
    }

    // ---- watches ----

    pub(crate) fn cmd_watch_add(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            return self.cmd_watch_list(m);
        }
        let mut i = 1;
        let mut w: Option<usize> = None;
        if a.n() > 1 {
            let k = a.value(1) as usize;
            if k >= MAX_WATCHES {
                self.console.print(&format!("Watch index too big.  (Max: {})", MAX_WATCHES - 1));
                return upd::CONSOLE_DISPLAY;
            }
            w = Some(k);
            i += 1;
        }
        let mut added = false;
        while i <= a.n() {
            if a.text(i).starts_with('v') {
                let k = w.or_else(|| self.watches.first_free()).unwrap_or(MAX_WATCHES);
                if k < MAX_WATCHES {
                    self.watches.add(k, Source::Video, Op::Eq, 0xDEAD, 1, false, Prefix::default());
                    added = true;
                    w = Some(k + 1);
                }
                i += 1;
                continue;
            }
            let addr = a.value(i);
            if (0xC000..=0xC0FF).contains(&addr) {
                self.console.error("You cannot watch an I/O location.");
                return upd::CONSOLE_DISPLAY;
            }
            let k = match w {
                Some(k) => k,
                None => self.watches.first_free().unwrap_or(MAX_WATCHES),
            };
            if k >= MAX_WATCHES && !added {
                self.console.print(&format!("All watches are currently in use.  (Max: {MAX_WATCHES})"));
                return upd::CONSOLE_DISPLAY;
            }
            if k < MAX_WATCHES && self.watches.count < MAX_WATCHES {
                self.watches.add(k, Source::MemRw, Op::Eq, addr, 1, false, Prefix::default());
                added = true;
                w = Some(k + 1);
            }
            i += 1;
        }
        if !added {
            return self.help_for(Cmd::WatchAdd);
        }
        upd::WATCH
    }

    pub(crate) fn cmd_watch_clear(&mut self, a: &Args) -> Update {
        if self.watches.count == 0 {
            self.console.error("There are no watches defined.");
            return upd::CONSOLE_DISPLAY;
        }
        if a.n() == 0 {
            return self.help_for(Cmd::WatchClear);
        }
        bwz_clear_via_args(&mut self.watches, a);
        upd::CONSOLE_DISPLAY | upd::WATCH
    }

    pub(crate) fn cmd_watch_enable(&mut self, a: &Args, on: bool) -> Update {
        if self.watches.count == 0 {
            self.console.error("There are no watches defined.");
            return upd::CONSOLE_DISPLAY;
        }
        if a.n() == 0 {
            return self.help_for(if on { Cmd::WatchEnable } else { Cmd::WatchDisable });
        }
        bwz_enable_via_args(&mut self.watches, a, on);
        upd::WATCH
    }

    pub(crate) fn cmd_watch_list(&mut self, _m: &mut Machine) -> Update {
        if self.watches.count == 0 {
            self.console.print(&format!("  There are no current watches.  (Max: {MAX_WATCHES})"));
        } else {
            self.console.print("  ID  On Stop Temp HitCounter    Prefix/Addr: End Mem Symbol");
            let lines: Vec<String> = self.watches.iter_set().map(|(i, b)| self.bwz_line(i, b)).collect();
            for l in lines {
                self.console.print(&l);
            }
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_watch_save(&mut self, a: &Args) -> Update {
        let mut s = String::from("// Watches\nWC *\n");
        for (i, b) in self.watches.iter_set() {
            if b.source == Source::MemRw {
                s += &format!("WA {i:x} {:04X}\n", b.address);
            }
            if !b.enabled {
                s += &format!("WD {i:x}\n");
            }
        }
        self.save_script(a, &s, Cmd::WatchSave)
    }

    // ---- zero page pointers ----

    pub(crate) fn cmd_zp_add(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            return self.cmd_zp_list(m);
        }
        let mut i = 1;
        let mut z: Option<usize> = None;
        if a.n() > 1 {
            let k = a.value(1) as usize;
            if k >= MAX_ZEROPAGE_POINTERS {
                self.console.print(&format!("Zero page pointer index too big.  (Max: {})", MAX_ZEROPAGE_POINTERS - 1));
                return upd::CONSOLE_DISPLAY;
            }
            z = Some(k);
            i += 1;
        }
        let mut added = false;
        while i <= a.n() {
            let addr = a.value(i);
            if addr > 0xFF {
                self.console.print("Zero page pointer must be in the range: [00..FF].");
                return upd::CONSOLE_DISPLAY;
            }
            let k = match z {
                Some(k) => k,
                None => self.zero_page.first_free().unwrap_or(MAX_ZEROPAGE_POINTERS),
            };
            if k >= MAX_ZEROPAGE_POINTERS && !added {
                self.console.print(&format!("All zero page pointers are currently in use.  (Max: {MAX_ZEROPAGE_POINTERS})"));
                return upd::CONSOLE_DISPLAY;
            }
            if k < MAX_ZEROPAGE_POINTERS && self.zero_page.count < MAX_ZEROPAGE_POINTERS {
                self.zero_page.add(k, Source::MemRw, Op::Eq, addr, 1, false, Prefix::default());
                added = true;
                z = Some(k + 1);
            }
            i += 1;
        }
        if !added {
            return self.help_for(Cmd::ZeropagePointerAdd);
        }
        upd::ZERO_PAGE | upd::CONSOLE_DISPLAY
    }

    fn zp_none(&mut self) -> Update {
        self.console.error(&format!("  There are no current (ZP) pointers.  (Max: {MAX_ZEROPAGE_POINTERS})"));
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_zp_clear(&mut self, a: &Args) -> Update {
        if self.zero_page.count == 0 {
            return self.zp_none();
        }
        if a.n() == 0 {
            return self.help_for(Cmd::ZeropagePointerClear);
        }
        bwz_clear_via_args(&mut self.zero_page, a);
        upd::CONSOLE_DISPLAY | upd::ZERO_PAGE
    }

    pub(crate) fn cmd_zp_enable(&mut self, a: &Args, on: bool) -> Update {
        if a.n() == 0 {
            return self.help_for(if on { Cmd::ZeropagePointerEnable } else { Cmd::ZeropagePointerDisable });
        }
        if self.zero_page.count == 0 {
            return self.zp_none();
        }
        bwz_enable_via_args(&mut self.zero_page, a, on);
        upd::ZERO_PAGE
    }

    pub(crate) fn cmd_zp_list(&mut self, _m: &mut Machine) -> Update {
        if self.zero_page.count == 0 {
            return self.zp_none();
        }
        self.console.print("  ID  On Stop Temp HitCounter    Prefix/Addr: End Mem Symbol");
        let lines: Vec<String> = self.zero_page.iter_set().map(|(i, b)| self.bwz_line(i, b)).collect();
        for l in lines {
            self.console.print(&l);
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_zp_save(&mut self, a: &Args) -> Update {
        let mut s = String::from("// Zero Page Pointers\nZPC *\n");
        for (i, b) in self.zero_page.iter_set() {
            s += &format!("ZPA {i:x} {:02X}\n", b.address);
            if !b.enabled {
                s += &format!("ZPD {i:x}\n");
            }
        }
        self.save_script(a, &s, Cmd::ZeropagePointerSave)
    }

    /// `P0`..`P7` / `ZP0`..`ZP7`: no argument disables, one argument sets and enables.
    pub(crate) fn cmd_zp_pointer(&mut self, _m: &mut Machine, a: &Args, cmd: Cmd) -> Update {
        if a.n() > 1 {
            return self.help_for(cmd);
        }
        let k = (cmd as usize) - (Cmd::ZeropagePointer0 as usize);
        if a.n() == 0 {
            self.zero_page.slots[k].enabled = false;
        } else {
            let was_set = self.zero_page.slots[k].set;
            self.zero_page.add(k, Source::MemRw, Op::Eq, a.value(1) & 0xFF, 1, false, Prefix::default());
            let _ = was_set;
        }
        upd::ZERO_PAGE
    }
}

/// `_BWZ_ClearViaArgs`
pub fn bwz_clear_via_args(t: &mut BpTable, a: &Args) {
    let mut n = a.n();
    while n > 0 {
        if a.text(n) == "*" {
            t.remove_all();
            break;
        }
        let slot = a.value(n) as usize;
        if slot < t.slots.len() {
            t.remove(slot);
        }
        n -= 1;
    }
}

/// `_BWZ_EnableDisableViaArgs`
pub fn bwz_enable_via_args(t: &mut BpTable, a: &Args, on: bool) {
    let mut n = a.n();
    while n > 0 {
        let slot = a.value(n) as usize;
        if a.text(n) == "*" {
            for s in slot..t.slots.len() {
                t.set_enabled(s, on);
            }
        } else if slot < t.slots.len() {
            t.set_enabled(slot, on);
        }
        n -= 1;
    }
}
