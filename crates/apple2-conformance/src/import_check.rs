//! Format oracle for a freshly imported save-state (HU-0401, CA "comparação de estado observável").
//!
//! Right after [`crate::state::import_text`] the machine must hold what the file says. Two checks:
//!
//! * [`direct_checks`] reads the machine through its public bus and CPU (registers, the status
//!   switches `$C011-$C01F`, main and aux memory, the cumulative cycle counter) and compares with
//!   the values of the YAML;
//! * [`tree_diff`] saves the machine again in the AppleWin format (without our extension) and
//!   compares the two documents field by field, so registers, flags, the 64K of main memory, every
//!   aux and LC/Saturn bank and the state of every card are covered. Numbers are compared as
//!   numbers (`0x0A` = `10`); a field the file has and the re-save lacks is a difference.

use apple2_core::memory::mf;
use apple2_core::{Machine, Model};
use apple2_cpu::Bus;
use apple2_savestate::yaml::{self, Node};
use apple2_savestate::{save_aws_yaml, SaveOptions};
use std::collections::BTreeMap;

/// One field that differs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diff {
    pub path: String,
    pub file: String,
    pub machine: String,
}

impl std::fmt::Display for Diff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: arquivo `{}`, máquina `{}`",
            self.path, self.file, self.machine
        )
    }
}

fn norm(v: &str) -> String {
    let v = v.trim();
    let hex = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X"));
    if let Some(h) = hex {
        if let Ok(n) = u64::from_str_radix(h, 16) {
            return n.to_string();
        }
    }
    if let Ok(n) = v.parse::<u64>() {
        return n.to_string();
    }
    if let Ok(f) = v.parse::<f64>() {
        if v.contains('.') {
            return format!("{f:.6}");
        }
    }
    if v.is_empty() || v == "\"\"" || v == "~" {
        return String::new();
    }
    v.to_ascii_uppercase()
}

/// True for a map made only of `AAAA: hexbytes` lines (a `SaveMemory` dump).
fn is_memory_dump(m: &[(String, Node)]) -> bool {
    !m.is_empty()
        && m.iter().all(|(k, n)| {
            k.len() >= 4
                && k.bytes().all(|b| b.is_ascii_hexdigit())
                && matches!(n, Node::Scalar(v) if v.len() % 2 == 0 && v.bytes().all(|b| b.is_ascii_hexdigit()))
        })
}

/// A memory dump as one value, whatever the width of its lines.
fn dump_value(m: &[(String, Node)]) -> String {
    let mut lines: Vec<(usize, &str)> = m
        .iter()
        .filter_map(|(k, n)| match n {
            Node::Scalar(v) => Some((usize::from_str_radix(k, 16).unwrap_or(0), v.as_str())),
            _ => None,
        })
        .collect();
    lines.sort();
    let total: usize = lines.iter().map(|(_, v)| v.len() / 2).sum();
    let mut s = format!("{total} bytes @{:X}: ", lines.first().map_or(0, |l| l.0));
    for (_, v) in lines {
        s.push_str(&v.to_ascii_uppercase());
    }
    s
}

/// Fields of the file that depend on the host (paths, serial back end), not on the machine.
const HOST_FIELDS: [&str; 3] = ["Print Filename", "Serial Port Name", "Absolute Path"];

/// Path fragments of fields the port does not model or normalises on purpose; they are listed in
/// the report as such and not counted as differences.
const NOT_MODELLED: [&str; 17] = [
    "/AY891x/",       // AY-3-891x phase counters and RNG: audio only, not visible to the CPU
    "/State/Enabled", // mouse card flags AppleWin saves but never reads back as behaviour
    "/State/VBL",
    "/SY6522/Timer1 Latch", // v<7 files: a zero latch becomes 0xFFFF on purpose (6522.cpp:690)
    "/Shift Key Mod",       // frontend setting (see misc.rs)
    "/Card",                // card names differ in spelling (`Disk][` / `Disk II`)
    "/RGB mode",            // AppleColor adaptor: AUD-C-023
    "/Previous AN3",
    "/80COL changed",
    "/Invert bit7",
    "/Format Track state/", // Disk II write-format tracking
    "/DIPSW Default/",      // SSC factory DIP defaults come from the host configuration
    "/Deferred Stepper Address",
    "/Video/Cycles This Frame", // reduced modulo the frame length on purpose (AUD-C-060)
    "/SSI263/Current Mode",
    "/Joystick/", // trim values and counters of old versions
    "/Memory/Last Slot to Set Main Mem LC",
];

