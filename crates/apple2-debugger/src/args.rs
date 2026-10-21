//! Command-line parser (Debugger_Parser.cpp): `ArgsGet` splits the input into tokens, `ArgsCook`
//! evaluates registers, symbols, `$`/`#`/`@`/`( )` and left-to-right arithmetic (no precedence), and
//! leaves the range operators `:` `,` `/` for the commands (Debugger_Range.cpp).

use crate::mem::{DebugMem, Regs};
use crate::symbols::Symbols;

pub const MAX_ARGS: usize = 64;
pub const MAX_ARG_LEN: usize = 172;

/// Operators and punctuation recognised as separate arguments (`g_aTokens`, :53).
const SINGLE: &str = "&@{}[]\\^:,$=!/>#<-()%|+';\"* ~";
/// Of those, the ones typed `TYPE_OPERATOR`.
const OPERATORS: &str = "&@\\^:,=!/>#<-()%|+*~";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Alphanumeric text (also anything that is not a token).
    Text,
    /// `"..."`
    Quoted2,
    /// `'...'`
    Quoted1,
    /// An operator token; `text` holds it.
    Operator,
    /// A non-operator token: `$ { } [ ]`.
    Punct,
}

#[derive(Clone, Debug)]
pub struct Arg {
    pub text: String,
    pub kind: Kind,
    /// Cooked numeric value (`nValue`).
    pub value: u16,
    /// Value already final (`TYPE_VALUE`): arithmetic was applied.
    pub value_set: bool,
    /// Resolved through a symbol table.
    pub is_symbol: bool,
    pub no_reg: bool,
    pub no_sym: bool,
}

impl Arg {
    fn new(text: &str, kind: Kind) -> Arg {
        Arg {
            text: text.to_string(),
            kind,
            value: 0,
            value_set: false,
            is_symbol: false,
            no_reg: false,
            no_sym: false,
        }
    }

    pub fn is_op(&self, op: &str) -> bool {
        self.kind == Kind::Operator && self.text == op
    }

    pub fn is_operator(&self) -> bool {
        self.kind == Kind::Operator
    }

    pub fn quoted(&self) -> bool {
        matches!(self.kind, Kind::Quoted1 | Kind::Quoted2)
    }
}

/// Parsed command line. `cooked[0]` is the command; `n()` is the number of arguments after it.
#[derive(Clone, Debug, Default)]
pub struct Args {
    pub raw: Vec<Arg>,
    pub cooked: Vec<Arg>,
}

impl Args {
    pub fn n(&self) -> usize {
        self.cooked.len().saturating_sub(1)
    }

    pub fn cmd(&self) -> &str {
        self.cooked.first().map(|a| a.text.as_str()).unwrap_or("")
    }

    pub fn get(&self, i: usize) -> Option<&Arg> {
        self.cooked.get(i)
    }

    pub fn value(&self, i: usize) -> u16 {
        self.cooked.get(i).map(|a| a.value).unwrap_or(0)
    }

    pub fn text(&self, i: usize) -> &str {
        self.cooked.get(i).map(|a| a.text.as_str()).unwrap_or("")
    }

    pub fn raw_text(&self, i: usize) -> &str {
        self.raw.get(i).map(|a| a.text.as_str()).unwrap_or("")
    }

    /// Text of the line after the command, as typed (`g_pConsoleFirstArg`).
    pub fn rest_of(line: &str) -> &str {
        let l = line.trim_start();
        match l.find(|c: char| c.is_whitespace()) {
            Some(i) => l[i..].trim_start(),
            None => "",
        }
    }
}

fn is_token_char(c: char) -> bool {
    SINGLE.contains(c) || c == '\t'
}

