//! The [`Timeline`]: checkpoints + input log + replay.

use crate::snapshot::{apply_pages, decode, diff_pages, encode};
use apple2_core::Machine;
use apple2_storage::disk2::Disk2Card;
use std::collections::VecDeque;
use std::fmt;
use std::path::Path;

/// Limits and cadence. All fields are public; use `..Default::default()`.
#[derive(Clone, Debug)]
pub struct RewindConfig {
    /// A checkpoint every N frames of [`Timeline::run_frame`] (N >= 1).
    pub frames_per_checkpoint: u32,
    /// Also a checkpoint after this many instructions without one (single-stepping, `run_cycles`).
    pub max_instrs_between_checkpoints: u64,
    /// Every K-th checkpoint holds the complete state, the others only page deltas (K >= 1).
    pub full_every: usize,
    /// Most checkpoints kept; the oldest is discarded first (at least 1).
    pub max_checkpoints: usize,
    /// Budget for checkpoints plus log, in bytes; the oldest checkpoint is discarded first.
    pub max_ram_bytes: usize,
}

impl Default for RewindConfig {
    fn default() -> Self {
        RewindConfig {
            frames_per_checkpoint: 5,
            max_instrs_between_checkpoints: 50_000,
            full_every: 16,
            max_checkpoints: 1024,
            max_ram_bytes: 256 << 20,
        }
    }
}

impl RewindConfig {
    /// Sets the RAM budget in megabytes.
    pub fn with_ram_mb(mut self, mb: usize) -> Self {
        self.max_ram_bytes = mb << 20;
        self
    }
}

/// Something that enters the machine from outside and must be replayed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Key(u8),
    Paste(String),
    AnyKeyDown(bool),
    CapsLock(bool),
    OpenApple(bool),
    ClosedApple(bool),
    Button {
        n: usize,
        down: bool,
    },
    Paddle {
        n: u8,
        pos: u8,
    },
    JoystickAttached {
        n: usize,
        attached: bool,
    },
    Reset,
    PowerCycle,
    DiskInsert {
        slot: u8,
        drive: usize,
        path: String,
    },
    DiskEject {
        slot: u8,
        drive: usize,
    },
    /// End of an execution batch (`Machine::update_cards`).
    CardUpdate(u64),
}

/// One input-log record: `instr` instructions had run, at absolute machine cycle `cycle`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub instr: u64,
    pub cycle: u64,
    pub event: Event,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RewindError {
    /// The target is older than the oldest kept checkpoint (`oldest` is its instruction count).
    OutOfRange { oldest: u64 },
    /// The target is newer than anything that ever ran.
    BeyondHead { head: u64 },
    /// A checkpoint could not be loaded into the machine.
    Restore(String),
    /// Reopening or ejecting a disk image failed.
    Disk(String),
}

impl fmt::Display for RewindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RewindError::OutOfRange { oldest } => {
                write!(
                    f,
                    "target is older than the oldest checkpoint (instruction {oldest})"
                )
            }
            RewindError::BeyondHead { head } => {
                write!(
                    f,
                    "target is beyond the newest recorded instruction ({head})"
                )
            }
            RewindError::Restore(e) => write!(f, "cannot restore checkpoint: {e}"),
            RewindError::Disk(e) => write!(f, "disk: {e}"),
        }
    }
}

impl std::error::Error for RewindError {}

enum Payload {
    Full(Vec<u8>),
    Delta { pages: Vec<(u32, Vec<u8>)> },
}

struct Checkpoint {
    instr: u64,
    cycles: u64,
    /// Absolute index of the first log entry NOT yet applied in this state.
    ev: usize,
    meta: Vec<u8>,
    payload: Payload,
    mounts: Vec<(u8, usize, String)>,
}

impl Checkpoint {
    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.meta.len()
            + match &self.payload {
                Payload::Full(b) => b.len(),
                Payload::Delta { pages } => pages.iter().map(|(_, p)| p.len() + 8).sum(),
            }
    }
}

fn entry_bytes(e: &Entry) -> usize {
    std::mem::size_of::<Entry>()
        + match &e.event {
            Event::Paste(s) | Event::DiskInsert { path: s, .. } => s.len(),
            _ => 0,
        }
}

/// Names (`Card::name`) of the cards that talk to the host; see the crate docs.
const EXTERNAL_CARDS: [&str; 5] = [
    "Uthernet",
    "Uthernet II",
    "Super Serial Card",
    "Generic Printer",
    "Mouse Card",
];