fn flatten(nodes: &[(String, Node)], prefix: &str, out: &mut BTreeMap<String, String>) {
    for (i, (k, n)) in nodes.iter().enumerate() {
        match n {
            Node::Scalar(s) => {
                if !HOST_FIELDS.contains(&k.as_str()) {
                    out.insert(format!("{prefix}/{k}"), norm(s));
                }
            }
            Node::Map(m) if is_memory_dump(m) => {
                out.insert(format!("{prefix}/{k}"), dump_value(m));
            }
            Node::Map(m) => {
                // Units are identified by their Type (their order is the writer's choice).
                let name = if prefix.is_empty() && k == "Unit" {
                    let t = m.iter().find_map(|(kk, nn)| match (kk.as_str(), nn) {
                        ("Type", Node::Scalar(s)) => Some(s.clone()),
                        _ => None,
                    });
                    format!("Unit[{}]", t.unwrap_or_else(|| i.to_string()))
                } else {
                    k.clone()
                };
                flatten(m, &format!("{prefix}/{name}"), out);
            }
        }
    }
}

/// `Memory Mode` of the file holds the language-card bits in old versions and `MMU LC Mode` has
/// them in new ones: compare the union.
fn fold_memory_mode(map: &mut BTreeMap<String, String>) {
    let mm = "/Unit[Apple2]/State/Memory/Memory Mode".to_string();
    let lc = "/Unit[Apple2]/State/Memory/MMU LC Mode".to_string();
    let a = map
        .get(&mm)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let b = map
        .remove(&lc)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    if map.contains_key(&mm) {
        map.insert(mm, (a | b).to_string());
    }
}

/// Unit/card path prefixes whose `Version` differs between the two documents: for those only
/// values both sides have are compared (a newer writer adds keys an older file never had).
fn older_prefixes(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> Vec<String> {
    a.iter()
        .filter(|(k, v)| k.ends_with("/Version") && b.get(*k).is_some_and(|w| w != *v))
        .map(|(k, _)| k[..k.len() - "Version".len()].to_string())
        .collect()
}

/// Differences between the original document and the machine saved again.
pub fn tree_diff(text: &str, machine: &Machine) -> Result<Vec<Diff>, String> {
    let saved = save_aws_yaml(
        machine,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let (mut a, mut b) = (BTreeMap::new(), BTreeMap::new());
    flatten(&yaml::parse(text).map_err(|e| e.to_string())?, "", &mut a);
    flatten(
        &yaml::parse(&saved.text).map_err(|e| e.to_string())?,
        "",
        &mut b,
    );
    // Version 1 of the Apple2 unit stored SLOTCXROM where later ones store INTCXROM.
    if a.get("/Unit[Apple2]/Version").map(String::as_str) == Some("1") {
        if let Some(v) = a.get_mut("/Unit[Apple2]/State/Memory/Memory Mode") {
            if let Ok(n) = v.parse::<u64>() {
                *v = (n ^ mf::INTCXROM as u64).to_string();
            }
        }
    }
    // CPU.cpp:963 ORs AF_RESERVED|AF_BREAK into P on load.
    for m in [&mut a, &mut b] {
        if let Some(v) = m.get_mut("/Unit[Apple2]/State/CPU/P") {
            if let Ok(n) = v.parse::<u64>() {
                *v = (n | 0x30).to_string();
            }
        }
    }
    // The ][ and ][+ have no MMU flags: the old files' Memory Mode there only carries the
    // language-card bits of the card in slot 0, compared through that card's own entry.
    let model = a
        .get("/Unit[Apple2]/State/Model")
        .cloned()
        .unwrap_or_default();
    if !(model.contains("//E") || model.contains("TK3000") || model.contains("8A")) {
        a.remove("/Unit[Apple2]/State/Memory/Memory Mode");
        b.remove("/Unit[Apple2]/State/Memory/Memory Mode");
    }
    fold_memory_mode(&mut a);
    fold_memory_mode(&mut b);
    let older = older_prefixes(&a, &b);
    let mut out = Vec::new();
    for (k, v) in &a {
        if k.ends_with("/Version") {
            continue; // versions are listed in the migration table, not compared
        }
        if NOT_MODELLED.iter().any(|f| k.contains(f)) {
            continue;
        }
        // A partial dump (stub state) is checked byte by byte by `direct_checks` instead.
        if let (Some(x), Some(y)) = (
            v.split_once(" bytes @"),
            b.get(k).and_then(|w| w.split_once(" bytes @")),
        ) {
            if x.0 != y.0 {
                continue;
            }
        }
        let is_older = older.iter().any(|p| k.starts_with(p.as_str()));
        match b.get(k) {
            None if is_older => {}
            Some(w) if w == v => {}
            Some(w) => out.push(Diff {
                path: k.clone(),
                file: v.clone(),
                machine: w.clone(),
            }),
            None => out.push(Diff {
                path: k.clone(),
                file: v.clone(),
                machine: "(ausente)".into(),
            }),
        }
    }
    Ok(out)
}

fn top_unit<'a>(top: &'a [(String, Node)], ty: &str) -> Option<&'a [(String, Node)]> {
    top.iter().find_map(|(k, n)| match (k.as_str(), n) {
        ("Unit", Node::Map(m))
            if m.iter()
                .any(|(kk, nn)| kk == "Type" && matches!(nn, Node::Scalar(s) if s == ty)) =>
        {
            m.iter().find_map(|(kk, nn)| match (kk.as_str(), nn) {
                ("State", Node::Map(s)) => Some(s.as_slice()),
                _ => None,
            })
        }
        _ => None,
    })
}

