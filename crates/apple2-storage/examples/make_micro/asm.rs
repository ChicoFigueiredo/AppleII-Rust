//! A tiny two-pass 6502 assembler: just enough for the oracle micro-programs.
//!
//! Syntax: `label:`, `lda #$12`, `sta $c08d,x`, `lda ($06),y`, `bne label`, `.org $0801`,
//! `.byte 1,2,$ff`, `.fill 10,$ea`, `.word label`. Numbers: `$hex`, decimal, `<label`, `>label`,
//! labels, `label+3`, `*` (current address). A literal below $100 is zero page when the
//! mnemonic has a zero-page form; labels are always absolute. Comments start with `;`.

use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Imp,
    Acc,
    Imm,
    Zp,
    Zpx,
    Zpy,
    Abs,
    Abx,
    Aby,
    Inx,
    Iny,
    Ind,
    Rel,
}

fn opcode(mn: &str, mode: Mode) -> Option<u8> {
    use Mode::*;
    let t: &[(Mode, u8)] = match mn {
        "lda" => &[
            (Imm, 0xA9),
            (Zp, 0xA5),
            (Zpx, 0xB5),
            (Abs, 0xAD),
            (Abx, 0xBD),
            (Aby, 0xB9),
            (Inx, 0xA1),
            (Iny, 0xB1),
        ],
        "ldx" => &[
            (Imm, 0xA2),
            (Zp, 0xA6),
            (Zpy, 0xB6),
            (Abs, 0xAE),
            (Aby, 0xBE),
        ],
        "ldy" => &[
            (Imm, 0xA0),
            (Zp, 0xA4),
            (Zpx, 0xB4),
            (Abs, 0xAC),
            (Abx, 0xBC),
        ],
        "sta" => &[
            (Zp, 0x85),
            (Zpx, 0x95),
            (Abs, 0x8D),
            (Abx, 0x9D),
            (Aby, 0x99),
            (Inx, 0x81),
            (Iny, 0x91),
        ],
        "stx" => &[(Zp, 0x86), (Zpy, 0x96), (Abs, 0x8E)],
        "sty" => &[(Zp, 0x84), (Zpx, 0x94), (Abs, 0x8C)],
        "cmp" => &[
            (Imm, 0xC9),
            (Zp, 0xC5),
            (Zpx, 0xD5),
            (Abs, 0xCD),
            (Abx, 0xDD),
            (Aby, 0xD9),
            (Inx, 0xC1),
            (Iny, 0xD1),
        ],
        "cpx" => &[(Imm, 0xE0), (Zp, 0xE4), (Abs, 0xEC)],
        "cpy" => &[(Imm, 0xC0), (Zp, 0xC4), (Abs, 0xCC)],
        "adc" => &[
            (Imm, 0x69),
            (Zp, 0x65),
            (Abs, 0x6D),
            (Abx, 0x7D),
            (Aby, 0x79),
        ],
        "sbc" => &[
            (Imm, 0xE9),
            (Zp, 0xE5),
            (Abs, 0xED),
            (Abx, 0xFD),
            (Aby, 0xF9),
        ],
        "and" => &[
            (Imm, 0x29),
            (Zp, 0x25),
            (Abs, 0x2D),
            (Abx, 0x3D),
            (Aby, 0x39),
        ],
        "ora" => &[
            (Imm, 0x09),
            (Zp, 0x05),
            (Abs, 0x0D),
            (Abx, 0x1D),
            (Aby, 0x19),
        ],
        "eor" => &[
            (Imm, 0x49),
            (Zp, 0x45),
            (Abs, 0x4D),
            (Abx, 0x5D),
            (Aby, 0x59),
        ],
        "inc" => &[(Zp, 0xE6), (Abs, 0xEE), (Abx, 0xFE)],
        "dec" => &[(Zp, 0xC6), (Abs, 0xCE), (Abx, 0xDE)],
        "asl" => &[(Acc, 0x0A), (Zp, 0x06), (Abs, 0x0E)],
        "lsr" => &[(Acc, 0x4A), (Zp, 0x46), (Abs, 0x4E)],
        "rol" => &[(Acc, 0x2A), (Zp, 0x26), (Abs, 0x2E)],
        "ror" => &[(Acc, 0x6A), (Zp, 0x66), (Abs, 0x6E)],
        "bit" => &[(Zp, 0x24), (Abs, 0x2C)],
        "jmp" => &[(Abs, 0x4C), (Ind, 0x6C)],
        "jsr" => &[(Abs, 0x20)],
        "bpl" => &[(Rel, 0x10)],
        "bmi" => &[(Rel, 0x30)],
        "bvc" => &[(Rel, 0x50)],
        "bvs" => &[(Rel, 0x70)],
        "bcc" => &[(Rel, 0x90)],
        "bcs" => &[(Rel, 0xB0)],
        "bne" => &[(Rel, 0xD0)],
        "beq" => &[(Rel, 0xF0)],
        "rts" => &[(Imp, 0x60)],
        "brk" => &[(Imp, 0x00)],
        "inx" => &[(Imp, 0xE8)],
        "iny" => &[(Imp, 0xC8)],
        "dex" => &[(Imp, 0xCA)],
        "dey" => &[(Imp, 0x88)],
        "tax" => &[(Imp, 0xAA)],
        "txa" => &[(Imp, 0x8A)],
        "tay" => &[(Imp, 0xA8)],
        "tya" => &[(Imp, 0x98)],
        "tsx" => &[(Imp, 0xBA)],
        "txs" => &[(Imp, 0x9A)],
        "pha" => &[(Imp, 0x48)],
        "pla" => &[(Imp, 0x68)],
        "php" => &[(Imp, 0x08)],
        "plp" => &[(Imp, 0x28)],
        "clc" => &[(Imp, 0x18)],
        "sec" => &[(Imp, 0x38)],
        "cld" => &[(Imp, 0xD8)],
        "sed" => &[(Imp, 0xF8)],
        "cli" => &[(Imp, 0x58)],
        "sei" => &[(Imp, 0x78)],
        "clv" => &[(Imp, 0xB8)],
        "nop" => &[(Imp, 0xEA)],
        _ => return None,
    };
    t.iter().find(|(m, _)| *m == mode).map(|(_, o)| *o)
}

