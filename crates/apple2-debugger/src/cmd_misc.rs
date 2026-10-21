//! Config, output, scripts, windows, views (Debug.cpp:2876-3115, 4397-4516, 6408-6745, 7623-7860,
//! 9487-9473) and the debugger start-up sequence.

use apple2_core::Machine;

use crate::args::*;
use crate::cmdtable::*;
use crate::color::{DebugColor, Scheme};
use crate::console::*;
use crate::debugger::*;
use crate::disasm::*;

impl Debugger {
    /// `DebugInitialize` tail: symbols, auto-run script, message of the day.
    pub fn initialize(&mut self, m: &mut Machine) {
        self.startup_symbols(m);
        if let Ok(b) = self.host.read("DebuggerAutoRun.txt") {
            let text = String::from_utf8_lossy(&b).into_owned();
            self.console.print(&format!("{CHC_INFO}Run script: "));
            self.console
                .print(&format!("{CHC_PATH}DebuggerAutoRun.txt"));
            self.run_script_text(m, &text);
        } else {
            self.run_script_text(m, include_str!("../symbols/DebuggerAutoRun.txt"));
        }
        self.console.level = OutputLevel::All;
        self.cmd_motd(m);
    }

    /// Runs the lines of a script through the command processor (no echo).
    pub fn run_script_text(&mut self, m: &mut Machine, text: &str) {
        if self.script_depth > 8 {
            self.console.error("Script nesting too deep");
            return;
        }
        self.script_depth += 1;
        for line in text.lines() {
            let l = line.trim_end_matches('\r');
            if self.echo_scripts && !l.trim().is_empty() {
                self.console.print(&format!("{CHC_DEFAULT}{l}"));
            }
            self.execute_line(m, l, false);
        }
        self.script_depth -= 1;
    }

    pub(crate) fn cmd_run(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() != 1 {
            return self.help_for(Cmd::OutputRun);
        }
        let name = a.text(1).to_string();
        match self.host.read(&name) {
            Ok(b) => {
                self.console.print(&format!("{CHC_INFO}Run script: "));
                self.console.print(&format!("{CHC_PATH}{name}"));
                let text = String::from_utf8_lossy(&b).into_owned();
                self.run_script_text(m, &text);
            }
            Err(_) => {
                self.console
                    .print(&format!("{CHC_WARNING}Couldn't load script:"));
                self.console.print(&format!("{CHC_STRING}{name}"));
            }
        }
        upd::ALL
    }

    // ---- config ----

    pub(crate) fn cmd_config_color(&mut self, a: &Args, cmd: Cmd) -> Update {
        let scheme = match cmd {
            Cmd::ConfigColor => Scheme::Color,
            Cmd::ConfigMonochrome => Scheme::Mono,
            _ => Scheme::Bw,
        };
        if a.n() == 0 {
            self.colors.scheme = scheme;
            return upd::ALL;
        }
        if a.n() > 4 {
            return self.help_last();
        }
        let first = a.text(1).to_ascii_uppercase();
        if first == "RESET" {
            self.colors.reset();
            self.console.print(" Resetting colors.");
            return upd::ALL;
        }
        if first == "SAVE" || first == "LOAD" {
            return upd::ALL;
        }
        let i = a.value(1) as usize;
        if i >= DebugColor::COUNT {
            return self.help_last();
        }
        let c = DebugColor::ALL[i];
        if a.n() == 1 {
            let (r, g, b) = self.colors.get(c);
            self.console
                .print(&format!(" Color {i:01X}: {r:02X} {g:02X} {b:02X}"));
            return upd::CONSOLE_DISPLAY;
        }
        if a.n() == 4 {
            self.colors.set(
                scheme,
                c,
                (a.value(2) as u8, a.value(3) as u8, a.value(4) as u8),
            );
            return upd::ALL;
        }
        self.help_last()
    }

