//! Command line entry point: `FindCommand`, the Apple monitor shortcuts (`300:A9 00`, `300L`,
//! `FA62G`, `4000<300.3FFM`), the mini-assembler prompt and the dispatch to the handlers.

use apple2_core::Machine;

use crate::args::*;
use crate::cmdtable::*;
use crate::console::*;
use crate::debugger::*;

/// Result of `FindCommand`.
pub enum Found {
    None,
    One(Cmd),
    /// Ambiguous: the matching command ids (primary names).
    Many(Vec<usize>),
}

/// `FindCommand`: exact name wins, otherwise a unique prefix (compared in upper case).
pub fn find_command(name: &str) -> Found {
    if name.is_empty() {
        return Found::None;
    }
    let up = name.to_ascii_uppercase();
    let mut matches: Vec<usize> = Vec::new();
    for (i, e) in COMMANDS.iter().enumerate() {
        if e.name.starts_with(&up) {
            if e.name.eq_ignore_ascii_case(&up) {
                return Found::One(e.cmd);
            }
            if !matches.iter().any(|j| COMMANDS[*j].cmd == e.cmd) {
                matches.push(i);
            }
        }
    }
    match matches.len() {
        0 => Found::None,
        1 => Found::One(COMMANDS[matches[0]].cmd),
        _ => Found::Many(matches),
    }
}

fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

impl Debugger {
    /// Cooks `raw` arguments with the current machine state.
    pub fn cook_args(&self, m: &Machine, raw: Vec<Arg>) -> Result<Args, String> {
        let ctx = CookCtx { regs: crate::mem::regs_of(m), symbols: &self.symbols, mem: m, search_results: &self.search_results };
        let cooked = cook(&raw, &ctx)?;
        Ok(Args { raw, cooked })
    }

    /// The frontend's Enter key: echoes and executes the console input line.
    pub fn input_enter(&mut self, m: &mut Machine) -> Update {
        let line = self.console.submit();
        let mut u = self.execute_line(m, &line, false);
        u |= upd::CONSOLE_DISPLAY | upd::CONSOLE_INPUT;
        u
    }

    /// Executes one command line (no echo).
    pub fn execute(&mut self, m: &mut Machine, line: &str) -> Update {
        self.execute_line(m, line, false)
    }

