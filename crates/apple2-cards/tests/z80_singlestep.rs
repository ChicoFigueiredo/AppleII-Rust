//! Z80 validation against the SingleStepTests/z80 vectors (MIT, github.com/SingleStepTests/z80).
//!
//! The vectors are not in Git: `tools/conformance/fetch-z80-tests.sh` downloads them at a pinned
//! SHA into `target/conformance-cache/z80-tests/v1`. Without them the test reports BLOCKED and
//! returns. The test is `#[ignore]` (1.3 GB of JSON); run it with
//! `cargo test -p apple2-cards --release --test z80_singlestep -- --ignored --nocapture`.
//! It writes `tests/conformance/reports/z80-singlestep.md`.
//!
//! Compared per vector: all registers, WZ, IFF1/IFF2, IM, EI, P, Q, RAM, I/O port transaction and
//! the T-state count (= number of bus cycles in the vector).

use apple2_cards::z80::{Z80Bus, Z80};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(Deserialize)]
struct State {
    pc: u16,
    sp: u16,
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: u8,
    h: u8,
    l: u8,
    i: u8,
    r: u8,
    ei: u8,
    wz: u16,
    ix: u16,
    iy: u16,
    af_: u16,
    bc_: u16,
    de_: u16,
    hl_: u16,
    im: u8,
    p: u8,
    q: u8,
    iff1: u8,
    iff2: u8,
    ram: Vec<(u16, u8)>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    initial: State,
    #[serde(rename = "final")]
    fin: State,
    cycles: Vec<serde::de::IgnoredAny>,
    #[serde(default)]
    ports: Vec<(u16, u8, String)>,
}

struct Bus {
    ram: Vec<u8>,
    port_in: u8,
    out: Vec<(u16, u8)>,
    ins: Vec<u16>,
}

impl Z80Bus for Bus {
    fn read(&mut self, a: u16) -> u8 {
        self.ram[a as usize]
    }
    fn write(&mut self, a: u16, v: u8) {
        self.ram[a as usize] = v;
    }
    fn io_in(&mut self, p: u16) -> u8 {
        self.ins.push(p);
        self.port_in
    }
    fn io_out(&mut self, p: u16, v: u8) {
        self.out.push((p, v));
    }
}

fn load(s: &State) -> Z80 {
    let mut z = Z80 {
        a: s.a,
        f: s.f,
        b: s.b,
        c: s.c,
        d: s.d,
        e: s.e,
        h: s.h,
        l: s.l,
        i: s.i,
        r: s.r,
        pc: s.pc,
        sp: s.sp,
        ix: s.ix,
        iy: s.iy,
        wz: s.wz,
        im: s.im,
        iff1: s.iff1 != 0,
        iff2: s.iff2 != 0,
        ei: s.ei != 0,
        p: s.p != 0,
        q: s.q,
        ..Default::default()
    };
    (z.a2, z.f2) = ((s.af_ >> 8) as u8, s.af_ as u8);
    (z.b2, z.c2) = ((s.bc_ >> 8) as u8, s.bc_ as u8);
    (z.d2, z.e2) = ((s.de_ >> 8) as u8, s.de_ as u8);
    (z.h2, z.l2) = ((s.hl_ >> 8) as u8, s.hl_ as u8);
    z
}

/// Differences between the CPU after the step and the expected final state.
fn diff(z: &Z80, bus: &Bus, c: &Case, t: u32) -> Vec<String> {
    let e = &c.fin;
    let mut d = Vec::new();
    let mut chk = |n: &str, got: u32, want: u32| {
        if got != want {
            d.push(format!("{n} got {got:#x} want {want:#x}"));
        }
    };
    chk("a", z.a.into(), e.a.into());
    chk("f", z.f.into(), e.f.into());
    chk("b", z.b.into(), e.b.into());
    chk("c", z.c.into(), e.c.into());
    chk("d", z.d.into(), e.d.into());
    chk("e", z.e.into(), e.e.into());
    chk("h", z.h.into(), e.h.into());
    chk("l", z.l.into(), e.l.into());
    chk("i", z.i.into(), e.i.into());
    chk("r", z.r.into(), e.r.into());
    chk("pc", z.pc.into(), e.pc.into());
    chk("sp", z.sp.into(), e.sp.into());
    chk("ix", z.ix.into(), e.ix.into());
    chk("iy", z.iy.into(), e.iy.into());
    chk("wz", z.wz.into(), e.wz.into());
    chk("af_", u32::from(z.a2) << 8 | u32::from(z.f2), e.af_.into());
    chk("bc_", u32::from(z.b2) << 8 | u32::from(z.c2), e.bc_.into());
    chk("de_", u32::from(z.d2) << 8 | u32::from(z.e2), e.de_.into());
    chk("hl_", u32::from(z.h2) << 8 | u32::from(z.l2), e.hl_.into());
    chk("iff1", z.iff1.into(), e.iff1.into());
    chk("iff2", z.iff2.into(), e.iff2.into());
    chk("im", z.im.into(), e.im.into());
    chk("ei", z.ei.into(), e.ei.into());
    chk("p", z.p.into(), e.p.into());
    chk("q", z.q.into(), e.q.into());
    chk("tstates", t, c.cycles.len() as u32);
    for &(a, v) in &e.ram {
        chk(
            &format!("ram[{a:#06x}]"),
            bus.ram[a as usize].into(),
            v.into(),
        );
    }
    for (p, v, kind) in &c.ports {
        if kind == "w" {
            if bus.out.last() != Some(&(*p, *v)) {
                d.push(format!("out {:?} want ({p:#x},{v:#x})", bus.out.last()));
            }
        } else if bus.ins.last() != Some(p) {
            d.push(format!("in {:?} want {p:#x}", bus.ins.last()));
        }
    }
    d
}

