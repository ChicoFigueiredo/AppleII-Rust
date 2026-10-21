//! Host audio: a lock-protected stereo ring between the emulation thread (producer) and the cpal
//! callback (consumer). The fill level of the ring is what paces the emulation at 1.0x (ADR 0003).
//!
//! Like AppleWin's `SoundCore_SetFade`, the output fades out when emulation stops feeding the
//! ring (pause, dialogs, full speed) and fades in when it resumes, so there are no clicks. The
//! only user-facing sound option AppleWin has is the master volume (AppleWin has no drive sounds
//! and no separate speaker/Mockingboard volume; the PC speaker and the cards are mixed upstream).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Length of the fade in/out.
pub const FADE_SECS: f32 = 0.02;

/// One step of a linear fade from `level` toward `target`.
pub fn ramp(level: f32, target: f32, step: f32) -> f32 {
    if level < target {
        (level + step).min(target)
    } else {
        (level - step).max(target)
    }
}

struct Inner {
    data: VecDeque<f32>,
    /// Fade multiplier now, and where it is heading (0 = muted, 1 = open).
    level: f32,
    target: f32,
    /// A fade-out ends by dropping the queued samples (they belong to a stopped timeline).
    clear_when_silent: bool,
}

pub struct AudioRing {
    inner: Mutex<Inner>,
    capacity_frames: usize,
    fade_step: f32,
    gain: AtomicU32,
    /// Underruns only count while the emulation is feeding the ring at real time.
    active: AtomicBool,
    underruns: AtomicU64,
    overruns: AtomicU64,
}

impl AudioRing {
    /// A ring of `capacity_frames` stereo frames at 48 kHz.
    pub fn new(capacity_frames: usize) -> Arc<Self> {
        Self::with_rate(capacity_frames, 48_000)
    }