/// `ArgsGet`: splits `input` into raw arguments; the command is element 0.
pub fn tokenize(input: &str) -> Vec<Arg> {
    let ch: Vec<char> = input.trim_end().chars().collect();
    let mut out: Vec<Arg> = Vec::new();
    let mut i = 0;
    while i < ch.len() && out.len() < MAX_ARGS {
        while i < ch.len() && (ch[i] == ' ' || ch[i] == '\t') {
            i += 1;
        }
        if i >= ch.len() {
            break;
        }
        let c = ch[i];
        // multi-char tokens
        let two: String = ch[i..(i + 2).min(ch.len())].iter().collect();
        if ["//", ">=", "<=", "!="].contains(&two.as_str()) {
            if two == "//" && out.is_empty() {
                break;
            }
            out.push(Arg::new(&two, Kind::Operator));
            i += 2;
            continue;
        }
        if c == ';' {
            break;
        }
        if c == '"' || c == '\'' {
            let start = i + 1;
            let mut end = start;
            while end < ch.len() && ch[end] != c {
                end += 1;
            }
            let s: String = ch[start..end].iter().collect();
            if !s.is_empty() {
                out.push(Arg::new(
                    &s,
                    if c == '"' {
                        Kind::Quoted2
                    } else {
                        Kind::Quoted1
                    },
                ));
            }
            i = (end + 1).min(ch.len().max(end + 1));
            continue;
        }
        if !c.is_alphanumeric() && is_token_char(c) {
            let kind = if OPERATORS.contains(c) {
                Kind::Operator
            } else {
                Kind::Punct
            };
            out.push(Arg::new(&c.to_string(), kind));
            i += 1;
            continue;
        }
        // alphanumeric run (or a non-token character such as '.' or '?')
        let mut j = i + 1;
        while j < ch.len() && !is_token_char(ch[j]) {
            j += 1;
        }
        let s: String = ch[i..j].iter().take(MAX_ARG_LEN).collect();
        out.push(Arg::new(&s, Kind::Text));
        i = j;
    }
    out
}

/// `strtoul(s, &end, 16) & 0xFFFF`: leading hex digits, 0 if none.
pub fn hex_prefix(s: &str) -> u16 {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let mut v: u32 = 0;
    for c in s.chars() {
        match c.to_digit(16) {
            Some(d) => v = v << 4 | d,
            None => break,
        }
    }
    v as u16
}

/// What the cooker needs from the debugger.
pub struct CookCtx<'a> {
    pub regs: Regs,
    pub symbols: &'a Symbols,
    pub mem: &'a dyn DebugMem,
    /// `g_vMemorySearchResults` for `@n`.
    pub search_results: &'a [u16],
}

/// `ArgsGetRegisterValue`: A X Y P S (one letter) or PC.
pub fn register_value(text: &str, regs: &Regs) -> Option<u16> {
    match text.to_ascii_uppercase().as_str() {
        "A" => Some(regs.a as u16),
        "X" => Some(regs.x as u16),
        "Y" => Some(regs.y as u16),
        "P" => Some(regs.p as u16),
        "S" => Some(regs.sp as u16),
        "PC" => Some(regs.pc),
        _ => None,
    }
}

/// Value of an operand text: register, symbol, then hex.
fn operand_value(a: &Arg, ctx: &CookCtx) -> u16 {
    if !a.no_reg {
        if let Some(v) = register_value(&a.text, &ctx.regs) {
            return v;
        }
    }
    if !a.no_sym {
        if let Some((v, _)) = ctx.symbols.find_address(&a.text) {
            return v;
        }
    }
    hex_prefix(&a.text)
}