pub struct Timeline {
    cfg: RewindConfig,
    checkpoints: VecDeque<Checkpoint>,
    /// Blob of the newest checkpoint (the base of the next delta).
    tail_blob: Vec<u8>,
    log: VecDeque<Entry>,
    log_base: usize,
    /// Cursor: instructions executed and first unapplied log entry (absolute index).
    instr: u64,
    ev: usize,
    head_instr: u64,
    last_cycles: u64,
    frames_since_cp: u32,
    replaying: bool,
}

fn mounts_of(m: &Machine) -> Vec<(u8, usize, String)> {
    let mut v = Vec::new();
    for s in 0..8u8 {
        if let Some(c) = m.card::<Disk2Card>(s) {
            for d in 0..2 {
                let p = c.pathname(d);
                if !p.is_empty() {
                    v.push((s, d, p.to_string()));
                }
            }
        }
    }
    v
}

fn apply_event(m: &mut Machine, e: &Event) -> Result<(), RewindError> {
    match e {
        Event::Key(k) => {
            m.key_press(*k);
        }
        Event::Paste(s) => {
            m.paste(s);
        }
        Event::AnyKeyDown(b) => m.set_any_key_down(*b),
        Event::CapsLock(b) => m.set_caps_lock(*b),
        Event::OpenApple(b) => m.set_open_apple(*b),
        Event::ClosedApple(b) => m.set_closed_apple(*b),
        Event::Button { n, down } => m.set_button(*n, *down),
        Event::Paddle { n, pos } => m.set_paddle(*n, *pos),
        Event::JoystickAttached { n, attached } => m.set_joystick_attached(*n, *attached),
        Event::Reset => m.reset(),
        Event::PowerCycle => m.power_cycle(),
        Event::DiskInsert { slot, drive, path } => {
            let c = m
                .card_mut::<Disk2Card>(*slot)
                .ok_or_else(|| RewindError::Disk(format!("no Disk II in slot {slot}")))?;
            c.insert_path(*drive, Path::new(path), false)
                .map_err(|e| RewindError::Disk(format!("{path}: {e:?}")))?;
        }
        Event::DiskEject { slot, drive } => {
            if let Some(c) = m.card_mut::<Disk2Card>(*slot) {
                c.eject_disk(*drive);
            }
        }
        Event::CardUpdate(n) => m.update_cards(*n),
    }
    Ok(())
}

impl Timeline {
    /// Starts recording at the machine's current state (the first, always full, checkpoint).
    pub fn new(m: &Machine, cfg: RewindConfig) -> Self {
        let mut t = Timeline {
            cfg,
            checkpoints: VecDeque::new(),
            tail_blob: Vec::new(),
            log: VecDeque::new(),
            log_base: 0,
            instr: 0,
            ev: 0,
            head_instr: 0,
            last_cycles: m.cycles(),
            frames_since_cp: 0,
            replaying: false,
        };
        t.checkpoint(m);
        t
    }

    // ---- queries ----

    pub fn config(&self) -> &RewindConfig {
        &self.cfg
    }

    /// Changes the limits; the new limits are enforced at once.
    pub fn set_config(&mut self, cfg: RewindConfig) {
        self.cfg = cfg;
        self.enforce_limits();
    }

    /// Instructions executed since recording started (the cursor).
    pub fn instr(&self) -> u64 {
        self.instr
    }

    /// Newest instruction count that was ever recorded.
    pub fn head_instr(&self) -> u64 {
        self.head_instr
    }

    /// Oldest instruction count that can be reached.
    pub fn oldest_instr(&self) -> u64 {
        self.checkpoints.front().map_or(0, |c| c.instr)
    }

    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// RAM used by checkpoints and the input log.
    pub fn memory_bytes(&self) -> usize {
        self.checkpoints
            .iter()
            .map(Checkpoint::bytes)
            .sum::<usize>()
            + self.log.iter().map(entry_bytes).sum::<usize>()
    }

    pub fn log(&self) -> impl Iterator<Item = &Entry> {
        self.log.iter()
    }

    /// True while a replay is executing; the frontend must not pump host I/O then.
    pub fn is_replaying(&self) -> bool {
        self.replaying
    }

    /// True if the cursor was moved back and has not reached the newest recorded point yet.
    pub fn is_behind_head(&self) -> bool {
        self.instr < self.head_instr || self.ev < self.log_end()
    }

