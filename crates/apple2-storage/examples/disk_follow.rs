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
//! usage: disk_follow --model M --disk IMAGE --oracle FILE.csv [--memclear N] [--limit N]
//!        [--no-force]   (no forcing: stop at the first difference of any kind)
//! exit status: 0 = identical (modulo counted floating-bus syncs), 1 = real divergence.

use apple2_conformance::case::parse_model;
use apple2_conformance::trace::{TraceReader, TraceRow};
use apple2_core::observer::{AccessKind, AccessObserver};
use apple2_core::{Machine, MachineConfig};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Clone, Default)]
struct Log(Rc<RefCell<Vec<(u16, u8)>>>);
impl AccessObserver for Log {
    fn access(&mut self, addr: u16, value: u8, kind: AccessKind) {
        if kind == AccessKind::Read && (0xC000..0xD000).contains(&addr) {
            self.0.borrow_mut().push((addr, value));
        }
    }
}

/// Reads whose value comes (partly) from the floating bus. Deterministic ones are excluded: the
/// keyboard, the Disk II latch (even $C0E0-$C0EF) and the Disk II ROM page.
fn floating_capable(addr: u16, slot6: bool) -> bool {
    match addr {
        0xC000 | 0xC010 => false,
        0xC0E0..=0xC0EF => addr & 1 == 1 && slot6,
        0xC600..=0xC6FF => !slot6,
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
    let model = parse_model(&get("--model").expect("--model")).expect("model");
    let disk = get("--disk").expect("--disk");
    let oracle = get("--oracle").expect("--oracle");
    let memclear: u32 = get("--memclear").map_or(2, |v| v.parse().unwrap());
    let limit: usize = get("--limit").map_or(usize::MAX, |v| v.parse().unwrap());
    let force = !args.iter().any(|a| a == "--no-force");

    let tr = TraceReader::read_file(std::path::Path::new(&oracle)).expect("oracle trace");
    let mut m = Machine::new(MachineConfig::for_model(model));
    let mut card = Disk2Card::new();
    let img = DiskImage::from_bytes(&disk, std::fs::read(&disk).expect("disk")).expect("image");
    card.insert_disk(0, img);
    assert!(m.insert_card(6, Box::new(card)));
    m.power_cycle();
    // same RAM power-on pattern as the oracle
    for a in (0..0xC000u32).step_by(4) {
        if memclear != 0 {
            m.poke_main(a as u16, 0xFF);
            m.poke_main(a as u16 + 1, 0xFF);
        }
    }
    if memclear != 0 {
        for base in (0..0xC000u32).step_by(512) {
            for off in [0x28u32, 0x29, 0x68, 0x69] {
                m.poke_main((base + off) as u16, 0);
            }
        }
    }
    for (a, v) in [
        (0x4Eu16, 0x20u8),
        (0x4F, 0x20),
        (0x620B, 0),
        (0xBFFD, 0),
        (0xBFFE, 0),
        (0xBFFF, 0),
    ] {
        m.poke_main(a, v);
    }
    let log = Log::default();
    m.set_access_observer(Some(Box::new(log.clone())));

    let mut forced: BTreeMap<(u16, u16), u64> = BTreeMap::new(); // (pc, addr) -> count
    let mut forced_total = 0u64;
    let mut rows = 0usize;
    let mut verdict = None;
    for (i, want) in tr.rows.iter().enumerate().take(limit) {
        let mut got = TraceRow::from_machine(&m);
        got.p |= 0x30;
        let cx_pc = (0xC000..0xD000).contains(&want.pc);
        // `op` of code running in slot ROM space is read by peek(), which does not see it
        let same_flow = got.cycle == want.cycle && got.pc == want.pc && (cx_pc || got.op == want.op);
        let same_regs = (got.a, got.x, got.y, got.sp, got.p) == (want.a, want.x, want.y, want.sp, want.p);
        if !same_flow {
            verdict = Some(format!(
                "row {i}: FLOW  oracle cycle={} pc={:04X} op={:02X}  rust cycle={} pc={:04X} op={:02X}",
                want.cycle, want.pc, want.op, got.cycle, got.pc, got.op
            ));
            break;
        }
        if !same_regs {
            let reads = log.0.borrow();
            let fl = reads
                .iter()
                .rev()
                .find(|(a, _)| floating_capable(*a, true))
                .copied();
            if force && fl.is_some() && got.sp == want.sp {
                let (addr, _) = fl.unwrap();
                *forced.entry((m_prev_pc(&reads, i), addr)).or_default() += 1;
                forced_total += 1;
                let c = m.cpu_mut();
                (c.a, c.x, c.y) = (want.a, want.x, want.y);
                c.p = want.p;
            } else {
                verdict = Some(format!(
                    "row {i}: REGS  cycle={} pc={:04X}\n   oracle a={:02X} x={:02X} y={:02X} sp={:02X} p={:02X}\n   rust   a={:02X} x={:02X} y={:02X} sp={:02X} p={:02X}\n   last I/O reads of the previous instruction: {:04X?}",
                    want.cycle, want.pc, want.a, want.x, want.y, want.sp, want.p, got.a, got.x, got.y, got.sp, got.p,
                    reads.iter().map(|r| r.0).collect::<Vec<_>>()
                ));
                break;
            }
        }
        drop(log.0.borrow_mut().drain(..));
        m.run_cycles(1); // like a 1 ms slice: also runs Card::update (motor spin-down)
        rows += 1;
    }
    println!("rows matched: {rows} of {}", tr.rows.len().min(limit));
    println!("floating-bus register syncs: {forced_total}");
    let mut top: Vec<_> = forced.iter().collect();
    top.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for ((pc, addr), n) in top.iter().take(8) {
        println!("   read of {addr:04X} (previous-instruction I/O) x{n} [{pc:04X}]");
    }
    match verdict {
        None => println!("RESULT: identical (modulo the floating-bus syncs above)"),
        Some(v) => {
            println!("RESULT: DIVERGENCE\n{v}");
            std::process::exit(1);
        }
    }
}

fn m_prev_pc(_reads: &[(u16, u8)], _i: usize) -> u16 {
    0
}