    pub(crate) fn cmd_config_disasm(&mut self, a: &Args) -> Update {
        let n = a.n();
        if n > 2 {
            return self.help_for(Cmd::ConfigDisasm);
        }
        let onoff = |b: bool| if b { "ON" } else { "OFF" };
        let show = n == 0;
        let params = [
            "BRANCH", "CLICK", "COLON", "OPCODE", "POINTER", "SPACES", "TARGET",
        ];
        let names: Vec<String> = if show {
            params.iter().map(|s| s.to_string()).collect()
        } else {
            vec![a.text(1).to_ascii_uppercase()]
        };
        for name in names {
            // fuzzy: unique prefix
            let Some(p) = params.iter().find(|p| p.starts_with(name.as_str())) else {
                return self.help_for(Cmd::ConfigDisasm);
            };
            let set = !show && n > 1;
            let v = a.value(2);
            match *p {
                "BRANCH" => {
                    if set {
                        self.cfg.branch_type = (v as u8).min(2);
                    } else {
                        self.console
                            .print(&format!("Branch Type: {}", self.cfg.branch_type));
                    }
                }
                "CLICK" => self.console.print(&format!(
                    "Click: {} = Left click",
                    if set { v & 7 } else { 4 }
                )),
                "COLON" => {
                    if set {
                        self.cfg.address_colon = v != 0;
                    } else {
                        self.console
                            .print(&format!("Colon: {}", onoff(self.cfg.address_colon)));
                    }
                }
                "OPCODE" => {
                    if set {
                        self.cfg.opcodes_view = v != 0;
                    } else {
                        self.console
                            .print(&format!("Opcodes: {}", onoff(self.cfg.opcodes_view)));
                    }
                }
                "POINTER" => {
                    if set {
                        self.info_target_pointer = v != 0;
                    } else {
                        self.console.print(&format!(
                            "Info Target Pointer: {}",
                            onoff(self.info_target_pointer)
                        ));
                    }
                }
                "SPACES" => {
                    if set {
                        self.cfg.opcode_spaces = v != 0;
                    } else {
                        self.console
                            .print(&format!("Opcode spaces: {}", onoff(self.cfg.opcode_spaces)));
                    }
                }
                _ => {
                    if set {
                        self.cfg.targets = (v as u8).min(DISASM_TARGET_BOTH);
                    } else {
                        self.console.print(&format!("Target: {}", self.cfg.targets));
                    }
                }
            }
        }
        upd::CONSOLE_DISPLAY | upd::DISASM
    }

    /// Everything `SAVE` writes: breakpoints, bookmarks, watches, zero-page pointers, symbols user.
    pub fn config_script(&self) -> String {
        let mut s = String::new();
        s += &self.breakpoints_script();
        s += "// Bookmarks\nBMC *\n";
        for (i, b) in self.bookmarks.iter().enumerate() {
            if let Some(a) = b {
                s += &format!("BMA {i:x} {a:04X}\n");
            }
        }
        s += "// Watches\nWC *\n";
        for (i, b) in self.watches.iter_set() {
            s += &format!("WA {i:x} {:04X}\n", b.address);
        }
        s += "// Zero Page Pointers\nZPC *\n";
        for (i, b) in self.zero_page.iter_set() {
            s += &format!("ZPA {i:x} {:02X}\n", b.address);
        }
        s += &format!("DISASM BRANCH {}\nDISASM COLON {}\nDISASM OPCODE {}\nDISASM SPACES {}\nDISASM TARGET {}\n",
            self.cfg.branch_type, self.cfg.address_colon as u8, self.cfg.opcodes_view as u8, self.cfg.opcode_spaces as u8, self.cfg.targets);
        s
    }

