//! Instruction-level conformance against SingleStepTests/65x02 (MIT, github.com/SingleStepTests/65x02):
//! NMOS 6502 (with the undocumented opcodes) and the WDC / Rockwell / Synertek 65C02.
//!
//! The vectors are not in Git: `tools/conformance/fetch-65x02-tests.sh` downloads them at a pinned
//! SHA into `target/conformance-cache/65x02-tests`. Without them the test reports BLOCKED and
//! returns. Run it with
//! `cargo test -p apple2-cpu --release --test singlestep_65x02 -- --ignored --nocapture`;
//! it writes `tests/conformance/reports/65x02-singlestep.md`.
//!
//! Per vector the test compares PC, A, X, Y, SP, P (bits 4 and 5 masked: AppleWin keeps them as
//! `EF_TO_AF` does and the port leaves the register alone), the final RAM, the cycle count and the
//! per-cycle bus accesses (address, value, read/write).
//!
//! The vectors are real hardware. AppleWin differs from it in documented ways (no dummy bus
//! accesses, unstable opcodes, Rockwell/WDC bit instructions). Those differences are listed in
//! `DIVERGENCES` with the C++ line that causes them; a vector that fails inside such a class is
//! "esperado-divergente" and does not fail the test. Anything else is a regression.
//!
//! `C65X02_SHOW=cpu:op[:n],...` prints the first n unexplained-or-diverging vectors of an opcode.

use apple2_cpu::{Bus, Cpu, Variant};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(Deserialize)]
struct State {
    pc: u16,
    s: u8,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    ram: Vec<(u16, u8)>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    initial: State,
    #[serde(rename = "final")]
    fin: State,
    cycles: Vec<(u16, u8, String)>,
}

/// One cycle's bus access as the port performs it: `None` is an internal cycle (no access).
type Access = Option<(u16, u8, bool)>;

struct RecBus {
    ram: Vec<u8>,
    cur: Access,
    log: Vec<Access>,
}

impl Bus for RecBus {
    fn read(&mut self, a: u16) -> u8 {
        let v = self.ram[a as usize];
        self.cur = Some((a, v, false));
        v
    }
    fn write(&mut self, a: u16, v: u8) {
        self.ram[a as usize] = v;
        self.cur = Some((a, v, true));
    }
    fn tick(&mut self) {
        self.log.push(self.cur.take());
    }
}

/// How the per-cycle bus accesses compare.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BusKind {
    /// Identical in every cycle.
    Exact,
    /// The port makes no access in cycles where the hardware does a dummy read/write (AppleWin
    /// has none: `_READ`/`_WRITE` of cpu_general.inl are single accesses); every access it does
    /// make is right.
    Phantom,
    /// An access differs in address, value or direction (or the cycle count differs).
    Mismatch,
}

/// Result of one vector.
struct Verdict {
    /// Final-state fields that differ (`pc`, `a`, `x`, `y`, `s`, `p`, `ram`).
    fields: Vec<&'static str>,
    cycles: bool,
    bus: BusKind,
    detail: String,
}

impl Verdict {
    fn strict(&self) -> bool {
        self.fields.is_empty() && !self.cycles && self.bus == BusKind::Exact
    }
    /// Right except for the dummy accesses AppleWin does not perform.
    fn phantom_only(&self) -> bool {
        self.fields.is_empty() && !self.cycles && self.bus == BusKind::Phantom
    }
}

fn note(v: &mut Verdict, name: &'static str, got: u32, want: u32) {
    if got != want {
        if !v.fields.contains(&name) {
            v.fields.push(name);
        }
        v.detail
            .push_str(&format!("{name} got {got:#x} want {want:#x}; "));
    }
}