    pub(crate) fn execute_line(&mut self, m: &mut Machine, line: &str, _echo: bool) -> Update {
        if self.asm_active {
            return self.exec_asm_line(m, line);
        }
        let mut raw = tokenize(line);
        if raw.is_empty() {
            return upd::CONSOLE_DISPLAY;
        }
        let name = raw[0].text.clone();
        let mut found = find_command(&name);
        let nlen = name.chars().count();
        let mut override_cmd: Option<Cmd> = None;
        // Commands that are also valid hex addresses: ####:# [#] ####L ####G #<#.#M
        if (matches!(found, Found::None) || nlen < 6) && nlen > 0 {
            let chars: Vec<char> = name.chars().collect();
            let hex_prefix_ok = nlen >= 2 && chars[..nlen - 1].iter().all(|c| is_hex(*c));
            if hex_prefix_ok {
                let last = chars[nlen - 1].to_ascii_uppercase();
                let addr = hex_prefix(&name[..name.len() - 1]);
                if last == 'G' {
                    m.cpu_mut().pc = addr;
                    self.exit(m);
                    return upd::ALL;
                } else if last == 'L' && !name.eq_ignore_ascii_case("cl") {
                    raw[0].text = name[..name.len() - 1].to_string();
                    raw.insert(0, Arg { text: "U".into(), kind: Kind::Text, value: 0, value_set: false, is_symbol: false, no_reg: false, no_sym: false });
                    override_cmd = Some(Cmd::Unassemble);
                } else if raw.get(1).map(|a| a.is_op(":")).unwrap_or(false) {
                    raw.remove(1);
                    raw.insert(0, Arg { text: "MEB".into(), kind: Kind::Text, value: 0, value_set: false, is_symbol: false, no_reg: false, no_sym: false });
                    override_cmd = Some(Cmd::MemoryEnterByte);
                } else if raw.get(1).map(|a| a.is_op("<")).unwrap_or(false) && raw.len() >= 3 {
                    // dst<src.endM
                    let t = raw[2].text.clone();
                    if let Some((src, rest)) = t.split_once('.') {
                        if let Some(end) = rest.strip_suffix(['M', 'm']) {
                            if !src.is_empty() && !end.is_empty() && src.chars().all(is_hex) && end.chars().all(is_hex) {
                                let (s, e) = (hex_prefix(src), hex_prefix(end));
                                if e < s {
                                    self.console.print(&format!(
                                        "{CHC_WARNING}WARN{CHC_ARG_SEP}:{CHC_DEFAULT} End source address {CHC_ARG_SEP}${CHC_ADDRESS}{e:04X}{CHC_ARG_SEP} <{CHC_DEFAULT} Start source address {CHC_ARG_SEP}${CHC_ADDRESS}{s:04X}{CHC_DEFAULT}.{CHC_INFO} Aborting."
                                    ));
                                    return upd::CONSOLE_DISPLAY;
                                }
                                let mk = |t: &str, k: Kind| Arg { text: t.into(), kind: k, value: 0, value_set: false, is_symbol: false, no_reg: false, no_sym: false };
                                raw = vec![mk("M", Kind::Text), mk(&name, Kind::Text), mk(src, Kind::Text), mk(":", Kind::Operator), mk(end, Kind::Text)];
                                override_cmd = Some(Cmd::MemoryMove);
                            }
                        }
                    }
                }
                if override_cmd.is_some() {
                    found = Found::None;
                }
            }
        }
        let cmd = match (override_cmd, found) {
            (Some(c), _) => c,
            (None, Found::One(c)) => c,
            (None, Found::Many(v)) => {
                self.console.print(&format!("Ambiguous {CHC_NUM_DEC}{}{CHC_DEFAULT} Commands:", v.len()));
                let mut text = format!("{CHC_COMMAND} ");
                for i in v {
                    let n = COMMANDS[i].name;
                    if text.len() + n.len() >= CONSOLE_WIDTH - 1 {
                        self.console.print(&text);
                        text = format!("{CHC_COMMAND} ");
                    }
                    text += n;
                    text.push(' ');
                }
                self.console.print(&text);
                return upd::CONSOLE_DISPLAY;
            }
            (None, Found::None) => {
                self.console.error("Illegal Command");
                return upd::CONSOLE_DISPLAY;
            }
        };
        self.last_cmd = Some(cmd);
        let args = if cmd == Cmd::OutputEcho {
            Args { cooked: raw.clone(), raw }
        } else {
            match self.cook_args(m, raw) {
                Ok(a) => a,
                Err(e) => {
                    self.console.error(&e);
                    return upd::CONSOLE_DISPLAY;
                }
            }
        };
        let u = self.dispatch(m, cmd, &args, line);
        u | upd::CONSOLE_DISPLAY
    }

    /// One line typed at the `!` prompt (`_CmdAssemble` from `DebuggerProcessCommand`).
    fn exec_asm_line(&mut self, m: &mut Machine, line: &str) -> Update {
        let line = line.trim();
        if line.is_empty() {
            self.asm_active = false;
            self.console.set_prompt('>');
            let n = self.asm.pending();
            if n > 0 {
                self.console.print(&format!(" Asm: {n} sym declared, not defined"));
            }
            return upd::CONSOLE_DISPLAY | upd::CONSOLE_INPUT;
        }
        self.assemble_text(m, line)
    }