    /// Cards whose external resources (network, serial, printer, mouse) are suspended in replay.
    pub fn suspended_cards(m: &Machine) -> Vec<(u8, &'static str)> {
        (0..8u8)
            .filter_map(|s| m.board().slots.card(s).map(|c| (s, c.name())))
            .filter(|(_, n)| EXTERNAL_CARDS.contains(n) || n.starts_with("ThunderClock"))
            .collect()
    }

    fn log_end(&self) -> usize {
        self.log_base + self.log.len()
    }

    // ---- checkpoints ----

    fn blob_at(&self, idx: usize) -> Vec<u8> {
        let full = (0..=idx)
            .rev()
            .find(|i| matches!(self.checkpoints[*i].payload, Payload::Full(_)))
            .expect("the oldest checkpoint is always full");
        let Payload::Full(base) = &self.checkpoints[full].payload else {
            unreachable!()
        };
        let mut blob = base.clone();
        for c in self.checkpoints.iter().take(idx + 1).skip(full + 1) {
            if let Payload::Delta { pages } = &c.payload {
                apply_pages(&mut blob, pages);
            }
        }
        blob
    }

    fn deltas_since_full(&self) -> usize {
        self.checkpoints
            .iter()
            .rev()
            .take_while(|c| matches!(c.payload, Payload::Delta { .. }))
            .count()
    }

    /// Takes a checkpoint at the cursor (which must be the live head).
    fn checkpoint(&mut self, m: &Machine) {
        if self
            .checkpoints
            .back()
            .is_some_and(|c| c.instr == self.instr && c.ev == self.ev)
        {
            return;
        }
        let enc = encode(m);
        let full = self.checkpoints.is_empty()
            || self.deltas_since_full() + 1 >= self.cfg.full_every.max(1)
            || self.tail_blob.len() != enc.blob.len();
        let payload = if full {
            Payload::Full(enc.blob.clone())
        } else {
            Payload::Delta {
                pages: diff_pages(&self.tail_blob, &enc.blob),
            }
        };
        self.checkpoints.push_back(Checkpoint {
            instr: self.instr,
            cycles: m.cycles(),
            ev: self.ev,
            meta: enc.meta,
            payload,
            mounts: mounts_of(m),
        });
        self.tail_blob = enc.blob;
        self.frames_since_cp = 0;
        self.enforce_limits();
    }

    fn drop_oldest(&mut self) {
        if self.checkpoints.len() < 2 {
            return;
        }
        if matches!(self.checkpoints[1].payload, Payload::Delta { .. }) {
            let blob = self.blob_at(1);
            self.checkpoints[1].payload = Payload::Full(blob);
        }
        self.checkpoints.pop_front();
        let keep_from = self.checkpoints[0].ev;
        while self.log_base < keep_from && !self.log.is_empty() {
            self.log.pop_front();
            self.log_base += 1;
        }
    }

    fn enforce_limits(&mut self) {
        let max = self.cfg.max_checkpoints.max(1);
        while self.checkpoints.len() > max
            || (self.checkpoints.len() > 1 && self.memory_bytes() > self.cfg.max_ram_bytes)
        {
            self.drop_oldest();
        }
    }

    // ---- cursor / divergence ----

    /// Drops everything after the cursor so new input can be recorded (a "divergence").
    fn ensure_live(&mut self) {
        if !self.is_behind_head() {
            return;
        }
        self.log.truncate(self.ev - self.log_base);
        while self.checkpoints.len() > 1
            && self
                .checkpoints
                .back()
                .is_some_and(|c| c.instr > self.instr || (c.instr == self.instr && c.ev > self.ev))
        {
            self.checkpoints.pop_back();
        }
        self.head_instr = self.instr;
        self.tail_blob = self.blob_at(self.checkpoints.len() - 1);
    }

    /// A machine that advanced without us (debugger `T`, `G`, `MEB`...) opens a gap: we
    /// checkpoint the new state and never replay across it.
    fn resync(&mut self, m: &Machine) {
        if m.cycles() != self.last_cycles {
            self.note_external_change(m);
        }
    }

    /// Call after changing the machine behind the Timeline's back (poking memory, running it
    /// directly): it becomes a new, unreplayable boundary. Earlier history stays reachable.
    pub fn note_external_change(&mut self, m: &Machine) {
        self.ensure_live();
        self.instr += 1;
        self.head_instr = self.instr;
        self.last_cycles = m.cycles();
        self.checkpoint(m);
    }

    fn apply_pending(&mut self, m: &mut Machine) -> Result<(), RewindError> {
        while self.ev < self.log_end() && self.log[self.ev - self.log_base].instr == self.instr {
            let e = self.log[self.ev - self.log_base].event.clone();
            apply_event(m, &e)?;
            self.ev += 1;
        }
        Ok(())
    }