/// `ArgsCook`. Errors with "Syntax Error".
pub fn cook(raw: &[Arg], ctx: &CookCtx) -> Result<Vec<Arg>, String> {
    let mut a: Vec<Arg> = raw.to_vec();
    if a.is_empty() {
        return Ok(a);
    }
    let err = || "Syntax Error".to_string();
    let mut n = a.len() - 1;
    let mut i = 1usize;
    while i <= n {
        // `$` forces an address: drop it and do not treat the next argument as a register.
        if a[i].text == "$" && a[i].kind == Kind::Punct {
            if n - i == 0 {
                return Err(err());
            }
            a.remove(i);
            n -= 1;
            a[i].no_reg = true;
        }
        if a[i].is_operator() {
            let tok = a[i].text.clone();
            let left = n - i;
            let need = (tok == "#") as usize;
            if left < need {
                return Err(err());
            }
            if tok == "*" && n == 1 {
                // wildcard as the only argument
            } else if left > 0 {
                let next_val = |a: &Vec<Arg>, i: usize| -> u16 {
                    // `ArgsGetImmediateValue` then register, else the plain value of the next arg
                    if a[i + 1].is_op("#") && i + 2 <= n {
                        return a[i + 2].value.max(hex_prefix(&a[i + 2].text));
                    }
                    operand_value(&a[i + 1], ctx)
                };
                match tok.as_str() {
                    "+" | "-" | "*" | "%" | "//" | "&" | "|" | "^" => {
                        let rhs = next_val(&a, i);
                        let l = a[i - 1].value;
                        a[i - 1].value = match tok.as_str() {
                            "+" => l.wrapping_add(rhs),
                            "-" => l.wrapping_sub(rhs),
                            "*" => l.wrapping_mul(rhs),
                            "%" => l.checked_rem(rhs).unwrap_or(0),
                            "//" => l / rhs.max(1),
                            "&" => l & rhs,
                            "|" => l | rhs,
                            _ => l ^ rhs,
                        };
                        a[i - 1].value_set = true;
                        a.drain(i..i + 2);
                        n -= 2;
                        i = 0;
                    }
                    "=" => {
                        let rhs = operand_value(&a[i + 1], ctx);
                        a[i - 1].value = rhs;
                        a[i - 1].value_set = true;
                    }
                    "@" => {
                        let idx = hex_prefix(&a[i + 1].text) as usize;
                        let mut r = a.remove(i + 1);
                        n -= 1;
                        r.value = ctx.search_results.get(idx).copied().unwrap_or(0);
                        r.value_set = true;
                        r.no_reg = true;
                        r.no_sym = true;
                        a[i] = r;
                    }
                    "#" => {
                        let rhs = hex_prefix(&a[i + 1].text);
                        a[i].value = rhs;
                        a[i].value_set = true;
                        a[i].no_reg = true;
                        a[i].no_sym = true;
                    }
                    "!" => {
                        let rhs = next_val(&a, i);
                        a[i].value = !rhs;
                        a[i].value_set = true;
                    }
                    "(" => {
                        if left >= 2 && a[i + 2].is_op(")") {
                            let addr = operand_value(&a[i + 1], ctx);
                            let mut r = a[i + 1].clone();
                            r.value = ctx.mem.read_word(addr);
                            r.value_set = true;
                            r.no_reg = true;
                            r.is_symbol = false;
                            a.drain(i..i + 3);
                            a.insert(i, r);
                            n -= 2;
                        } else {
                            return Err(err());
                        }
                    }
                    ")" => return Err(err()),
                    _ => {} // : , / < > ~ \ are for the commands
                }
            } else {
                return Err(err());
            }
        } else if !a[i].value_set {
            let mut v = hex_prefix(&a[i].text);
            if !a[i].no_reg {
                if let Some(r) = register_value(&a[i].text, &ctx.regs) {
                    v = r;
                }
            }
            if !a[i].no_sym {
                if let Some((s, _)) = ctx.symbols.find_address(&a[i].text) {
                    v = s;
                    a[i].is_symbol = true;
                }
            }
            a[i].value = v;
        }
        i += 1;
    }
    a.truncate(n + 1);
    Ok(a)
}

/// Range kind of `Range_Get`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeType {
    MissingArg2,
    HasEnd,
    HasLen,
}

/// `Range_Get(a1, a2, iArg)`: `addr`, `addr:end` (ordered) or `addr,len`.
pub fn range_get(args: &Args, i: usize) -> (u16, u16, RangeType) {
    let a1 = args.value(i);
    match args.get(i + 1) {
        Some(t) if t.is_op(",") && args.get(i + 2).is_some() => {
            (a1, args.value(i + 2), RangeType::HasLen)
        }
        Some(t) if t.is_op(":") && args.get(i + 2).is_some() => {
            let e = args.value(i + 2);
            if a1 > e {
                (e, a1, RangeType::HasEnd)
            } else {
                (a1, e, RangeType::HasEnd)
            }
        }
        _ => (a1, 0, RangeType::MissingArg2),
    }
}

