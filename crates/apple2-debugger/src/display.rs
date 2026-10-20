//! The debugger screen as a text/attribute grid (Debugger_Display.cpp). The frontend draws the
//! grid with a fixed-width font; AppleWin's layout is 80 columns by 48 rows of 7x8 pixel cells
//! (560x384): disassembly (or data) window on the left, registers / stack / targets / zero-page
//! pointers / soft switches in the first info column, breakpoints / watches / video scanner / two
//! memory dumps in the second, and the console along the bottom.

use apple2_core::memory::mf;
use apple2_core::Machine;

use crate::bp::*;
use crate::color::{DebugColor, Rgb};
use crate::console::{Cell as ConCell, CONSOLE_PALETTE};
use crate::debugger::*;
use crate::disasm::*;
use crate::mem::regs_of;

pub const GRID_W: usize = 80;
pub const GRID_H: usize = 48;
/// `MIN_DISPLAY_CONSOLE_LINES`: console rows at the bottom (input + 4 output).
pub const CONSOLE_LINES: usize = 5;
/// Rows of the disassembly / data window (`MAX_DISPLAY_LINES - g_nConsoleDisplayLines`).
pub const WINDOW_HEIGHT: usize = GRID_H - CONSOLE_LINES;
/// Column where the first info column starts (`INFO_COL_1`).
pub const INFO_COL_1: usize = 51;
/// Second info column: breakpoints and watches (`INFO_COL_2` ~ 61.5).
pub const INFO_COL_2: usize = 62;
/// Third column: video scanner and memory dumps (`INFO_COL_3`).
pub const INFO_COL_3: usize = 63;
pub const DATA_BYTES_PER_LINE: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridCell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
}

/// A rendered debugger screen.
#[derive(Clone, Debug)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub cells: Vec<GridCell>,
    /// Text cursor of the console input line (the frontend blinks it).
    pub cursor: Option<(usize, usize)>,
}

impl Grid {
    pub fn new(bg: Rgb, fg: Rgb) -> Self {
        Grid { w: GRID_W, h: GRID_H, cells: vec![GridCell { ch: ' ', fg, bg }; GRID_W * GRID_H], cursor: None }
    }

    pub fn cell(&self, x: usize, y: usize) -> &GridCell {
        &self.cells[y * self.w + x]
    }

    /// Writes `text` at (x, y), clipped to the grid; returns the column after it.
    pub fn put(&mut self, x: usize, y: usize, text: &str, fg: Rgb, bg: Rgb) -> usize {
        let mut cx = x;
        if y >= self.h {
            return cx;
        }
        for ch in text.chars() {
            if cx >= self.w {
                break;
            }
            self.cells[y * self.w + cx] = GridCell { ch, fg, bg };
            cx += 1;
        }
        cx
    }

    pub fn fill(&mut self, x: usize, y: usize, w: usize, bg: Rgb) {
        for cx in x..(x + w).min(self.w) {
            if y < self.h {
                self.cells[y * self.w + cx].bg = bg;
            }
        }
    }

    /// The characters of row `y` with trailing blanks removed (used by snapshot tests).
    pub fn row_text(&self, y: usize) -> String {
        let s: String = (0..self.w).map(|x| self.cell(x, y).ch).collect();
        s.trim_end().to_string()
    }

    pub fn text(&self) -> String {
        (0..self.h).map(|y| self.row_text(y)).collect::<Vec<_>>().join("\n")
    }
}

fn hex(b: u8) -> String {
    format!("{b:02X}")
}

impl Debugger {
    /// Rows available to the code/data window.
    pub fn window_height(&self) -> usize {
        if self.split {
            (GRID_H - CONSOLE_LINES) / 2
        } else {
            WINDOW_HEIGHT
        }
    }

    /// Recomputes the window size and cursor line (`WindowUpdateDisasmSize`).
    pub fn update_window_size(&mut self) {
        self.disasm_height = self.window_height();
        self.disasm_cur_line = (self.disasm_height.saturating_sub(1)) / 2;
    }

