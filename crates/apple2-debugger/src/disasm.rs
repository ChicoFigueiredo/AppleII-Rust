//! Disassembler (Debugger_Disassembler.cpp, Debugger_DisassemblerData.cpp,
//! `_6502_GetTargets` in Debugger_Assembler.cpp:625): one decoded line with AppleWin's pieces
//! (address, opcode bytes, mnemonic, target, symbol offset, pointer:value, char, branch arrow) and
//! the user "data blocks" (`DB`/`DW`/`DA`/`ASC`/`DF`).

use crate::color::DebugColor;
use crate::mem::{DebugMem, Regs};
use crate::opcodes::*;
use crate::symbols::{Symbols, Table};

pub const DISASM_DISPLAY_MAX_OPCODES: usize = 3;
pub const DISASM_DISPLAY_MAX_IMMEDIATE_LEN: usize = 20;
pub const NO_TARGET: i32 = -1;

/// `Nopcode_e` (Debugger_Types.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nop {
    Byte1,
    Byte2,
    Byte4,
    Byte8,
    Word1,
    Word2,
    Word4,
    Address,
    Hex,
    Char,
    StringAscii,
    StringApple,
    StringMixed,
    Fac,
    Bmp,
}

impl Nop {
    /// `g_aNopcodeTypes`.
    pub fn name(self) -> &'static str {
        match self {
            Nop::Byte1 => "byte1",
            Nop::Byte2 => "byte2",
            Nop::Byte4 => "byte4",
            Nop::Byte8 => "byte8",
            Nop::Word1 => "word1",
            Nop::Word2 => "word2",
            Nop::Word4 => "word4",
            Nop::Address => "addr ",
            Nop::Hex => "hex  ",
            Nop::Char => "char ",
            Nop::StringAscii => "ascii",
            Nop::StringApple => "apple",
            Nop::StringMixed => "mixed",
            Nop::Fac => "FAC  ",
            Nop::Bmp => "bmp  ",
        }
    }
}

/// `DisasmData_t`: a range (inclusive) shown as data.
#[derive(Clone, Debug)]
pub struct DisasmData {
    pub symbol: String,
    pub start: u16,
    pub end: u16,
    pub target: u16,
    pub directive: &'static str,
    pub kind: Nop,
    pub symbol_lookup: bool,
    pub removed: bool,
}

/// `g_aDisassemblerData`.
#[derive(Clone, Debug, Default)]
pub struct DataBlocks(pub Vec<DisasmData>);

impl DataBlocks {
    /// `Disassembly_IsDataAddress`.
    pub fn find(&self, addr: u16) -> Option<&DisasmData> {
        self.0
            .iter()
            .find(|d| !d.removed && addr >= d.start && addr <= d.end)
    }

    pub fn find_mut(&mut self, addr: u16) -> Option<&mut DisasmData> {
        self.0
            .iter_mut()
            .find(|d| !d.removed && addr >= d.start && addr <= d.end)
    }

    pub fn add(&mut self, d: DisasmData) {
        self.0.push(d);
    }

