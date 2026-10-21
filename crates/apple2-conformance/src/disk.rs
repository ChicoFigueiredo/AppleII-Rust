//! Disk boot cases (HU-0301): the Rust machine with a Disk II in slot 6 is run in lock step
//! against an AppleWin oracle run, through a compact checkpoint file instead of the full trace
//! (a 12-million-cycle boot is >100 MB of CSV).
//!
//! The checkpoint file (`.ckpt`, UTF-8) is produced by `apple2-storage`'s `disk_follow` example
//! from a real oracle Trace v1 (`tools/conformance/oracle/run-audetto-oracle.sh --disk ...`):
//!
//! ```text
//! #key=value          metadata (model, memclear, disk, oracle_trace_sha256, rows, ...)
//! sync,ROW,A,X,Y,P    before ROW the registers are forced to the oracle's (the previous
//!                     instruction read a floating-bus address and the byte differed). The
//!                     shipped cases have none: with the oracle's video clock running (see
//!                     docs/conformance/audit/cpu-trace-differential.md, D4) the floating bus
//!                     agrees byte for byte.
//! ck,ROW,CYCLE,PC,A,X,Y,SP,P,HASH
//!                     state before instruction ROW, and the FNV-1a hash of every row before it
//! ```
//!
//! The hash covers `(cycle, pc, a, x, y, sp, p|0x30)` of every oracle row, so a pass means the
//! whole instruction stream, cycle by cycle, equals the oracle's up to the listed syncs.

use crate::case::Case;
use crate::runner::{machine_config, Env};
use crate::sha256::sha256_hex;
use crate::trace::BATCH_CYCLES;
use apple2_core::Machine;
use apple2_storage::disk2::{manager, Disk2Card};
use apple2_storage::hdd::{HardDiskCard, HdcMode};
use std::collections::VecDeque;
use std::path::PathBuf;

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

