//! The mixer the frontend calls every frame: speaker + every audio card -> interleaved stereo.
//!
//! Replaces SoundCore's DirectSound voices and MockingboardCardManager's mix: each source is a
//! function of the Cycle counter, so `produce(cycles)` always returns exactly the frames that
//! became complete since the previous call, whatever the emulation speed.

use crate::bus::{lock, AudioBus};
use crate::speaker::{SpeakerEvent, SpeakerPcm};
use apple2_core::Machine;

pub struct Mixer {
    bus: AudioBus,
    speaker: SpeakerPcm,
    events: Vec<(u64, SpeakerEvent)>,
    clock: crate::SampleClock,
    last_cycles: u64,
    speaker_gain: f32,
    speaker_mute: bool,
}

impl Mixer {
    pub fn new(bus: &AudioBus) -> Self {
        Mixer {
            bus: bus.clone(),
            speaker: SpeakerPcm::new(),
            events: Vec::new(),
            clock: bus.clock(),
            last_cycles: 0,
            speaker_gain: 1.0,
            speaker_mute: false,
        }
    }

    /// Linear speaker gain (see [`crate::volume_gain`] for AppleWin's dB scale).
    pub fn set_speaker_gain(&mut self, gain: f32) {
        self.speaker_gain = gain;
    }

    /// AppleWin-style UI volume, `0 = loudest`.
    pub fn set_speaker_volume(&mut self, volume: u32, volume_max: u32) {
        self.speaker_gain = crate::clock::volume_gain(volume, volume_max);
    }

    pub fn set_speaker_mute(&mut self, mute: bool) {
        self.speaker_mute = mute;
    }

    /// Power cycle / new machine: forget the speaker's state.
    pub fn reset(&mut self) {
        self.speaker.reset();
        self.events.clear();
        self.last_cycles = 0;
    }

    /// Stereo samples (`L, R, L, R...`, `-1.0..=1.0`) for the time since the previous call, given
    /// the machine's current Cycle count and the speaker toggles since the previous call
    /// (absolute cycles, oldest first). Call after running the machine so the cards have caught
    /// up (`Machine::run_*` calls `Card::update` for them).
    pub fn produce(&mut self, cycles: u64, toggles: impl IntoIterator<Item = u64>) -> Vec<f32> {
        let clock = self.bus.clock();
        if clock != self.clock || cycles < self.last_cycles {
            // New audio format or the machine went back in time (state load): restart cleanly.
            self.clock = clock;
            self.events.clear();
            self.speaker.reset();
            self.speaker.skip_to(clock.complete_frames(cycles));
            self.bus.take_dac();
        }
        self.last_cycles = cycles;

        self.events.extend(toggles.into_iter().map(|c| (c, SpeakerEvent::Toggle)));
        self.events.extend(self.bus.take_dac().into_iter().map(|(c, d)| (c, SpeakerEvent::Dac(d))));
        self.events.sort_by_key(|e| e.0); // stable: toggle before DAC at the same cycle

        let start = self.speaker.frames_done();
        let mut mono = Vec::new();
        self.speaker.render(&clock, &mut self.events, cycles, &mut mono);
        let n = mono.len();
        let g = if self.speaker_mute { 0.0 } else { self.speaker_gain };
        let mut out: Vec<f32> = mono.iter().flat_map(|&s| [s * g, s * g]).collect();

        for src in self.bus.sources() {
            let mut s = lock(&src);
            let frames = s.take(start, n);
            let g = if s.mute { 0.0 } else { s.gain };
            for (i, f) in frames.iter().enumerate() {
                out[2 * i] += f[0] * g;
                out[2 * i + 1] += f[1] * g;
            }
        }
        for v in &mut out {
            *v = v.clamp(-1.0, 1.0);
        }
        out
    }

    /// [`produce`](Self::produce) fed from a [`Machine`]: drains its speaker log.
    pub fn produce_machine(&mut self, m: &mut Machine) -> Vec<f32> {
        let cycles = m.cycles();
        let toggles: Vec<u64> = m.drain_speaker_toggles().collect();
        self.produce(cycles, toggles)
    }
}
