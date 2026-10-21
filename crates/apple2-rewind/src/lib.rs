//! Temporal debugging (HU-1201): rewind, reverse step and deterministic replay.
//!
//! This is an extension of this project: AppleWin has no such feature, so the debugger commands
//! `REWIND` and `STEPBACK` (see [`commands`]) are named by us and documented as non-AppleWin.
//!
//! # Model
//!
//! A [`Timeline`] drives a [`Machine`](apple2_core::Machine) instead of the frontend calling
//! `step`/`run_frame` directly. Time is the instruction counter ([`Timeline::instr`]). The
//! Timeline keeps:
//!
//! * **checkpoints** taken every N frames (and at least every `max_instrs_between_checkpoints`
//!   instructions when the machine is only single-stepped). Every K-th checkpoint holds the whole
//!   state; the others hold the changed 256-byte pages of the RAM blob plus the (small) rest of
//!   the state;
//! * an **input log**: keys, paste, buttons, paddles, resets, disk changes and the end-of-batch
//!   card updates, each tagged with the instruction count and the cycle at which it happened.
//!
//! Going back to instruction `t` restores the newest checkpoint at or before `t` and re-executes
//! up to `t`, re-applying the logged inputs at the same instruction counts. The machine core is
//! deterministic (ADR 0001), so the result is bit-identical to the original run.
//!
//! # Non-reproducible external resources: suspended, not logged
//!
//! Uthernet (network), Super Serial Card (real serial port), the printer (host file) and the
//! ThunderClock (host clock) get data from outside the machine. We chose to **suspend** them
//! during replay instead of logging their traffic: a log of network packets or serial bytes would
//! be large, would reflect the host at that moment rather than the program under debug, and could
//! not be replayed on a host without the same peer. Concretely:
//!
//! * the card state inside the machine *is* checkpointed and restored like any other state;
//! * data that arrives from outside while a replay runs is not recorded; the frontend must not
//!   pump host I/O while [`Timeline::is_replaying`] is true (and should not while the cursor is
//!   behind the live head, see [`Timeline::is_behind_head`]);
//! * [`Timeline::suspended_cards`] lists these cards so the UI can warn the user.
//!
//! # Limitations
//!
//! Disk *mounts* are rewound (the log and checkpoints reopen the image path), but the contents
//! of an image that the program wrote to are not: the image files are outside the Machine.
//! Stepping the machine behind the Timeline's back is detected and handled as a "gap": rewinding
//! never crosses it.

#![forbid(unsafe_code)]

pub mod commands;
mod snapshot;
mod timeline;

pub use snapshot::state_hash;
pub use timeline::{Entry, Event, RewindConfig, RewindError, Timeline};