fn size(mode: Mode) -> usize {
    match mode {
        Mode::Imp | Mode::Acc => 1,
        Mode::Imm | Mode::Zp | Mode::Zpx | Mode::Zpy | Mode::Inx | Mode::Iny | Mode::Rel => 2,
        Mode::Abs | Mode::Abx | Mode::Aby | Mode::Ind => 3,
    }
}

fn eval(
    e: &str,
    labels: &HashMap<String, u16>,
    pc: u16,
    final_pass: bool,
) -> Result<(i32, bool), String> {
    // returns (value, is_literal_zero_page_candidate)
    let e = e.trim();
    if let Some(r) = e.strip_prefix('<') {
        return Ok((eval(r, labels, pc, final_pass)?.0 & 0xFF, true));
    }
    if let Some(r) = e.strip_prefix('>') {
        return Ok(((eval(r, labels, pc, final_pass)?.0 >> 8) & 0xFF, true));
    }
    for (i, c) in e.char_indices().skip(1) {
        if c == '+' || c == '-' {
            let (l, _) = eval(&e[..i], labels, pc, final_pass)?;
            let (r, _) = eval(&e[i + 1..], labels, pc, final_pass)?;
            return Ok((if c == '+' { l + r } else { l - r }, false));
        }
    }
    if e == "*" {
        return Ok((pc as i32, false));
    }
    if let Some(h) = e.strip_prefix('$') {
        let v = i32::from_str_radix(h, 16).map_err(|_| format!("bad hex `{e}`"))?;
        return Ok((v, v < 0x100));
    }
    if e.bytes().all(|b| b.is_ascii_digit()) && !e.is_empty() {
        let v: i32 = e.parse().map_err(|_| format!("bad number `{e}`"))?;
        return Ok((v, v < 0x100));
    }
    match labels.get(e) {
        Some(v) => Ok((*v as i32, false)),
        None if !final_pass => Ok((0x1000, false)),
        None => Err(format!("unknown label `{e}`")),
    }
}

fn parse_operand(mn: &str, op: &str) -> (Mode, String) {
    let op = op.trim();
    if op.is_empty() {
        return (
            if matches!(mn, "asl" | "lsr" | "rol" | "ror") {
                Mode::Acc
            } else {
                Mode::Imp
            },
            String::new(),
        );
    }
    if op.eq_ignore_ascii_case("a") {
        return (Mode::Acc, String::new());
    }
    if let Some(r) = op.strip_prefix('#') {
        return (Mode::Imm, r.to_string());
    }
    let lower = op.to_ascii_lowercase();
    if lower.starts_with('(') {
        if let Some(r) = lower.strip_suffix("),y") {
            return (Mode::Iny, r[1..].to_string());
        }
        if let Some(r) = lower.strip_suffix(",x)") {
            return (Mode::Inx, r[1..].to_string());
        }
        if let Some(r) = lower.strip_suffix(')') {
            return (Mode::Ind, r[1..].to_string());
        }
    }
    if let Some(r) = lower.strip_suffix(",x") {
        return (Mode::Abx, r.to_string()); // narrowed to Zpx later
    }
    if let Some(r) = lower.strip_suffix(",y") {
        return (Mode::Aby, r.to_string());
    }
    if mn.starts_with('b') && opcode(mn, Mode::Rel).is_some() {
        return (Mode::Rel, op.to_string());
    }
    (Mode::Abs, op.to_string())
}

