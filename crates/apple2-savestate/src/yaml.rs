//! The YAML subset AppleWin reads and writes (YamlHelper.cpp / YamlHelper.h).
//!
//! AppleWin emits block-style maps only, with `fprintf` (no libyaml emitter), two-space indents
//! and a top level that repeats the key `Unit`, so a general YAML crate (which would collapse or
//! reject duplicate keys, and cannot reproduce the hex conventions) is a poor fit. This module is
//! a small line parser for that grammar and a writer mirroring `YamlSaveHelper`.

use crate::Error;
use std::fmt::Write as _;

/// A parsed value: a scalar or an ordered map (duplicate keys kept, as at the top level).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Scalar(String),
    Map(Vec<(String, Node)>),
}

/// Borrowed view of a map with `YamlLoadHelper`-style typed getters. Errors read
/// `"<map name>: Missing: <key>"` like AppleWin's.
#[derive(Clone, Copy)]
pub struct Map<'a> {
    pub name: &'a str,
    pub entries: &'a [(String, Node)],
}

/// `strtoul(s, NULL, 0)`: `0x` hex, leading `0` octal, else decimal; stops at the first bad
/// character and returns 0 for no digits.
pub fn strtoul(s: &str) -> u64 {
    let s = s.trim();
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let (radix, digits) = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (16, h)
    } else if s.len() > 1 && s.starts_with('0') {
        (8, &s[1..])
    } else {
        (10, s)
    };
    let end = digits.find(|c: char| !c.is_digit(radix)).unwrap_or(digits.len());
    let v = u64::from_str_radix(&digits[..end], radix).unwrap_or(0);
    if neg {
        v.wrapping_neg()
    } else {
        v
    }
}

impl<'a> Map<'a> {
    pub fn root(entries: &'a [(String, Node)]) -> Self {
        Map { name: "", entries }
    }

    fn find(&self, key: &str) -> Option<&'a Node> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, n)| n)
    }

    pub fn has(&self, key: &str) -> bool {
        self.find(key).is_some()
    }

    fn missing(&self, key: &str) -> Error {
        Error::Format(format!("{}: Missing: {key}", self.name))
    }

    /// Sub-map `key`; `None` when absent or a scalar.
    pub fn sub(&self, key: &str) -> Option<Map<'a>> {
        match self.entries.iter().find(|(k, _)| k == key) {
            Some((k, Node::Map(m))) => Some(Map { name: k, entries: m }),
            _ => None,
        }
    }

    /// Sub-map `key`, or an error. A key with an empty value (`State:` of a stateless card)
    /// counts as an empty map.
    pub fn need_sub(&self, key: &str) -> Result<Map<'a>, Error> {
        match self.entries.iter().find(|(k, _)| k == key) {
            Some((k, Node::Map(m))) => Ok(Map { name: k, entries: m }),
            Some((k, Node::Scalar(s))) if s.is_empty() => Ok(Map { name: k, entries: &[] }),
            _ => Err(Error::Format(format!("{}: Expected sub-map name: {key}", self.name))),
        }
    }

    pub fn str_opt(&self, key: &str) -> Option<&'a str> {
        match self.find(key) {
            Some(Node::Scalar(s)) => Some(s),
            _ => None,
        }
    }

    pub fn string(&self, key: &str) -> Result<String, Error> {
        // GH#1510: undo backslash doubling.
        self.str_opt(key).map(|s| s.replace("\\\\", "\\")).ok_or_else(|| self.missing(key))
    }

    fn nonempty(&self, key: &str) -> Result<&'a str, Error> {
        match self.str_opt(key) {
            Some(s) if !s.is_empty() => Ok(s),
            _ => Err(self.missing(key)),
        }
    }

    pub fn u64(&self, key: &str) -> Result<u64, Error> {
        self.nonempty(key).map(strtoul)
    }

    pub fn u32(&self, key: &str) -> Result<u32, Error> {
        self.u64(key).map(|v| v as u32)
    }

    pub fn i32(&self, key: &str) -> Result<i32, Error> {
        self.u64(key).map(|v| v as i32)
    }

    pub fn bool(&self, key: &str) -> Result<bool, Error> {
        match self.str_opt(key) {
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            _ => Err(self.missing(key)),
        }
    }

    pub fn f64(&self, key: &str) -> Result<f64, Error> {
        self.nonempty(key).map(|s| s.trim().parse().unwrap_or(0.0))
    }

    /// Decodes a `YamlSaveHelper::SaveMemory` block (`AAAA: hex` lines, any order) into `dst`
    /// (`YamlHelper::LoadMemory`); returns the bytes loaded.
    pub fn load_memory(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut bytes = 0;
        for (k, n) in self.entries {
            let addr = u64::from_str_radix(k.trim_start_matches("0x"), 16).unwrap_or(0) as usize;
            if addr >= dst.len() {
                return Err(Error::Format(format!("Memory: line address too big: {k}")));
            }
            let Node::Scalar(v) = n else { return Err(Error::Format("Memory: unexpected sub-map".into())) };
            if v.len() % 2 != 0 {
                return Err(Error::Format(format!("Memory: hex data must be an even number of nibbles on line address: {k}")));
            }
            for (p, pair) in (addr..).zip(v.as_bytes().chunks(2)) {
                let hex = |c: u8| (c as char).to_digit(16);
                let (Some(h), Some(l)) = (hex(pair[0]), hex(pair[1])) else {
                    return Err(Error::Format(format!("Memory: hex data contains illegal character on line address: {k}")));
                };
                if p >= dst.len() {
                    return Err(Error::Format(format!("Memory: hex data overflowed address space on line address: {k}")));
                }
                dst[p] = (h * 16 + l) as u8;
                bytes += 1;
            }
        }
        Ok(bytes)
    }
}

