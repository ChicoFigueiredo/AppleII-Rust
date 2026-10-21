//! Lock-step differential against an oracle Trace v1 of a disk boot (HU-0301).
//!
//! The Rust machine runs the same boot as the oracle (`tools/conformance/oracle/run-audetto-oracle.sh
//! --disk IMAGE --memclear M`), and before each instruction its state is compared with the
//! oracle's row. The video scanner's floating bus is NOT what this differential is about (a
//! separate finding, see docs/conformance/audit/disk-differential.md): when the registers differ
//! right after an instruction that read a floating-capable I/O address, the Rust registers are
//! forced to the oracle's and the event is counted. Everything else (cycle, pc, opcode, registers
//! after a deterministic read such as the data latch, the Disk II ROM, RAM) must be identical.
//!
//! usage: disk_follow --case tests/conformance/cases/ID.toml --oracle FILE.csv [--limit N]
//!        [--emit-ckpt FILE.ckpt [--every N]]   (writes the checkpoint file the harness case uses)
//! The machine is built exactly as the harness builds it for the case (`disk::build`).
//! exit status: 0 = identical (modulo counted floating-bus syncs), 1 = real divergence.

use apple2_conformance::disk::{self, Check, Checkpoints, Stepper, Sync};
use apple2_conformance::runner::Env;
use apple2_conformance::sha256::sha256_hex;
use apple2_conformance::trace::{TraceReader, TraceRow};
use apple2_conformance::Case;
use apple2_core::observer::{AccessKind, AccessObserver};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Clone, Default)]
struct Log(Rc<RefCell<Vec<u16>>>);
impl AccessObserver for Log {
    fn access(&mut self, addr: u16, _value: u8, kind: AccessKind) {
        if kind == AccessKind::Read && (0xC000..0xD000).contains(&addr) {
            self.0.borrow_mut().push(addr);
        }
    }
}