/// The `Unit: Apple2` map itself (for its `Version`).
fn a2_hdr(top: &[(String, Node)]) -> &[(String, Node)] {
    top.iter()
        .find_map(|(k, n)| match (k.as_str(), n) {
            ("Unit", Node::Map(m))
                if m.iter().any(|(kk, nn)| {
                    kk == "Type" && matches!(nn, Node::Scalar(s) if s == "Apple2")
                }) =>
            {
                Some(m.as_slice())
            }
            _ => None,
        })
        .unwrap_or(&[])
}

fn sub<'a>(m: &'a [(String, Node)], key: &str) -> Option<&'a [(String, Node)]> {
    m.iter().find_map(|(k, n)| match n {
        Node::Map(s) if k == key => Some(s.as_slice()),
        _ => None,
    })
}

fn num(m: &[(String, Node)], key: &str) -> Option<u64> {
    m.iter().find_map(|(k, n)| match n {
        Node::Scalar(s) if k == key => norm(s).parse().ok(),
        _ => None,
    })
}

/// Bytes the dump lists; the rest of a partial dump (the AppleWin-Test stubs) keeps the power-on
/// pattern, so only these are compared.
fn decode_memory(m: &[(String, Node)], len: usize) -> (Vec<u8>, Vec<bool>) {
    let mut v = vec![0u8; len];
    let mut seen = vec![false; len];
    for (k, n) in m {
        let Node::Scalar(s) = n else { continue };
        let Ok(addr) = usize::from_str_radix(k, 16) else {
            continue;
        };
        for (i, pair) in s.as_bytes().chunks(2).enumerate() {
            if let Ok(h) = std::str::from_utf8(pair) {
                if let (Ok(b), true) = (u8::from_str_radix(h, 16), addr + i < len) {
                    v[addr + i] = b;
                    seen[addr + i] = true;
                }
            }
        }
    }
    (v, seen)
}

fn bank_diff(label: &str, want: &[u8], seen: &[bool], got: &[u8], out: &mut Vec<Diff>) {
    if want.len() != got.len() {
        out.push(Diff {
            path: label.into(),
            file: format!("{} bytes", want.len()),
            machine: format!("{} bytes", got.len()),
        });
        return;
    }
    let differs = |i: usize| seen[i] && want[i] != got[i];
    if let Some(i) = (0..want.len()).find(|&i| differs(i)) {
        let n = (0..want.len()).filter(|&i| differs(i)).count();
        out.push(Diff {
            path: format!("{label}/{i:04X}"),
            file: format!("{:02X}", want[i]),
            machine: format!("{:02X} ({n} bytes diferem)", got[i]),
        });
    }
}