    /// `Disassembly_DelData`: removes the block containing `addr`.
    pub fn del(&mut self, addr: u16) {
        for d in &mut self.0 {
            if !d.removed && addr >= d.start && addr <= d.end {
                d.removed = true;
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &DisasmData> {
        self.0.iter().filter(|d| !d.removed)
    }
}

/// `DISASM_TARGET_*`
pub const DISASM_TARGET_OFF: u8 = 0;
pub const DISASM_TARGET_VAL: u8 = 1;
pub const DISASM_TARGET_ADDR: u8 = 2;
pub const DISASM_TARGET_BOTH: u8 = 3;

/// `g_bConfigDisasm*` / `g_iConfigDisasm*` (Debug.cpp:212).
#[derive(Clone, Debug)]
pub struct DisasmConfig {
    pub address_view: bool,
    pub address_colon: bool,
    pub opcodes_view: bool,
    pub opcode_spaces: bool,
    /// `DISASM_TARGET_*`
    pub targets: u8,
    /// 0 off, 1 plain (`^ = v`), 2 fancy (arrows).
    pub branch_type: u8,
}

impl Default for DisasmConfig {
    fn default() -> Self {
        DisasmConfig {
            address_view: true,
            address_colon: true,
            opcodes_view: true,
            opcode_spaces: true,
            targets: DISASM_TARGET_BOTH,
            branch_type: 2,
        }
    }
}

/// Everything a decode needs.
pub struct DisasmCtx<'a> {
    pub mem: &'a dyn DebugMem,
    pub regs: Regs,
    pub cmos: bool,
    pub symbols: &'a Symbols,
    pub data: &'a DataBlocks,
    pub cfg: &'a DisasmConfig,
}

/// `DisasmLine_t` plus the format flags returned by `GetDisassemblyLine`.
#[derive(Clone, Debug, Default)]
pub struct DisasmLine {
    pub address: String,
    pub opcodes: String,
    pub mnemonic: String,
    pub target: String,
    pub target_offset: i32,
    pub target_offset_str: String,
    pub target_pointer: String,
    pub target_value: String,
    pub immediate: String,
    pub immediate_signed_dec: String,
    pub branch: String,
    pub n_target: u16,
    pub n_immediate: u8,
    pub opcode: u8,
    pub mode: Option<Mode>,
    /// Bytes this line covers.
    pub bytes: usize,
    pub is_data: bool,
    pub target_immediate: bool,
    pub target_indirect: bool,
    pub target_indexed: bool,
    pub target_relative: bool,
    pub target_value_flag: bool,
    pub target_x: bool,
    pub target_y: bool,
    pub fmt_symbol: bool,
    pub fmt_offset: bool,
    pub fmt_branch: bool,
    pub fmt_target_pointer: bool,
    pub fmt_target_value: bool,
    pub fmt_char: bool,
    pub target_table: Option<Table>,
}

/// `FormatCharTxtCtrl(FormatCharTxtHigh(b))`: high bit stripped, control chars shown as `@A..`.
pub fn printable(b: u8) -> char {
    let c = b & 0x7F;
    if c < 0x20 {
        (c + b'@') as char
    } else {
        c as char
    }
}

/// `FormatCharTxtCtrl` as used for `ASC` data: no high-bit stripping beyond `& 0x7F`.
fn ctrl_char(b: u8) -> char {
    printable(b)
}

/// Length and mode of the thing at `addr` (`_6502_GetOpmodeOpbyte`).
pub fn opmode_opbyte<'a>(
    ctx: &DisasmCtx<'a>,
    addr: u16,
) -> (u8, Option<Mode>, usize, Option<&'a DisasmData>) {
    let op = ctx.mem.read(addr);
    let info = table(ctx.cmos)[op as usize];
    if let Some(d) = ctx.data.find(addr) {
        let left = (d.end as usize).wrapping_sub(addr as usize).wrapping_add(1) & 0xFFFF;
        let (n, m) = match d.kind {
            Nop::Byte1 => (1, Some(Mode::Imm)),
            Nop::Byte2 => (2, Some(Mode::Imm)),
            Nop::Byte4 => (4, Some(Mode::Imm)),
            Nop::Byte8 => (8, Some(Mode::Imm)),
            Nop::Fac => (5, Some(Mode::Imm)),
            Nop::Word1 => (2, Some(Mode::Imm)),
            Nop::Word2 => (4, Some(Mode::Imm)),
            Nop::Word4 => (8, Some(Mode::Imm)),
            Nop::Address => (2, Some(Mode::Abs)),
            // AM_DATA: whole range is one line.
            Nop::StringApple | Nop::StringAscii | Nop::StringMixed => (left, None),
            _ => (1, Some(Mode::Imm)),
        };
        return (OPCODE_NOP, m, n, Some(d));
    }
    (op, Some(info.mode), info.mode.bytes() as usize, None)
}

/// `pData->nTargetAddress`: for `DA` it is the word stored at the line itself, read when the line
/// is evaluated (`_6502_GetOpmodeOpbyte`, Debugger_Assembler.cpp:574); it is 0 for other data.
fn data_target(ctx: &DisasmCtx, d: &DisasmData, addr: u16) -> u16 {
    if d.kind == Nop::Address {
        ctx.mem.read_word(addr)
    } else {
        d.target
    }
}

/// `_6502_GetTargets`: effective addresses of the instruction at `addr` with current registers.
/// Returns (partial, partial2, pointer, bytes); `NO_TARGET` (-1) when absent.
pub fn get_targets(
    ctx: &DisasmCtx,
    addr: u16,
    ignore_branch: bool,
    include_next: bool,
) -> (i32, i32, i32, u8) {
    let (mut p1, mut p2, mut ptr, mut nbytes) = (NO_TARGET, NO_TARGET, NO_TARGET, 0u8);
    let m = ctx.mem;
    let r = ctx.regs;
    let op = m.read(addr);
    let t8 = m.read(addr.wrapping_add(1));
    let mut t16 = (m.read(addr.wrapping_add(2)) as u16) << 8 | t8 as u16;
    let info = table(ctx.cmos)[op as usize];
    let stack = |off: i32| 0x100 + ((r.sp as i32 + off) & 0xFF);
    match info.mode {
        Mode::Implied => {
            if info.mem & MEM_S != 0 {
                if op == OPCODE_RTI || op == OPCODE_RTS {
                    let mut sp = r.sp as i32;
                    if op == OPCODE_RTI {
                        sp += 1;
                    }
                    p1 = 0x100 + ((sp + 1) & 0xFF);
                    p2 = 0x100 + ((sp + 2) & 0xFF);
                    t16 = m.read(p1 as u16) as u16 | (m.read(p2 as u16) as u16) << 8;
                    if op == OPCODE_RTS {
                        t16 = t16.wrapping_add(1);
                    }
                } else if op == OPCODE_BRK {
                    p1 = stack(0);
                    p2 = stack(-1);
                    t16 = m.read_word(0xFFFE);
                } else if info.mem & MEM_WI != 0 {
                    t16 = stack(0) as u16;
                } else {
                    t16 = stack(1) as u16;
                }
                if include_next || (op != OPCODE_RTI && op != OPCODE_RTS && op != OPCODE_BRK) {
                    ptr = t16 as i32;
                }
                nbytes = 1;
            }
        }
        Mode::Abs => {
            if op == OPCODE_JSR {
                p1 = stack(0);
                p2 = stack(-1);
            }
            if include_next || (op != OPCODE_JSR && op != OPCODE_JMP_A) {
                ptr = t16 as i32;
            }
            nbytes = 2;
        }
        Mode::IndAbsX => {
            t16 = t16.wrapping_add(r.x as u16);
            p1 = t16 as i32;
            p2 = t16.wrapping_add(1) as i32;
            if include_next {
                ptr = m.read_word(t16) as i32;
            }
            nbytes = 2;
        }
        Mode::AbsX => {
            ptr = t16.wrapping_add(r.x as u16) as i32;
            nbytes = 2;
        }
        Mode::AbsY => {
            ptr = t16.wrapping_add(r.y as u16) as i32;
            nbytes = 2;
        }
        Mode::IndAbs => {
            p1 = t16 as i32;
            p2 = t16.wrapping_add(1) as i32;
            if !ctx.cmos && (t16 & 0xFF) == 0xFF {
                p2 = (t16 & 0xFF00) as i32;
            }
            if include_next {
                ptr = m.read(p1 as u16) as i32 | (m.read(p2 as u16) as i32) << 8;
            }
            nbytes = 2;
        }
        Mode::IndZpX => {
            let z = t8.wrapping_add(r.x);
            p1 = z as i32;
            ptr = m.read_word(z as u16) as i32;
            nbytes = 2;
        }
        Mode::IndZpY => {
            p1 = t8 as i32;
            ptr = m.read_word(t8 as u16).wrapping_add(r.y as u16) as i32;
            nbytes = 1;
        }
        Mode::IndZp => {
            p1 = t8 as i32;
            ptr = m.read_word(t8 as u16) as i32;
            nbytes = 2;
        }
        Mode::Rel => {
            if !ignore_branch {
                p1 = t8 as i32;
                ptr = addr.wrapping_add(2).wrapping_add(t8 as i8 as u16) as i32;
                nbytes = 1;
            }
        }
        Mode::Zp => {
            ptr = t8 as i32;
            nbytes = 1;
        }
        Mode::ZpX => {
            ptr = t8.wrapping_add(r.x) as i32;
            nbytes = 1;
        }
        Mode::ZpY => {
            ptr = t8.wrapping_add(r.y) as i32;
            nbytes = 1;
        }
        Mode::Imm => {}
    }
    (p1, p2, ptr, nbytes)
}

/// `_6502_GetTargetAddress`: where the instruction at `addr` goes/refers to (for the cursor jump).
pub fn target_address(ctx: &DisasmCtx, addr: u16) -> Option<u16> {
    let (op, mode, _n, data) = opmode_opbyte(ctx, addr);
    let mode = mode?;
    if let Some(d) = data {
        return Some(data_target(ctx, d, addr));
    }
    if mode != Mode::Implied {
        let (_, _, ptr, _) = get_targets(ctx, addr, false, true);
        return (ptr != NO_TARGET).then_some(ptr as u16);
    }
    if op == OPCODE_RTS || op == OPCODE_RTI {
        let mut sp = ctx.regs.sp as u16 + 1;
        if op == OPCODE_RTI {
            sp += 1;
        }
        let lo = ctx.mem.read(0x100 | (sp & 0xFF));
        let hi = ctx.mem.read(0x100 | ((sp + 1) & 0xFF));
        let mut ret = (hi as u16) << 8 | lo as u16;
        if op == OPCODE_RTS {
            ret = ret.wrapping_add(1);
        }
        return Some(ret);
    }
    None
}

fn fac_text(ctx: &DisasmCtx, a: u16) -> String {
    let e0 = ctx.mem.read(a);
    if e0 == 0 {
        return "0.0".into();
    }
    let m1 = ctx.mem.read(a.wrapping_add(1));
    let mant = ((m1 | 0x80) as u32) << 24
        | (ctx.mem.read(a.wrapping_add(2)) as u32) << 16
        | (ctx.mem.read(a.wrapping_add(3)) as u32) << 8
        | ctx.mem.read(a.wrapping_add(4)) as u32;
    let exp = e0 as i32 - 0x80;
    let f = mant as f64 * 2f64.powi(exp - 32);
    format!("{}{}", if m1 & 0x80 != 0 { '-' } else { '+' }, c_exp(f))
}

/// C `%e`: six decimals and at least two exponent digits.
pub fn c_exp(f: f64) -> String {
    let s = format!("{f:.6e}");
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let e: i32 = e.parse().unwrap_or(0);
    format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// `FormatNopcodeBytes`: the operand text of a data line.
fn nopcode_text(ctx: &DisasmCtx, d: &DisasmData, addr: u16, nbytes: usize) -> String {
    let mut s = String::new();
    let mut i = 0usize;
    match d.kind {
        Nop::Byte1 | Nop::Byte2 | Nop::Byte4 | Nop::Byte8 => {
            while i < nbytes {
                s += &format!("{:02X}", ctx.mem.read(addr.wrapping_add(i as u16)));
                i += 1;
                if d.kind == Nop::Byte1 && i < nbytes {
                    s.push(',');
                }
            }
        }
        Nop::Fac => s = fac_text(ctx, addr),
        Nop::Word1 | Nop::Word2 | Nop::Word4 => {
            while i < nbytes {
                s += &format!("{:04X}", ctx.mem.read_word(addr.wrapping_add(i as u16)));
                i += 2;
                if i < nbytes {
                    s.push(',');
                }
            }
        }
        Nop::Address => {}
        Nop::StringApple | Nop::StringAscii | Nop::StringMixed => {
            let len = (d.end as i32 - addr as i32 + 1).max(0) as usize;
            let copy = |n: usize| {
                (0..n)
                    .map(|k| ctrl_char(ctx.mem.read(d.start.wrapping_add(k as u16))))
                    .collect::<String>()
            };
            if len > DISASM_DISPLAY_MAX_IMMEDIATE_LEN - 2 {
                let ellipsis = len > DISASM_DISPLAY_MAX_IMMEDIATE_LEN;
                let n = if ellipsis {
                    DISASM_DISPLAY_MAX_IMMEDIATE_LEN - 3
                } else {
                    len
                };
                s = copy(n);
                if ellipsis {
                    s += "...";
                }
            } else {
                s = format!("\"{}\"", copy(len));
            }
        }
        _ => {}
    }
    s
}

/// `GetDisassemblyLine` (Debugger_Disassembler.cpp:171).
pub fn disassemble(ctx: &DisasmCtx, base: u16) -> DisasmLine {
    let mut line = DisasmLine::default();
    let (opcode, mode, nbytes, data) = opmode_opbyte(ctx, base);
    line.opcode = opcode;
    line.mode = mode;
    line.bytes = nbytes;
    line.is_data = data.is_some();
    let m = mode;
    if m == Some(Mode::Imm) {
        line.target_immediate = true;
    }
    if let Some(mm) = m {
        if matches!(
            mm,
            Mode::IndZpX | Mode::IndAbsX | Mode::IndZpY | Mode::IndZp | Mode::IndAbs
        ) {
            line.target_indirect = true;
        }
        if matches!(mm, Mode::IndZpX | Mode::IndAbsX | Mode::IndZpY) {
            line.target_indexed = true;
        }
        if matches!(
            mm,
            Mode::Abs | Mode::Zp | Mode::AbsX | Mode::AbsY | Mode::ZpX | Mode::ZpY
        ) || line.target_indirect
        {
            line.target_value_flag = true;
        }
        if matches!(mm, Mode::AbsX | Mode::ZpX | Mode::IndZpX | Mode::IndAbsX) {
            line.target_x = true;
        }
        if matches!(mm, Mode::AbsY | Mode::ZpY | Mode::IndZpY) {
            line.target_y = true;
        }
    }

    if let Some(mm) = m.filter(|m| *m != Mode::Implied) {
        let mut target: u16 = match data {
            Some(d) => data_target(ctx, d, base),
            None => {
                let w = ctx.mem.read_word(base.wrapping_add(1));
                if nbytes == 2 {
                    w & 0xFF
                } else {
                    w
                }
            }
        };
        if mm == Mode::Rel {
            line.target_relative = true;
            target = base.wrapping_add(2).wrapping_add(target as i8 as u16);
            line.n_target = target;
            line.target_value = format!("{target:04X}");
            line.fmt_branch = true;
            line.branch = match (ctx.cfg.branch_type, target.cmp(&base)) {
                (0, _) => " ",
                (1, std::cmp::Ordering::Less) => "^",
                (1, std::cmp::Ordering::Greater) => "v",
                (1, _) => "=",
                (_, std::cmp::Ordering::Less) => "\u{2191}",
                (_, std::cmp::Ordering::Greater) => "\u{2193}",
                _ => "\u{2194}",
            }
            .into();
            line.fmt_target_pointer = true;
            if ctx.cfg.targets & DISASM_TARGET_ADDR != 0 {
                line.target_pointer = format!("{target:04X}");
            }
        }
        if !matches!(mm, Mode::Imm) {
            line.n_target = target;
            let mut sym = ctx
                .symbols
                .find_symbol(target)
                .map(|(s, t)| (s.to_string(), t));
            if let Some(d) = data {
                if !d.symbol_lookup {
                    sym = None;
                }
            }
            let mut tgt: Option<String> = None;
            if let Some((s, t)) = sym {
                line.fmt_symbol = true;
                line.target_table = Some(t);
                tgt = Some(s);
            }
            if !line.fmt_symbol {
                if let Some((s, t)) = ctx
                    .symbols
                    .find_symbol(target.wrapping_sub(1))
                    .map(|(s, t)| (s.to_string(), t))
                {
                    line.fmt_symbol = true;
                    line.fmt_offset = true;
                    line.target_offset = 1;
                    line.target_table = Some(t);
                    tgt = Some(s);
                }
            }
            // Approved divergence (ADR 0006, #1381): an exact symbol is not replaced by `target+1`.
            if !line.fmt_symbol {
                if let Some((s, t)) = ctx
                    .symbols
                    .find_symbol(target.wrapping_add(1))
                    .map(|(s, t)| (s.to_string(), t))
                {
                    line.fmt_symbol = true;
                    line.fmt_offset = true;
                    line.target_offset = -1;
                    line.target_table = Some(t);
                    tgt = Some(s);
                }
            }
            let tgt = tgt.unwrap_or_else(|| {
                let width = if mm != Mode::Rel { nbytes } else { 3 };
                match width {
                    2 => format!("${target:02X}"),
                    3 => format!("${target:04X}"),
                    _ => String::new(),
                }
            });
            if line.fmt_offset {
                line.target_offset_str = line.target_offset.abs().to_string();
            }
            line.target = tgt;
            let (_, _, mut ptr, _) = get_targets(ctx, base, true, true);
            if opcode == OPCODE_JSR || opcode == OPCODE_JMP_A {
                ptr = NO_TARGET;
            }
            if ptr != NO_TARGET {
                line.fmt_target_pointer = true;
                let v = ctx.mem.read_word(ptr as u16);
                if ctx.cfg.targets & DISASM_TARGET_ADDR != 0 {
                    line.target_pointer = format!("{:04X}", ptr as u16);
                }
                if opcode != OPCODE_JMP_NA && opcode != OPCODE_JMP_IAX {
                    line.fmt_target_value = true;
                    if ctx.cfg.targets & DISASM_TARGET_VAL != 0 {
                        line.target_value = format!("{:02X}", v & 0xFF);
                    }
                    line.fmt_char = true;
                    line.n_immediate = v as u8;
                    line.immediate = printable(v as u8).to_string();
                }
            }
        } else {
            line.target = format!("{:02X}", target as u8);
            let t = target as u8;
            line.immediate_signed_dec = if t == 0 {
                String::new()
            } else if t < 128 {
                format!("+{t}")
            } else {
                format!("-{}", (!t).wrapping_add(1))
            };
            line.fmt_char = true;
            line.n_immediate = t;
            line.immediate = printable(t).to_string();
        }
    }

    line.address = format!("{base:04X}");
    let nshow = nbytes.min(DISASM_DISPLAY_MAX_OPCODES);
    for i in 0..nshow {
        line.opcodes += &format!("{:02X}", ctx.mem.read(base.wrapping_add(i as u16)));
        if ctx.cfg.opcode_spaces {
            line.opcodes.push(' ');
        }
    }
    if let Some(d) = data {
        line.mnemonic = d.directive.to_string();
        if d.kind != Nop::Address {
            line.target = nopcode_text(ctx, d, base, nbytes);
        }
    } else {
        line.mnemonic = table(ctx.cmos)[opcode as usize].mnemonic.to_string();
    }
    let min = DISASM_DISPLAY_MAX_OPCODES * (2 + ctx.cfg.opcode_spaces as usize);
    while line.opcodes.len() < min {
        line.opcodes.push(' ');
    }
    line
}

/// One colored run of a rendered line (`col` is the cell column).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub col: usize,
    pub text: String,
    pub fg: DebugColor,
}

/// Tab stops in cells (Debugger_Display.cpp:1423, non-Apple-font set, adjusted as DrawDisassemblyLine).
pub struct Tabs {
    pub opcode: usize,
    pub label: usize,
    pub instruction: usize,
    pub immediate: usize,
    pub branch: usize,
    pub ch: usize,
}

impl Tabs {
    pub fn new(cfg: &DisasmConfig) -> Tabs {
        let mut t = [5.75f32, 15.5, 25.0, 40.5, 45.5, 48.5];
        if !cfg.address_colon {
            t[0] -= 1.0;
        }
        if cfg.opcodes_view && !cfg.opcode_spaces {
            t[1] -= 3.0;
            t[2] -= 2.0;
            t[3] -= 1.0;
        }
        let mut spacer = 11.0;
        for (i, v) in t.iter_mut().enumerate() {
            if !cfg.address_view && i < 3 {
                *v -= 4.0;
            }
            if !cfg.opcodes_view && i < 3 {
                *v -= spacer;
                if spacer > 0.0 {
                    spacer -= 2.0;
                }
            }
        }
        let f = |x: f32| x.max(0.0) as usize;
        Tabs {
            opcode: f(t[0]),
            label: f(t[1]),
            instruction: f(t[2]),
            immediate: f(t[3]),
            branch: f(t[4]),
            ch: f(t[5]),
        }
    }
}

/// Lays a decoded line out like `DrawDisassemblyLine` (:1376), without cursor/breakpoint colors
/// (the caller overrides foreground for the cursor line). `label` is the symbol at the line's address.
pub fn render_line(
    line: &DisasmLine,
    label: Option<(&str, Table)>,
    cfg: &DisasmConfig,
) -> Vec<Span> {
    use DebugColor::*;
    let tabs = Tabs::new(cfg);
    let mut out: Vec<Span> = Vec::new();
    let mut col = 0usize;
    let put = |out: &mut Vec<Span>, col: &mut usize, text: &str, fg: DebugColor| {
        if !text.is_empty() {
            out.push(Span {
                col: *col,
                text: text.to_string(),
                fg,
            });
            *col += text.chars().count();
        }
    };
    if cfg.address_view {
        put(&mut out, &mut col, &line.address, FG_DISASM_ADDRESS);
    }
    put(
        &mut out,
        &mut col,
        if cfg.address_colon { ":" } else { " " },
        FG_DISASM_OPERATOR,
    );
    col = tabs.opcode;
    if cfg.opcodes_view {
        put(&mut out, &mut col, &line.opcodes, FG_DISASM_OPCODE);
    }
    col = tabs.label;
    if let Some((s, t)) = label {
        let room = tabs.instruction.saturating_sub(tabs.label + 1).max(1);
        let s: String = s.chars().take(room).collect();
        put(
            &mut out,
            &mut col,
            &s,
            if t == Table::User2 {
                FG_INFO_ADDRESS
            } else {
                FG_DISASM_SYMBOL
            },
        );
    }
    col = tabs.instruction;
    put(
        &mut out,
        &mut col,
        &line.mnemonic,
        if line.is_data {
            FG_DISASM_DIRECTIVE
        } else {
            FG_DISASM_MNEMONIC
        },
    );
    put(&mut out, &mut col, " ", FG_DISASM_MNEMONIC);
    if line.target_immediate {
        put(&mut out, &mut col, "#$", FG_DISASM_OPERATOR);
    }
    if line.target_indexed || line.target_indirect {
        put(&mut out, &mut col, "(", FG_DISASM_OPERATOR);
    }
    let mut target = line.target.as_str();
    if let Some(rest) = target.strip_prefix('$') {
        target = rest;
        put(&mut out, &mut col, "$", FG_DISASM_OPERATOR);
    }
    let tcolor = if line.fmt_symbol {
        if line.target_table == Some(Table::User2) {
            FG_INFO_ADDRESS
        } else {
            FG_DISASM_SYMBOL
        }
    } else if line.mode == Some(Mode::Imm) {
        FG_DISASM_OPCODE
    } else {
        FG_DISASM_TARGET
    };
    // Clamp as :1684-1739 so a long symbol cannot run into the pointer column.
    let mut max = DISASM_DISPLAY_MAX_IMMEDIATE_LEN as i32;
    if !cfg.address_view {
        max += 4;
    }
    if !cfg.opcodes_view {
        max += 9;
    }
    let mut over = 0i32;
    if line.fmt_offset {
        over += 1 + line.target_offset_str.len() as i32;
    }
    if line.target_x || (line.target_y && !line.target_indirect) {
        over += 2;
    }
    if line.target_indexed || line.target_indirect {
        over += 1;
    }
    if line.target_indexed && line.target_y {
        over += 2;
    }
    if line.fmt_target_pointer {
        over += line.target_pointer.len() as i32 + 4;
    }
    if line.fmt_char {
        over += line.immediate.len() as i32;
    }
    let lim = (max - over).max(0) as usize;
    let t: String = if target.chars().count() >= lim {
        target.chars().take(lim).collect()
    } else {
        target.to_string()
    };
    put(&mut out, &mut col, &t, tcolor);
    if line.fmt_offset {
        put(
            &mut out,
            &mut col,
            if line.target_offset > 0 { "+" } else { "-" },
            FG_DISASM_OPERATOR,
        );
        put(
            &mut out,
            &mut col,
            &line.target_offset_str,
            FG_DISASM_OPCODE,
        );
    }
    if line.target_indirect || line.target_x || line.target_y {
        if line.target_x {
            put(&mut out, &mut col, ",", FG_DISASM_OPERATOR);
            put(&mut out, &mut col, "X", FG_INFO_REG);
        } else if line.target_y && !line.target_indirect {
            put(&mut out, &mut col, ",", FG_DISASM_OPERATOR);
            put(&mut out, &mut col, "Y", FG_INFO_REG);
        }
    }
    if line.target_indexed || line.target_indirect {
        put(&mut out, &mut col, ")", FG_DISASM_OPERATOR);
    }
    if line.target_indexed && line.target_y {
        put(&mut out, &mut col, ",", FG_DISASM_OPERATOR);
        put(&mut out, &mut col, "Y", FG_INFO_REG);
    }
    if line.is_data {
        return out;
    }
    if line.fmt_target_pointer {
        col = tabs.immediate;
        put(&mut out, &mut col, &line.target_pointer, FG_DISASM_ADDRESS);
        if line.fmt_target_value {
            if !line.target_pointer.is_empty() && !line.target_value.is_empty() {
                put(&mut out, &mut col, ":", FG_DISASM_OPERATOR);
            }
            put(&mut out, &mut col, &line.target_value, FG_DISASM_OPCODE);
            put(&mut out, &mut col, " ", FG_DISASM_OPCODE);
        }
    }
    if line.target_immediate && line.n_immediate != 0 {
        let len = line.immediate_signed_dec.len();
        col = tabs.immediate + 2 + 4usize.saturating_sub(len);
        put(&mut out, &mut col, "#", FG_INFO_OPERATOR);
        put(
            &mut out,
            &mut col,
            &line.immediate_signed_dec,
            FG_DISASM_SINT8,
        );
    }
    if line.fmt_char {
        col = tabs.ch;
        put(&mut out, &mut col, &line.immediate, FG_DISASM_OPERATOR);
    }
    if line.fmt_branch {
        col = tabs.branch;
        put(&mut out, &mut col, &line.branch, FG_DISASM_BRANCH);
    }
    out
}

/// Plain text of [`render_line`] (what a trace file or `U` listing shows).
pub fn line_text(spans: &[Span]) -> String {
    let mut s = String::new();
    for sp in spans {
        while s.chars().count() < sp.col {
            s.push(' ');
        }
        s.push_str(&sp.text);
    }
    s.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::Ram;

    fn dis(bytes: &[u8], at: u16, cmos: bool) -> String {
        let mut ram = Ram::default();
        ram.load(at, bytes);
        let syms = Symbols::default();
        let data = DataBlocks::default();
        let cfg = DisasmConfig {
            branch_type: 1,
            ..Default::default()
        };
        let ctx = DisasmCtx {
            mem: &ram,
            regs: Regs::default(),
            cmos,
            symbols: &syms,
            data: &data,
            cfg: &cfg,
        };
        let l = disassemble(&ctx, at);
        line_text(&render_line(&l, None, &cfg))
    }

    #[test]
    fn plain_instructions_match_applewin_layout() {
        // Columns: addr(0-3) ':' opcodes@5 mnemonic@25 operand
        assert_eq!(
            dis(&[0xA9, 0x41], 0x300, true),
            "0300:A9 41               LDA #$41          #+65 A"
        );
        assert_eq!(
            dis(&[0x4C, 0xED, 0xFD], 0x300, true),
            "0300:4C ED FD            JMP $FDED"
        );
        assert_eq!(dis(&[0xEA], 0x300, true), "0300:EA                  NOP");
    }

    #[test]
    fn indirect_and_indexed_modes() {
        assert!(dis(&[0xB1, 0x10], 0x300, true).contains("LDA ($10),Y"));
        assert!(dis(&[0xA1, 0x10], 0x300, true).contains("LDA ($10,X)"));
        assert!(dis(&[0xB2, 0x10], 0x300, true).contains("LDA ($10)"));
        assert!(dis(&[0x6C, 0x00, 0x03], 0x300, true).contains("JMP ($0300)"));
        assert!(dis(&[0x7C, 0x00, 0x03], 0x300, true).contains("JMP ($0300,X)"));
        assert!(dis(&[0xBD, 0x00, 0x04], 0x300, false).contains("LDA $0400,X"));
        assert!(dis(&[0xB6, 0x80], 0x300, false).contains("LDX $80,Y"));
    }

    #[test]
    fn branch_shows_target_and_arrow() {
        let s = dis(&[0xD0, 0xFE], 0x300, true);
        assert!(s.contains("BNE $0300"), "{s}");
        assert!(s.ends_with('='), "{s}");
        let s = dis(&[0xF0, 0x10], 0x300, true);
        assert!(s.contains("BEQ $0312") && s.ends_with('v'), "{s}");
    }

    #[test]
    fn symbols_replace_addresses_and_offsets() {
        let mut ram = Ram::default();
        ram.load(0x300, &[0x20, 0xED, 0xFD, 0xAD, 0xEE, 0xFD]);
        let mut syms = Symbols::default();
        syms.set(Table::Main, "COUT", 0xFDED);
        let data = DataBlocks::default();
        let cfg = DisasmConfig::default();
        let ctx = DisasmCtx {
            mem: &ram,
            regs: Regs::default(),
            cmos: false,
            symbols: &syms,
            data: &data,
            cfg: &cfg,
        };
        let l = disassemble(&ctx, 0x300);
        assert_eq!(
            line_text(&render_line(&l, Some(("START", Table::User1)), &cfg)),
            "0300:20 ED FD  START     JSR COUT"
        );
        let l = disassemble(&ctx, 0x303);
        assert!(line_text(&render_line(&l, None, &cfg)).contains("LDA COUT+1"));
    }

    #[test]
    fn every_65c02_opcode_decodes_with_table_length_and_mnemonic() {
        for op in 0..=255u8 {
            let s = dis(&[op, 0x12, 0x34], 0x1000, true);
            let info = &OPCODES_65C02[op as usize];
            assert!(s.contains(info.mnemonic), "{op:02X}: {s}");
            let shown = s[5..14].trim_end().split(' ').count();
            assert_eq!(shown, info.mode.bytes() as usize, "{op:02X}: {s}");
        }
    }

    #[test]
    fn spot_check_65c02_formats() {
        let cases: &[(&[u8], &str)] = &[
            (&[0x00], "BRK"),
            (&[0x04, 0x10], "TSB $10"),
            (&[0x1A], "INC"),
            (&[0x3A], "DEC"),
            (&[0x5A], "PHY"),
            (&[0x64, 0x10], "STZ $10"),
            (&[0x80, 0x00], "BRA $1002"),
            (&[0x89, 0x01], "BIT #$01"),
            (&[0x9C, 0x00, 0x20], "STZ $2000"),
            (&[0xFA], "PLX"),
            (&[0xDB], "nop"),
        ];
        for (bytes, expect) in cases {
            let s = dis(bytes, 0x1000, true);
            assert!(s.contains(expect), "{bytes:02X?}: {s}");
        }
    }

    #[test]
    fn nmos_undocumented_opcodes_use_lowercase_names() {
        assert!(dis(&[0xA7, 0x10], 0x300, false).contains("lax $10"));
        assert!(dis(&[0x02], 0x300, false).contains("hlt"));
    }

    #[test]
    fn target_pointer_and_value_follow_registers() {
        let mut ram = Ram::default();
        ram.load(0x300, &[0xBD, 0x00, 0x04]);
        ram.load(0x405, &[0xC1]);
        let syms = Symbols::default();
        let data = DataBlocks::default();
        let cfg = DisasmConfig::default();
        let regs = Regs {
            x: 5,
            ..Default::default()
        };
        let ctx = DisasmCtx {
            mem: &ram,
            regs,
            cmos: false,
            symbols: &syms,
            data: &data,
            cfg: &cfg,
        };
        let l = disassemble(&ctx, 0x300);
        assert_eq!(l.target_pointer, "0405");
        assert_eq!(l.target_value, "C1");
        assert_eq!(l.immediate, "A");
    }

    #[test]
    fn data_blocks_render_as_directives() {
        let mut ram = Ram::default();
        ram.load(0x2000, &[0xC8, 0xC9, 0xA1]);
        ram.load(0x2010, &[0x34, 0x12, 0x78, 0x56]);
        let syms = Symbols::default();
        let mut data = DataBlocks::default();
        data.add(DisasmData {
            symbol: "T".into(),
            start: 0x2000,
            end: 0x2002,
            target: 0,
            directive: "ASC",
            kind: Nop::StringApple,
            symbol_lookup: false,
            removed: false,
        });
        data.add(DisasmData {
            symbol: "W".into(),
            start: 0x2010,
            end: 0x2013,
            target: 0,
            directive: "dw",
            kind: Nop::Word2,
            symbol_lookup: false,
            removed: false,
        });
        let cfg = DisasmConfig::default();
        let ctx = DisasmCtx {
            mem: &ram,
            regs: Regs::default(),
            cmos: true,
            symbols: &syms,
            data: &data,
            cfg: &cfg,
        };
        let l = disassemble(&ctx, 0x2000);
        assert_eq!(
            (l.mnemonic.as_str(), l.target.as_str(), l.bytes),
            ("ASC", "\"HI!\"", 3)
        );
        let l = disassemble(&ctx, 0x2010);
        assert_eq!(
            (l.mnemonic.as_str(), l.target.as_str(), l.bytes),
            ("dw", "1234,5678", 4)
        );
    }

    #[test]
    fn fac_data_prints_c_style_exponent() {
        assert_eq!(c_exp(1.0), "1.000000e+00");
        assert_eq!(c_exp(0.0001234), "1.234000e-04");
        let mut ram = Ram::default();
        ram.load(0x2000, &[0x81, 0x00, 0x00, 0x00, 0x00]);
        let syms = Symbols::default();
        let mut data = DataBlocks::default();
        data.add(DisasmData {
            symbol: String::new(),
            start: 0x2000,
            end: 0x2004,
            target: 0,
            directive: "df",
            kind: Nop::Fac,
            symbol_lookup: false,
            removed: false,
        });
        let cfg = DisasmConfig::default();
        let ctx = DisasmCtx {
            mem: &ram,
            regs: Regs::default(),
            cmos: true,
            symbols: &syms,
            data: &data,
            cfg: &cfg,
        };
        assert_eq!(disassemble(&ctx, 0x2000).target, "+1.000000e+00");
    }
}
