//! Disk boot cases (HU-0301): the Rust machine with a Disk II in slot 6 is run in lock step
//! against an AppleWin oracle run, through a compact checkpoint file instead of the full trace
//! (a 12-million-cycle boot is >100 MB of CSV).
//!
//! The checkpoint file (`.ckpt`, UTF-8) is produced by `apple2-storage`'s `disk_follow` example
//! from a real oracle Trace v1 (`tools/conformance/oracle/run-audetto-oracle.sh --disk ...`):
//!
//! ```text
//! #key=value          metadata (model, memclear, disk, oracle_trace_sha256, rows, ...)
//! sync,ROW,A,X,Y,P    the video scanner's floating bus is a separate finding (see
//!                     docs/conformance/audit/disk-differential.md): before ROW the registers
//!                     are forced to the oracle's, because the instruction before it read a
//!                     floating-capable address and the Rust byte differed
//! ck,ROW,CYCLE,PC,A,X,Y,SP,P,HASH
//!                     state before instruction ROW, and the FNV-1a hash of every row before it
//! ```
//!
//! The hash covers `(cycle, pc, a, x, y, sp, p|0x30)` of every oracle row, so a pass means the
//! whole instruction stream, cycle by cycle, equals the oracle's up to the listed syncs.

use apple2_core::{Machine, MachineConfig};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use std::collections::VecDeque;