    /// `Debugger_Display.cpp:UpdateDisplay`: draws everything.
    pub fn render(&mut self, m: &Machine) -> Grid {
        let col = |c: DebugColor| self.colors.get(c);
        let mut g = Grid::new(col(DebugColor::BG_CONSOLE_OUTPUT), col(DebugColor::FG_CONSOLE_OUTPUT));
        if self.disasm_height != self.window_height() {
            self.update_window_size();
            self.calc_top_bot(m);
        }
        match self.window {
            Window::Console => {
                self.draw_console(&mut g, 0, GRID_H);
                return g;
            }
            Window::Data => {
                self.draw_data_window(&mut g, m, 0, self.window_height());
                if self.split {
                    self.draw_code_window(&mut g, m, self.window_height(), self.window_height());
                }
            }
            _ => {
                self.draw_code_window(&mut g, m, 0, self.window_height());
                if self.split {
                    self.draw_data_window(&mut g, m, self.window_height(), self.window_height());
                }
            }
        }
        self.draw_info(&mut g, m);
        self.draw_console(&mut g, GRID_H - CONSOLE_LINES, CONSOLE_LINES);
        g
    }

    // ---- code window ----

    fn draw_code_window(&mut self, g: &mut Grid, m: &Machine, y0: usize, height: usize) {
        let mut addr = self.disasm_top;
        let pc = m.cpu().pc;
        for line in 0..height {
            let y = y0 + line;
            let l = self.disasm_line(m, addr);
            let (active, enabled) = self.breakpoint_info(addr);
            let at_pc = addr == pc;
            let cursor_line = (!self.disasm_cur_bad && line == self.disasm_cur_line) || (self.disasm_cur_bad && line == 0);
            use DebugColor::*;
            let mut bg_id = if line & 1 == 1 { BG_DISASM_1 } else { BG_DISASM_2 };
            let mut fg_id = FG_DISASM_ADDRESS;
            if cursor_line {
                if active {
                    (bg_id, fg_id) = if enabled { (BG_DISASM_BP_S_C, FG_DISASM_BP_S_C) } else { (BG_DISASM_BP_0_C, FG_DISASM_BP_0_C) };
                } else if at_pc {
                    (bg_id, fg_id) = (BG_DISASM_PC_C, FG_DISASM_PC_C);
                } else {
                    (bg_id, fg_id) = (BG_DISASM_C, FG_DISASM_C);
                    self.disasm_cur = addr;
                }
            } else if active {
                fg_id = if enabled { FG_DISASM_BP_S_X } else { FG_DISASM_BP_0_X };
            } else if at_pc {
                (bg_id, fg_id) = (BG_DISASM_PC_X, FG_DISASM_PC_X);
            }
            let bg = self.colors.get(bg_id);
            g.fill(0, y, INFO_COL_1 - 1, bg);
            let label = self.symbols.find_symbol(addr);
            let spans = render_line(&l, label, &self.cfg);
            for sp in spans {
                let fg = if cursor_line { self.colors.get(fg_id) } else { self.colors.get(sp.fg) };
                let mut sp_bg = bg;
                let mut text = sp.text.clone();
                if sp.col == 0 && sp.text == l.address {
                    // address keeps the line foreground
                }
                // Address separator: bookmark digit replaces the colon.
                if self.cfg.address_colon && sp.text == ":" && sp.col == 4 {
                    if let Some(n) = self.bookmark_find(addr) {
                        text = char::from(b'0' + (n as u8 % 10)).to_string();
                        sp_bg = self.colors.get(BG_DISASM_BOOKMARK);
                        g.put(sp.col, y, &text, self.colors.get(FG_DISASM_BOOKMARK), sp_bg);
                        continue;
                    }
                }
                let fg = if sp.col == 0 && !cursor_line { self.colors.get(fg_id) } else { fg };
                g.put(sp.col, y, &text, fg, sp_bg);
            }
            addr = addr.wrapping_add(l.bytes as u16);
        }
    }

    /// `GetBreakpointInfo`: a breakpoint range covering `addr` (set, enabled).
    fn breakpoint_info(&self, addr: u16) -> (bool, bool) {
        for b in &self.breakpoints.slots {
            if b.length != 0 && addr >= b.address && (addr as u32) < b.address as u32 + b.length {
                return (b.set, b.enabled);
            }
        }
        (false, false)
    }

    // ---- data window (DrawSubWindow_Data) ----