#[derive(Default)]
struct FileResult {
    pass: u32,
    fail: u32,
    first: Option<String>,
}

#[test]
#[ignore = "needs tools/conformance/fetch-z80-tests.sh (1.3 GB); writes tests/conformance/reports/z80-singlestep.md"]
fn z80_singlestep_vectors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::var_os("Z80_TESTS_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/conformance-cache/z80-tests"))
        .join("v1");
    let report_path = root.join("tests/conformance/reports/z80-singlestep.md");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        let msg = format!(
            "# Z80 SingleStepTests\n\nBLOCKED: vetores ausentes em `{}`. Rode `tools/conformance/fetch-z80-tests.sh`.\n",
            dir.display()
        );
        println!("{msg}");
        return;
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();

    let mut results: BTreeMap<String, FileResult> = BTreeMap::new();
    for f in &files {
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(f).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let mut res = FileResult::default();
        for c in &cases {
            let mut bus = Bus {
                ram: vec![0; 0x10000],
                port_in: c.ports.iter().find(|p| p.2 == "r").map_or(0, |p| p.1),
                out: vec![],
                ins: vec![],
            };
            for &(a, v) in &c.initial.ram {
                bus.ram[a as usize] = v;
            }
            let mut z = load(&c.initial);
            let t = z.step(&mut bus);
            let d = diff(&z, &bus, c, t);
            if d.is_empty() {
                res.pass += 1;
            } else {
                res.fail += 1;
                res.first
                    .get_or_insert_with(|| format!("`{}`: {}", c.name, d.join("; ")));
            }
        }
        results.insert(name, res);
    }

    let (pass, fail): (u32, u32) = results
        .values()
        .fold((0, 0), |a, r| (a.0 + r.pass, a.1 + r.fail));
    let bad: Vec<_> = results.iter().filter(|(_, r)| r.fail > 0).collect();
    let mut md = String::new();
    writeln!(md, "# Z80: SingleStepTests/z80 (v1)\n").unwrap();
    writeln!(
        md,
        "Vetores: github.com/SingleStepTests/z80 (MIT), SHA fixo em `tools/conformance/fetch-z80-tests.sh`."
    )
    .unwrap();
    writeln!(
        md,
        "Teste: `cargo test -p apple2-cards --release --test z80_singlestep -- --ignored --nocapture`.\n"
    )
    .unwrap();
    writeln!(
        md,
        "Compara registradores, WZ, IFF1/IFF2, IM, EI, P, Q, RAM, transação de porta e T-states.\n"
    )
    .unwrap();
    writeln!(md, "## Resultado\n").unwrap();
    writeln!(
        md,
        "- Arquivos (grupos de opcode): {} ({} com falha)",
        results.len(),
        bad.len()
    )
    .unwrap();
    writeln!(md, "- Vetores que passam: {pass}").unwrap();
    writeln!(md, "- Vetores que falham: {fail}\n").unwrap();
    if !bad.is_empty() {
        writeln!(md, "## Grupos com falha\n").unwrap();
        writeln!(md, "| Grupo | Falhas / total | Primeira divergência |").unwrap();
        writeln!(md, "|---|---|---|").unwrap();
        for (n, r) in bad {
            writeln!(
                md,
                "| `{n}` | {} / {} | {} |",
                r.fail,
                r.pass + r.fail,
                r.first.as_deref().unwrap_or("")
            )
            .unwrap();
        }
    }
    std::fs::write(&report_path, &md).unwrap();
    println!("{md}");
    assert_eq!(fail, 0, "veja {}", report_path.display());
}