/// `Range_CalcEndLen`: (inclusive end, length).
pub fn range_end_len(r: RangeType, a1: u16, a2: u16) -> Option<(u16, u32)> {
    match r {
        RangeType::HasLen => {
            let end = (a1 as u32 + a2 as u32).saturating_sub(1).min(0xFFFF);
            Some((end as u16, a2 as u32))
        }
        RangeType::HasEnd => Some((a2, a2 as u32 - a1 as u32 + 1)),
        RangeType::MissingArg2 => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::Ram;
    use crate::symbols::Table;

    fn cooked(line: &str, regs: Regs, syms: &Symbols, mem: &Ram) -> Args {
        let raw = tokenize(line);
        let ctx = CookCtx {
            regs,
            symbols: syms,
            mem,
            search_results: &[10, 20],
        };
        Args {
            cooked: cook(&raw, &ctx).unwrap(),
            raw,
        }
    }

    fn texts(line: &str) -> Vec<String> {
        tokenize(line).into_iter().map(|a| a.text).collect()
    }

    #[test]
    fn tokenizer_splits_operators_and_keeps_names_together() {
        assert_eq!(texts("BPR A = 10"), ["BPR", "A", "=", "10"]);
        assert_eq!(texts("m 300:3ff"), ["m", "300", ":", "3ff"]);
        assert_eq!(texts("HELP BREAKPOINTS"), ["HELP", "BREAKPOINTS"]);
        assert_eq!(texts("bp $FDED ; comment"), ["bp", "$", "FDED"]);
        assert_eq!(
            texts("S 300:3FF \"Hi there\""),
            ["S", "300", ":", "3FF", "Hi there"]
        );
        assert_eq!(texts("BPM 300 >= 5"), ["BPM", "300", ">=", "5"]);
        assert_eq!(texts("// whole line comment"), Vec::<String>::new());
        assert_eq!(texts("D.COS0+1"), ["D.COS0", "+", "1"]);
    }

    #[test]
    fn hex_default_registers_symbols_and_arithmetic() {
        let mut syms = Symbols::default();
        syms.set(Table::Main, "COUT", 0xFDED);
        let mem = Ram::default();
        let regs = Regs {
            a: 0x41,
            x: 2,
            y: 3,
            sp: 0xF0,
            pc: 0x300,
            p: 0x30,
        };
        assert_eq!(cooked("u fded", regs, &syms, &mem).value(1), 0xFDED);
        assert_eq!(cooked("u cout", regs, &syms, &mem).value(1), 0xFDED);
        assert_eq!(cooked("u pc + 3", regs, &syms, &mem).value(1), 0x303);
        assert_eq!(cooked("u cout - 1D", regs, &syms, &mem).value(1), 0xFDD0);
        assert_eq!(cooked("u a * 2 + 1", regs, &syms, &mem).value(1), 0x83); // left to right
        assert_eq!(cooked("u 100 & F0", regs, &syms, &mem).value(1), 0);
        assert_eq!(cooked("u 1 | 6 ^ 3", regs, &syms, &mem).value(1), 4);
        assert_eq!(cooked("u 10 // 3", regs, &syms, &mem).value(1), 5);
        assert_eq!(cooked("u 10 % 3", regs, &syms, &mem).value(1), 1);
        assert_eq!(cooked("u ! 0", regs, &syms, &mem).value(1), 0xFFFF);
        // `$` stops register lookup: $A is the number 10.
        assert_eq!(cooked("u $a", regs, &syms, &mem).value(1), 0x0A);
        assert_eq!(cooked("u s", regs, &syms, &mem).value(1), 0xF0);
    }

    #[test]
    fn indirection_and_search_results() {
        let syms = Symbols::default();
        let mut mem = Ram::default();
        mem.load(0x3F2, &[0x00, 0x03]);
        let regs = Regs::default();
        assert_eq!(cooked("u (3F2)", regs, &syms, &mem).value(1), 0x300);
        assert_eq!(cooked("u @1", regs, &syms, &mem).value(1), 20);
        assert_eq!(cooked("u @9", regs, &syms, &mem).value(1), 0);
    }

    #[test]
    fn ranges() {
        let syms = Symbols::default();
        let mem = Ram::default();
        let regs = Regs::default();
        let a = cooked("m 300:30F", regs, &syms, &mem);
        let (s, e, t) = range_get(&a, 1);
        assert_eq!((s, e, t), (0x300, 0x30F, RangeType::HasEnd));
        assert_eq!(range_end_len(t, s, e), Some((0x30F, 16)));
        let a = cooked("m 2000,10", regs, &syms, &mem);
        let (s, l, t) = range_get(&a, 1);
        assert_eq!(range_end_len(t, s, l), Some((0x200F, 0x10)));
        let a = cooked("m 3FF:300", regs, &syms, &mem);
        assert_eq!(range_get(&a, 1).0, 0x300); // swapped
        assert_eq!(
            range_get(&cooked("m 300", regs, &syms, &mem), 1).2,
            RangeType::MissingArg2
        );
    }

    #[test]
    fn syntax_errors() {
        let syms = Symbols::default();
        let mem = Ram::default();
        let raw = tokenize("u 300 +");
        let ctx = CookCtx {
            regs: Regs::default(),
            symbols: &syms,
            mem: &mem,
            search_results: &[],
        };
        assert!(cook(&raw, &ctx).is_err());
        let raw = tokenize("u (300");
        assert!(cook(&raw, &ctx).is_err());
    }
}