/// Checks the machine against the file through the machine's own interfaces.
pub fn direct_checks(text: &str, machine: &mut Machine) -> Result<Vec<Diff>, String> {
    let top = yaml::parse(text).map_err(|e| e.to_string())?;
    let a2 = top_unit(&top, "Apple2").ok_or("sem a unidade Apple2")?;
    let mut out = Vec::new();
    let mut chk = |path: &str, want: u64, got: u64| {
        if want != got {
            out.push(Diff {
                path: path.into(),
                file: format!("{want:#X}"),
                machine: format!("{got:#X}"),
            });
        }
    };
    // Registers and flags. B and the reserved bit read as 1 (CPU.cpp:963).
    let cpu = sub(a2, "CPU").ok_or("sem CPU")?;
    let c = machine.cpu();
    chk("CPU/A", num(cpu, "A").unwrap_or(0), c.a as u64);
    chk("CPU/X", num(cpu, "X").unwrap_or(0), c.x as u64);
    chk("CPU/Y", num(cpu, "Y").unwrap_or(0), c.y as u64);
    chk("CPU/S", num(cpu, "S").unwrap_or(0), c.sp as u64);
    chk("CPU/PC", num(cpu, "PC").unwrap_or(0), c.pc as u64);
    chk("CPU/P", num(cpu, "P").unwrap_or(0) | 0x30, c.p as u64);
    let cycles = machine.cycles();
    chk(
        "CPU/Cumulative Cycles",
        num(cpu, "Cumulative Cycles").unwrap_or(0),
        cycles,
    );

    // Main memory ($0000-$BFFF) and, for the //e family, the language-card area.
    let model = machine.config().model;
    let st = machine.save_state();
    let want_main = sub(a2, "Main Memory").map(|m| decode_memory(m, 0x10000));
    if let Some((want, seen)) = want_main {
        let len = if model == Model::Apple2 && want.len() > 0xC000 {
            0xC000
        } else {
            0xC000
        };
        bank_diff(
            "Main Memory/0000-BFFF",
            &want[..len],
            &seen[..len],
            &st.memory.main[..len],
            &mut out,
        );
        if !model.is_apple2() {
            let lc = &st.memory.lc;
            let n = lc.len().min(0x4000);
            bank_diff(
                "Main Memory/C000-FFFF (LC)",
                &want[0xC000..0xC000 + n],
                &seen[0xC000..0xC000 + n],
                &lc[..n],
                &mut out,
            );
        }
    }
    // Aux banks.
    if let Some(aux_unit) = top_unit(&top, "Auxiliary Slot") {
        if let Some(inner) = sub(aux_unit, "State").or(Some(aux_unit)) {
            let banks = num(inner, "Num Aux Banks").unwrap_or(0) as usize;
            for b in 0..banks {
                if let Some(m) = sub(inner, &format!("Auxiliary Memory Bank{b:02X}")) {
                    let (want, seen) = decode_memory(m, 0x10000);
                    let off = b * 0x10000;
                    if off + 0x10000 <= st.memory.aux.len() {
                        bank_diff(
                            &format!("Aux bank {b:02X}"),
                            &want,
                            &seen,
                            &st.memory.aux[off..off + 0x10000],
                            &mut out,
                        );
                    }
                }
            }
        }
    }

    // Soft switches, through the status reads $C011-$C01F (//e family only).
    if model.is_apple2e_family() {
        let mem = sub(a2, "Memory").ok_or("sem Memory")?;
        let mut mode = num(mem, "Memory Mode").unwrap_or(0) as u32
            | num(mem, "MMU LC Mode").unwrap_or(0) as u32;
        if num(a2_hdr(&top), "Version").unwrap_or(9) == 1 {
            mode ^= mf::INTCXROM; // v1 stored SLOTCXROM
        }
        let vmode = sub(a2, "Video")
            .and_then(|v| num(v, "Video Mode"))
            .unwrap_or(0) as u32;
        let alt = sub(a2, "Video")
            .map(|v| {
                v.iter().any(|(k, n)| {
                    k == "Alt Char Set" && matches!(n, Node::Scalar(s) if s == "true")
                })
            })
            .unwrap_or(false);
        let want: [(u16, &str, bool); 14] = [
            (0xC011, "BANK2", mode & mf::BANK2 != 0),
            (0xC012, "HIGHRAM", mode & mf::HIGHRAM != 0),
            (0xC013, "RAMRD", mode & mf::AUXREAD != 0),
            (0xC014, "RAMWRT", mode & mf::AUXWRITE != 0),
            (0xC015, "INTCXROM", mode & mf::INTCXROM != 0),
            (0xC016, "ALTZP", mode & mf::ALTZP != 0),
            (0xC017, "SLOTC3ROM", mode & mf::SLOTC3ROM != 0),
            (0xC018, "80STORE", mode & mf::STORE80 != 0),
            (0xC01A, "TEXT", vmode & 0x40 != 0),
            (0xC01B, "MIXED", vmode & 0x10 != 0),
            (0xC01C, "PAGE2", mode & mf::PAGE2 != 0),
            (0xC01D, "HIRES", mode & mf::HIRES != 0),
            (0xC01E, "ALTCHARSET", alt),
            (0xC01F, "80COL", vmode & 0x01 != 0),
        ];
        for (addr, name, on) in want {
            let got = machine.board_mut().read(addr) & 0x80 != 0;
            if got != on {
                out.push(Diff {
                    path: format!("switch {name} (${addr:04X})"),
                    file: on.to_string(),
                    machine: got.to_string(),
                });
            }
        }
    }
    Ok(out)
}