    fn draw_data_window(&self, g: &mut Grid, m: &Machine, y0: usize, height: usize) {
        let col = |c: DebugColor| self.colors.get(c);
        let mut a = self.data_addr;
        for line in 0..height {
            let y = y0 + line;
            let bg = col(if line & 1 == 1 { DebugColor::BG_DATA_1 } else { DebugColor::BG_DATA_2 });
            g.fill(0, y, INFO_COL_1 - 1, bg);
            g.put(0, y, &format!("{a:04X}"), col(DebugColor::FG_DISASM_ADDRESS), bg);
            if self.cfg.address_colon {
                g.put(4, y, ":", col(DebugColor::FG_DISASM_OPERATOR), bg);
            }
            let bytes: String = (0..DATA_BYTES_PER_LINE).map(|i| format!("{:02X} ", m.peek(a.wrapping_add(i as u16)))).collect();
            g.put(6, y, &bytes, col(DebugColor::FG_DATA_BYTE), bg);
            let x = 6 + DATA_BYTES_PER_LINE * 3;
            g.put(x, y, "  |  ", col(DebugColor::FG_DISASM_OPERATOR), bg);
            let chars: String = (0..DATA_BYTES_PER_LINE).map(|i| printable(m.peek(a.wrapping_add(i as u16)))).collect();
            g.put(x + 5, y, &chars, col(DebugColor::FG_DISASM_CHAR), bg);
            g.put(x + 5 + DATA_BYTES_PER_LINE, y, "  |  ", col(DebugColor::FG_DISASM_OPERATOR), bg);
            a = a.wrapping_add(DATA_BYTES_PER_LINE as u16);
        }
    }

    // ---- info panel ----

    fn draw_info(&mut self, g: &mut Grid, m: &Machine) {
        if self.window == Window::Console || !self.info_visible {
            return;
        }
        let r = regs_of(m);
        let c = |me: &Debugger, id: DebugColor| me.colors.get(id);
        let bg_info = c(self, DebugColor::BG_INFO);
        // Background of both info columns.
        for y in 0..(GRID_H - CONSOLE_LINES) {
            g.fill(INFO_COL_1 - 1, y, GRID_W - INFO_COL_1 + 1, bg_info);
        }
        self.draw_registers(g, m, 0);
        self.draw_stack(g, m, 7);
        self.draw_targets(g, m, 14);
        self.draw_zero_page(g, m, 16);
        let soft_y = 16 + 2 * MAX_ZEROPAGE_POINTERS;
        self.draw_soft_switches(g, m, soft_y);
        self.draw_video_irq(g, m, soft_y.saturating_sub(3));
        // Right side
        self.draw_breakpoints(g, 0);
        self.draw_watches(g, m, MAX_BREAKPOINTS);
        let y_mem = MAX_BREAKPOINTS + 4 + MAX_WATCHES * 2;
        self.draw_memory(g, m, y_mem, 0);
        self.draw_memory(g, m, y_mem + 6, 1);
        let _ = r;
    }

    fn draw_register(&self, g: &mut Grid, y: usize, name: &str, bytes: u8, value: u16, source_reg: bool, sp: bool) {
        let c = |id: DebugColor| self.colors.get(id);
        let bg = c(DebugColor::BG_DATA_1);
        g.fill(INFO_COL_1, y, 10, bg);
        let mut x = INFO_COL_1;
        if !source_reg {
            x += 1;
        }
        g.put(x, y, &format!("{name:<2}"), if source_reg { c(DebugColor::FG_INFO_REG) } else { c(DebugColor::FG_INFO_TITLE) }, bg);
        if sp {
            let depth = 0xFFu8.wrapping_sub(value as u8);
            g.put(INFO_COL_1 + 2, y, &hex(depth), c(DebugColor::FG_INFO_OPERATOR), bg);
        }
        let bg2 = c(DebugColor::BG_DATA_1);
        if bytes == 2 {
            g.put(INFO_COL_1 + 6, y, &format!("{value:04X}"), c(DebugColor::FG_INFO_ADDRESS), bg2);
        } else {
            let b = value as u8;
            g.put(INFO_COL_1 + 3, y, "'", c(DebugColor::FG_INFO_OPERATOR), bg2);
            g.put(INFO_COL_1 + 4, y, &printable(b).to_string(), c(DebugColor::FG_INFO_CHAR_HI), bg2);
            g.put(INFO_COL_1 + 5, y, "'", c(DebugColor::FG_INFO_OPERATOR), bg2);
            g.put(INFO_COL_1 + 8, y, &hex(b), c(DebugColor::FG_INFO_OPCODE), bg2);
        }
    }