    fn after_step(&mut self, m: &mut Machine) -> Result<(), RewindError> {
        self.instr += 1;
        self.apply_pending(m)?;
        self.head_instr = self.head_instr.max(self.instr);
        self.last_cycles = m.cycles();
        if !self.is_behind_head() {
            let last = self.checkpoints.back().map_or(0, |c| c.instr);
            if self.instr - last >= self.cfg.max_instrs_between_checkpoints.max(1) {
                self.checkpoint(m);
            }
        }
        Ok(())
    }

    // ---- driving the machine ----

    /// One instruction. Behind the head this replays the logged inputs.
    pub fn step(&mut self, m: &mut Machine) -> Result<u32, RewindError> {
        self.resync(m);
        let c = m.step();
        self.after_step(m)?;
        Ok(c)
    }

    /// Like [`Timeline::step`], but `f` performs the step (a debugger `T`). `f` must execute
    /// exactly one instruction.
    pub fn step_via(
        &mut self,
        m: &mut Machine,
        f: impl FnOnce(&mut Machine),
    ) -> Result<u64, RewindError> {
        self.resync(m);
        self.ensure_live();
        let before = m.cycles();
        f(m);
        self.after_step(m)?;
        Ok(m.cycles() - before)
    }

    /// Like `Machine::run_cycles`, recording the end-of-batch card update.
    pub fn run_cycles(&mut self, m: &mut Machine, n: u64) -> Result<u64, RewindError> {
        let mut done = 0;
        while done < n {
            done += self.step(m)? as u64;
        }
        self.end_batch(m, done)?;
        Ok(done)
    }

    /// Like `Machine::run_frame`; takes a checkpoint every `frames_per_checkpoint` frames.
    pub fn run_frame(&mut self, m: &mut Machine) -> Result<u64, RewindError> {
        let target = m.frames() + 1;
        let mut done = 0;
        while m.frames() < target {
            done += self.step(m)? as u64;
        }
        self.end_batch(m, done)?;
        if !self.is_behind_head() {
            self.frames_since_cp += 1;
            if self.frames_since_cp >= self.cfg.frames_per_checkpoint.max(1) {
                self.checkpoint(m);
            }
        }
        Ok(done)
    }

    fn end_batch(&mut self, m: &mut Machine, done: u64) -> Result<(), RewindError> {
        // Replaying forward over a recorded batch: the step loop already applied its update.
        let replayed = self.ev > self.log_base
            && self
                .log
                .get(self.ev - 1 - self.log_base)
                .is_some_and(|e| e.instr == self.instr && matches!(e.event, Event::CardUpdate(_)));
        if replayed {
            return Ok(());
        }
        self.input(m, Event::CardUpdate(done))
    }

    // ---- inputs ----

    /// Applies `e` to the machine and logs it. Behind the head this starts a new future.
    pub fn input(&mut self, m: &mut Machine, e: Event) -> Result<(), RewindError> {
        self.resync(m);
        self.ensure_live();
        let entry = Entry {
            instr: self.instr,
            cycle: m.cycles(),
            event: e,
        };
        apply_event(m, &entry.event)?;
        self.log.push_back(entry);
        self.ev = self.log_end();
        self.last_cycles = m.cycles();
        Ok(())
    }

    pub fn key_press(&mut self, m: &mut Machine, key: u8) -> Result<(), RewindError> {
        self.input(m, Event::Key(key))
    }

    pub fn paste(&mut self, m: &mut Machine, text: &str) -> Result<(), RewindError> {
        self.input(m, Event::Paste(text.to_string()))
    }

    pub fn set_button(&mut self, m: &mut Machine, n: usize, down: bool) -> Result<(), RewindError> {
        self.input(m, Event::Button { n, down })
    }

    pub fn set_paddle(&mut self, m: &mut Machine, n: u8, pos: u8) -> Result<(), RewindError> {
        self.input(m, Event::Paddle { n, pos })
    }

    pub fn reset(&mut self, m: &mut Machine) -> Result<(), RewindError> {
        self.input(m, Event::Reset)
    }

    /// Mounts the image at `path` in a Disk II (the path is logged for replay).
    pub fn insert_disk(
        &mut self,
        m: &mut Machine,
        slot: u8,
        drive: usize,
        path: &str,
    ) -> Result<(), RewindError> {
        self.input(
            m,
            Event::DiskInsert {
                slot,
                drive,
                path: path.to_string(),
            },
        )
    }