fn run_case(variant: Variant, c: &Case) -> Verdict {
    let mut bus = RecBus {
        ram: vec![0; 0x10000],
        cur: None,
        log: Vec::with_capacity(8),
    };
    for &(a, v) in &c.initial.ram {
        bus.ram[a as usize] = v;
    }
    let mut cpu = Cpu::new(variant);
    cpu.pc = c.initial.pc;
    cpu.sp = c.initial.s;
    cpu.a = c.initial.a;
    cpu.x = c.initial.x;
    cpu.y = c.initial.y;
    cpu.p = c.initial.p;
    cpu.step(&mut bus);

    let e = &c.fin;
    let mut v = Verdict {
        fields: Vec::new(),
        cycles: false,
        bus: BusKind::Exact,
        detail: String::new(),
    };
    note(&mut v, "pc", cpu.pc.into(), e.pc.into());
    note(&mut v, "a", cpu.a.into(), e.a.into());
    note(&mut v, "x", cpu.x.into(), e.x.into());
    note(&mut v, "y", cpu.y.into(), e.y.into());
    note(&mut v, "s", cpu.sp.into(), e.s.into());
    note(&mut v, "p", (cpu.p & 0xCF).into(), (e.p & 0xCF).into());
    for &(a, val) in &e.ram {
        note(&mut v, "ram", bus.ram[a as usize].into(), val.into());
    }
    if bus.log.len() != c.cycles.len() {
        v.cycles = true;
        v.bus = BusKind::Mismatch;
        v.detail.push_str(&format!(
            "cycles got {} want {}; ",
            bus.log.len(),
            c.cycles.len()
        ));
        return v;
    }
    for (i, (want, got)) in c.cycles.iter().zip(&bus.log).enumerate() {
        let w = (want.0, want.1, want.2 == "write");
        match got {
            Some(g) if *g == w => {}
            Some(_) => {
                v.bus = BusKind::Mismatch;
                v.detail
                    .push_str(&format!("cycle {i}: got {got:x?} want {w:x?}; "));
            }
            None if v.bus == BusKind::Exact => v.bus = BusKind::Phantom,
            None => {}
        }
    }
    v
}

const NMOS: &[&str] = &["6502"];
const CMOS: &[&str] = &["wdc65c02", "rockwell65c02", "synertek65c02"];
const ROCKWELL_SYNERTEK: &[&str] = &["rockwell65c02", "synertek65c02"];

/// One documented AppleWin x hardware difference. A vector whose differing state fields are a
/// subset of `fields`, and whose cycle/bus mismatches are allowed by `cycles`/`bus`, is
/// "esperado-divergente".
struct Divergence {
    id: &'static str,
    cpus: &'static [&'static str],
    ops: &'static [u8],
    /// Final-state fields allowed to differ.
    fields: &'static [&'static str],
    cycles: bool,
    bus: bool,
    /// Extra condition on the vector (narrows the divergence to the inputs that cause it).
    when: Option<fn(&Case) -> bool>,
    what: &'static str,
    cpp: &'static str,
}

/// ARR in decimal mode.
fn decimal_set(c: &Case) -> bool {
    c.initial.p & 0x08 != 0
}

/// A memory byte that was $FF before and $00 after (INC wrapping, the operand of ISC/INS).
fn wraps_ff_to_00(c: &Case) -> bool {
    c.fin
        .ram
        .iter()
        .any(|&(a, v)| v == 0 && c.initial.ram.iter().any(|&(ia, iv)| ia == a && iv == 0xFF))
}

const HLT_OPS: &[u8] = &[
    0x02, 0x12, 0x22, 0x32, 0x42, 0x52, 0x62, 0x72, 0x92, 0xB2, 0xD2, 0xF2,
];
const RMB_SMB_OPS: &[u8] = &[
    0x07, 0x17, 0x27, 0x37, 0x47, 0x57, 0x67, 0x77, 0x87, 0x97, 0xA7, 0xB7, 0xC7, 0xD7, 0xE7, 0xF7,
];
const BBR_BBS_OPS: &[u8] = &[
    0x0F, 0x1F, 0x2F, 0x3F, 0x4F, 0x5F, 0x6F, 0x7F, 0x8F, 0x9F, 0xAF, 0xBF, 0xCF, 0xDF, 0xEF, 0xFF,
];
const ISC_OPS: &[u8] = &[0xE3, 0xE7, 0xEF, 0xF3, 0xF7, 0xFB, 0xFF];