    /// `DrawRegisters` / `DrawFlags`
    fn draw_registers(&self, g: &mut Grid, m: &Machine, y: usize) {
        let r = regs_of(m);
        self.draw_register(g, y, "A", 1, r.a as u16, true, false);
        self.draw_register(g, y + 1, "X", 1, r.x as u16, true, false);
        self.draw_register(g, y + 2, "Y", 1, r.y as u16, true, false);
        self.draw_register(g, y + 3, "PC", 2, r.pc, true, false);
        // flags: "P " title, hex below; letters N V R B D I Z C with 0/1 under each
        let c = |id: DebugColor| self.colors.get(id);
        let fy = y + 4;
        g.put(INFO_COL_1, fy, "P ", c(DebugColor::FG_INFO_REG), c(DebugColor::BG_DATA_1));
        g.put(INFO_COL_1, fy + 1, &hex(r.p), c(DebugColor::FG_INFO_OPCODE), c(DebugColor::BG_INFO));
        for (i, ch) in ['C', 'Z', 'I', 'D', 'B', 'R', 'V', 'N'].iter().enumerate() {
            let set = r.p & (1 << i) != 0;
            let x = INFO_COL_1 + 2 + (7 - i);
            let (fg, bg) = if set { (c(DebugColor::FG_INFO_INVERSE), c(DebugColor::BG_INFO_INVERSE)) } else { (c(DebugColor::FG_INFO_TITLE), c(DebugColor::BG_INFO)) };
            g.put(x, fy, &ch.to_string(), fg, bg);
            g.put(x, fy + 1, if set { "1" } else { "0" }, c(DebugColor::FG_INFO_TITLE), c(DebugColor::BG_INFO));
        }
        self.draw_register(g, y + 6, "S", 2, 0x100 | r.sp as u16, true, true);
    }

    /// `DrawStack` (Debugger_Display.cpp:2983)
    fn draw_stack(&self, g: &mut Grid, m: &Machine, y: usize) {
        let c = |id: DebugColor| self.colors.get(id);
        let sp = m.cpu().sp as u32;
        let n_head = sp + 8;
        let n_tail = 0x100u32;
        // regs.sp in AppleWin is 0x1xx; the loop uses its low byte + base.
        let mut stack = 0x100 + sp;
        let end = if (stack & 0xFF) >= 0xFE { (((stack & 0xFF) & 1) + 1) | 0x100 } else { (0x100 + n_head).min(0x100 + n_tail + 0xFF + 1) };
        let _ = n_head;
        let mut i = 0;
        while i < 8 {
            stack += 1;
            if stack == end {
                break;
            }
            stack = 0x100 | (stack & 0xFF);
            g.put(INFO_COL_1, y + i, &format!("{stack:04X}: "), c(DebugColor::FG_INFO_TITLE), c(DebugColor::BG_DATA_1));
            g.put(INFO_COL_1 + 6, y + i, &format!("  {:02X}", m.peek(stack as u16)), c(DebugColor::FG_INFO_OPCODE), c(DebugColor::BG_DATA_1));
            i += 1;
        }
    }