fn strip_comment(line: &str) -> &str {
    let b = line.as_bytes();
    let mut quote = 0u8;
    for i in 0..b.len() {
        match b[i] {
            b'"' | b'\'' if quote == 0 && (i == 0 || b[i - 1] == b' ' || b[i - 1] == b':') => quote = b[i],
            q if q == quote => quote = 0,
            b'\\' if quote == b'"' => {}
            b'#' if quote == 0 && (i == 0 || b[i - 1] == b' ' || b[i - 1] == b'\t') => return &line[..i],
            _ => {}
        }
    }
    line
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        let inner = &v[1..v.len() - 1];
        let mut out = String::new();
        let mut it = inner.chars();
        while let Some(c) = it.next() {
            if c == '\\' {
                match it.next() {
                    Some('\\') => out.push_str("\\\\"), // AppleWin undoes the doubling itself
                    Some('"') => out.push('"'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(o) => {
                        out.push('\\');
                        out.push(o)
                    }
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        out
    } else if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        v[1..v.len() - 1].replace("''", "'")
    } else if v == "~" || v == "null" {
        String::new()
    } else {
        v.to_string()
    }
}

/// Parses an AppleWin save-state document into its top-level entries (`File_hdr`, repeated
/// `Unit`, ...). Sequences and flow style are rejected, like `YamlHelper::ParseMap`.
pub fn parse(text: &str) -> Result<Vec<(String, Node)>, Error> {
    // (line number, indent, content) of the meaningful lines.
    let lines: Vec<(usize, usize, &str)> = text
        .lines()
        .enumerate()
        .filter_map(|(n, raw)| {
            let line = strip_comment(raw.trim_end_matches('\r')).trim_end();
            let content = line.trim_start();
            (!content.is_empty() && content != "---" && content != "...").then_some((n + 1, line.len() - content.len(), content))
        })
        .collect();
    // Stack of (indent of the keys in this map, entries); `keys` holds the pending key per level.
    let mut stack: Vec<(usize, Vec<(String, Node)>)> = vec![(0, Vec::new())];
    let mut keys: Vec<String> = Vec::new();
    for (i, &(n, indent, content)) in lines.iter().enumerate() {
        if content.starts_with('-') && (content.len() == 1 || content.as_bytes()[1] == b' ') {
            return Err(Error::Format("ParseMap: Sequence event unsupported".into()));
        }
        let err = |m: &str| Error::Format(format!("Save-state parser error: {m} @ {n}:{indent}"));
        // Close finished sub-maps.
        while stack.len() > 1 && indent < stack.last().unwrap().0 {
            let (_, done) = stack.pop().unwrap();
            let k = keys.pop().unwrap();
            stack.last_mut().unwrap().1.push((k, Node::Map(done)));
        }
        let (key, rest) = split_key(content).ok_or_else(|| err("expected `key: value`"))?;
        if indent > stack.last().unwrap().0 {
            // A deeper line must follow a `key:` with no value (opened below), never otherwise.
            return Err(err("unexpected indentation"));
        }
        let rest = rest.trim();
        if rest.is_empty() {
            // A sub-map if the next line is deeper, else an empty value.
            match lines.get(i + 1) {
                Some(&(_, deeper, _)) if deeper > indent => {
                    keys.push(key);
                    stack.push((deeper, Vec::new()));
                }
                _ => stack.last_mut().unwrap().1.push((key, Node::Scalar(String::new()))),
            }
        } else {
            if rest.starts_with('[') || rest.starts_with('{') {
                return Err(Error::Format("ParseMap: Sequence event unsupported".into()));
            }
            stack.last_mut().unwrap().1.push((key, Node::Scalar(unquote(rest))));
        }
    }
    while stack.len() > 1 {
        let (_, done) = stack.pop().unwrap();
        let k = keys.pop().unwrap();
        stack.last_mut().unwrap().1.push((k, Node::Map(done)));
    }
    Ok(stack.pop().unwrap().1)
}

/// Splits `key: value` / `key:` at the first `:` followed by a space or end of line.
fn split_key(content: &str) -> Option<(String, &str)> {
    let b = content.as_bytes();
    if let Some(inner) = content.strip_prefix('"') {
        let end = inner.find('"')? + 1;
        let rest = content[end + 1..].strip_prefix(':')?;
        return Some((content[1..end].to_string(), rest));
    }
    (0..b.len()).find(|&i| b[i] == b':' && (i + 1 == b.len() || b[i + 1] == b' ')).map(|i| (content[..i].to_string(), &content[i + 1..]))
}

/// `YamlSaveHelper`: indentation-tracking writer. Output uses `\n`.
pub struct Writer {
    out: String,
    indent: usize,
}

impl Writer {
    /// File prologue (`YamlSaveHelper` ctor): comment lines and the `---` marker.
    pub fn new(date_stamp: &str, version: &str) -> Self {
        let mut out = String::new();
        let _ = write!(out, "# Date-stamp: {date_stamp}\n# {version}\n\n---\n");
        Writer { out, indent: 0 }
    }

    pub fn finish(self) -> String {
        self.out
    }

    /// Raw indented line (`Save`).
    pub fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push(' ');
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// `Label`: writes `name:` then runs `f` one level deeper.
    pub fn map(&mut self, name: &str, f: impl FnOnce(&mut Self)) {
        self.line(&format!("{name}:"));
        self.indent += 2;
        f(self);
        self.indent -= 2;
    }

    pub fn int(&mut self, key: &str, v: i64) {
        self.line(&format!("{key}: {v}"));
    }
    pub fn uint(&mut self, key: &str, v: u64) {
        self.line(&format!("{key}: {v}"));
    }
    pub fn hex4(&mut self, key: &str, v: u32) {
        self.line(&format!("{key}: 0x{:01X}", v & 0xF));
    }
    pub fn hex8(&mut self, key: &str, v: u32) {
        self.line(&format!("{key}: 0x{:02X}", v & 0xFF));
    }
    pub fn hex12(&mut self, key: &str, v: u32) {
        self.line(&format!("{key}: 0x{v:03X}"));
    }
    pub fn hex16(&mut self, key: &str, v: u32) {
        self.line(&format!("{key}: 0x{:04X}", v & 0xFFFF));
    }
    pub fn hex24(&mut self, key: &str, v: u32) {
        self.line(&format!("{key}: 0x{v:06X}"));
    }
    pub fn hex32(&mut self, key: &str, v: u32) {
        self.line(&format!("{key}: 0x{v:08X}"));
    }
    pub fn hex64(&mut self, key: &str, v: u64) {
        self.line(&format!("{key}: 0x{v:016X}"));
    }
    pub fn bool(&mut self, key: &str, v: bool) {
        self.line(&format!("{key}: {v}"));
    }
    /// `%f` formatting (six decimals), as `SaveFloat`/`SaveDouble`.
    pub fn float(&mut self, key: &str, v: f64) {
        self.line(&format!("{key}: {v:.6}"));
    }

    /// `SaveString`: empty becomes `""`, backslashes are doubled (GH#1499) and a string with `#`
    /// is quoted (GH#1066).
    pub fn string(&mut self, key: &str, v: &str) {
        let mut s = if v.is_empty() { "\"\"".to_string() } else { v.to_string() };
        if s.contains('\\') && !s.contains("\\\\") {
            s = s.replace('\\', "\\\\");
        }
        if s.contains('#') {
            self.line(&format!("{key}: \"{s}\""));
        } else {
            self.line(&format!("{key}: {s}"));
        }
    }

    /// `SaveMemory` with `offset` 0: 64 bytes per line, upper-case hex, `AAAA: ` prefix.
    /// `data.len()` must be a multiple of 8.
    pub fn memory(&mut self, data: &[u8]) {
        debug_assert!(data.len().is_multiple_of(8));
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for (i, chunk) in data.chunks(64).enumerate() {
            let addr = i * 64;
            let mut line = String::with_capacity(6 + chunk.len() * 2);
            for sh in [12, 8, 4, 0] {
                line.push(HEX[(addr >> sh) & 0xF] as char);
            }
            line.push_str(": ");
            for b in chunk {
                line.push(HEX[(b >> 4) as usize] as char);
                line.push(HEX[(b & 0xF) as usize] as char);
            }
            self.line(&line);
        }
    }

    /// Like [`Writer::memory`] with 8-digit addresses, for dumps over 64K (the extension block).
    pub fn memory_wide(&mut self, data: &[u8]) {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for (i, chunk) in data.chunks(64).enumerate() {
            let mut line = format!("{:08X}: ", i * 64);
            for b in chunk {
                line.push(HEX[(b >> 4) as usize] as char);
                line.push(HEX[(b & 0xF) as usize] as char);
            }
            self.line(&line);
        }
    }

    /// `FileHdr`.
    pub fn file_hdr(&mut self, version: u32) {
        self.out.push_str("File_hdr:\n");
        self.indent = 2;
        self.string("Tag", "AppleWin Save State");
        self.int("Version", version as i64);
        self.indent = 0;
    }

    /// `UnitHdr` followed by `State:` (the unit's body goes in `body`).
    pub fn unit(&mut self, kind: &str, version: u32, body: impl FnOnce(&mut Self)) {
        self.out.push_str("\nUnit:\n");
        self.indent = 2;
        self.string("Type", kind);
        self.int("Version", version as i64);
        self.map("State", body);
        self.indent = 0;
    }

    /// `YamlSaveHelper::Slot`: `<slot>:` with `Card` and `Version`, then `State:` (body).
    pub fn slot(&mut self, slot: u8, card: &str, version: u32, body: impl FnOnce(&mut Self)) {
        self.map(&slot.to_string(), |w| {
            w.line(&format!("Card: {card}"));
            w.line(&format!("Version: {version}"));
            w.map("State", body);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strtoul_base0() {
        assert_eq!(strtoul("0x1F"), 31);
        assert_eq!(strtoul("42"), 42);
        assert_eq!(strtoul("010"), 8);
        assert_eq!(strtoul("0"), 0);
        assert_eq!(strtoul("0x02  junk"), 2);
        assert_eq!(strtoul("-1") as i32, -1);
    }

    #[test]
    fn writer_follows_yamlhelper_formats() {
        let mut w = Writer::new("d", "v");
        w.file_hdr(2);
        w.unit("Apple2", 9, |w| {
            w.hex8("A", 0x5);
            w.hex16("PC", 0xFA62);
            w.hex64("Cumulative Cycles", 0x1234);
            w.string("Empty", "");
            w.string("Path", "C:\\x#y");
            w.float("Extra Cycles", 0.5);
            w.map("Mem", |w| w.memory(&[0, 1, 2, 3, 4, 5, 6, 7, 0xAB, 0xCD, 0xEF, 0x10, 0, 0, 0, 0]));
        });
        let s = w.finish();
        let expect = "# Date-stamp: d\n# v\n\n---\nFile_hdr:\n  Tag: AppleWin Save State\n  Version: 2\n\nUnit:\n  Type: Apple2\n  Version: 9\n  State:\n    A: 0x05\n    PC: 0xFA62\n    Cumulative Cycles: 0x0000000000001234\n    Empty: \"\"\n    Path: \"C:\\\\x#y\"\n    Extra Cycles: 0.500000\n    Mem:\n      0000: 0001020304050607ABCDEF1000000000\n";
        assert_eq!(s, expect);
    }

    #[test]
    fn parser_reads_repeated_units_comments_and_quotes() {
        let t = "# c\n---\nFile_hdr:\n  Tag: AppleWin Save State\n  Version: 2\n\nUnit:\n  Type: A\n  Version: 1\n  State:\n    X: 0x08   # note\n    P: \"C:\\\\a#b\"\n    E: \"\"\n    Sub:\n      0000: 0102\n\nUnit:\n  Type: B\n";
        let top = parse(t).unwrap();
        assert_eq!(top.len(), 3);
        assert_eq!(top[1].0, "Unit");
        assert_eq!(top[2].0, "Unit");
        let Node::Map(u) = &top[1].1 else { panic!() };
        let m = Map { name: "Unit", entries: u };
        assert_eq!(m.string("Type").unwrap(), "A");
        let st = m.need_sub("State").unwrap();
        assert_eq!(st.u32("X").unwrap(), 8);
        assert_eq!(st.string("P").unwrap(), "C:\\a#b");
        assert_eq!(st.string("E").unwrap(), "");
        let mut buf = [0u8; 8];
        assert_eq!(st.need_sub("Sub").unwrap().load_memory(&mut buf).unwrap(), 2);
        assert_eq!(&buf[..3], &[1, 2, 0]);
        assert!(st.u32("Nope").unwrap_err().to_string().contains("State: Missing: Nope"));
    }

    #[test]
    fn rejects_sequences_and_odd_hex() {
        assert!(parse("a:\n  - 1\n").is_err());
        let top = parse("M:\n  0000: 123\n").unwrap();
        let Node::Map(e) = &top[0].1 else { panic!() };
        assert!(Map { name: "M", entries: e }.load_memory(&mut [0; 8]).is_err());
    }

    #[test]
    fn empty_state_value_is_an_empty_map() {
        let top = parse("Unit:\n  State:\n  Other: 1\n").unwrap();
        let Node::Map(u) = &top[0].1 else { panic!() };
        let m = Map { name: "Unit", entries: u };
        assert!(m.need_sub("State").unwrap().entries.is_empty());
        assert_eq!(m.u32("Other").unwrap(), 1);
    }
}
