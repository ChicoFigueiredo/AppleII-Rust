//! Help (Debugger_Help.cpp): `?`, `HELP [command | category | *]`, and the per-command usage
//! text (`helptext.rs`, generated from the `switch (iCommand)` of `CmdHelpSpecific`).

use crate::args::Args;
use crate::cmdtable::*;
use crate::console::*;
use crate::debugger::*;

/// Help categories (`_PARAM_HELPCATEGORIES`), with the command ranges they list.
pub const CATEGORIES: [&str; 15] = [
    "BOOKMARKS", "BREAKPOINTS", "CONFIG", "CPU", "FLAGS", "HELP", "KEYBOARD", "MEMORY", "OUTPUT", "OPERATORS", "RANGE", "SYMBOLS", "VIEW", "WATCHES", "WINDOW",
];

/// `Colorize` (Debugger_Help.cpp:292): colors the markup of a plain help line.
pub fn colorize(src: &str) -> String {
    let mut out = String::new();
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    let starts = |i: usize, s: &str| b[i..].iter().collect::<String>().starts_with(s);
    while i < b.len() {
        let mut matched = false;
        for h in ["Usage:", "See also:", "Note:", "Total:", "Examples:"] {
            if starts(i, h) {
                out += &format!("{CHC_USAGE}{}{CHC_ARG_SEP}:{CHC_DEFAULT}", &h[..h.len() - 1]);
                i += h.len();
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }
        let c = b[i];
        match c {
            '[' | ']' => {
                out += &format!("{CHC_ARG_OPT}{c}{CHC_DEFAULT}");
                i += 1;
            }
            '<' | '>' => {
                out += &format!("{CHC_ARG_MAND}{c}{CHC_DEFAULT}");
                i += 1;
            }
            '|' | '\'' => {
                out += &format!("{CHC_ARG_SEP}{c}{CHC_DEFAULT}");
                i += 1;
            }
            '$' if b.get(i + 1).map(|x| x.is_ascii_hexdigit()).unwrap_or(false) => {
                out += &format!("{CHC_ARG_SEP}${CHC_DEFAULT}");
                i += 1;
                let mut j = i;
                while j < b.len() && b[j].is_ascii_hexdigit() {
                    j += 1;
                }
                out += &format!("{CHC_NUM_HEX}{}{CHC_DEFAULT}", b[i..j].iter().collect::<String>());
                i = j;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

impl Debugger {
    /// Name of the first table entry for `cmd`.
    pub fn command_name(cmd: Cmd) -> &'static str {
        COMMANDS.iter().find(|e| e.cmd == cmd).map(|e| e.name).unwrap_or("?")
    }

    /// `Help_Arg_1` / `CmdHelpSpecific` for one command: summary line then usage.
    pub fn help_for(&mut self, cmd: Cmd) -> Update {
        let entry = COMMANDS.iter().find(|e| e.cmd == cmd);
        if let Some(e) = entry {
            if let Some(h) = e.help {
                self.console.print(&format!("{CHC_COMMAND}{}{CHC_ARG_SEP}, {CHC_DEFAULT}{h}", e.name));
            }
        }
        if let Some(lines) = crate::helptext::usage(cmd) {
            let name = entry.map(|e| e.name).unwrap_or("");
            for (colorized, l) in lines {
                let l = l.replace("{cmd}", name);
                self.console.print(&if *colorized { colorize(&l) } else { l });
            }
        }
        upd::CONSOLE_DISPLAY
    }

    /// `Help_Categories`
    fn help_categories(&mut self) {
        let mut text = format!("{CHC_USAGE}Usage{CHC_DEFAULT}: {CHC_ARG_OPT}[ {CHC_ARG_MAND}< ");
        let mut plain = 11usize;
        for (i, c) in CATEGORIES.iter().enumerate() {
            if plain + c.len() >= CONSOLE_WIDTH - 1 {
                self.console.print(&text);
                text = "    ".into();
                plain = 4;
            }
            text += &format!("{CHC_COMMAND}{c}");
            plain += c.len();
            if i + 1 < CATEGORIES.len() {
                text += &format!("{CHC_ARG_SEP} | ");
                plain += 3;
            }
        }
        text += &format!("{CHC_ARG_MAND} >{CHC_ARG_OPT} ]");
        self.console.print(&text);
        self.console.print(&format!(
            "{CHC_USAGE}Notes{CHC_DEFAULT}: {CHC_ARG_MAND}<>{CHC_DEFAULT} = mandatory, {CHC_ARG_OPT}[]{CHC_DEFAULT} = optional, {CHC_ARG_SEP}|{CHC_DEFAULT} argument option"
        ));
    }

    fn help_operators(&mut self) {
        let u = CHC_USAGE;
        let d = CHC_DEFAULT;
        for l in [
            format!("  Operators: ({u}Math{d})"),
            format!("    {u}+{d}   Addition"),
            format!("    {u}-{d}   Subtraction"),
            format!("    {u}*{d}   Multiplication"),
            format!("    {u}/{d}   Division"),
            format!("    {u}%{d}   Modulas or Remainder"),
            format!("  Operators: ({u}Bit Wise{d})"),
            format!("    {u}&{d}   Bit-wise and (AND)"),
            format!("    {u}|{d}   Bit-wise or  (OR )"),
            format!("    {u}^{d}   Bit-wise exclusive-or (EOR/XOR)"),
            format!("    {u}!{d}   Bit-wise negation (NOT)"),
            format!("  Operators: ({u}Input{d})"),
            format!("    {u}@{d}   next number refers to search results"),
            format!("    {u}\"{d}   Designate string in ASCII format"),
            format!("    {u}'{d}   Desginate string in High-Bit apple format"),
            format!("    {u}${d}   Designate number/symbol"),
            format!("    {u}#{d}   Designate number in hex"),
            format!("  Operators: ({u}Range{d})"),
            format!("    {u},{d}   range separator (2nd address is relative)"),
            format!("    {u}:{d}   range separator (2nd address is absolute)"),
            format!("  Operators: ({u}Misc{d})"),
            format!("    {u}//{d}  comment until end of line"),
            format!("  Operators: ({u}Breakpoint{d})"),
            format!("    {u}<= < = != > >= {d}"),
        ] {
            self.console.print(&l);
        }
    }

    fn help_range(&mut self) {
        for l in ["  Where <range> is of the form:", "    address , length   [address,address+length)", "    address : end      [address,end]"] {
            self.console.print(l);
        }
    }

    fn help_keyboard(&mut self) {
        for l in [
            "  Scrolling:", "    Up Arrow", "    Down Arrow", "    Shift + Up Arrow", "    Shift + Down Arrow", "    Page Up", "    Page Down", "    Shift + Page Up",
            "    Shift + Page Down", "  Bookmarks:", "    Ctrl-Shift-#", "    Ctrl-#      ",
        ] {
            self.console.print(l);
        }
    }

    /// Commands of a help category (the unaliased table order).
    fn category_commands(cat: &str) -> Vec<Cmd> {
        let all: Vec<Cmd> = COMMANDS[..NUM_PRIMARY].iter().map(|e| e.cmd).collect();
        let span = |from: Cmd, to: Cmd| -> Vec<Cmd> {
            let a = all.iter().position(|c| *c == from).unwrap_or(0);
            let b = all.iter().position(|c| *c == to).unwrap_or(all.len() - 1);
            all[a..=b].to_vec()
        };
        match cat {
            "BOOKMARKS" => span(Cmd::Bookmark, Cmd::BookmarkSave),
            "BREAKPOINTS" => span(Cmd::BreakInvalid, Cmd::BreakpointChange),
            "CONFIG" => span(Cmd::Benchmark, Cmd::ConfigSetDebugDir),
            "CPU" => span(Cmd::Assemble, Cmd::Unassemble),
            "FLAGS" => span(Cmd::FlagClear, Cmd::FlagSetN),
            "HELP" => span(Cmd::HelpList, Cmd::Motd),
            "MEMORY" => span(Cmd::MemoryCompare, Cmd::MemoryFill),
            "OUTPUT" => span(Cmd::OutputCalc, Cmd::OutputRun),
            "SYMBOLS" => span(Cmd::SymbolsLookup, Cmd::SymbolsList),
            "VIEW" => span(Cmd::ViewText4x, Cmd::ViewDhgr2),
            "WATCHES" => span(Cmd::Watch, Cmd::WatchSave),
            "WINDOW" => span(Cmd::Window, Cmd::WindowOutput),
            _ => vec![],
        }
    }

    /// `HELP [what ...]`
    pub(crate) fn cmd_help(&mut self, a: &Args) -> Update {
        if a.n() == 0 {
            self.help_categories();
            return upd::CONSOLE_DISPLAY;
        }
        if a.text(1) == "*" || a.text(1) == "?" {
            let cmds: Vec<&CmdEntry> = COMMANDS[..NUM_PRIMARY].iter().collect();
            for e in cmds {
                self.help_for(e.cmd);
            }
            return upd::CONSOLE_DISPLAY;
        }
        for i in 1..=a.n() {
            let up = a.text(i).to_ascii_uppercase();
            // exact command first, then category (unique prefix)
            if let crate::commands::Found::One(c) = crate::commands::find_command(&up) {
                if COMMANDS.iter().any(|e| e.name.eq_ignore_ascii_case(&up)) {
                    self.help_for(c);
                    continue;
                }
            }
            let cats: Vec<&&str> = CATEGORIES.iter().filter(|c| c.starts_with(up.as_str())).collect();
            if cats.len() == 1 {
                let cat = **cats.first().unwrap();
                self.console.print(&format!("{CHC_USAGE}Category{CHC_DEFAULT}: {CHC_CATEGORY}{cat}"));
                match cat {
                    "OPERATORS" => self.help_operators(),
                    "RANGE" => self.help_range(),
                    "KEYBOARD" => self.help_keyboard(),
                    _ => {
                        for c in Self::category_commands(cat) {
                            if let Some(e) = COMMANDS.iter().find(|e| e.cmd == c) {
                                if let Some(h) = e.help {
                                    self.console.print(&format!("{CHC_COMMAND}{:>8}{CHC_ARG_SEP}, {CHC_DEFAULT}{h}", e.name));
                                }
                            }
                        }
                    }
                }
                continue;
            }
            match crate::commands::find_command(&up) {
                crate::commands::Found::One(c) => {
                    self.help_for(c);
                }
                crate::commands::Found::Many(v) => {
                    self.console.print(&format!("Ambiguous {CHC_NUM_DEC}{}{CHC_DEFAULT} Commands:", v.len()));
                }
                crate::commands::Found::None => {
                    self.console.error(&format!("Illegal Command: {}", a.text(i)));
                }
            }
        }
        upd::CONSOLE_DISPLAY
    }

    /// `?`: every command name, sorted, wrapped to the console width.
    pub(crate) fn cmd_help_list(&mut self) -> Update {
        let mut names: Vec<&str> = COMMANDS.iter().map(|e| e.name).collect();
        names.sort_unstable();
        names.dedup();
        let max = CONSOLE_WIDTH - 1;
        let mut text = format!("{CHC_USAGE}Commands{CHC_DEFAULT}: ");
        let mut plain = 10usize;
        for n in names {
            if plain + n.len() + 1 >= max {
                self.console.print(&text);
                text = String::new();
                plain = 0;
            }
            text += &format!("{CHC_COMMAND}{n}{CHC_ARG_SEP} ");
            plain += n.len() + 1;
        }
        self.console.print(&text);
        upd::CONSOLE_DISPLAY
    }
}
