//! State shared between the audio cards and the [`Mixer`](crate::Mixer): sample clock, PCM
//! queues, the 8-bit DAC log and the Mockingboard IRQ aggregate.
//!
//! The core hands cards a `Box<dyn Card>` and never gets it back, so a card publishes what the
//! frontend needs through this bus (cheap `Arc` clones; the lock is never contended because
//! emulation and mixing run on the same thread).

use crate::clock::SampleClock;
use apple2_core::VideoStandard;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Frames a source keeps if nobody drains it (1 s at 48 kHz): the newest win.
pub(crate) const MAX_PENDING: usize = 48_000;

/// A card's output: stereo frames indexed by absolute frame number (frame `k` is sampled at
/// `SampleClock::cycle_of_frame(k)`), so the mixer can line sources up exactly.
#[derive(Debug)]
pub struct SourceBuf {
    frames: VecDeque<[f32; 2]>,
    /// Index of `frames[0]`.
    first: u64,
    /// Linear gain applied by the mixer (per-card volume).
    pub gain: f32,
    pub(crate) mute: bool,
}

impl SourceBuf {
    pub(crate) fn push(&mut self, f: [f32; 2]) {
        if self.frames.len() >= MAX_PENDING {
            self.frames.pop_front();
            self.first += 1;
        }
        self.frames.push_back(f);
    }

    /// Forgets everything and continues at frame `idx` (time jumped).
    pub(crate) fn restart_at(&mut self, idx: u64) {
        self.frames.clear();
        self.first = idx;
    }

    /// Removes and returns frames `start..start+n`; frames that are not there read as silence.
    pub(crate) fn take(&mut self, start: u64, n: usize) -> Vec<[f32; 2]> {
        while self.first < start && !self.frames.is_empty() {
            self.frames.pop_front();
            self.first += 1;
        }
        let mut out = vec![[0.0; 2]; n];
        for (i, o) in out.iter_mut().enumerate() {
            let idx = start + i as u64;
            if idx >= self.first && !self.frames.is_empty() {
                *o = self.frames.pop_front().unwrap();
                self.first += 1;
            }
        }
        out
    }
}

pub type Source = Arc<Mutex<SourceBuf>>;

struct Inner {
    clock: Mutex<SampleClock>,
    sources: Mutex<Vec<Source>>,
    /// 8-bit DAC writes `(cycle, value)` from SAM cards, merged into the speaker stream.
    dac: Mutex<Vec<(u64, u8)>>,
    /// Bit `slot`: that slot's 6522s assert IRQ; bit `8 + slot`: its speech chip does.
    irq: AtomicU32,
}

/// Cheap-to-clone handle; create one per machine and pass it to every audio card and the mixer.
#[derive(Clone)]
pub struct AudioBus(Arc<Inner>);

impl AudioBus {
    pub fn new(rate_hz: u32, standard: VideoStandard) -> Self {
        Self::with_clock(SampleClock::new(rate_hz, standard))
    }

    pub fn with_clock(clock: SampleClock) -> Self {
        AudioBus(Arc::new(Inner {
            clock: Mutex::new(clock),
            sources: Mutex::new(Vec::new()),
            dac: Mutex::new(Vec::new()),
            irq: AtomicU32::new(0),
        }))
    }

    pub fn clock(&self) -> SampleClock {
        *lock(&self.0.clock)
    }

    /// Changes host rate / CPU clock (e.g. PAL machine, new audio device). Sources re-base on
    /// their next update.
    pub fn set_clock(&self, clock: SampleClock) {
        *lock(&self.0.clock) = clock;
    }

    pub(crate) fn new_source(&self) -> Source {
        let s = Arc::new(Mutex::new(SourceBuf {
            frames: VecDeque::new(),
            first: 0,
            gain: 1.0,
            mute: false,
        }));
        lock(&self.0.sources).push(s.clone());
        s
    }

    pub(crate) fn sources(&self) -> Vec<Source> {
        lock(&self.0.sources).clone()
    }

    pub(crate) fn push_dac(&self, cycle: u64, value: u8) {
        let mut d = lock(&self.0.dac);
        if d.len() >= 1 << 16 {
            d.remove(0);
        }
        d.push((cycle, value));
    }

    pub(crate) fn take_dac(&self) -> Vec<(u64, u8)> {
        std::mem::take(&mut *lock(&self.0.dac))
    }

    /// Sets or clears an IRQ-asserting bit and returns whether any card still asserts.
    pub(crate) fn irq_bits(&self, bit: u32, on: bool) -> u32 {
        if on {
            self.0.irq.fetch_or(bit, Ordering::SeqCst) | bit
        } else {
            self.0.irq.fetch_and(!bit, Ordering::SeqCst) & !bit
        }
    }
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Handle to one card's volume/mute (kept by the frontend; the card itself is owned by the
/// machine).
#[derive(Clone)]
pub struct CardVolume(pub(crate) Source);

impl CardVolume {
    /// Linear gain, 1.0 = unity.
    pub fn set_gain(&self, gain: f32) {
        lock(&self.0).gain = gain;
    }

    /// AppleWin-style UI volume (see [`crate::volume_gain`]).
    pub fn set_volume(&self, volume: u32, volume_max: u32) {
        self.set_gain(crate::clock::volume_gain(volume, volume_max));
    }

    pub fn set_mute(&self, mute: bool) {
        lock(&self.0).mute = mute;
    }
}