    /// `DrawTargets`
    fn draw_targets(&self, g: &mut Grid, m: &Machine, y: usize) {
        let c = |id: DebugColor| self.colors.get(id);
        let ctx = self.ctx(m);
        let pc = m.cpu().pc;
        let (_, _, mut ptr, _) = get_targets(&ctx, pc, true, true);
        let op = m.peek(pc);
        if op == crate::opcodes::OPCODE_JSR || op == crate::opcodes::OPCODE_JMP_A {
            ptr = NO_TARGET;
        }
        for idx in (0..2).rev() {
            let (addr, data) = if idx == 1 && ptr != NO_TARGET {
                (format!("{:04X}", ptr as u16), hex(m.peek(ptr as u16)))
            } else {
                ("-none-".to_string(), String::new())
            };
            let (a0, d0) = if idx == 0 { ("-none-".to_string(), String::new()) } else { (addr, data) };
            let row = y + idx;
            let bg = c(DebugColor::BG_INFO);
            g.put(INFO_COL_1, row, &format!("{a0:<7}"), if idx == 0 { c(DebugColor::FG_INFO_TITLE) } else { c(DebugColor::FG_INFO_ADDRESS) }, bg);
            g.put(INFO_COL_1 + 7, row, &format!("{d0:<3}"), if idx == 0 { c(DebugColor::FG_INFO_ADDRESS) } else { c(DebugColor::FG_INFO_OPCODE) }, bg);
        }
    }

    /// `DrawZeroPagePointers`
    fn draw_zero_page(&self, g: &mut Grid, m: &Machine, y: usize) {
        let c = |id: DebugColor| self.colors.get(id);
        let bg = c(DebugColor::BG_INFO_ZEROPAGE);
        for (i, zp) in self.zero_page.slots.iter().enumerate() {
            if !zp.enabled {
                continue;
            }
            let row = y + i * 2;
            let a1 = zp.address as u8;
            let a2 = a1.wrapping_add(1);
            g.put(INFO_COL_1, row, "Z", c(DebugColor::FG_INFO_TITLE), bg);
            g.put(INFO_COL_1 + 1, row, &format!("{i:X} "), c(DebugColor::FG_INFO_BULLET), bg);
            let sym = self.symbols.find_symbol(a1 as u16).map(|s| s.0.chars().take(7).collect::<String>());
            let shown = if self.symbols.find_symbol(a2 as u16).is_some() && sym.is_none() { String::new() } else { sym.clone().unwrap_or_default() };
            g.put(INFO_COL_1 + 3, row, &format!("{shown:<7}"), c(DebugColor::FG_DISASM_SYMBOL), bg);
            let target = m.peek(a1 as u16) as u16 | (m.peek(a2 as u16) as u16) << 8;
            let x = g.put(INFO_COL_1, row + 1, &hex(a1), c(DebugColor::FG_DISASM_ADDRESS), bg);
            let x = g.put(x, row + 1, ":", c(DebugColor::FG_INFO_OPERATOR), bg);
            let x = g.put(x, row + 1, &format!("{target:04X}"), c(DebugColor::FG_INFO_ADDRESS), bg);
            let x = g.put(x, row + 1, ":", c(DebugColor::FG_INFO_OPERATOR), bg);
            g.put(x, row + 1, &hex(m.peek(target)), c(DebugColor::FG_INFO_OPCODE), bg);
        }
    }

    fn switch_line(&self, g: &mut Grid, y: usize, prefix: &str, on: &str, off: &str, suffix: &str, set: bool) {
        let c = |id: DebugColor| self.colors.get(id);
        let bg = c(DebugColor::BG_INFO);
        let mut x = INFO_COL_1;
        if !prefix.is_empty() {
            x = g.put(x, y, prefix, c(DebugColor::FG_INFO_TITLE), bg);
        }
        let hi = |on: bool| if on { (c(DebugColor::FG_INFO_INVERSE), c(DebugColor::BG_INFO_INVERSE)) } else { (c(DebugColor::FG_INFO_TITLE), bg) };
        let (f, b) = hi(set);
        x = g.put(x, y, on, f, b);
        x = g.put(x, y, "/", c(DebugColor::FG_INFO_OPERATOR), bg);
        let (f, b) = hi(!set);
        x = g.put(x, y, off, f, b);
        if !suffix.is_empty() {
            g.put(x, y, suffix, c(DebugColor::FG_INFO_TITLE), bg);
        }
    }

