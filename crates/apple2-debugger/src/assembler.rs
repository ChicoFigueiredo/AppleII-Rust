//! Mini assembler (Debugger_Assembler.cpp `Assemble`, :1545): one instruction per line, bare
//! numbers decimal and `$` hex (as `AssemblerGetArgs` does, :1173), symbols from the tables, and
//! "delayed targets" for symbols not defined yet (:1477) that are patched when they appear.

use crate::mem::DebugMem;
use crate::opcodes::*;
use crate::symbols::Symbols;

#[derive(Clone, Debug)]
struct Delayed {
    name: String,
    base: u16,
    mode: Mode,
}

#[derive(Clone, Debug, Default)]
pub struct Assembler {
    /// `g_nAssemblerAddress`: where the next instruction goes.
    pub address: u16,
    delayed: Vec<Delayed>,
}

fn parse_number(s: &str, symbols: &Symbols) -> Result<Option<u32>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Syntax Error: missing operand".into());
    }
    // symbol+n / symbol-n
    let split = s.char_indices().skip(1).find(|(_, c)| *c == '+' || *c == '-');
    if let Some((i, c)) = split {
        if !s.starts_with('$') || s[..i].chars().skip(1).any(|c| !c.is_ascii_hexdigit()) {
            if let Some(base) = parse_number(&s[..i], symbols)? {
                let off = parse_number(&s[i + 1..], symbols)?.ok_or("Syntax Error: offset")?;
                return Ok(Some(if c == '+' { base.wrapping_add(off) } else { base.wrapping_sub(off) } & 0xFFFF));
            }
            return Ok(None);
        }
    }
    if let Some(h) = s.strip_prefix('$') {
        return u32::from_str_radix(h, 16).map(|v| Some(v & 0xFFFF)).map_err(|_| format!("Syntax Error: '{s}'"));
    }
    if s.bytes().all(|b| b.is_ascii_digit()) {
        return s.parse::<u32>().map(|v| Some(v & 0xFFFF)).map_err(|_| format!("Syntax Error: '{s}'"));
    }
    Ok(symbols.find_address(s).map(|(a, _)| a as u32))
}

impl Assembler {
    pub fn new(address: u16) -> Self {
        Assembler { address, delayed: Vec::new() }
    }

    /// Number of forward references still to be resolved (`AssemblerDelayedTargetsSize`).
    pub fn pending(&self) -> usize {
        self.delayed.len()
    }