/// AppleWin x hardware differences. The "ciclo fantasma" (D0) is not listed: it is the rule for
/// `BusKind::Phantom` and applies to every opcode of every CPU.
const DIVERGENCES: &[Divergence] = &[
    Divergence {
        id: "D1",
        cpus: NMOS,
        ops: HLT_OPS,
        fields: &["pc"],
        cycles: true,
        bus: true,
        when: None,
        what: "HLT/KIL: o AppleWin trava com PC sobre o opcode (bJammed, 2 ciclos); o hardware para com PC no byte seguinte",
        cpp: "CPU/cpu_instructions.inl:379-380 (`HLT`: bJammed = 1; --regs.pc), CPU/cpu6502.h:122",
    },
    Divergence {
        id: "D2",
        cpus: NMOS,
        ops: &[0x8B],
        fields: &["a", "p"],
        cycles: false,
        bus: false,
        when: None,
        what: "XAA/ANE ($8B): o AppleWin faz A = X & M; o hardware instavel faz A = (A | constante) & X & M",
        cpp: "CPU/cpu_instructions.inl:680-683 (`XAA`)",
    },
    Divergence {
        id: "D3",
        cpus: NMOS,
        ops: &[0x6B],
        fields: &["a", "p"],
        cycles: false,
        bus: false,
        when: Some(decimal_set),
        what: "ARR ($6B) com D=1: o ajuste BCD do AppleWin difere do hardware (a versao binaria passa)",
        cpp: "CPU/cpu_instructions.inl:246-264 (`ARR`, ramo `AF_DECIMAL`)",
    },
    Divergence {
        id: "D4",
        cpus: NMOS,
        ops: ISC_OPS,
        fields: &["a", "p"],
        cycles: false,
        bus: false,
        when: Some(wraps_ff_to_00),
        what: "INS/ISC com o operando $FF: o AppleWin subtrai o WORD $100 (val = READ+1 sem truncar), o hardware subtrai $00 (o carry sai diferente)",
        cpp: "CPU/cpu_instructions.inl:387-393 (`INS`: `temp = val` com val WORD)",
    },
    Divergence {
        id: "D5",
        cpus: CMOS,
        ops: RMB_SMB_OPS,
        fields: &["pc", "ram"],
        cycles: true,
        bus: true,
        when: None,
        what: "RMBn/SMBn ($x7): o AppleWin os trata como NOP de 1 byte e 1 ciclo; o hardware executa um RMB/SMB de 2 bytes e 3 a 5 ciclos (os vetores da Synertek tambem)",
        cpp: "CPU/cpu65C02.h:127,143,... (`case 0x07: NOP CYC(1)`; idem $17..$F7)",
    },
    Divergence {
        id: "D6",
        cpus: CMOS,
        ops: BBR_BBS_OPS,
        fields: &["pc"],
        cycles: true,
        bus: true,
        when: None,
        what: "BBRn/BBSn ($xF): o AppleWin os trata como NOP de 1 byte e 1 ciclo; o hardware executa um desvio de 3 bytes",
        cpp: "CPU/cpu65C02.h:135,151,... (`case 0x0F: NOP CYC(1)`; idem $1F..$FF)",
    },
    Divergence {
        id: "D7",
        cpus: CMOS,
        ops: &[0x5C],
        fields: &[],
        cycles: true,
        bus: true,
        when: None,
        what: "$5C (NOP absoluto): 8 ciclos no AppleWin (tabela do WDC), 4 nos vetores",
        cpp: "CPU/cpu65C02.h:212 (`case 0x5C: ABS NOP CYC(8)`)",
    },
    Divergence {
        id: "D8",
        cpus: ROCKWELL_SYNERTEK,
        ops: &[0xCB, 0xDB],
        fields: &["pc"],
        cycles: true,
        bus: true,
        when: None,
        what: "WAI ($CB) e STP ($DB): o AppleWin os trata como NOP de 1 ciclo; o hardware espera/para (os vetores do WDC omitem os dois)",
        cpp: "CPU/cpu65C02.h:323,339 (`case 0xCB/0xDB: NOP CYC(1)`)",
    },
    Divergence {
        id: "D9",
        cpus: CMOS,
        ops: &[0xDC, 0xFC],
        fields: &[],
        cycles: false,
        bus: true,
        when: None,
        what: "$DC/$FC (NOP absoluto de 4 ciclos): o AppleWin le o endereco do operando (`LDD`); o hardware le outro endereco no 4o ciclo. Estado e ciclos iguais",
        cpp: "CPU/cpu65C02.h:340,372 (`ABS LDD CYC(4)`), CPU/cpu_instructions.inl:439-440",
    },
    Divergence {
        id: "D10",
        cpus: CMOS,
        ops: &[0x6C],
        fields: &[],
        cycles: true,
        bus: true,
        when: None,
        what: "JMP (abs) com o ponteiro em $xxFF: o AppleWin acrescenta 1 ciclo (7), o hardware gasta 6",
        cpp: "CPU/cpu_general.inl:200-208 (`_IABS_CMOS`: uExtraCycles=1 se (base & $FF) == $FF), CPU/cpu65C02.h:228",
    },
];

