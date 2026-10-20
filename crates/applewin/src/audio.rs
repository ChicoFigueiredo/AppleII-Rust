//! Host audio: a lock-protected stereo ring between the emulation thread (producer) and the cpal
//! callback (consumer). The fill level of the ring is what paces the emulation at 1.0x (ADR 0003).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub struct AudioRing {
    data: Mutex<VecDeque<f32>>,
    capacity_frames: usize,
    gain: AtomicU32,
    /// Underruns only count while the emulation is feeding the ring at real time.
    active: AtomicBool,
    underruns: AtomicU64,
    overruns: AtomicU64,
}

impl AudioRing {
    pub fn new(capacity_frames: usize) -> Arc<Self> {
        Arc::new(AudioRing {
            data: Mutex::new(VecDeque::with_capacity(capacity_frames * 2)),
            capacity_frames,
            gain: AtomicU32::new(1.0f32.to_bits()),
            active: AtomicBool::new(false),
            underruns: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<f32>> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queues interleaved stereo samples; what does not fit is dropped (overrun).
    pub fn push(&self, stereo: &[f32]) {
        let mut d = self.lock();
        let room = (self.capacity_frames * 2).saturating_sub(d.len());
        let n = stereo.len().min(room) & !1;
        if n < stereo.len() {
            self.overruns.fetch_add(1, Ordering::Relaxed);
        }
        d.extend(&stereo[..n]);
    }

    pub fn fill_frames(&self) -> usize {
        self.lock().len() / 2
    }

    pub fn clear(&self) {
        self.lock().clear();
    }

    pub fn set_gain(&self, gain: f32) {
        self.gain.store(gain.to_bits(), Ordering::Relaxed);
    }

    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
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
            let (l, r) = match (d.pop_front(), d.pop_front()) {
                (Some(l), Some(r)) => (l * gain, r * gain),
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
        if starved && self.active.load(Ordering::Relaxed) {
            self.underruns.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// An open output stream. Keep it alive (and on the thread that created it) while playing.
pub struct AudioOut {
    _stream: cpal::Stream,
    pub ring: Arc<AudioRing>,
    pub sample_rate: u32,
}

impl AudioOut {
    pub fn open() -> Result<AudioOut, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let rate = supported.sample_rate().0;
        let channels = usize::from(supported.channels());
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let ring = AudioRing::new(rate as usize * 3 / 10);
        let err = |e| eprintln!("audio stream error: {e}");
        let stream = match format {
            SampleFormat::F32 => {
                let r = ring.clone();
                device.build_output_stream(&config, move |o: &mut [f32], _| r.pop_into(o, channels), err, None)
            }
            SampleFormat::I16 => {
                let r = ring.clone();
                device.build_output_stream(&config, move |o: &mut [i16], _| r.pop_into(o, channels), err, None)
            }
            SampleFormat::U16 => {
                let r = ring.clone();
                device.build_output_stream(&config, move |o: &mut [u16], _| r.pop_into(o, channels), err, None)
            }
            other => return Err(format!("unsupported sample format {other:?}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(AudioOut { _stream: stream, ring, sample_rate: rate })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pop_applies_gain_and_pads_underrun_with_silence() {
        let ring = AudioRing::new(100);
        ring.set_active(true);
        ring.set_gain(0.5);
        ring.push(&[1.0, -1.0]);
        let mut out = [9.0f32; 6]; // 3 stereo frames
        ring.pop_into(&mut out, 2);
        assert_eq!(out, [0.5, -0.5, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(ring.underruns(), 1);
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
    fn overrun_drops_the_excess() {
        let ring = AudioRing::new(2);
        ring.push(&[0.0; 10]);
        assert_eq!(ring.fill_frames(), 2);
        assert_eq!(ring.overruns(), 1);
    }
}
