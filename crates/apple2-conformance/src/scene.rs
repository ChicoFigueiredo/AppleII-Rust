//! Video scenes ("scene v1"): RAM segments and an entry point, loaded into the Rust machine and
//! into the AppleWin oracle (`audetto-oracle --scene`) right after power-on.
//!
//! ```text
//! # comment
//! name text40
//! model apple2ee
//! main 0400 C1 C2 ...      main RAM bytes at $0400
//! aux  0400 C1 C2 ...      aux RAM bytes at $0400
//! entry 0300               the CPU starts here (cycle 0)
//! ```

use apple2_core::Machine;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scene {
    pub name: String,
    pub model: String,
    /// `(aux, address, bytes)` in file order (later segments overwrite earlier ones).
    pub segments: Vec<(bool, u16, Vec<u8>)>,
    pub entry: u16,
}

impl Scene {
    pub fn parse(text: &str) -> Result<Scene, String> {
        let mut s = Scene::default();
        let mut have_entry = false;
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            let mut it = line.split_whitespace();
            let Some(kind) = it.next() else { continue };
            let bad = |what: &str| format!("scene line {}: {what}", n + 1);
            match kind {
                "name" => s.name = it.next().unwrap_or("").to_string(),
                "model" => s.model = it.next().unwrap_or("").to_string(),
                "entry" => {
                    let a = it.next().ok_or_else(|| bad("entry without address"))?;
                    s.entry = u16::from_str_radix(a, 16).map_err(|_| bad("bad entry"))?;
                    have_entry = true;
                }
                "main" | "aux" => {
                    let a = it.next().ok_or_else(|| bad("segment without address"))?;
                    let addr = u16::from_str_radix(a, 16).map_err(|_| bad("bad address"))?;
                    let bytes = it
                        .map(|b| u8::from_str_radix(b, 16).map_err(|_| bad("bad byte")))
                        .collect::<Result<Vec<u8>, _>>()?;
                    s.segments.push((kind == "aux", addr, bytes));
                }
                o => return Err(bad(&format!("unknown directive `{o}`"))),
            }
        }
        if !have_entry {
            return Err("scene without `entry`".into());
        }
        Ok(s)
    }

    /// Pokes the segments and points the CPU at the entry (no cycle is consumed).
    pub fn apply(&self, m: &mut Machine) {
        for (aux, addr, bytes) in &self.segments {
            for (i, &b) in bytes.iter().enumerate() {
                let a = addr.wrapping_add(i as u16);
                if *aux {
                    m.poke_aux(a, b);
                } else {
                    m.poke_main(a, b);
                }
            }
        }
        m.cpu_mut().pc = self.entry;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_segments_and_entry() {
        let s = Scene::parse("# x\nname a\nmodel apple2ee\nmain 0300 A9 00\naux 0400 FF # c\nentry 0300\n")
            .unwrap();
        assert_eq!(s.name, "a");
        assert_eq!(s.segments, vec![(false, 0x300, vec![0xA9, 0]), (true, 0x400, vec![0xFF])]);
        assert_eq!(s.entry, 0x300);
        assert!(Scene::parse("main 0300 00\n").is_err());
    }
}