    /// `DrawSoftSwitches` (:2837): video, 80 column, paging and Language Card state.
    fn draw_soft_switches(&self, g: &mut Grid, m: &Machine, y: usize) {
        let b = m.board();
        let mm = m.memmode();
        let v = &b.video;
        self.switch_line(g, y, "", "GR.", "TEXT", "", !v.text);
        self.switch_line(g, y + 1, "", "FULL", "MIX", "", !v.mixed);
        self.switch_line(g, y + 2, "PAGE ", "1", "2", "", mm & mf::PAGE2 == 0);
        self.switch_line(g, y + 3, "", "LO", "HI", "RES", mm & mf::HIRES == 0);
        self.switch_line(g, y + 4, "", "DHGR", "HGR", "", v.dhires);
        self.switch_line(g, y + 5, "80Sto", "0", "1", "", mm & mf::STORE80 == 0);
        self.switch_line(g, y + 6, "ZP ", "M", "A", "", mm & mf::ALTZP == 0);
        self.switch_line(g, y + 7, "RD ", "M", "A", "", mm & mf::AUXREAD == 0);
        self.switch_line(g, y + 8, "WR ", "M", "A", "", mm & mf::AUXWRITE == 0);
        self.switch_line(g, y + 9, "Col", "40", "80", "", !v.col80);
        self.switch_line(g, y + 10, "", "ASC", "MOUS", "", !v.altcharset);
        self.switch_line(g, y + 11, "LC", "2", "1", "", mm & mf::BANK2 != 0);
        self.switch_line(g, y + 12, "LCR", "RAM", "ROM", "", mm & mf::HIGHRAM != 0);
        self.switch_line(g, y + 13, "LCW", "RAM", "ROM", "", mm & mf::WRITERAM != 0);
    }

    /// `DrawVideoScannerInfo` + `DrawIRQInfo`
    fn draw_video_irq(&self, g: &mut Grid, m: &Machine, y: usize) {
        let c = |id: DebugColor| self.colors.get(id);
        let bgt = c(DebugColor::BG_VIDEOSCANNER_TITLE);
        let (mut v, mut h) = m.video_position();
        let (mut vi, mut hi) = (v as i32, h as i32);
        if self.video_horz_real {
            hi -= 13;
            if hi < 0 {
                hi += 65;
                vi -= 1;
                if vi < 0 {
                    vi += m.video_lines() as i32;
                }
            }
            v = vi as u32;
            h = hi as u32;
        }
        let visible = v < 192 && (h >= 25);
        let numc = if visible { c(DebugColor::FG_VIDEOSCANNER_VISIBLE) } else { c(DebugColor::FG_VIDEOSCANNER_INVISIBLE) };
        let fmt = |n: u32| if self.video_decimal { format!("{n:03}") } else { format!("{n:03X}") };
        let mut x = g.put(INFO_COL_3, y, "v:", c(DebugColor::FG_VIDEOSCANNER_TITLE), bgt);
        x = g.put(x, y, &fmt(v), numc, bgt);
        x = g.put(x, y, " h:", c(DebugColor::FG_VIDEOSCANNER_TITLE), bgt);
        g.put(x, y, &fmt(h), numc, bgt);
        let cycles = match self.cycle_mode {
            0 => m.cycles(),
            1 => self.cycle_delta,
            _ => m.cycles().saturating_sub(self.cycle_mark),
        };
        let x = g.put(INFO_COL_3, y + 1, "cycles:", c(DebugColor::FG_VIDEOSCANNER_TITLE), bgt);
        g.put(x, y + 1, &format!("{:08X}", cycles as u32), c(DebugColor::FG_VIDEOSCANNER_TITLE), bgt);
        let irq = m.board().interrupts.irq_line();
        g.put(INFO_COL_3 + 12, y, if irq { "IRQ" } else { "   " }, c(DebugColor::FG_IRQ_TITLE), c(DebugColor::BG_IRQ_TITLE));
    }