    pub fn eject_disk(
        &mut self,
        m: &mut Machine,
        slot: u8,
        drive: usize,
    ) -> Result<(), RewindError> {
        self.input(m, Event::DiskEject { slot, drive })
    }

    // ---- going back (and forward again) ----

    fn restore(&mut self, m: &mut Machine, idx: usize) -> Result<(), RewindError> {
        let blob = self.blob_at(idx);
        let cp = &self.checkpoints[idx];
        let st = decode(&cp.meta, &blob)
            .ok_or_else(|| RewindError::Restore("corrupt checkpoint".into()))?;
        m.load_state(&st)
            .map_err(|e| RewindError::Restore(e.to_string()))?;
        let want = cp.mounts.clone();
        for s in 0..8u8 {
            if m.card::<Disk2Card>(s).is_none() {
                continue;
            }
            for d in 0..2 {
                let w = want.iter().find(|(ws, wd, _)| *ws == s && *wd == d);
                let cur = m.card::<Disk2Card>(s).map(|c| c.pathname(d).to_string());
                match (w, cur.as_deref()) {
                    (Some((_, _, p)), Some(c)) if p != c => {
                        let card = m.card_mut::<Disk2Card>(s).expect("checked above");
                        card.insert_path(d, Path::new(p), false)
                            .map_err(|e| RewindError::Disk(format!("{p}: {e:?}")))?;
                    }
                    (None, Some(c)) if !c.is_empty() => {
                        m.card_mut::<Disk2Card>(s)
                            .expect("checked above")
                            .eject_disk(d);
                    }
                    _ => {}
                }
            }
        }
        self.instr = self.checkpoints[idx].instr;
        self.ev = self.checkpoints[idx].ev;
        Ok(())
    }

    /// Moves the cursor to instruction `target` (back or forward up to the head) with the
    /// state of the machine exactly as it was there, including the inputs logged at it.
    pub fn rewind_to_instr(&mut self, m: &mut Machine, target: u64) -> Result<(), RewindError> {
        self.resync(m);
        if target > self.head_instr {
            return Err(RewindError::BeyondHead {
                head: self.head_instr,
            });
        }
        let idx = self
            .checkpoints
            .iter()
            .rposition(|c| c.instr <= target)
            .ok_or(RewindError::OutOfRange {
                oldest: self.oldest_instr(),
            })?;
        let continue_forward = target >= self.instr && self.checkpoints[idx].instr <= self.instr;
        self.replaying = true;
        let r = (|| {
            if !continue_forward {
                self.restore(m, idx)?;
            }
            self.apply_pending(m)?;
            while self.instr < target {
                m.step();
                self.instr += 1;
                self.apply_pending(m)?;
            }
            Ok(())
        })();
        self.replaying = false;
        self.last_cycles = m.cycles();
        r
    }

    /// Reverse step: goes back `n` instructions.
    pub fn step_back(&mut self, m: &mut Machine, n: u64) -> Result<(), RewindError> {
        let target = self.instr.saturating_sub(n);
        if target < self.oldest_instr() {
            return Err(RewindError::OutOfRange {
                oldest: self.oldest_instr(),
            });
        }
        self.rewind_to_instr(m, target)
    }

    /// Goes to the last instruction boundary whose cycle counter is at most `cycle`.
    pub fn rewind_to_cycle(&mut self, m: &mut Machine, cycle: u64) -> Result<(), RewindError> {
        self.resync(m);
        // Find the instruction by a dry replay from the newest checkpoint that starts at or
        // before the cycle.
        let start = (0..self.checkpoints.len())
            .rev()
            .find(|i| self.checkpoints[*i].cycles <= cycle)
            .ok_or(RewindError::OutOfRange {
                oldest: self.oldest_instr(),
            })?;
        self.replaying = true;
        let r = (|| {
            self.restore(m, start)?;
            self.apply_pending(m)?;
            let mut last = self.instr;
            while m.cycles() < cycle && self.instr < self.head_instr {
                last = self.instr;
                m.step();
                self.instr += 1;
                self.apply_pending(m)?;
            }
            Ok(if m.cycles() > cycle { last } else { self.instr })
        })();
        self.replaying = false;
        let target = r?;
        self.rewind_to_instr(m, target)
    }

    /// Replays forward to the newest recorded instruction.
    pub fn replay_to_head(&mut self, m: &mut Machine) -> Result<(), RewindError> {
        self.rewind_to_instr(m, self.head_instr)
    }
}