/// The machine of a disk boot case plus the scratch copies of its disk images.
pub struct Rig {
    pub machine: Machine,
    /// `(role, path of the scratch copy)`: `fixture`, `fixture2`, `hdd`, `hdd2`.
    pub files: Vec<(&'static str, PathBuf)>,
    dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        // read-only copies must be writable again before the directory can be removed on Windows
        for (_, p) in &self.files {
            if let Ok(m) = std::fs::metadata(p) {
                let mut perm = m.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                perm.set_readonly(false);
                let _ = std::fs::set_permissions(p, perm);
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Rig {
    /// Writes back everything the emulation still holds (dirty tracks) and returns the SHA-256
    /// of each scratch image, `(role, hex)`.
    pub fn final_hashes(&mut self) -> Vec<(&'static str, String)> {
        manager::flush_all(&mut self.machine);
        if let Some(h) = self.machine.card_mut::<HardDiskCard>(7) {
            h.flush();
        }
        self.files
            .iter()
            .map(|(r, p)| (*r, sha256_hex(&std::fs::read(p).unwrap_or_default())))
            .collect()
    }
}

/// The machine of a disk boot case: the case's configuration (model, cards, `memclear` with the
/// oracle's deterministic RAM, see `runner::machine_config`), its Disk II (slot of
/// `cards = ["N:disk2"]`, default 6) with `fixture`/`fixture2` in drives 1/2, the hard disk
/// controller in slot 7 with `hdd`/`hdd2`, power cycled. The images are copied to a scratch
/// directory and opened writable, so the emulation can write and the result can be hashed.
pub fn build(env: &Env, case: &Case) -> Result<Rig, String> {
    let cfg = machine_config(case)?;
    let model = cfg.model;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "conf-disk-{}-{}-{}",
        std::process::id(),
        case.id,
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for (role, fx) in case.devices() {
        let src = env.workspace.join(&fx.path);
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let dst = dir.join(format!("{role}-{name}"));
        std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        if case.readonly.iter().any(|r| r == role) {
            let mut perm = std::fs::metadata(&dst).map_err(|e| e.to_string())?.permissions();
            perm.set_readonly(true);
            std::fs::set_permissions(&dst, perm).map_err(|e| e.to_string())?;
        }
        files.push((role, dst));
    }
    let path_of = |r: &str| files.iter().find(|(x, _)| *x == r).map(|(_, p)| p.clone());
    let mut m = apple2_core::Machine::try_new(cfg).map_err(|e| e.to_string())?;
    let slot = case.disk2_slot()?;
    if slot.is_some() || path_of("fixture").is_some() || path_of("fixture2").is_some() {
        let mut card = Disk2Card::new();
        for (drive, role) in [(0, "fixture"), (1, "fixture2")] {
            if let Some(p) = path_of(role) {
                card.insert_path(drive, &p, false)
                    .map_err(|e| format!("{}: {e}", p.display()))?;
            }
        }
        let slot = slot.unwrap_or(6);
        if !m.insert_card(slot, Box::new(card)) {
            return Err(format!("slot {slot} refused the Disk II"));
        }
        if case.authentic {
            manager::set_enhance_disk(&mut m, false);
        }
    }
    if path_of("hdd").is_some() || path_of("hdd2").is_some() {
        let mut card = HardDiskCard::new(7, model, HdcMode::Default);
        for (unit, role) in [(0, "hdd"), (1, "hdd2")] {
            if let Some(p) = path_of(role) {
                card.insert_path(unit, &p)
                    .map_err(|e| format!("{}: {e}", p.display()))?;
            }
        }
        if !m.insert_card(7, Box::new(card)) {
            return Err("slot 7 refused the hard disk controller".into());
        }
    }
    // the cards are installed after power-on: redo it so they see the reset
    m.power_cycle();
    // `authentic` is a setting of the card manager: apply it again after the reset
    if case.authentic {
        manager::set_enhance_disk(&mut m, false);
    }
    Ok(Rig {
        machine: m,
        files,
        dir,
    })
}

/// Why a checkpoint run did not pass.
#[derive(Debug)]
pub enum Failure {
    Blocked(String),
    Fail(String, String),
}

/// Steps like the oracle: interrupt entries produce no row, and the cards are updated at the end
/// of every 1 ms batch (`trace::BATCH_CYCLES`; the Disk II counts its motor-off delay there).
#[derive(Default)]
pub struct Stepper {
    batch: u64,
}

impl Stepper {
    /// Takes the interrupt entry that is due, if any, so that `m` is at the next row.
    pub fn skip_interrupt_entries(&mut self, m: &mut Machine) {
        while m.next_step_is_interrupt_entry() {
            self.step(m);
        }
    }

    /// One instruction (or interrupt entry).
    pub fn step(&mut self, m: &mut Machine) {
        self.batch += m.step() as u64;
        if self.batch >= BATCH_CYCLES {
            m.update_cards(self.batch);
            self.batch = 0;
        }
    }
}

/// Runs `m` against `ck`. Ok(summary) when every checkpoint agrees.
pub fn check(rig: &mut Rig, ck: &Checkpoints) -> Result<String, Failure> {
    let mut stepper = Stepper::default();
    let m = &mut rig.machine;
    let mut syncs: VecDeque<Sync> = ck.syncs.iter().copied().collect();
    let mut checks: VecDeque<Check> = ck.checks.iter().copied().collect();
    let total = ck.rows();
    let mut h = HASH_SEED;
    for row in 0..total {
        stepper.skip_interrupt_entries(m);
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
        stepper.step(m);
    }
    // what the oracle left in the image files
    let mut finals = String::new();
    for (role, got) in rig.final_hashes() {
        if let Some(want) = ck.meta(&format!("final_sha256_{role}")) {
            if want != got {
                return Err(Failure::Fail(
                    format!(
                        "imagem {role} ao final: sha256 {got}, o oráculo deixou {want}"
                    ),
                    String::new(),
                ));
            }
            finals.push_str(&format!(" {role}={}", &got[..8]));
        }
    }
    Ok(format!(
        "{total} instruções, {} checkpoints e {} sincronizações do barramento flutuante idênticos ao oráculo{}",
        ck.checks.len(),
        ck.syncs.len(),
        if finals.is_empty() {
            String::new()
        } else {
            format!("; imagens finais idênticas ({})", finals.trim())
        }
    ))
}