    /// Assembles `text` ("LDA #$10", "BNE LOOP", ...) at [`Self::address`]; returns the bytes written.
    pub fn assemble(&mut self, mem: &mut dyn DebugMem, symbols: &Symbols, cmos: bool, text: &str) -> Result<usize, String> {
        self.resolve_delayed(mem, symbols);
        let text = text.split(';').next().unwrap_or("").trim();
        let (mn, rest) = match text.find(char::is_whitespace) {
            Some(i) => (&text[..i], text[i..].trim()),
            None => (text, ""),
        };
        let tbl = table(cmos);
        let mut cands: Vec<u8> = (0..=255u8).filter(|o| tbl[*o as usize].mnemonic.eq_ignore_ascii_case(mn)).collect();
        // Exact-case spelling first, then documented (upper-case) entries, then the aliases.
        cands.sort_by_key(|o| {
            let m = tbl[*o as usize].mnemonic;
            if m == mn {
                0
            } else if !tbl[*o as usize].is_extension() {
                1
            } else {
                2
            }
        });
        if cands.is_empty() {
            return Err("Syntax Error: Invalid mnemonic".into());
        }
        let has = |m: Mode| cands.iter().copied().find(|o| tbl[*o as usize].mode == m);
        let base = self.address;

        // Classify the operand text.
        enum Shape<'a> {
            None,
            Imm(&'a str),
            Ind(&'a str),
            IndX(&'a str),
            IndY(&'a str),
            IdxX(&'a str),
            IdxY(&'a str),
            Plain(&'a str),
        }
        let up = rest.to_ascii_uppercase();
        let shape = if rest.is_empty() || up == "A" {
            Shape::None
        } else if let Some(r) = rest.strip_prefix('#') {
            Shape::Imm(r)
        } else if rest.starts_with('(') {
            let compact: String = rest.chars().filter(|c| !c.is_whitespace()).collect();
            let cu = compact.to_ascii_uppercase();
            if cu.ends_with(",X)") {
                Shape::IndX(&rest[1..rest.rfind(',').ok_or("Syntax Error")?])
            } else if cu.ends_with("),Y") {
                Shape::IndY(&rest[1..rest.rfind(')').ok_or("Syntax Error")?])
            } else if cu.ends_with(')') && rest.trim_end().ends_with(')') {
                Shape::Ind(&rest[1..rest.rfind(')').ok_or("Syntax Error: Missing ')'")?])
            } else {
                return Err("Syntax Error: Missing ')'".into());
            }
        } else if let Some(i) = rest.rfind(',') {
            match rest[i + 1..].trim().to_ascii_uppercase().as_str() {
                "X" => Shape::IdxX(&rest[..i]),
                "Y" => Shape::IdxY(&rest[..i]),
                _ => return Err("Syntax Error: Index 'X' or 'Y'".into()),
            }
        } else {
            Shape::Plain(rest)
        };

        let operand_text = match &shape {
            Shape::None => "",
            Shape::Imm(t) | Shape::Ind(t) | Shape::IndX(t) | Shape::IndY(t) | Shape::IdxX(t) | Shape::IdxY(t) | Shape::Plain(t) => t,
        };
        // `#<sym` / `#>sym`
        let (hi_lo, operand_text) = match (&shape, operand_text.chars().next()) {
            (Shape::Imm(_), Some('<')) => (Some(false), &operand_text[1..]),
            (Shape::Imm(_), Some('>')) => (Some(true), &operand_text[1..]),
            _ => (None, operand_text),
        };
        let value = if matches!(shape, Shape::None) { Some(0) } else { parse_number(operand_text, symbols)? };
        let unresolved = value.is_none();
        let mut v = value.unwrap_or(0);
        if let Some(hi) = hi_lo {
            v = if hi { v >> 8 } else { v & 0xFF } & 0xFF;
        }
        let zp = v <= 0xFF && !unresolved;

        let (opcode, mode) = match shape {
            Shape::None => (has(Mode::Implied), Mode::Implied),
            Shape::Imm(_) => (has(Mode::Imm), Mode::Imm),
            Shape::Ind(_) => {
                if zp && has(Mode::IndZp).is_some() {
                    (has(Mode::IndZp), Mode::IndZp)
                } else {
                    (has(Mode::IndAbs), Mode::IndAbs)
                }
            }
            Shape::IndX(_) => {
                if zp && has(Mode::IndZpX).is_some() {
                    (has(Mode::IndZpX), Mode::IndZpX)
                } else {
                    (has(Mode::IndAbsX), Mode::IndAbsX)
                }
            }
            Shape::IndY(_) => (has(Mode::IndZpY), Mode::IndZpY),
            Shape::IdxX(_) => {
                if zp && has(Mode::ZpX).is_some() {
                    (has(Mode::ZpX), Mode::ZpX)
                } else {
                    (has(Mode::AbsX), Mode::AbsX)
                }
            }
            Shape::IdxY(_) => {
                if zp && has(Mode::ZpY).is_some() {
                    (has(Mode::ZpY), Mode::ZpY)
                } else {
                    (has(Mode::AbsY), Mode::AbsY)
                }
            }
            Shape::Plain(_) => {
                if has(Mode::Rel).is_some() {
                    (has(Mode::Rel), Mode::Rel)
                } else if zp && has(Mode::Zp).is_some() {
                    (has(Mode::Zp), Mode::Zp)
                } else {
                    (has(Mode::Abs), Mode::Abs)
                }
            }
        };
        let Some(opcode) = opcode else {
            return Err(format!("Syntax Error: {mn} does not support {} addressing", mode.name()));
        };

        let value16 = if mode == Mode::Rel && !unresolved {
            Self::rel_offset(base, v as u16)? as u16
        } else {
            v as u16
        };
        mem.write(base, opcode);
        let n = mode.bytes() as usize;
        if n > 1 {
            mem.write(base.wrapping_add(1), value16 as u8);
        }
        if n > 2 {
            mem.write(base.wrapping_add(2), (value16 >> 8) as u8);
        }
        if unresolved {
            if hi_lo.is_some() || matches!(mode, Mode::Imm) {
                return Err("Syntax Error: undefined symbol in immediate operand".into());
            }
            self.delayed.push(Delayed { name: operand_text.trim().to_string(), base, mode });
        }
        self.address = base.wrapping_add(n as u16);
        Ok(n)
    }

    /// Branch displacement from `base` to `target` (`_6502_CalcRelativeOffset`).
    fn rel_offset(base: u16, target: u16) -> Result<u8, String> {
        let d = target as i32 - base as i32 - 2;
        if !(-128..=127).contains(&d) {
            return Err("Branch out of range".into());
        }
        Ok(d as i8 as u8)
    }

    /// `AssemblerProcessDelayedSymols`: patches instructions whose symbol exists now.
    pub fn resolve_delayed(&mut self, mem: &mut dyn DebugMem, symbols: &Symbols) {
        let mut i = 0;
        while i < self.delayed.len() {
            let d = self.delayed[i].clone();
            if let Some((target, _)) = symbols.find_address(&d.name) {
                let value = if d.mode == Mode::Rel {
                    match Self::rel_offset(d.base, target) {
                        Ok(o) => o as u16,
                        Err(_) => {
                            i += 1;
                            continue;
                        }
                    }
                } else {
                    target
                };
                let n = d.mode.bytes();
                if n > 1 {
                    mem.write(d.base.wrapping_add(1), value as u8);
                }
                if n > 2 {
                    mem.write(d.base.wrapping_add(2), (value >> 8) as u8);
                }
                self.delayed.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disasm::*;
    use crate::mem::{Ram, Regs};
    use crate::symbols::Table;

    fn asm(src: &str, cmos: bool) -> Vec<u8> {
        let mut ram = Ram::default();
        let syms = Symbols::default();
        let mut a = Assembler::new(0x300);
        let n = a.assemble(&mut ram, &syms, cmos, src).unwrap();
        ram.0[0x300..0x300 + n].to_vec()
    }

    #[test]
    fn encodes_each_addressing_mode() {
        assert_eq!(asm("LDA #$41", false), [0xA9, 0x41]);
        assert_eq!(asm("lda $10", false), [0xA5, 0x10]);
        assert_eq!(asm("LDA $1234", false), [0xAD, 0x34, 0x12]);
        assert_eq!(asm("LDA $10,X", false), [0xB5, 0x10]);
        assert_eq!(asm("LDA $1234,Y", false), [0xB9, 0x34, 0x12]);
        assert_eq!(asm("LDA ($10,X)", false), [0xA1, 0x10]);
        assert_eq!(asm("LDA ($10),Y", false), [0xB1, 0x10]);
        assert_eq!(asm("JMP ($0300)", false), [0x6C, 0x00, 0x03]);
        assert_eq!(asm("JMP $FDED", false), [0x4C, 0xED, 0xFD]);
        assert_eq!(asm("JMP $10", false), [0x4C, 0x10, 0x00]);
        assert_eq!(asm("LDX $10,Y", false), [0xB6, 0x10]);
        assert_eq!(asm("LDA $10,Y", false), [0xB9, 0x10, 0x00]);
        assert_eq!(asm("ASL A", false), [0x0A]);
        assert_eq!(asm("NOP", false), [0xEA]);
        assert_eq!(asm("LDA #10", false), [0xA9, 10]); // bare numbers are decimal
    }

    #[test]
    fn cmos_only_modes() {
        assert_eq!(asm("LDA ($10)", true), [0xB2, 0x10]);
        assert_eq!(asm("JMP ($1234,X)", true), [0x7C, 0x34, 0x12]);
        assert_eq!(asm("STZ $10", true), [0x64, 0x10]);
        assert_eq!(asm("PHX", true), [0xDA]);
        assert_eq!(asm("BRA $302", true), [0x80, 0x00]);
        let mut ram = Ram::default();
        let mut a = Assembler::new(0x300);
        assert!(a.assemble(&mut ram, &Symbols::default(), false, "PHX").is_err());
    }

    #[test]
    fn branches_use_relative_offsets_and_check_range() {
        assert_eq!(asm("BNE $300", false), [0xD0, 0xFE]);
        assert_eq!(asm("BEQ $302", false), [0xF0, 0x00]);
        let mut ram = Ram::default();
        let mut a = Assembler::new(0x300);
        assert!(a.assemble(&mut ram, &Symbols::default(), false, "BNE $400").is_err());
    }

    #[test]
    fn symbols_and_forward_references() {
        let mut ram = Ram::default();
        let mut syms = Symbols::default();
        syms.set(Table::Main, "COUT", 0xFDED);
        let mut a = Assembler::new(0x300);
        a.assemble(&mut ram, &syms, false, "JSR COUT").unwrap();
        a.assemble(&mut ram, &syms, false, "BNE DONE").unwrap();
        a.assemble(&mut ram, &syms, false, "JMP LATER").unwrap();
        assert_eq!(a.pending(), 2);
        assert_eq!(&ram.0[0x300..0x303], [0x20, 0xED, 0xFD]);
        syms.set(Table::User1, "DONE", 0x310);
        syms.set(Table::User1, "LATER", 0x1234);
        a.assemble(&mut ram, &syms, false, "RTS").unwrap();
        assert_eq!(a.pending(), 0);
        assert_eq!(&ram.0[0x303..0x309], [0xD0, 0x0B, 0x4C, 0x34, 0x12, 0x60]);
    }

    #[test]
    fn round_trip_through_the_disassembler_for_every_opcode() {
        // Assemble the disassembly of each documented 65C02 opcode and compare the bytes.
        let syms = Symbols::default();
        let data = DataBlocks::default();
        let cfg = DisasmConfig { branch_type: 0, ..Default::default() };
        let mut checked = 0;
        for op in 0..=255u8 {
            let info = &OPCODES_65C02[op as usize];
            let mut ram = Ram::default();
            ram.load(0x300, &[op, 0x12, 0x03]);
            let ctx = DisasmCtx { mem: &ram, regs: Regs::default(), cmos: true, symbols: &syms, data: &data, cfg: &cfg };
            let l = disassemble(&ctx, 0x300);
            let spans = render_line(&l, None, &cfg);
            // Rebuild "MNEMONIC operand" from the instruction column on, dropping the pointer columns.
            let ins_col = Tabs::new(&cfg).instruction;
            let text = line_text(&spans);
            let tail: String = text.chars().skip(ins_col).collect();
            let imm_col = Tabs::new(&cfg).immediate - ins_col;
            let src: String = tail.chars().take(imm_col).collect::<String>().trim().to_string();
            let mut out = Ram::default();
            let mut a = Assembler::new(0x300);
            let res = a.assemble(&mut out, &syms, true, &src);
            let n = info.mode.bytes() as usize;
            let Ok(got) = res else { panic!("{op:02X} '{src}': {res:?}") };
            assert_eq!(got, n, "{op:02X} '{src}'");
            // Same instruction semantics: same mnemonic family and mode => same bytes unless aliased NOP.
            if info.mnemonic != "nop" {
                assert_eq!(out.0[0x300..0x300 + n], ram.0[0x300..0x300 + n], "{op:02X} '{src}'");
            }
            checked += 1;
        }
        assert_eq!(checked, 256);
    }
}