/// Reads whose value comes (partly) from the floating bus. Deterministic ones are excluded: the
/// keyboard, the Disk II latch (even $C0E0-$C0EF) and the Disk II ROM page.
fn floating_capable(addr: u16) -> bool {
    match addr {
        0xC000 | 0xC010 => false,
        0xC0E0..=0xC0EF => addr & 1 == 1,
        0xC600..=0xC6FF => false,
        _ => true,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let case_path = get("--case").expect("--case");
    let oracle = get("--oracle").expect("--oracle");
    let limit: usize = get("--limit").map_or(usize::MAX, |v| v.parse().unwrap());
    let every: u64 = get("--every").map_or(2000, |v| v.parse().unwrap());
    let emit = get("--emit-ckpt");

    let case = Case::load(std::path::Path::new(&case_path)).expect("case");
    let env = Env::detect(None);
    let oracle_bytes = std::fs::read(&oracle).expect("oracle trace");
    let tr = TraceReader::read_file(std::path::Path::new(&oracle)).expect("oracle trace");
    let mut rig = disk::build(&env, &case).expect("machine");
    let m = &mut rig.machine;
    let mut stepper = Stepper::default();
    // `--oracle-final role=sha256`: the images the oracle left behind (run on scratch copies)
    let oracle_finals: Vec<(String, String)> = args
        .windows(2)
        .filter(|w| w[0] == "--oracle-final")
        .filter_map(|w| w[1].split_once('=').map(|(r, h)| (r.to_string(), h.to_string())))
        .collect();
    let log = Log::default();
    m.set_access_observer(Some(Box::new(log.clone())));

    let mut forced: BTreeMap<u16, u64> = BTreeMap::new(); // address read -> count
    let mut syncs: Vec<Sync> = Vec::new();
    let mut checks: Vec<Check> = Vec::new();
    let mut hash = disk::HASH_SEED;
    let mut rows = 0usize;
    let mut verdict = None;
    let n = tr.rows.len().min(limit);
    for (i, want) in tr.rows.iter().enumerate().take(n) {
        stepper.skip_interrupt_entries(m);
        let mut got = TraceRow::from_machine(m);
        got.p |= 0x30;
        let cx_pc = (0xC000..0xD000).contains(&want.pc);
        // `op` of code running in slot ROM space is read by peek(), which does not see it
        let same_flow = got.cycle == want.cycle && got.pc == want.pc && (cx_pc || got.op == want.op);
        let same_regs =
            (got.a, got.x, got.y, got.sp, got.p) == (want.a, want.x, want.y, want.sp, want.p);
        if !same_flow {
            verdict = Some(format!(
                "row {i}: FLOW  oracle cycle={} pc={:04X} op={:02X}  rust cycle={} pc={:04X} op={:02X}",
                want.cycle, want.pc, want.op, got.cycle, got.pc, got.op
            ));
            break;
        }
        if !same_regs {
            let reads = log.0.borrow();
            let fl = reads.iter().rev().find(|a| floating_capable(**a)).copied();
            if let (Some(addr), true) = (fl, got.sp == want.sp) {
                *forced.entry(addr).or_default() += 1;
                if std::env::var_os("DISK_FOLLOW_VERBOSE").is_some() {
                    println!(
                        "sync row {i} cycle {} (frame pos {}) read {addr:04X}: oracle a={:02X} rust a={:02X}",
                        want.cycle,
                        want.cycle % 17030,
                        want.a,
                        got.a
                    );
                    let sc = apple2_core::video_scanner::VideoScanner::new(
                        apple2_core::VideoStandard::Ntsc,
                        false,
                    );
                    let sw = apple2_core::video_scanner::Switches {
                        text: true,
                        mixed: false,
                        hires: false,
                        page2: false,
                    };
                    for d in -6i64..=6 {
                        let c = ((want.cycle % 17030) as i64 + d).rem_euclid(17030) as u32;
                        let a = sc.address(c, sw);
                        println!(
                            "     pos+{d:>2}: scanner addr {a:04X} -> main[{a:04X}]={:02X}",
                            m.peek_main(a)
                        );
                    }
                }
                syncs.push(Sync {
                    row: i as u64,
                    a: want.a,
                    x: want.x,
                    y: want.y,
                    p: want.p,
                });
                let c = m.cpu_mut();
                (c.a, c.x, c.y, c.p) = (want.a, want.x, want.y, want.p);
            } else {
                verdict = Some(format!(
                    "row {i}: REGS  cycle={} pc={:04X}\n   oracle a={:02X} x={:02X} y={:02X} sp={:02X} p={:02X}\n   rust   a={:02X} x={:02X} y={:02X} sp={:02X} p={:02X}\n   I/O reads of the previous instruction: {:04X?}",
                    want.cycle, want.pc, want.a, want.x, want.y, want.sp, want.p, got.a, got.x, got.y, got.sp, got.p,
                    reads
                ));
                break;
            }
        }
        if (i as u64).is_multiple_of(every) || i + 1 == n {
            checks.push(Check {
                row: i as u64,
                cycle: want.cycle,
                pc: want.pc,
                a: want.a,
                x: want.x,
                y: want.y,
                sp: want.sp,
                p: want.p,
                hash,
            });
        }
        hash = disk::fold_row(hash, want.cycle, want.pc, want.a, want.x, want.y, want.sp, want.p);
        log.0.borrow_mut().clear();
        stepper.step(m);
        rows += 1;
    }
    println!("rows matched: {rows} of {n}");
    println!("floating-bus register syncs: {}", syncs.len());
    let mut top: Vec<_> = forced.iter().collect();
    top.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for (addr, c) in top.iter().take(8) {
        println!("   read of {addr:04X} x{c}");
    }
    if let Some(v) = verdict {
        println!("RESULT: DIVERGENCE\n{v}");
        std::process::exit(1);
    }
    let mut final_meta: Vec<(String, String)> = Vec::new();
    for (role, got) in rig.final_hashes() {
        match oracle_finals.iter().find(|(r, _)| r == role) {
            Some((_, want)) if *want != got => {
                println!("RESULT: DIVERGENCE\nfinal image {role}: rust {got}, oracle {want}");
                std::process::exit(1);
            }
            Some((_, want)) => {
                println!("final image {role}: identical to the oracle's ({})", &want[..12]);
                final_meta.push((format!("final_sha256_{role}"), want.clone()));
            }
            None => {}
        }
    }
    println!("RESULT: identical (modulo the floating-bus syncs above)");
    if let Some(path) = emit {
        let meta = |k: &str, v: String| (k.to_string(), v);
        let ck = Checkpoints {
            meta: vec![
                meta("format", disk::FORMAT.into()),
                meta("case", case.id.clone()),
                meta("model", case.model.clone()),
                meta("memclear", case.memclear.unwrap_or(2).to_string()),
                meta("authentic", case.authentic.to_string()),
                meta("every", every.to_string()),
                meta("rows", n.to_string()),
                meta(
                    "oracle",
                    tr.meta("oracle").unwrap_or("audetto-oracle").to_string(),
                ),
                meta(
                    "oracle_fork_sha",
                    tr.meta("fork_sha").unwrap_or("?").to_string(),
                ),
                meta(
                    "oracle_applewin_sha",
                    tr.meta("applewin_sha").unwrap_or("?").to_string(),
                ),
                meta("oracle_trace_sha256", sha256_hex(&oracle_bytes)),
                meta(
                    "oracle_command",
                    format!(
                        "run-audetto-oracle.sh --model {} --cycles {} --disk <fixture> --memclear {}",
                        tr.meta("model").unwrap_or("?"),
                        tr.meta("cycles").unwrap_or("?"),
                        case.memclear.unwrap_or(2)
                    ),
                ),
            ],
            syncs,
            checks,
        };
        let mut ck = ck;
        ck.meta.extend(final_meta);
        std::fs::write(&path, ck.to_text()).expect("write ckpt");
        println!("checkpoints written to {path}");
    }
}