pub const FORMAT: &str = "trace-checkpoints-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sync {
    pub row: u64,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub p: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Check {
    pub row: u64,
    pub cycle: u64,
    pub pc: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub p: u8,
    pub hash: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Checkpoints {
    pub meta: Vec<(String, String)>,
    pub syncs: Vec<Sync>,
    pub checks: Vec<Check>,
}

impl Checkpoints {
    pub fn meta(&self, key: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn rows(&self) -> u64 {
        self.checks.last().map_or(0, |c| c.row + 1)
    }

    pub fn parse(text: &str) -> Result<Checkpoints, String> {
        let mut c = Checkpoints::default();
        for (n, line) in text.lines().enumerate() {
            let at = |m: &str| format!("linha {}: {m}", n + 1);
            if line.is_empty() {
                continue;
            }
            if let Some(kv) = line.strip_prefix('#') {
                let (k, v) = kv.split_once('=').ok_or_else(|| at("metadado sem ="))?;
                c.meta.push((k.to_string(), v.to_string()));
                continue;
            }
            let f: Vec<&str> = line.split(',').collect();
            let h8 = |i: usize| {
                f.get(i)
                    .and_then(|s| u8::from_str_radix(s, 16).ok())
                    .ok_or_else(|| at("campo hexa inválido"))
            };
            let num = |i: usize| {
                f.get(i)
                    .and_then(|s| s.parse::<u64>().ok())
                    .ok_or_else(|| at("campo numérico inválido"))
            };
            match f.first().copied() {
                Some("sync") if f.len() == 6 => c.syncs.push(Sync {
                    row: num(1)?,
                    a: h8(2)?,
                    x: h8(3)?,
                    y: h8(4)?,
                    p: h8(5)?,
                }),
                Some("ck") if f.len() == 10 => c.checks.push(Check {
                    row: num(1)?,
                    cycle: num(2)?,
                    pc: f
                        .get(3)
                        .and_then(|s| u16::from_str_radix(s, 16).ok())
                        .ok_or_else(|| at("pc inválido"))?,
                    a: h8(4)?,
                    x: h8(5)?,
                    y: h8(6)?,
                    sp: h8(7)?,
                    p: h8(8)?,
                    hash: f
                        .get(9)
                        .and_then(|s| u64::from_str_radix(s, 16).ok())
                        .ok_or_else(|| at("hash inválido"))?,
                }),
                _ => return Err(at("registro desconhecido")),
            }
        }
        if c.meta("format") != Some(FORMAT) {
            return Err(format!("esperado #format={FORMAT}"));
        }
        if c.checks.is_empty() {
            return Err("arquivo sem checkpoints".into());
        }
        if c.syncs.windows(2).any(|w| w[0].row >= w[1].row)
            || c.checks.windows(2).any(|w| w[0].row >= w[1].row)
        {
            return Err("linhas fora de ordem".into());
        }
        Ok(c)
    }

    pub fn to_text(&self) -> String {
        let mut o = String::new();
        for (k, v) in &self.meta {
            o.push_str(&format!("#{k}={v}\n"));
        }
        for s in &self.syncs {
            o.push_str(&format!(
                "sync,{},{:02X},{:02X},{:02X},{:02X}\n",
                s.row, s.a, s.x, s.y, s.p
            ));
        }
        for c in &self.checks {
            o.push_str(&format!(
                "ck,{},{},{:04X},{:02X},{:02X},{:02X},{:02X},{:02X},{:016X}\n",
                c.row, c.cycle, c.pc, c.a, c.x, c.y, c.sp, c.p, c.hash
            ));
        }
        o
    }
}

/// FNV-1a over one trace row; `p` is normalised the way the oracle reports it (bits 5 and 4 set).
pub fn fold_row(h: u64, cycle: u64, pc: u16, a: u8, x: u8, y: u8, sp: u8, p: u8) -> u64 {
    let mut h = h;
    let mut eat = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    };
    for b in cycle.to_le_bytes() {
        eat(b);
    }
    for b in pc.to_le_bytes() {
        eat(b);
    }
    for b in [a, x, y, sp, p | 0x30] {
        eat(b);
    }
    h
}

pub const HASH_SEED: u64 = 0xCBF2_9CE4_8422_2325;

/// `MemReset()` with the oracle patch (Memory.cpp:2342-2435; `audetto-trace.patch`): for
/// `memclear` 2 `FF FF 00 00` in $0000-$BFFF with the "random" exceptions fixed at 00; for 0 all
/// zeros; then RNDL/RNDH = $20 and three spots set to 0.
pub fn applewin_power_on_ram(m: &mut Machine, memclear: u32) {
    if memclear == 2 {
        for a in (0..0xC000u32).step_by(4) {
            m.poke_main(a as u16, 0xFF);
            m.poke_main(a as u16 + 1, 0xFF);
        }
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
}

/// The machine of a disk boot case: `cfg`, a Disk II in slot 6 with `bytes` in drive 1, power
/// cycled, RAM filled like the oracle's.
pub fn power_on(
    cfg: MachineConfig,
    disk_name: &str,
    bytes: Vec<u8>,
    memclear: u32,
) -> Result<Machine, String> {
    let mut m = Machine::try_new(cfg).map_err(|e| e.to_string())?;
    let mut card = Disk2Card::new();
    card.insert_disk(
        0,
        DiskImage::from_bytes(disk_name, bytes).map_err(|e| format!("imagem: {e}"))?,
    );
    if !m.insert_card(6, Box::new(card)) {
        return Err("slot 6 recusou o Disk II".into());
    }
    m.power_cycle();
    applewin_power_on_ram(&mut m, memclear);
    Ok(m)
}

/// Why a checkpoint run did not pass.
#[derive(Debug)]
pub enum Failure {
    Blocked(String),
    Fail(String, String),
}

/// Runs `m` against `ck`. Ok(summary) when every checkpoint agrees.
pub fn check(m: &mut Machine, ck: &Checkpoints) -> Result<String, Failure> {
    let mut syncs: VecDeque<Sync> = ck.syncs.iter().copied().collect();
    let mut checks: VecDeque<Check> = ck.checks.iter().copied().collect();
    let total = ck.rows();
    let mut h = HASH_SEED;
    for row in 0..total {
        if syncs.front().is_some_and(|s| s.row == row) {
            let s = syncs.pop_front().unwrap();
            let c = m.cpu_mut();
            (c.a, c.x, c.y, c.p) = (s.a, s.x, s.y, s.p);
        }
        let c = m.cpu();
        let (cycle, pc, a, x, y, sp, p) = (m.cycles(), c.pc, c.a, c.x, c.y, c.sp, c.p | 0x30);
        if checks.front().is_some_and(|k| k.row == row) {
            let k = checks.pop_front().unwrap();
            let want = (k.cycle, k.pc, k.a, k.x, k.y, k.sp, k.p | 0x30, k.hash);
            let got = (cycle, pc, a, x, y, sp, p, h);
            if want != got {
                return Err(Failure::Fail(
                    format!(
                        "divergência antes da linha {row}: esperado cycle={} pc={:04X} a={:02X} x={:02X} y={:02X} sp={:02X} p={:02X} hash={:016X}; obtido cycle={cycle} pc={pc:04X} a={a:02X} x={x:02X} y={y:02X} sp={sp:02X} p={p:02X} hash={h:016X}",
                        want.0, want.1, want.2, want.3, want.4, want.5, want.6, want.7
                    ),
                    format!("o hash cobre as linhas anteriores; o checkpoint anterior está a no máximo {} linhas", ck.meta("every").unwrap_or("?")),
                ));
            }
        }
        h = fold_row(h, cycle, pc, a, x, y, sp, p);
        // like a 1 ms slice: also runs Card::update (motor spin-down)
        m.run_cycles(1);
    }
    Ok(format!(
        "{total} instruções, {} checkpoints e {} sincronizações do barramento flutuante idênticos ao oráculo",
        ck.checks.len(),
        ck.syncs.len()
    ))
}