fn covered(cpu: &str, op: u8, c: &Case, v: &Verdict) -> Option<&'static str> {
    if v.phantom_only() {
        return Some("D0");
    }
    DIVERGENCES
        .iter()
        .find(|d| {
            d.cpus.contains(&cpu)
                && d.ops.contains(&op)
                && v.fields.iter().all(|f| d.fields.contains(f))
                && (!v.cycles || d.cycles)
                && (v.bus != BusKind::Mismatch || d.bus)
                && d.when.is_none_or(|w| w(c))
        })
        .map(|d| d.id)
}

#[derive(Default)]
struct OpResult {
    total: u32,
    strict: u32,
    /// Only the dummy accesses are missing (D0).
    phantom: u32,
    /// Covered by an entry of `DIVERGENCES`, by id.
    diverged: BTreeMap<&'static str, u32>,
    unexpected: u32,
    first: Option<String>,
}

#[test]
#[ignore = "needs tools/conformance/fetch-65x02-tests.sh (2.5 GB); writes tests/conformance/reports/65x02-singlestep.md"]
fn singlestep_65x02_vectors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cache = std::env::var_os("C65X02_TESTS_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/conformance-cache/65x02-tests"));
    let report_path = root.join("tests/conformance/reports/65x02-singlestep.md");
    if !cache.join("6502/v1").is_dir() {
        let msg = format!(
            "# 65x02 SingleStepTests\n\nBLOCKED: vetores ausentes em `{}`. Rode `tools/conformance/fetch-65x02-tests.sh`.\n",
            cache.display()
        );
        println!("{msg}");
        return;
    }
    let cpus: [(&str, Variant); 4] = [
        ("6502", Variant::Nmos6502),
        ("wdc65c02", Variant::Cmos65C02),
        ("rockwell65c02", Variant::Cmos65C02),
        ("synertek65c02", Variant::Cmos65C02),
    ];
    let show = std::env::var("C65X02_SHOW").unwrap_or_default();
    let mut all: BTreeMap<&str, BTreeMap<u8, OpResult>> = BTreeMap::new();
    for (cpu_name, variant) in cpus {
        let mut per_op = BTreeMap::new();
        for op in 0..=255u8 {
            let f = cache
                .join(cpu_name)
                .join("v1")
                .join(format!("{op:02x}.json"));
            // WAI ($CB) and STP ($DB) are empty files in the WDC set (not testable single-step).
            let Ok(bytes) = std::fs::read(&f) else {
                continue;
            };
            if bytes.is_empty() {
                continue;
            }
            let cases: Vec<Case> =
                serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            let mut r = OpResult::default();
            let mut shown = 0u32;
            for c in &cases {
                let v = run_case(variant, c);
                r.total += 1;
                if v.strict() {
                    r.strict += 1;
                    continue;
                }
                for sel in show.split(',') {
                    let mut it = sel.split(':');
                    let (sc, so) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                    let n: u32 = it.next().and_then(|n| n.parse().ok()).unwrap_or(5);
                    if sc == cpu_name
                        && u8::from_str_radix(so, 16) == Ok(op)
                        && !v.phantom_only()
                        && shown < n
                    {
                        shown += 1;
                        println!(
                            "SHOW {cpu_name} {op:02x}: {} :: {} :: init pc={:#x} a={:#x} x={:#x} y={:#x} s={:#x} p={:#x} ram={:x?}",
                            c.name,
                            v.detail,
                            c.initial.pc,
                            c.initial.a,
                            c.initial.x,
                            c.initial.y,
                            c.initial.s,
                            c.initial.p,
                            c.initial.ram
                        );
                    }
                }
                match covered(cpu_name, op, c, &v) {
                    Some("D0") => r.phantom += 1,
                    Some(id) => *r.diverged.entry(id).or_default() += 1,
                    None => {
                        r.unexpected += 1;
                        r.first
                            .get_or_insert_with(|| format!("`{}`: {}", c.name, v.detail));
                    }
                }
            }
            per_op.insert(op, r);
        }
        all.insert(cpu_name, per_op);
    }

    let mut md = String::new();
    writeln!(md, "# 65x02: SingleStepTests/65x02 (v1)\n").unwrap();
    writeln!(md, "Vetores: github.com/SingleStepTests/65x02 (MIT, Thomas Harte et al.), SHA fixo em `tools/conformance/fetch-65x02-tests.sh`. Sao medidas de hardware real, nao do AppleWin.").unwrap();
    writeln!(md, "Teste: `cargo test -p apple2-cpu --release --test singlestep_65x02 -- --ignored --nocapture` (`#[ignore]`, 2,5 GB de JSON).\n").unwrap();
    writeln!(md, "Compara, por vetor: PC, A, X, Y, SP, P (bits 4 e 5 mascarados: o AppleWin os mantem fixos, `EF_TO_AF`), RAM final, numero de ciclos e o acesso ao barramento de cada ciclo (endereco, valor, leitura/escrita).\n").unwrap();
    writeln!(md, "Classes de resultado de cada vetor:\n").unwrap();
    writeln!(md, "- **Estrito**: tudo igual ao hardware.").unwrap();
    writeln!(md, "- **D0, ciclo fantasma**: estado, RAM e ciclos iguais; o port nao faz o acesso espurio (leitura/escrita falsa) que o hardware faz em alguns ciclos, e todo acesso que faz esta certo. O AppleWin tambem nao os faz (`_READ`/`_WRITE` em `CPU/cpu_general.inl:70-78` sao acessos unicos; cruzar pagina so soma `uExtraCycles`, `cpu_general.inl:146-155`), entao e **diferenca AppleWin x hardware**, nao corrigida.").unwrap();
    writeln!(md, "- **D1 a D10**: demais diferencas AppleWin x hardware (tabela abaixo), tambem esperado-divergentes.").unwrap();
    writeln!(
        md,
        "- **Inesperado**: qualquer outro desvio. O teste falha se houver algum.\n"
    )
    .unwrap();
    writeln!(md, "Consequencia pratica de D0: leituras espurias em `$C0xx` tem efeito colateral no hardware (AppleWin GH#967, aberto); o port as omite como o AppleWin.\n").unwrap();

    writeln!(md, "## Resumo por CPU\n").unwrap();
    writeln!(
        md,
        "| CPU | Vetores | Estrito | D0 fantasma | D1 a D10 | Inesperado | Aprovados (estrito + esperado) |"
    )
    .unwrap();
    writeln!(md, "|---|---:|---:|---:|---:|---:|---:|").unwrap();
    let mut unexpected_total = 0u32;
    for (cpu, ops) in &all {
        let sum = |f: fn(&OpResult) -> u32| ops.values().map(f).sum::<u32>();
        let total = sum(|r| r.total);
        let strict = sum(|r| r.strict);
        let phantom = sum(|r| r.phantom);
        let diverged: u32 = ops.values().map(|r| r.diverged.values().sum::<u32>()).sum();
        let unexpected = sum(|r| r.unexpected);
        unexpected_total += unexpected;
        writeln!(
            md,
            "| {cpu} | {total} | {strict} | {phantom} | {diverged} | {unexpected} | {} |",
            strict + phantom + diverged
        )
        .unwrap();
    }

    writeln!(md, "\n## Diferencas AppleWin x hardware\n").unwrap();
    writeln!(
        md,
        "| Id | CPUs | Opcodes | Diferenca | C++ (linha) | Vetores |"
    )
    .unwrap();
    writeln!(md, "|---|---|---|---|---|---:|").unwrap();
    writeln!(
        md,
        "| D0 | todas | todos | Sem acesso nos ciclos espurios (leitura/escrita falsa) do hardware | `cpu_general.inl:70-78,146-155` | {} |",
        all.values()
            .flat_map(|o| o.values())
            .map(|r| r.phantom)
            .sum::<u32>()
    )
    .unwrap();
    for d in DIVERGENCES {
        let n: u32 = all
            .iter()
            .filter(|(c, _)| d.cpus.contains(c))
            .flat_map(|(_, o)| o.values())
            .map(|r| r.diverged.get(d.id).copied().unwrap_or(0))
            .sum();
        let ops = d
            .ops
            .iter()
            .map(|o| format!("{o:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(
            md,
            "| {} | {} | {} | {} | `{}` | {} |",
            d.id,
            d.cpus.join(", "),
            ops,
            d.what,
            d.cpp,
            n
        )
        .unwrap();
    }

    writeln!(md, "\n## Por opcode\n").unwrap();
    writeln!(md, "Cada CPU lista os opcodes sem nenhuma diferenca, os que so tem D0 e, em tabela, os que tem D1 a D10 ou algo inesperado.\n").unwrap();
    for (cpu, ops) in &all {
        let strict_ops: Vec<String> = ops
            .iter()
            .filter(|(_, r)| r.strict == r.total)
            .map(|(o, _)| format!("{o:02X}"))
            .collect();
        let phantom_ops: Vec<String> = ops
            .iter()
            .filter(|(_, r)| r.strict < r.total && r.phantom + r.strict == r.total)
            .map(|(o, _)| format!("{o:02X}"))
            .collect();
        writeln!(md, "### {cpu}\n").unwrap();
        writeln!(
            md,
            "- Opcodes 100% estritos ({}): {}",
            strict_ops.len(),
            strict_ops.join(" ")
        )
        .unwrap();
        writeln!(
            md,
            "- Opcodes so com D0 ({}): {}\n",
            phantom_ops.len(),
            phantom_ops.join(" ")
        )
        .unwrap();
        writeln!(
            md,
            "| Op | Vetores | Estrito | D0 | Divergencia (vetores) | Inesperado | Primeiro inesperado |"
        )
        .unwrap();
        writeln!(md, "|---|---:|---:|---:|---|---:|---|").unwrap();
        for (op, r) in ops {
            if r.strict + r.phantom == r.total {
                continue;
            }
            let div = r
                .diverged
                .iter()
                .map(|(i, n)| format!("{i} ({n})"))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                md,
                "| {op:02X} | {} | {} | {} | {div} | {} | {} |",
                r.total,
                r.strict,
                r.phantom,
                r.unexpected,
                r.first.as_deref().unwrap_or("")
            )
            .unwrap();
        }
        writeln!(md).unwrap();
    }
    std::fs::write(&report_path, &md).unwrap();
    println!("{}", &md[..md.len().min(5000)]);
    assert_eq!(unexpected_total, 0, "veja {}", report_path.display());
}