pub fn assemble(src: &str) -> Result<(u16, Vec<u8>), String> {
    let mut labels: HashMap<String, u16> = HashMap::new();
    let mut result = (0u16, Vec::new());
    for pass in 0..2 {
        let final_pass = pass == 1;
        let mut pc: u16 = 0;
        let mut org: Option<u16> = None;
        let mut out: Vec<u8> = Vec::new();
        for (n, raw) in src.lines().enumerate() {
            let err = |m: String| format!("line {}: {m}: `{}`", n + 1, raw.trim());
            let mut line = raw.split(';').next().unwrap().trim().to_ascii_lowercase();
            while let Some(i) = line.find(':') {
                let (l, rest) = line.split_at(i);
                let name = l.trim().to_string();
                if name.is_empty() || name.contains(' ') {
                    break;
                }
                if !final_pass && labels.insert(name.clone(), pc).is_some() {
                    return Err(err(format!("label `{name}` defined twice")));
                }
                line = rest[1..].trim().to_string();
            }
            if line.is_empty() {
                continue;
            }
            let (mn, rest) = line.split_once(char::is_whitespace).unwrap_or((&line, ""));
            let mn = mn.to_ascii_lowercase();
            match mn.as_str() {
                ".org" => {
                    let (v, _) = eval(rest, &labels, pc, final_pass).map_err(err)?;
                    pc = v as u16;
                    org.get_or_insert(pc);
                }
                ".byte" => {
                    for part in rest.split(',') {
                        let (v, _) = eval(part, &labels, pc, final_pass).map_err(err)?;
                        out.push(v as u8);
                        pc = pc.wrapping_add(1);
                    }
                }
                ".word" => {
                    for part in rest.split(',') {
                        let (v, _) = eval(part, &labels, pc, final_pass).map_err(err)?;
                        out.extend((v as u16).to_le_bytes());
                        pc = pc.wrapping_add(2);
                    }
                }
                ".fill" => {
                    let (a, b) = rest.split_once(',').ok_or_else(|| err("fill n,v".into()))?;
                    let (n, _) = eval(a, &labels, pc, final_pass).map_err(err)?;
                    let (v, _) = eval(b, &labels, pc, final_pass).map_err(err)?;
                    out.extend(std::iter::repeat_n(v as u8, n as usize));
                    pc = pc.wrapping_add(n as u16);
                }
                _ => {
                    org.get_or_insert(pc);
                    let (mut mode, expr) = parse_operand(&mn, rest);
                    let (val, small) = if expr.is_empty() {
                        (0, false)
                    } else {
                        eval(&expr, &labels, pc, final_pass).map_err(err)?
                    };
                    // narrow absolute to zero page when a literal < $100 and the form exists
                    if small
                        || (val < 0x100
                            && matches!(mode, Mode::Abs | Mode::Abx | Mode::Aby)
                            && expr.trim().starts_with([
                                '$', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
                            ])
                            && !expr.contains('+'))
                    {
                        let zp = match mode {
                            Mode::Abs => Mode::Zp,
                            Mode::Abx => Mode::Zpx,
                            Mode::Aby => Mode::Zpy,
                            m => m,
                        };
                        if opcode(&mn, zp).is_some() {
                            mode = zp;
                        }
                    }
                    let op = opcode(&mn, mode)
                        .ok_or_else(|| err(format!("`{mn}` has no {mode:?} form")))?;
                    out.push(op);
                    match size(mode) {
                        1 => {}
                        2 if mode == Mode::Rel => {
                            let off = val - (pc as i32 + 2);
                            if final_pass && !(-128..=127).contains(&off) {
                                return Err(err(format!("branch out of range ({off})")));
                            }
                            out.push(off as u8);
                        }
                        2 => out.push(val as u8),
                        _ => out.extend((val as u16).to_le_bytes()),
                    }
                    pc = pc.wrapping_add(size(mode) as u16);
                }
            }
        }
        result = (org.unwrap_or(0), out);
    }
    Ok(result)
}