    pub fn with_rate(capacity_frames: usize, sample_rate: u32) -> Arc<Self> {
        Arc::new(AudioRing {
            inner: Mutex::new(Inner {
                data: VecDeque::with_capacity(capacity_frames * 2),
                level: 1.0,
                target: 1.0,
                clear_when_silent: false,
            }),
            capacity_frames,
            fade_step: 1.0 / (sample_rate.max(1) as f32 * FADE_SECS),
            gain: AtomicU32::new(1.0f32.to_bits()),
            active: AtomicBool::new(false),
            underruns: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queues interleaved stereo samples; what does not fit is dropped (overrun).
    pub fn push(&self, stereo: &[f32]) {
        let mut d = self.lock();
        let room = (self.capacity_frames * 2).saturating_sub(d.data.len());
        let n = stereo.len().min(room) & !1;
        if n < stereo.len() {
            self.overruns.fetch_add(1, Ordering::Relaxed);
        }
        d.data.extend(&stereo[..n]);
    }

    pub fn fill_frames(&self) -> usize {
        self.lock().data.len() / 2
    }

    /// Drops everything at once (settings changes, reset of the timeline).
    pub fn clear(&self) {
        self.lock().data.clear();
    }

    pub fn set_gain(&self, gain: f32) {
        self.gain.store(gain.to_bits(), Ordering::Relaxed);
    }

    /// The emulation is (not) feeding the ring in real time. Turning it off fades the output out
    /// and then empties the ring; turning it on fades back in.
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
        let mut d = self.lock();
        d.target = if active { 1.0 } else { 0.0 };
        d.clear_when_silent = !active;
    }

    pub fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub fn overruns(&self) -> u64 {
        self.overruns.load(Ordering::Relaxed)
    }

    /// Fills `out` (`channels` interleaved) from the ring; silence on underrun.
    pub fn pop_into<T: SizedSample + FromSample<f32>>(&self, out: &mut [T], channels: usize) {
        let gain = f32::from_bits(self.gain.load(Ordering::Relaxed));
        let mut d = self.lock();
        let mut starved = false;
        for frame in out.chunks_mut(channels.max(1)) {
            d.level = ramp(d.level, d.target, self.fade_step);
            let fade = gain * d.level;
            let (l, r) = match (d.data.pop_front(), d.data.pop_front()) {
                (Some(l), Some(r)) => (l * fade, r * fade),
                _ => {
                    starved = true;
                    (0.0, 0.0)
                }
            };
            for (i, s) in frame.iter_mut().enumerate() {
                let v = match (channels, i) {
                    (1, _) => (l + r) * 0.5,
                    (_, 0) => l,
                    (_, 1) => r,
                    _ => 0.0,
                };
                *s = T::from_sample(v);
            }
        }
        if d.clear_when_silent && d.level <= 0.0 {
            d.data.clear();
            d.clear_when_silent = false;
        }
        if starved && self.active.load(Ordering::Relaxed) {
            self.underruns.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Output buffer wanted from the device: about 21 ms at 48 kHz keeps the audio-paced emulation
/// responsive without risking underruns.
const WANTED_BUFFER_FRAMES: u32 = 1024;

/// An open output stream. Keep it alive (and on the thread that created it) while playing.
pub struct AudioOut {
    _stream: cpal::Stream,
    pub ring: Arc<AudioRing>,
    pub sample_rate: u32,
}

impl AudioOut {
    pub fn open() -> Result<AudioOut, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let rate = supported.sample_rate().0;
        let channels = usize::from(supported.channels());
        let format = supported.sample_format();
        let mut config: cpal::StreamConfig = supported.config();
        let ring = AudioRing::with_rate(rate as usize * 3 / 10, rate);
        let err = |e| eprintln!("audio stream error: {e}");
        let build = |config: &cpal::StreamConfig| match format {
            SampleFormat::F32 => {
                let r = ring.clone();
                device.build_output_stream(
                    config,
                    move |o: &mut [f32], _| r.pop_into(o, channels),
                    err,
                    None,
                )
            }
            SampleFormat::I16 => {
                let r = ring.clone();
                device.build_output_stream(
                    config,
                    move |o: &mut [i16], _| r.pop_into(o, channels),
                    err,
                    None,
                )
            }
            SampleFormat::U16 => {
                let r = ring.clone();
                device.build_output_stream(
                    config,
                    move |o: &mut [u16], _| r.pop_into(o, channels),
                    err,
                    None,
                )
            }
            other => Err(cpal::BuildStreamError::BackendSpecific {
                err: cpal::BackendSpecificError {
                    description: format!("unsupported sample format {other:?}"),
                },
            }),
        };
        // A small fixed buffer when the device allows it; otherwise (or on failure) the default.
        let stream = match device
            .default_output_config()
            .ok()
            .map(|c| *c.buffer_size())
        {
            Some(cpal::SupportedBufferSize::Range { min, max }) => {
                config.buffer_size = cpal::BufferSize::Fixed(WANTED_BUFFER_FRAMES.clamp(min, max));
                build(&config).or_else(|_| {
                    config.buffer_size = cpal::BufferSize::Default;
                    build(&config)
                })
            }
            _ => build(&config),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(AudioOut {
            _stream: stream,
            ring,
            sample_rate: rate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pop(ring: &AudioRing, frames: usize) -> Vec<f32> {
        let mut out = vec![9.0f32; frames * 2];
        ring.pop_into(&mut out, 2);
        out
    }

    #[test]
    fn pop_applies_gain_and_pads_underrun_with_silence() {
        let ring = AudioRing::new(100);
        ring.set_active(true);
        ring.set_gain(0.5);
        ring.push(&[1.0, -1.0]);
        let out = pop(&ring, 3);
        assert_eq!(out, [0.5, -0.5, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(ring.underruns(), 1);
    }

    #[test]
    fn underruns_are_not_counted_when_the_emulation_is_not_feeding() {
        let ring = AudioRing::new(100);
        pop(&ring, 4);
        assert_eq!(ring.underruns(), 0);
    }

    #[test]
    fn mono_devices_get_the_mix() {
        let ring = AudioRing::new(100);
        ring.push(&[1.0, 0.0]);
        let mut out = [0.0f32; 1];
        ring.pop_into(&mut out, 1);
        assert_eq!(out, [0.5]);
    }

    #[test]
    fn overrun_drops_the_excess_keeping_whole_frames() {
        let ring = AudioRing::new(2);
        ring.push(&[0.0; 10]);
        assert_eq!(ring.fill_frames(), 2);
        assert_eq!(ring.overruns(), 1);
        ring.push(&[1.0; 3]); // full: dropped; an odd tail never splits a frame
        assert_eq!(ring.fill_frames(), 2);
    }

    #[test]
    fn ramp_is_linear_and_stops_on_target() {
        assert_eq!(ramp(0.0, 1.0, 0.25), 0.25);
        assert_eq!(ramp(0.9, 1.0, 0.25), 1.0);
        assert_eq!(ramp(1.0, 0.0, 0.25), 0.75);
        assert_eq!(ramp(0.1, 0.0, 0.25), 0.0);
        assert_eq!(ramp(0.5, 0.5, 0.25), 0.5);
    }

    #[test]
    fn stopping_fades_out_then_empties_the_ring() {
        let ring = AudioRing::with_rate(10_000, 1_000); // fade = 20 frames
        ring.set_active(true);
        ring.push(&[1.0; 200]); // 100 frames
        ring.set_active(false);
        let out = pop(&ring, 30);
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        assert!(
            left[0] < 1.0 && left[0] > 0.9,
            "starts fading at once: {}",
            left[0]
        );
        assert!(left.windows(2).all(|w| w[1] <= w[0]), "monotonic fade");
        assert_eq!(left[29], 0.0, "silent after the fade");
        assert_eq!(
            ring.fill_frames(),
            0,
            "stale samples are dropped once silent"
        );
    }

    #[test]
    fn resuming_fades_in_from_silence() {
        let ring = AudioRing::with_rate(10_000, 1_000);
        ring.set_active(false);
        pop(&ring, 40); // settle at silence
        ring.set_active(true);
        ring.push(&[1.0; 200]);
        let out = pop(&ring, 30);
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        assert!(left[0] > 0.0 && left[0] < 0.1);
        assert!(left.windows(2).all(|w| w[1] >= w[0]), "monotonic fade-in");
        assert_eq!(left[29], 1.0);
    }

    #[test]
    fn hard_clear_is_immediate() {
        let ring = AudioRing::new(100);
        ring.push(&[1.0; 20]);
        ring.clear();
        assert_eq!(ring.fill_frames(), 0);
    }

    #[test]
    fn gain_and_fade_multiply() {
        let ring = AudioRing::with_rate(100, 1_000);
        ring.set_active(true);
        ring.set_gain(0.5);
        ring.push(&[1.0, 1.0]);
        assert_eq!(pop(&ring, 1), [0.5, 0.5]);
    }
}
