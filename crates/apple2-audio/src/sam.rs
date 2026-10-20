//! SAM (Software Automated Mouth) 8-bit DAC card (AppleWin SAM.cpp).
//!
//! Writes to `$C0s0-$C0sF` drive the speaker cone directly with an unsigned byte
//! (`g_nSpeakerData = (d ^ 0x80) << 8`) and quieten the real speaker clicks until it has been
//! idle for 0.2 s. The card publishes its writes on the [`AudioBus`]; the [`Mixer`](crate::Mixer)
//! merges them into the speaker stream.

use crate::bus::AudioBus;
use apple2_core::{Card, CardCtx};

pub struct SamCard {
    bus: AudioBus,
}

impl SamCard {
    pub fn new(bus: &AudioBus) -> Self {
        SamCard { bus: bus.clone() }
    }
}

impl Card for SamCard {
    fn name(&self) -> &'static str {
        "SAM"
    }

    fn io_write(&mut self, _lo: u8, value: u8, ctx: &mut CardCtx) {
        self.bus.push_dac(ctx.cycles, value);
    }
}