    /// `DrawBreakpoints`
    fn draw_breakpoints(&self, g: &mut Grid, y0: usize) {
        let c = |id: DebugColor| self.colors.get(id);
        let mut y = y0;
        for (i, b) in self.breakpoints.slots.iter().enumerate() {
            if !b.set || b.length == 0 {
                continue;
            }
            let bg = c(DebugColor::BG_INFO);
            let mut x = g.put(INFO_COL_2, y, "B", c(DebugColor::FG_INFO_TITLE), bg);
            x = g.put(x, y, &format!("{i:X}"), c(DebugColor::FG_INFO_BULLET), bg);
            // mini hex bank / lang card
            let mut px = String::new();
            if let Some(b) = b.prefix.bank {
                px += &format!("{:X}", b & 0xF);
            }
            if let Some(l) = b.prefix.lang_card {
                px += &format!("L{l}");
            }
            if b.prefix.is_rom {
                px += "R";
            }
            x = g.put(x, y, &px, c(DebugColor::FG_INFO_MEM_BANK), bg);
            let name = b.source.name();
            x = g.put(x, y, name, c(DebugColor::FG_INFO_REG), bg);
            if name.len() < 2 {
                x += 1;
            }
            x = g.put(x, y, b.op.symbol(), c(DebugColor::FG_INFO_BULLET), bg);
            let (fbg, ffg) = if b.enabled { (DebugColor::BG_DISASM_BP_S_C, DebugColor::FG_DISASM_BP_S_C) } else { (DebugColor::BG_INFO, DebugColor::FG_DISASM_BP_0_X) };
            x = g.put(x, y, &format!("{:04X}", b.address), c(ffg), c(fbg));
            if b.length > 1 {
                x = g.put(x, y, ":", c(DebugColor::FG_INFO_OPERATOR), bg);
                x = g.put(x, y, &format!("{:04X}", b.address as u32 + b.length - 1), c(ffg), c(fbg));
            } else {
                x += 5;
            }
            match b.source {
                Source::MemRead => {
                    g.put(x, y, "R", c(DebugColor::FG_INFO_BP_MEM_READ), bg);
                }
                Source::MemWrite => {
                    g.put(x, y, "W", c(DebugColor::FG_INFO_BP_MEM_WRITE), bg);
                }
                _ => {}
            }
            y += 1;
        }
    }

    /// `DrawWatches`
    fn draw_watches(&self, g: &mut Grid, m: &Machine, y0: usize) {
        let c = |id: DebugColor| self.colors.get(id);
        let bg = c(DebugColor::BG_INFO_WATCH);
        for (i, w) in self.watches.slots.iter().enumerate() {
            let y = y0 + i * 2;
            if w.enabled && w.source == Source::MemRw {
                let lo = m.peek(w.address);
                let hi = m.peek(w.address.wrapping_add(1));
                let mut x = g.put(INFO_COL_2, y, "W", c(DebugColor::FG_INFO_TITLE), bg);
                x = g.put(x, y, &format!("{i:X} "), c(DebugColor::FG_INFO_BULLET), bg);
                x = g.put(x, y, &format!("{:04X}", w.address), c(DebugColor::FG_DISASM_ADDRESS), bg);
                x = g.put(x, y, ":", c(DebugColor::FG_INFO_OPERATOR), bg);
                x = g.put(x, y, &format!("{lo:02X}{hi:02X}"), c(DebugColor::FG_INFO_OPCODE), bg);
                x = g.put(x, y, "(", c(DebugColor::FG_INFO_OPERATOR), bg);
                x = g.put(x, y, &format!("{:04X}", (hi as u16) << 8 | lo as u16), c(DebugColor::FG_INFO_ADDRESS), bg);
                g.put(x, y, ")", c(DebugColor::FG_INFO_OPERATOR), bg);
                let t16 = (hi as u16) << 8 | lo as u16;
                let mut x = INFO_COL_2;
                for b in 0..8u16 {
                    if b & 3 == 0 {
                        x = g.put(x, y + 1, " ", c(DebugColor::FG_INFO_OPCODE), bg);
                    }
                    let cbg = if b & 1 == 1 { bg } else { c(DebugColor::BG_DATA_2) };
                    x = g.put(x, y + 1, &hex(m.peek(t16.wrapping_add(b))), c(DebugColor::FG_INFO_OPCODE), cbg);
                }
            } else if w.enabled && w.source == Source::Video {
                let (v, h) = m.video_position();
                let mut x = g.put(INFO_COL_2, y, "W", c(DebugColor::FG_INFO_TITLE), bg);
                x = g.put(x, y, &format!("{i:X} "), c(DebugColor::FG_INFO_BULLET), bg);
                x = g.put(x, y, &format!("{v:04X}"), c(DebugColor::FG_DISASM_ADDRESS), bg);
                g.put(x, y, &format!(":{h:02X}"), c(DebugColor::FG_INFO_OPERATOR), bg);
            }
        }
    }

