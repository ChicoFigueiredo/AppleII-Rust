//! CPU commands: assemble, unassemble, go/trace/step, registers, flags, stack, I/O, profile,
//! cursor movement and cycle counters (Debug.cpp:2358-2850, 3273-3910, 6362-6402, 7090-7170).

use apple2_core::Machine;
use apple2_cpu::Bus;

use crate::args::*;
use crate::cmdtable::Cmd;
use crate::console::*;
use crate::debugger::*;
use crate::opcodes::*;

const FLAG_NAMES: [char; 8] = ['C', 'Z', 'I', 'D', 'B', 'R', 'V', 'N'];

impl Debugger {
    pub(crate) fn cmd_assemble(&mut self, m: &mut Machine, a: &Args, line: &str) -> Update {
        if a.n() > 0 {
            self.asm.address = a.value(1);
        }
        self.disasm_cur = self.asm.address;
        self.calc_top_bot(m);
        if a.n() <= 1 {
            self.asm_active = true;
            self.console.set_prompt('!');
            return upd::CONSOLE_DISPLAY;
        }
        // "A addr mnemonic ...": the text after the address.
        let text = Args::rest_of(Args::rest_of(line));
        self.assemble_text(m, text)
    }

    pub(crate) fn cmd_unassemble(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::Unassemble);
        }
        self.disasm_top = a.value(1);
        self.calc_cur_from_top(m);
        upd::DISASM
    }

    pub(crate) fn cmd_cursor_jump_pc(&mut self, m: &mut Machine) -> Update {
        self.goto(m, m.cpu().pc);
        upd::ALL
    }

    pub(crate) fn cmd_cursor_set_pc(&mut self, m: &mut Machine) -> Update {
        m.cpu_mut().pc = self.disasm_cur;
        upd::ALL
    }

    pub(crate) fn cmd_cursor_jump_ret(&mut self, m: &mut Machine) -> Update {
        let a = Self::stack_return_address(m);
        self.goto(m, a);
        upd::ALL
    }

    /// `CmdCursorFollowTarget` (Ctrl-Right on a branch/jump): jump to the cursor's target.
    pub fn cursor_follow_target(&mut self, m: &Machine) -> Update {
        self.cursor_follow_target_at(m, crate::keys::Align::Keep)
    }

    /// `CmdCursorFollowTarget( align )`; the Right arrow key passes `CURSOR_ALIGN_TOP`.
    pub fn cursor_follow_target_at(&mut self, m: &Machine, align: crate::keys::Align) -> Update {
        let t = crate::disasm::target_address(&self.ctx(m), self.disasm_cur);
        if let Some(t) = t {
            self.disasm_cur = t;
            self.align_cursor(align);
            self.calc_top_bot(m);
        }
        upd::ALL
    }

    /// `CmdCursorLineDown( nBytes )` (Shift+Down): shifts the whole window by `n` bytes, with no
    /// smart disassembly. The data window still moves by a line.
    pub fn cursor_line_down_bytes(&mut self, m: &Machine, n: u16) -> Update {
        if self.window == Window::Data {
            return self.cursor_line_down(m);
        }
        self.disasm_top = self.disasm_top.wrapping_add(n);
        self.disasm_cur = self.disasm_cur.wrapping_add(n);
        self.calc_top_bot(m);
        upd::DISASM
    }

    /// `CmdCursorLineUp( nBytes )` (Shift+Up): moves the top back by one byte.
    pub fn cursor_line_up_bytes(&mut self, m: &Machine, _n: u16) -> Update {
        if self.window == Window::Data {
            return self.cursor_line_up(m);
        }
        self.disasm_top = self.disasm_top.wrapping_sub(1);
        self.calc_cur_from_top(m);
        upd::DISASM
    }

    /// `CmdCursorLineDown` (Down arrow).
    pub fn cursor_line_down(&mut self, m: &Machine) -> Update {
        if self.window == Window::Data {
            self.data_addr = self.data_addr.wrapping_add(8);
            return upd::DISASM;
        }
        let n = self.instr_len(m, self.disasm_cur) as u16;
        self.disasm_cur = self.disasm_cur.wrapping_add(n);
        let t = self.instr_len(m, self.disasm_top) as u16;
        self.disasm_top = self.disasm_top.wrapping_add(t);
        if self.disasm_cur_bad {
            self.calc_top_bot(m);
        }
        self.disasm_cur_bad = false;
        upd::DISASM
    }

    /// `CmdCursorLineUp` (Up arrow): picks the top whose listing keeps the cursor instruction on the
    /// next line, favouring the longest instruction (`g_iConfigDisasmScroll == 3`).
    pub fn cursor_line_up(&mut self, m: &Machine) -> Update {
        if self.window == Window::Data {
            self.data_addr = self.data_addr.wrapping_sub(8);
            return upd::DISASM;
        }
        let look = self.disasm_height.max(4);
        let mut best: Option<u16> = None;
        let mut ntop = self.disasm_top;
        for _ in 0..look {
            ntop = ntop.wrapping_sub(1);
            let target_off = self.disasm_cur.wrapping_sub(ntop) as u32;
            let mut off: u32 = 0;
            for line in 0..look {
                if line == self.disasm_cur_line + 1 && off == target_off {
                    best = Some(ntop);
                }
                off += self.instr_len(m, ntop.wrapping_add(off as u16)) as u32;
                if off > target_off {
                    break;
                }
            }
        }
        match best {
            Some(t) => {
                self.disasm_top = t;
                self.calc_cur_from_top(m);
                self.disasm_cur_bad = false;
            }
            None => {
                self.disasm_cur_bad = true;
                self.disasm_cur = self.disasm_cur.wrapping_sub(1);
                self.calc_top_bot(m);
            }
        }
        upd::DISASM
    }

    /// `CmdCursorPageUp/Down` and the 256/4K aligned variants (`align` 0 = one screen).
    pub(crate) fn cursor_page(&mut self, m: &Machine, up: bool, align: u16) -> Update {
        if self.window == Window::Data {
            let step = if align == 0 { 128 } else { align };
            self.data_addr = if up {
                self.data_addr.wrapping_sub(step)
            } else {
                self.data_addr.wrapping_add(step)
            };
            return upd::DISASM;
        }
        if align == 0 {
            let lines =
                (self.disasm_height.max(2) as i32 - (self.disasm_cur_line as i32 + 1)).max(1);
            for _ in 0..lines {
                if up {
                    self.cursor_line_up(m);
                } else {
                    self.cursor_line_down(m);
                }
            }
        } else {
            let mask = align - 1;
            let top = self.disasm_top;
            self.disasm_top = if up {
                if top & mask == 0 {
                    top.wrapping_sub(align)
                } else {
                    top.wrapping_sub(top & mask)
                }
            } else if top & mask == 0 {
                top.wrapping_add(align)
            } else {
                top.wrapping_add(align - (top & mask))
            };
        }
        upd::DISASM
    }

    // ---- running ----

    pub(crate) fn cmd_go(&mut self, m: &mut Machine, a: &Args, full: bool) -> Update {
        let kcmd = if full {
            Cmd::GoFullSpeed
        } else {
            Cmd::GoNormalSpeed
        };
        let n = a.n();
        if n > 4 {
            return self.help_for(kcmd);
        }
        let until = if n > 0 { Some(a.value(1)) } else { None };
        let mut skip = None;
        if n > 1 {
            let start = a.value(2);
            let end: i32;
            if n > 2 {
                if a.get(3).map(|t| t.is_op(",")).unwrap_or(false) {
                    if n > 3 {
                        let len = a.value(4) as i32;
                        end = (start as i32 + len).min(0x10000);
                    } else {
                        return self.help_for(kcmd);
                    }
                } else if a.get(3).map(|t| t.is_op(":")).unwrap_or(false) {
                    end = a.value(4) as i32 + 1;
                } else {
                    return self.help_for(kcmd);
                }
            } else {
                return self.help_for(kcmd);
            }
            let len = ((end - start as i32).unsigned_abs() & 0xFFFF) as u16;
            skip = Some((start, len));
        }
        let pc = m.cpu().pc;
        self.begin_go(pc, until, skip, full);
        self.attach(m);
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_trace(&mut self, m: &mut Machine, a: &Args, _cycles: bool) -> Update {
        self.trace_n(m, if a.n() > 0 { a.value(1) as i32 } else { 1 })
    }

    /// `CmdTrace` core: execute `n` instructions (the first one right now).
    pub fn trace_n(&mut self, m: &mut Machine, n: i32) -> Update {
        self.attach(m);
        self.steps = n;
        self.step_until = None;
        self.skip = None;
        self.enter_stepping();
        self.continue_stepping(m);
        upd::ALL
    }

    pub(crate) fn cmd_step_out(&mut self, m: &mut Machine) -> Update {
        let ret = Self::stack_return_address(m);
        let pc = m.cpu().pc;
        self.begin_go(pc, Some(ret), None, true);
        self.attach(m);
        upd::ALL
    }

    /// `CmdStepOver`: steps `n` instructions, running JSR bodies to completion.
    pub(crate) fn cmd_step_over(&mut self, m: &mut Machine, a: &Args) -> Update {
        let count = if a.n() > 0 { a.value(1) } else { 1 };
        self.step_over_n(m, count)
    }

    pub fn step_over_n(&mut self, m: &mut Machine, count: u16) -> Update {
        for _ in 0..count {
            let pc = m.cpu().pc;
            let op = m.peek(pc);
            let expected = pc.wrapping_add(3);
            self.trace_n(m, 1);
            if op == OPCODE_JSR {
                self.cmd_step_out(m);
                let max_steps = 0xFFFFF;
                self.steps = max_steps;
                while self.steps != 0 {
                    self.continue_stepping(m);
                }
                if m.cpu().pc != expected {
                    self.step_over_diagnostic(m, expected);
                }
            }
        }
        upd::ALL
    }

    fn step_over_diagnostic(&mut self, m: &Machine, expected: u16) {
        // _6502_FindStackReturnAddress: depth of `expected` on the stack, -1 if absent.
        let sp = m.cpu().sp as u32;
        let mut depth: i32 = -1;
        let mut stack = sp + 1;
        while stack <= 0x100 {
            let lo = m.peek(0x100 + (stack & 0xFF) as u16) as u16;
            stack += 1;
            let hi = m.peek(0x100 + (stack & 0xFF) as u16) as u16;
            if (hi << 8 | lo).wrapping_add(1) == expected {
                depth = stack as i32 - 2 - sp as i32;
                break;
            }
        }
        let c = &mut self.console;
        if depth < 0 {
            c.print(&format!("{CHC_ERROR}ERROR{CHC_ARG_SEP}:{CHC_ERROR} Didn't step over JSR! {CHC_ARG_SEP}({CHC_DEFAULT}RTS {CHC_ARG_SEP}${CHC_ADDRESS}{expected:04X}{CHC_DEFAULT} not found!{CHC_ARG_SEP})"));
        } else if depth == 0 {
            c.print(&format!("{CHC_INFO}INFO{CHC_ARG_SEP}:{CHC_INFO} Didn't step over JSR! {CHC_ARG_SEP}({CHC_DEFAULT}RTS {CHC_ARG_SEP}${CHC_ADDRESS}{expected:04X}{CHC_DEFAULT} on top of stack.{CHC_ARG_SEP})"));
        } else {
            c.print(&format!("{CHC_WARNING}WARN{CHC_ARG_SEP}:{CHC_WARNING} Didn't step over JSR! {CHC_ARG_SEP}({CHC_DEFAULT}Stack has RTS {CHC_ARG_SEP}${CHC_ADDRESS}{expected:04X}{CHC_DEFAULT} but needs fixup: {CHC_ARG_SEP}${CHC_NUM_HEX}{:02X}{CHC_DEFAULT} bytes{CHC_ARG_SEP})", depth & 0xFF));
        }
    }

    pub(crate) fn cmd_trace_file(&mut self, m: &mut Machine, a: &Args) -> Update {
        if self.trace.is_some() {
            if let Some(mut t) = self.trace.take() {
                let _ = t.flush();
            }
            self.console.print("Trace stopped.");
        } else {
            let name = if a.n() > 0 {
                a.text(1).to_string()
            } else {
                "Trace.txt".to_string()
            };
            self.trace_scanner = a.n() >= 2;
            match self.host.create(&name) {
                Ok(w) => {
                    self.trace = Some(w);
                    self.trace_header = true;
                    let hdr = if self.trace_scanner {
                        "Trace (with video info) started"
                    } else {
                        "Trace started"
                    };
                    self.console
                        .print(&format!("{hdr}: {}{name}", self.host.dir()));
                    self.attach(m);
                }
                Err(_) => self.console.print(&format!("Trace ERROR: {name}")),
            }
        }
        upd::ALL
    }

    // ---- registers, flags, stack, I/O ----

    pub(crate) fn cmd_register_set(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() < 2 {
            return self.help_for(Cmd::RegisterSet);
        }
        let mut i = 2;
        if a.get(2).map(|t| t.is_op("=")).unwrap_or(false) {
            i = 3;
        }
        if i > a.n() {
            return self.help_for(Cmd::RegisterSet);
        }
        let v = a.value(i);
        let b = v as u8;
        match a.text(1).to_ascii_uppercase().as_str() {
            "A" => m.cpu_mut().a = b,
            "X" => m.cpu_mut().x = b,
            "Y" => m.cpu_mut().y = b,
            "P" => m.cpu_mut().p = b,
            "S" | "SP" => m.cpu_mut().sp = b,
            "PC" => {
                m.cpu_mut().pc = v;
                self.goto(m, v);
            }
            _ => return self.help_for(Cmd::RegisterSet),
        }
        upd::ALL
    }

    pub(crate) fn cmd_flag_one(&mut self, m: &mut Machine, cmd: Cmd, set: bool) -> Update {
        use Cmd::*;
        let bit = match cmd {
            FlagClrC | FlagSetC => 0,
            FlagClrZ | FlagSetZ => 1,
            FlagClrI | FlagSetI => 2,
            FlagClrD | FlagSetD => 3,
            FlagClrB | FlagSetB => 4,
            FlagClrR | FlagSetR => 5,
            FlagClrV | FlagSetV => 6,
            _ => 7,
        };
        let p = &mut m.cpu_mut().p;
        if set {
            *p |= 1 << bit;
        } else {
            *p &= !(1 << bit);
        }
        upd::FLAGS
    }

    /// `SE n v c` / `CL n v c`
    pub(crate) fn cmd_flag_list(&mut self, m: &mut Machine, a: &Args, set: bool) -> Update {
        for i in 1..=a.n() {
            let c = a.text(i).chars().next().unwrap_or(' ').to_ascii_uppercase();
            if let Some(bit) = FLAG_NAMES.iter().position(|f| *f == c) {
                let p = &mut m.cpu_mut().p;
                if set {
                    *p |= 1 << bit;
                } else {
                    *p &= !(1 << bit);
                }
            }
        }
        upd::FLAGS
    }

    pub(crate) fn cmd_stack(&mut self, m: &mut Machine, a: &Args, cmd: Cmd) -> Update {
        match cmd {
            Cmd::StackPush => {
                for i in 1..=a.n() {
                    let c = m.cpu();
                    let v = match a.text(i).to_ascii_uppercase().as_str() {
                        "A" => c.a,
                        "X" => c.x,
                        "Y" => c.y,
                        "P" => c.p,
                        _ => a.value(i) as u8,
                    };
                    let sp = c.sp;
                    m.poke(0x100 | sp as u16, v);
                    m.cpu_mut().sp = sp.wrapping_sub(1);
                }
            }
            Cmd::StackPop => {
                let sp = m.cpu().sp.wrapping_add(1);
                let v = m.peek(0x100 | sp as u16);
                let c = m.cpu_mut();
                c.sp = sp;
                match a.text(1).to_ascii_uppercase().as_str() {
                    "A" => c.a = v,
                    "X" => c.x = v,
                    "Y" => c.y = v,
                    "P" => c.p = v,
                    _ => {}
                }
            }
            _ => {
                let c = m.cpu_mut();
                c.sp = c.sp.wrapping_add(2); // PPOP: discard a return address
            }
        }
        upd::ALL
    }

    pub(crate) fn cmd_in(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::In);
        }
        let v = m.board_mut().read(0xC000 | (a.value(1) & 0xFF));
        self.console.print(&format!(
            "{CHC_ARG_SEP}${CHC_ADDRESS}{:04X}{CHC_ARG_SEP}: {CHC_NUM_HEX}{v:02X}",
            0xC000 | (a.value(1) & 0xFF)
        ));
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_out(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::Out);
        }
        m.board_mut()
            .write(0xC000 | (a.value(1) & 0xFF), a.value(2) as u8);
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_key(&mut self, m: &mut Machine, a: &Args) -> Update {
        let k = if a.n() > 0 {
            if a.value(1) != 0 {
                a.value(1) as u8
            } else {
                a.text(1).bytes().next().unwrap_or(b' ')
            }
        } else {
            b' '
        };
        m.key_press(k);
        upd::CONSOLE_DISPLAY
    }

    pub(crate) fn cmd_jsr(&mut self, m: &mut Machine, a: &Args) -> Update {
        if a.n() == 0 {
            return self.help_for(Cmd::Jsr);
        }
        let ret = m.cpu().pc.wrapping_sub(1);
        let sp = m.cpu().sp;
        m.poke(0x100 | sp as u16, (ret >> 8) as u8);
        m.poke(0x100 | sp.wrapping_sub(1) as u16, ret as u8);
        m.cpu_mut().sp = sp.wrapping_sub(2);
        m.cpu_mut().pc = a.value(1);
        self.goto(m, a.value(1));
        upd::ALL
    }

    pub(crate) fn cmd_nop(&mut self, m: &mut Machine) -> Update {
        let pc = m.cpu().pc;
        let n = self.instr_len(m, pc);
        for i in 0..n {
            m.poke(pc.wrapping_add(i as u16), 0xEA);
        }
        upd::ALL
    }

    pub(crate) fn cmd_lbr(&mut self) -> Update {
        match self.lbr {
            None => self
                .console
                .print(" LBR not set yet. Hint: Run from the debugger via 'g' command."),
            Some(a) => self.console.print(&format!(" LBR = ${a:04X}")),
        }
        upd::CONSOLE_DISPLAY
    }

    // ---- profile ----

    pub(crate) fn cmd_profile(&mut self, m: &mut Machine, a: &Args) -> Update {
        let sub = if a.n() == 0 {
            "RESET".to_string()
        } else {
            a.text(1).to_ascii_uppercase()
        };
        if a.n() > 1 {
            return self.help_for(Cmd::Profile);
        }
        match sub.as_str() {
            "RESET" => {
                self.profile = Profile {
                    active: true,
                    begin_cycles: m.cycles(),
                    ..Default::default()
                };
                self.console.print(" Resetting profile data.");
            }
            "LIST" => {
                let text = self.profile_text(m, false);
                for l in text.lines() {
                    self.console.print(&l.replace('\t', "    "));
                }
            }
            "SAVE" => {
                let text = self.profile_text(m, true);
                match self.host.write("Profile.txt", text.as_bytes()) {
                    Ok(()) => self.console.print(" Saved: Profile.txt"),
                    Err(_) => self.console.print(" ERROR: Couldn't save file. (In use?)"),
                }
            }
            _ => return self.help_for(Cmd::Profile),
        }
        upd::CONSOLE_DISPLAY
    }

    /// `ProfileFormat` (Debug.cpp:8603).
    pub fn profile_text(&self, m: &Machine, export: bool) -> String {
        let cmos = m.cpu().is_cmos();
        let sep = if export { "\t" } else { "  " };
        let mut ops: Vec<(usize, u64)> = self.profile.opcodes.iter().copied().enumerate().collect();
        ops.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let total: u64 = ops.iter().map(|o| o.1).sum();
        let tdiv = total.max(1) as f64;
        let mut s = String::new();
        if export {
            s += &format!("\"Percent\"{sep}{sep}\"Count\"{sep}\"Opcode\"\t\"Mnemonic\"\t\"Addressing Mode\"\n");
        } else {
            s += &format!("Percent{sep}Count{sep}Mnemonic\tAddressing Mode\n");
        }
        for (op, n) in &ops {
            if *n == 0 && !export {
                continue;
            }
            let info = table(cmos)[*op];
            let opc = if export {
                format!("\"{op:02X}\"")
            } else {
                format!("{op:02X}")
            };
            let mode = if export {
                format!("\"{}\"", info.mode.name())
            } else {
                info.mode.name().to_string()
            };
            s += &format!(
                "{:7.4}%{sep}{n:9}{sep}{opc}{sep}{}{sep}{mode}\n",
                100.0 * *n as f64 / tdiv,
                info.mnemonic
            );
        }
        s += &format!("Total:  {sep}{:9}\n\n", if total == 0 { 0 } else { total });
        s += &format!("Percent{sep}Count{sep}Addressing Mode\n");
        let mtotal: u64 = self.profile.modes.iter().sum();
        let mut modes: Vec<(usize, u64)> = self.profile.modes.iter().copied().enumerate().collect();
        modes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (i, n) in modes {
            if n == 0 && !export {
                continue;
            }
            s += &format!(
                "{:7.4}%{sep}{n:9}{sep}{}\n",
                100.0 * n as f64 / mtotal.max(1) as f64,
                ALL_MODES[i].name()
            );
        }
        s += &format!("Total:  {sep}{:9}\n===================\n", mtotal);
        s += &format!(
            "Cycles: {sep}{:9}\n",
            m.cycles().saturating_sub(self.profile.begin_cycles)
        );
        s
    }

    // ---- cycles (CYCLES / RCC) ----

    /// `CmdCyclesInfo` (Debug.cpp:7137): exactly one argument; `part` also sets the base
    /// (GH#787), as `CmdCyclesReset(0)` does.
    pub(crate) fn cmd_cycles_info(&mut self, m: &Machine, a: &Args) -> Update {
        if a.n() != 1 {
            return self.help_for(Cmd::CyclesInfo);
        }
        match a.text(1).to_ascii_lowercase().as_str() {
            "abs" => self.cycle_mode = 0,
            "rel" => self.cycle_mode = 1,
            "part" => self.cycle_mode = 2,
            _ => return self.help_for(Cmd::CyclesInfo),
        }
        if self.cycle_mode == 2 {
            self.cycle_mark = m.cycles();
        }
        let arg = a.text(1).to_string();
        self.console
            .print(&format!("Cycles display updated: {arg}"));
        upd::ALL
    }

    pub(crate) fn cmd_cycles_reset(&mut self, m: &Machine) -> Update {
        self.cycle_mark = m.cycles();
        upd::VIDEOSCANNER
    }

    pub(crate) fn cmd_video_info(&mut self, a: &Args) -> Update {
        for i in 1..=a.n() {
            match a.text(i).to_ascii_uppercase().as_str() {
                "DEC" | "DECIMAL" => self.video_decimal = true,
                "HEX" => self.video_decimal = false,
                "REAL" => self.video_horz_real = true,
                "APPLE" => self.video_horz_real = false,
                _ => return self.help_for(Cmd::VideoScannerInfo),
            }
        }
        upd::VIDEOSCANNER
    }
}