    pub(crate) fn cmd_config_save(&mut self, _m: &mut Machine, a: &Args) -> Update {
        let text = self.config_script();
        let name = if a.n() > 0 {
            a.text(1).to_string()
        } else {
            "Debugger.cfg".to_string()
        };
        match self.host.write(&name, text.as_bytes()) {
            Ok(()) => self.console.print(&format!("Saved: {name}")),
            Err(e) => self.console.error(&format!("Error saving config: {e}")),
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_config_load(&mut self, m: &mut Machine, a: &Args) -> Update {
        let name = if a.n() > 0 {
            a.text(1).to_string()
        } else {
            "Debugger.cfg".to_string()
        };
        match self.host.read(&name) {
            Ok(b) => {
                let t = String::from_utf8_lossy(&b).into_owned();
                self.run_script_text(m, &t);
            }
            Err(_) => self.console.error(&format!("Couldn't load config: {name}")),
        }
        upd::ALL
    }

    pub(crate) fn cmd_pwd(&mut self, a: &Args) -> Update {
        if a.n() != 0 {
            return self.help_for(Cmd::ConfigGetDebugDir);
        }
        let d = self.host.dir();
        self.console.print(&format!("Path: {d}"));
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_cd(&mut self, a: &Args) -> Update {
        if a.n() > 1 {
            return self.help_for(Cmd::ConfigSetDebugDir);
        }
        if a.n() == 1 {
            let p = a.text(1).to_string();
            if !self.host.set_dir(&p) {
                self.console.error(&format!("Invalid directory: {p}"));
            }
        }
        self.cmd_pwd(&Args::default())
    }

    // ---- output ----

    pub(crate) fn cmd_calc(&mut self, a: &Args) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::OutputCalc);
        }
        let v = a.value(1);
        let c = v as u8;
        let hi = c > 0x7F;
        let lo = (c & 0x7F) < 0x20;
        let ch = printable(c);
        let mut bits = 0u32;
        for i in 0..8 {
            if (v >> i) & 1 != 0 {
                bits |= 1 << (i * 4);
            }
        }
        let mut s = format!("${v:04X}  0z{bits:08X}  {v:5}  '{ch}' ");
        if hi || lo {
            s.push('(');
            s += if hi && lo {
                "High Ctrl"
            } else if hi {
                "High"
            } else {
                "Ctrl"
            };
            s.push(')');
        }
        self.console.print(&s);
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_echo(&mut self, _a: &Args, line: &str) -> Update {
        let rest = Args::rest_of(line);
        let t = tokenize(line);
        if t.get(1).map(|x| x.kind == Kind::Quoted2).unwrap_or(false) {
            let s = t[1].text.clone();
            self.console.print(&s);
        } else if !rest.is_empty() {
            self.console.print(rest);
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_log(&mut self, a: &Args) -> Update {
        let names = [
            ("NONE", OutputLevel::None),
            ("ERROR", OutputLevel::Error),
            ("WARN", OutputLevel::Warn),
            ("INFO", OutputLevel::Info),
            ("DEFAULT", OutputLevel::Default),
            ("ALL", OutputLevel::All),
            ("OFF", OutputLevel::None),
            ("ON", OutputLevel::All),
        ];
        if a.n() > 1 {
            return self.help_for(Cmd::OutputLog);
        }
        if a.n() == 1 {
            let up = a.text(1).to_ascii_uppercase();
            match names.iter().find(|(n, _)| *n == up) {
                Some((_, l)) => self.console.level = *l,
                None => {
                    self.console.print(&format!("{CHC_ERROR}Invalid parameter"));
                    return upd::CONSOLE_DISPLAY;
                }
            }
        }
        let cur = names
            .iter()
            .find(|(_, l)| *l == self.console.level)
            .map(|x| x.0)
            .unwrap_or("?");
        self.console.print(&format!(
            "{CHC_INFO}Verbosity level set to {CHC_COMMAND}{cur}"
        ));
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_print(&mut self, a: &Args) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::OutputPrint);
        }
        let mut s = String::new();
        let mut i = 1;
        while i <= a.n() {
            if a.cooked[i].kind == Kind::Quoted2 {
                s += a.text(i);
            } else {
                s += &format!("{:04X}", a.value(i));
            }
            i += 1;
            if i <= a.n() && !a.cooked[i].is_op(",") {
                return self.help_for(Cmd::OutputPrint);
            }
            i += 1;
        }
        if !s.is_empty() {
            self.console.print(&s);
        }
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_printf(&mut self, a: &Args) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::OutputPrintf);
        }
        let values: Vec<u16> = (1..=a.n())
            .filter(|i| a.cooked[*i].kind != Kind::Quoted2 && !a.cooked[*i].is_op(","))
            .map(|i| a.value(i))
            .collect();
        let mut out = String::new();
        let mut iv = 0usize;
        let mut i = 1;
        while i <= a.n() {
            if a.cooked[i].kind == Kind::Quoted2 {
                let mut state = 0u8; // 0 literal, 1 type, 2 escape
                for c in a.text(i).chars() {
                    match state {
                        0 => match c {
                            '\\' => state = 2,
                            '%' => state = 1,
                            _ => out.push(c),
                        },
                        2 => {
                            if c == 'n' || c == 'r' {
                                out.push('\n');
                                state = 0;
                            }
                        }
                        _ => {
                            if iv >= values.len() {
                                self.console
                                    .print(&format!("Error: Missing value arg: {}", iv + 1));
                                return upd::CONSOLE_DISPLAY;
                            }
                            match c {
                                'x' | 'X' => {
                                    out += &format!("{:04X}", values[iv]);
                                    iv += 1;
                                }
                                'd' | 'D' => {
                                    out += &format!("{}", values[iv]);
                                    iv += 1;
                                }
                                'z' | 'Z' => {
                                    for b in (0..8).rev() {
                                        out.push(if (values[iv] >> b) & 1 != 0 {
                                            '1'
                                        } else {
                                            '0'
                                        });
                                    }
                                    iv += 1;
                                }
                                'c' => {
                                    out.push(values[iv] as u8 as char);
                                    iv += 1;
                                }
                                _ => out.push(c),
                            }
                            state = 0;
                        }
                    }
                }
            } else if a.cooked[i].is_op(",") {
                i += 1;
                if i > a.n() {
                    return self.help_for(Cmd::OutputPrintf);
                }
            } else {
                return self.help_for(Cmd::OutputPrintf);
            }
            i += 1;
        }
        if !out.is_empty() {
            self.console.print(&out);
        }
        upd::CONSOLE_DISPLAY
    }

    // ---- views and windows ----

    pub(crate) fn cmd_view(&mut self, cmd: Cmd) -> Update {
        use Cmd::*;
        use OutputView::*;
        let v = match cmd {
            ViewText4x => Text40(None),
            ViewText41 => Text40(Some(1)),
            ViewText42 => Text40(Some(2)),
            ViewText8x => Text80(None),
            ViewText81 => Text80(Some(1)),
            ViewText82 => Text80(Some(2)),
            ViewGrx => Gr(None),
            ViewGr1 => Gr(Some(1)),
            ViewGr2 => Gr(Some(2)),
            ViewDgrx => Dgr(None),
            ViewDgr1 => Dgr(Some(1)),
            ViewDgr2 => Dgr(Some(2)),
            ViewHgrx => Hgr(None),
            ViewHgr1 => Hgr(Some(1)),
            ViewHgr2 => Hgr(Some(2)),
            ViewHgr0 => HgrPseudo(0),
            ViewHgr3 => HgrPseudo(3),
            ViewHgr4 => HgrPseudo(4),
            ViewHgr5 => HgrPseudo(5),
            ViewHgr6 => HgrPseudo(6),
            ViewHgr7 => HgrPseudo(7),
            ViewHgr8 => HgrPseudo(8),
            ViewDhgrx => Dhgr(None),
            ViewDhgr1 => Dhgr(Some(1)),
            ViewDhgr2 => Dhgr(Some(2)),
            _ => Shr,
        };
        self.output_view = Some(v);
        upd::NOTHING
    }

    pub(crate) fn cmd_window(&mut self, m: &mut Machine, a: &Args, cmd: Cmd) -> Update {
        let _ = m;
        let go = |me: &mut Debugger, w: Window| {
            if me.window != w {
                me.last_window = me.window;
            }
            me.window = w;
        };
        match cmd {
            Cmd::WindowCode => {
                go(self, Window::Code);
                upd::ALL
            }
            Cmd::WindowData => {
                go(self, Window::Data);
                upd::ALL
            }
            Cmd::WindowConsole => {
                go(self, Window::Console);
                upd::ALL
            }
            Cmd::WindowOutput => {
                self.output_view = Some(OutputView::Current);
                upd::NOTHING
            }
            Cmd::WindowCode2 | Cmd::WindowData2 => {
                if matches!(self.window, Window::Code | Window::Data) {
                    self.split = !self.split;
                    upd::DISASM
                } else {
                    upd::CONSOLE_DISPLAY
                }
            }
            Cmd::Window => {
                if a.n() == 0 {
                    return self.help_for(Cmd::Window);
                }
                match a.text(1).to_ascii_uppercase().as_str() {
                    "CODE" => go(self, Window::Code),
                    "CONSOLE" => go(self, Window::Console),
                    "DATA" => go(self, Window::Data),
                    "SOURCE" | "SRC" => go(self, Window::Source),
                    "SYMBOLS" | "SYM" => go(self, Window::Symbols),
                    _ => return self.help_for(Cmd::Window),
                }
                upd::ALL
            }
            _ => upd::CONSOLE_DISPLAY,
        }
    }

    // ---- version / motd ----

    pub(crate) fn cmd_version(&mut self) -> Update {
        self.console.print(&format!(
            "  Emulator:  {CHC_SYMBOL}AppleII-Rust {}{CHC_DEFAULT}    Debugger: {CHC_SYMBOL}2.9.2.0{CHC_DEFAULT} (port of AppleWin's)",
            env!("CARGO_PKG_VERSION")
        ));
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_motd(&mut self, m: &mut Machine) -> Update {
        let _ = m;
        self.console
            .print("`9 Apple ][ ][+ //e `7Emulator`9 (Rust)");
        self.cmd_version();
        self.cmd_symbols_info(&Args::default(), None);
        self.console.print(&format!(
            "  '{CHC_KEY}Ctrl ~'{CHC_DEFAULT} console, '{CHC_COMMAND}HELP{CHC_DEFAULT}' (specific), '{CHC_COMMAND}?{CHC_DEFAULT}' (all)"
        ));
        upd::ALL
    }
}