    /// `DrawMemory` (the mini dumps MD1/MD2)
    fn draw_memory(&self, g: &mut Grid, m: &Machine, y: usize, which: usize) {
        let d = self.mem_dump[which];
        if !d.active {
            return;
        }
        let c = |id: DebugColor| self.colors.get(id);
        let bg = c(DebugColor::BG_INFO);
        let (title, ncols) = match d.view {
            MemView::Hex => ("HEX", 4usize),
            MemView::Ascii => ("ASCII", 16),
            MemView::Apple => ("TEXT", 16),
        };
        let mut x = g.put(INFO_COL_3, y, title, c(DebugColor::FG_INFO_TITLE), bg);
        x = g.put(x, y, " at ", c(DebugColor::FG_INFO_OPERATOR), bg);
        if d.prefix.is_rom {
            x = g.put(x, y, "ROM/", c(DebugColor::FG_INFO_MEM_BANK), bg);
        } else {
            let px = d.prefix.text();
            x = g.put(x, y, &px, c(DebugColor::FG_INFO_MEM_BANK), bg);
        }
        g.put(x, y, &format!("{:04X}", d.addr), c(DebugColor::FG_INFO_ADDRESS), bg);
        let mut a = d.addr;
        for line in 0..4 {
            let row = y + 1 + line;
            let mut x = INFO_COL_3;
            if d.view == MemView::Hex {
                x = g.put(x, row, &format!("{a:04X}"), c(DebugColor::FG_INFO_ADDRESS), bg);
                x = g.put(x, row, ":", c(DebugColor::FG_INFO_OPERATOR), bg);
            }
            for _ in 0..ncols {
                let b = crate::cmd_mem::read_prefixed(m, a, &d.prefix);
                let io = (0xC000..=0xC0FF).contains(&a);
                if d.view == MemView::Hex {
                    x = g.put(x, row, &format!("{b:02X} "), if io { c(DebugColor::FG_INFO_IO_BYTE) } else { c(DebugColor::FG_INFO_OPCODE) }, bg);
                } else {
                    let ch = printable(b);
                    x = g.put(x, row, &ch.to_string(), c(DebugColor::FG_INFO_OPCODE), if io { c(DebugColor::BG_INFO_IO_BYTE) } else { bg });
                }
                a = a.wrapping_add(1);
            }
        }
    }

    // ---- console ----

    /// Draws `lines` console rows at `y0`: output above, input on the last row.
    fn draw_console(&self, g: &mut Grid, y0: usize, lines: usize) {
        let c = |id: DebugColor| self.colors.get(id);
        let bg = c(DebugColor::BG_CONSOLE_OUTPUT);
        for y in y0..y0 + lines {
            g.fill(0, y, GRID_W, bg);
        }
        let out_rows = lines.saturating_sub(1);
        let vis = self.console.visible(out_rows);
        // newest line sits just above the input row
        let first = y0 + out_rows - vis.len();
        for (k, l) in vis.iter().enumerate() {
            self.draw_console_line(g, first + k, l);
        }
        // input line
        let ib = c(DebugColor::BG_CONSOLE_INPUT);
        let y = y0 + lines - 1;
        g.fill(0, y, GRID_W, ib);
        let text = format!("{}{}", self.console.prompt(), self.console.input());
        let width = GRID_W - 1;
        let shown: String = {
            let n = text.chars().count();
            if n > width {
                text.chars().skip(n - width).collect()
            } else {
                text
            }
        };
        let x = g.put(0, y, &shown, c(DebugColor::FG_CONSOLE_INPUT), ib);
        g.cursor = Some((x.min(GRID_W - 1), y));
    }

    fn draw_console_line(&self, g: &mut Grid, y: usize, l: &[ConCell]) {
        let bg = self.colors.get(DebugColor::BG_CONSOLE_OUTPUT);
        for (x, cell) in l.iter().enumerate().take(GRID_W) {
            let rgb = if cell.color == 0 { self.colors.get(DebugColor::FG_CONSOLE_OUTPUT) } else { CONSOLE_PALETTE[(cell.color as usize).min(10)] };
            g.put(x, y, &cell.ch.to_string(), rgb, bg);
        }
    }
}