    /// `_CmdAssemble`: optional leading label (defines an Asm symbol), then the instruction.
    pub(crate) fn assemble_text(&mut self, m: &mut Machine, text: &str) -> Update {
        let mut text = text;
        let first = text.chars().next().unwrap_or(' ');
        // A label starts in column 0 and is not a mnemonic.
        if first != ' ' && first != '\t' {
            let w: &str = text.split_whitespace().next().unwrap_or("");
            let is_mnemonic = crate::opcodes::OPCODES_65C02.iter().any(|o| o.mnemonic.eq_ignore_ascii_case(w));
            if !is_mnemonic && !w.is_empty() {
                let addr = self.asm.address;
                self.symbol_update(crate::symbols::Table::Asm, w, addr, false, true);
                text = text[w.len()..].trim_start();
            }
        }
        let cmos = m.cpu().is_cmos();
        match self.asm.assemble(m, &self.symbols, cmos, text) {
            Ok(_) => {
                self.disasm_cur = self.asm.address;
                self.calc_top_bot(m);
                upd::ALL
            }
            Err(e) => {
                self.console.print(&format!(" {e}"));
                upd::CONSOLE_DISPLAY
            }
        }
    }

    fn dispatch(&mut self, m: &mut Machine, cmd: Cmd, a: &Args, line: &str) -> Update {
        use Cmd::*;
        match cmd {
            // CPU
            Assemble => self.cmd_assemble(m, a, line),
            CursorJumpPc => self.cmd_cursor_jump_pc(m),
            CursorSetPc => self.cmd_cursor_set_pc(m),
            GoNormalSpeed => self.cmd_go(m, a, false),
            GoFullSpeed => self.cmd_go(m, a, true),
            In => self.cmd_in(m, a),
            InputKey => self.cmd_key(m, a),
            Jsr => self.cmd_jsr(m, a),
            Nop => self.cmd_nop(m),
            Out => self.cmd_out(m, a),
            Lbr => self.cmd_lbr(),
            Profile => self.cmd_profile(m, a),
            RegisterSet => self.cmd_register_set(m, a),
            StackPop | StackPopPseudo | StackPush => self.cmd_stack(m, a, cmd),
            StepOver => self.cmd_step_over(m, a),
            StepOut => self.cmd_step_out(m),
            Trace => self.cmd_trace(m, a, false),
            TraceLine => self.cmd_trace(m, a, true),
            TraceFile => self.cmd_trace_file(m, a),
            Unassemble => self.cmd_unassemble(m, a),
            // Bookmarks
            Bookmark | BookmarkAdd => self.cmd_bookmark_add(m, a),
            BookmarkClear => self.cmd_bookmark_clear(a),
            BookmarkList => self.cmd_bookmark_list(m),
            BookmarkGoto => self.cmd_bookmark_goto(m, a),
            BookmarkSave => self.cmd_bookmark_save(a),
            // Breakpoints
            BreakInvalid => self.cmd_break_invalid(a),
            BreakOpcode => self.cmd_break_opcode(a),
            BreakOnInterrupt => self.cmd_break_on_interrupt(a),
            Breakpoint | BreakpointAddPc => self.cmd_bp_add_pc(m, a),
            BreakpointAddSmart => self.cmd_bp_add_smart(m, a),
            BreakpointAddReg => self.cmd_bp_add_reg(m, a),
            BreakpointAddIo | BreakpointAddMem => self.cmd_bp_add_mem(m, a, crate::bp::Source::MemRw),
            BreakpointAddMemr => self.cmd_bp_add_mem(m, a, crate::bp::Source::MemRead),
            BreakpointAddMemw => self.cmd_bp_add_mem(m, a, crate::bp::Source::MemWrite),
            BreakpointAddVideo => self.cmd_bp_add_video(m, a),
            BreakpointClear => self.cmd_bp_clear(m, a),
            BreakpointDisable => self.cmd_bp_enable(m, a, false),
            BreakpointEnable => self.cmd_bp_enable(m, a, true),
            BreakpointEdit => upd::DISASM | upd::BREAKPOINTS,
            BreakpointList => self.cmd_bp_list(),
            BreakpointSave => self.cmd_bp_save(a),
            BreakpointChange => self.cmd_bp_change(m, a),
            // Config
            Benchmark => self.not_available("BENCHMARK (needs the frontend)"),
            ConfigBw | ConfigColor | ConfigMonochrome => self.cmd_config_color(a, cmd),
            ConfigDisasm => self.cmd_config_disasm(a),
            ConfigFont => self.not_available("FONT (the frontend owns fonts)"),
            ConfigHcolor => self.not_available("HCOLOR (needs the video crate)"),
            ConfigLoad => self.cmd_config_load(m, a),
            ConfigSave => self.cmd_config_save(m, a),
            ConfigGetDebugDir => self.cmd_pwd(a),
            ConfigSetDebugDir => self.cmd_cd(a),
            // Cursor
            CursorJumpRetAddr => self.cmd_cursor_jump_ret(m),
            CursorPageUp => self.cursor_page(m, true, 0),
            CursorPageUp256 => self.cursor_page(m, true, 256),
            CursorPageUp4k => self.cursor_page(m, true, 4096),
            CursorPageDown => self.cursor_page(m, false, 0),
            CursorPageDown256 => self.cursor_page(m, false, 256),
            CursorPageDown4k => self.cursor_page(m, false, 4096),
            // Cycles
            CyclesInfo => self.cmd_cycles_info(a),
            CyclesReset => self.cmd_cycles_reset(m),
            // Disassembler data
            DisasmData | DefineDataByte1 => self.cmd_define_data(m, a, crate::disasm::Nop::Byte1),
            DefineDataByte2 => self.cmd_define_data(m, a, crate::disasm::Nop::Byte2),
            DefineDataByte4 => self.cmd_define_data(m, a, crate::disasm::Nop::Byte4),
            DefineDataByte8 => self.cmd_define_data(m, a, crate::disasm::Nop::Byte8),
            DefineDataWord1 => self.cmd_define_data(m, a, crate::disasm::Nop::Word1),
            DefineDataWord2 => self.cmd_define_data(m, a, crate::disasm::Nop::Word2),
            DefineDataWord4 => self.cmd_define_data(m, a, crate::disasm::Nop::Word4),
            DefineDataStr => self.cmd_define_data(m, a, crate::disasm::Nop::StringApple),
            DefineDataFloat => self.cmd_define_data(m, a, crate::disasm::Nop::Fac),
            DefineAddrWord => self.cmd_define_data(m, a, crate::disasm::Nop::Address),
            DisasmCode => self.cmd_disasm_code(m, a),
            DisasmList => self.cmd_disasm_list(),
            Disk => self.not_available("DISK (the storage crate is wired by the frontend)"),
            // Flags
            FlagClear | FlagSet => self.cmd_flag_list(m, a, cmd == FlagSet),
            FlagClrC | FlagClrZ | FlagClrI | FlagClrD | FlagClrB | FlagClrR | FlagClrV | FlagClrN => self.cmd_flag_one(m, cmd, false),
            FlagSetC | FlagSetZ | FlagSetI | FlagSetD | FlagSetB | FlagSetR | FlagSetV | FlagSetN => self.cmd_flag_one(m, cmd, true),
            // Help
            HelpList => self.cmd_help_list(),
            HelpSpecific => self.cmd_help(a),
            Version => self.cmd_version(),
            Motd => self.cmd_motd(m),
            // Memory
            MemoryCompare => self.cmd_memory_compare(m, a),
            MemMiniDumpHex1 => self.cmd_mem_dump(m, a, 0, MemView::Hex),
            MemMiniDumpHex2 => self.cmd_mem_dump(m, a, 1, MemView::Hex),
            MemMiniDumpAscii1 => self.cmd_mem_dump(m, a, 0, MemView::Ascii),
            MemMiniDumpAscii2 => self.cmd_mem_dump(m, a, 1, MemView::Ascii),
            MemMiniDumpApple1 => self.cmd_mem_dump(m, a, 0, MemView::Apple),
            MemMiniDumpApple2 => self.cmd_mem_dump(m, a, 1, MemView::Apple),
            MemoryEdit => upd::CONSOLE_DISPLAY,
            MemoryEnterByte => self.cmd_memory_enter(m, a, false),
            MemoryEnterWord => self.cmd_memory_enter(m, a, true),
            MemoryLoad => self.cmd_memory_load(m, a),
            MemoryMove => self.cmd_memory_move(m, a),
            MemorySave => self.cmd_memory_save(m, a),
            MemorySearch | MemorySearchHex => self.cmd_memory_search(m, a),
            MemoryFindResults => self.search_display(),
            MemoryFill => self.cmd_memory_fill(m, a),
            Ntsc => self.not_available("NTSC (needs the video crate)"),
            TextSave => self.cmd_text_save(m, a),
            // Output / scripts
            OutputCalc => self.cmd_calc(a),
            OutputEcho => self.cmd_echo(a, line),
            OutputLog => self.cmd_log(a),
            OutputPrint => self.cmd_print(a),
            OutputPrintf => self.cmd_printf(a),
            OutputRun => self.cmd_run(m, a),
            Source | Sync => self.not_available("SOURCE/SYNC (source level debugging)"),
            // Symbols
            SymbolsLookup => self.cmd_symbols(m, a),
            SymbolsRom | SymbolsApplesoft | SymbolsAssembly | SymbolsUser1 | SymbolsUser2 | SymbolsSrc1 | SymbolsSrc2 | SymbolsDos33 | SymbolsProdos => {
                self.cmd_symbols_table(m, a, cmd)
            }
            SymbolsInfo => self.cmd_symbols_info(a, None),
            SymbolsList => self.cmd_symbols_list(m, a),
            // Video
            VideoScannerInfo => self.cmd_video_info(a),
            ViewText4x | ViewText41 | ViewText42 | ViewText8x | ViewText81 | ViewText82 | ViewGrx | ViewGr1 | ViewGr2 | ViewDgrx | ViewDgr1 | ViewDgr2 | ViewHgrx | ViewHgr0 | ViewHgr1
            | ViewHgr2 | ViewHgr3 | ViewHgr4 | ViewHgr5 | ViewHgr6 | ViewHgr7 | ViewHgr8 | ViewDhgrx | ViewDhgr1 | ViewDhgr2 | ViewShr => self.cmd_view(cmd),
            // Watches
            Watch | WatchAdd => self.cmd_watch_add(m, a),
            WatchClear => self.cmd_watch_clear(a),
            WatchDisable => self.cmd_watch_enable(a, false),
            WatchEnable => self.cmd_watch_enable(a, true),
            WatchList => self.cmd_watch_list(m),
            WatchSave => self.cmd_watch_save(a),
            // Windows
            Window | WindowCode | WindowCode1 | WindowCode2 | WindowConsole | WindowData | WindowData1 | WindowData2 | WindowSource1 | WindowSource2 | WindowOutput => {
                self.cmd_window(m, a, cmd)
            }
            // Zero page
            ZeropagePointer | ZeropagePointerAdd => self.cmd_zp_add(m, a),
            ZeropagePointer0 | ZeropagePointer1 | ZeropagePointer2 | ZeropagePointer3 | ZeropagePointer4 | ZeropagePointer5 | ZeropagePointer6 | ZeropagePointer7 => {
                self.cmd_zp_pointer(m, a, cmd)
            }
            ZeropagePointerClear => self.cmd_zp_clear(a),
            ZeropagePointerDisable => self.cmd_zp_enable(a, false),
            ZeropagePointerEnable => self.cmd_zp_enable(a, true),
            ZeropagePointerList => self.cmd_zp_list(m),
            ZeropagePointerSave => self.cmd_zp_save(a),
            Startup => self.cmd_startup(m),
        }
    }

    pub(crate) fn not_available(&mut self, what: &str) -> Update {
        self.console.print(&format!("{CHC_WARNING}Not available in the debugger core: {CHC_DEFAULT}{what}"));
        upd::CONSOLE_DISPLAY
    }

    /// `Help_Arg_1`: usage for the command that was just run.
    pub(crate) fn help_last(&mut self) -> Update {
        match self.last_cmd {
            Some(c) => self.help_for(c),
            None => upd::CONSOLE_DISPLAY,
        }
    }
}
