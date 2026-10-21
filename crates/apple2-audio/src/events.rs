//! Audio event log for differential conformance testing (HU-0501 / HU-0502).
//!
//! The oracle (`tools/conformance/oracle/audetto-oracle`, `audio.cpp`) writes one line per
//! observable audio event with the 6502 Cycle; this log produces the same lines from the Rust
//! cards so that the two can be compared exactly (policy: docs/conformance/test-oracles.md,
//! "Áudio"). Recording is off by default and costs one `Option` test per event.

/// IRQ line source, as `eIRQSRC` names it in the oracle's log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrqLine {
    /// `IS_6522`: any Mockingboard 6522 asserting.
    Via6522,
    /// `IS_SPEECH`: an SSI263 asserting (Phasor native mode).
    Speech,
}

impl IrqLine {
    fn name(self) -> &'static str {
        match self {
            IrqLine::Via6522 => "6522",
            IrqLine::Speech => "SPEECH",
        }
    }
}

/// One observable audio event (the kinds of the `audio-events v1` format).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioEvent {
    /// Write to the card page `$Cs00-$CsFF` (6522 and SSI263 registers by address).
    IoWrite { slot: u8, addr: u8, value: u8 },
    /// Read from the card page, with the value returned to the CPU.
    IoRead { slot: u8, addr: u8, value: u8 },
    /// AY register write (`_AYWriteReg`).
    AyWrite {
        slot: u8,
        unit: u8,
        ay: u8,
        reg: u8,
        value: u8,
    },
    /// AY register read (`AYReadReg`).
    AyRead {
        slot: u8,
        unit: u8,
        ay: u8,
        reg: u8,
        value: u8,
    },
    /// Transition of an IRQ line (only when the level changes).
    Irq { line: IrqLine, level: bool },
    /// Speaker toggle (`$C030-$C03F`).
    Speaker,
    /// An SSI263 phoneme starts; `samples` is the length of its 22.05 kHz wave.
    PhonemeStart {
        slot: u8,
        device: u8,
        phoneme: u8,
        samples: u32,
    },
    /// An SSI263 phoneme is complete (the "phoneme complete" interrupt).
    PhonemeEnd { slot: u8, device: u8 },
    /// Phasor mode-select access at `$C0(8+slot)x`.
    PhasorIo { slot: u8, addr: u8, write: bool },
}

impl AudioEvent {
    /// The `audio-events v1` line, without the newline.
    pub fn line(&self, cycle: u64) -> String {
        match *self {
            AudioEvent::IoWrite { slot, addr, value } => {
                format!("{cycle},IOW,{slot},{addr:02X},{value:02X}")
            }
            AudioEvent::IoRead { slot, addr, value } => {
                format!("{cycle},IOR,{slot},{addr:02X},{value:02X}")
            }
            AudioEvent::AyWrite {
                slot,
                unit,
                ay,
                reg,
                value,
            } => format!("{cycle},AYW,{slot},{unit},{ay},{reg:X},{value:02X}"),
            AudioEvent::AyRead {
                slot,
                unit,
                ay,
                reg,
                value,
            } => format!("{cycle},AYR,{slot},{unit},{ay},{reg:X},{value:02X}"),
            AudioEvent::Irq { line, level } => {
                format!("{cycle},IRQ,{},{}", line.name(), level as u8)
            }
            AudioEvent::Speaker => format!("{cycle},SPK"),
            AudioEvent::PhonemeStart {
                slot,
                device,
                phoneme,
                samples,
            } => format!("{cycle},PHON,{slot},{device},{phoneme:02X},{samples}"),
            AudioEvent::PhonemeEnd { slot, device } => format!("{cycle},PHEND,{slot},{device}"),
            AudioEvent::PhasorIo { slot, addr, write } => {
                format!("{cycle},PIO,{slot},{addr:02X},{}", write as u8)
            }
        }
    }
}

/// Renders `(cycle, event)` pairs as an `audio-events v1` file body (header lines start with `#`).
pub fn render(events: &[(u64, AudioEvent)]) -> String {
    let mut s = String::from("#audio_events=v1\n");
    for (c, e) in events {
        s.push_str(&e.line(*c));
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_match_the_oracle_format() {
        let l = |e: AudioEvent| e.line(1234);
        assert_eq!(
            l(AudioEvent::IoWrite {
                slot: 4,
                addr: 0x01,
                value: 0xAB
            }),
            "1234,IOW,4,01,AB"
        );
        assert_eq!(
            l(AudioEvent::AyWrite {
                slot: 4,
                unit: 1,
                ay: 0,
                reg: 0xD,
                value: 0x0E
            }),
            "1234,AYW,4,1,0,D,0E"
        );
        assert_eq!(
            l(AudioEvent::Irq {
                line: IrqLine::Via6522,
                level: true
            }),
            "1234,IRQ,6522,1"
        );
        assert_eq!(l(AudioEvent::Speaker), "1234,SPK");
        assert_eq!(
            l(AudioEvent::PhonemeStart {
                slot: 4,
                device: 1,
                phoneme: 0x0E,
                samples: 3000
            }),
            "1234,PHON,4,1,0E,3000"
        );
        assert_eq!(
            l(AudioEvent::PhonemeEnd { slot: 4, device: 1 }),
            "1234,PHEND,4,1"
        );
        assert_eq!(
            l(AudioEvent::PhasorIo {
                slot: 4,
                addr: 0xC5,
                write: false
            }),
            "1234,PIO,4,C5,0"
        );
    }
}
