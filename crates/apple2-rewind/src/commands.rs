//! Debugger commands for temporal debugging. **They are an extension of this project**: the
//! AppleWin debugger has no `REWIND`, `STEPBACK` or `REPLAY`.
//!
//! The commands live here, not in the debugger's command table, so that `apple2-debugger` stays
//! free of a dependency on this crate. A frontend that wants them calls [`execute`] instead of
//! `Debugger::execute`; anything that is not ours is passed through unchanged.
//!
//! | Command        | Effect                                                         |
//! |----------------|----------------------------------------------------------------|
//! | `REWIND n`     | go back `n` instructions (`n` in hex like all debugger numbers, default 1) |
//! | `STEPBACK [n]` | same as `REWIND`; `STEPBACK` alone is one instruction back      |
//! | `REPLAY`       | replay forward, with the logged inputs, to the newest point    |
//!
//! `T [n]` is intercepted too so that traced instructions are recorded and can be undone.
//! Other commands go to the debugger; if one changes the machine (memory edits, `G`...), the
//! Timeline opens a gap there (see [`Timeline::note_external_change`]).

use crate::{state_hash, RewindError, Timeline};
use apple2_core::Machine;
use apple2_debugger::{upd, Debugger, Update};

/// Names and one-line help of the commands added here.
pub const COMMANDS: [(&str, &str); 3] = [
    ("REWIND", "Go back n instructions (non-AppleWin extension)"),
    (
        "STEPBACK",
        "Reverse step, default 1 instruction (non-AppleWin extension)",
    ),
    (
        "REPLAY",
        "Replay forward to the newest recorded instruction (non-AppleWin extension)",
    ),
];

fn count(arg: Option<&&str>) -> Option<u64> {
    match arg {
        None => Some(1),
        Some(s) => u64::from_str_radix(s.trim_start_matches(['$', '#']), 16).ok(),
    }
}

fn report(dbg: &mut Debugger, m: &Machine, r: Result<(), RewindError>, ok: &str) -> Update {
    match r {
        Ok(()) => {
            dbg.console.print(ok);
            dbg.goto(m, m.cpu().pc);
        }
        Err(e) => dbg.console.error(&e.to_string()),
    }
    upd::ALL
}

/// Runs one debugger command line, handling the temporal ones.
pub fn execute(tl: &mut Timeline, dbg: &mut Debugger, m: &mut Machine, line: &str) -> Update {
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some(verb) = words.first().map(|w| w.to_ascii_uppercase()) else {
        return dbg.execute(m, line);
    };
    match verb.as_str() {
        "REWIND" | "STEPBACK" => {
            let Some(n) = count(words.get(1)) else {
                dbg.console.error("Syntax Error");
                return upd::ALL;
            };
            let r = tl.step_back(m, n);
            let msg = format!("Back at instruction {} (cycle {})", tl.instr(), m.cycles());
            report(dbg, m, r, &msg)
        }
        "REPLAY" => {
            let r = tl.replay_to_head(m);
            let msg = format!("Replayed to instruction {}", tl.instr());
            report(dbg, m, r, &msg)
        }
        "T" => {
            let Some(n) = count(words.get(1)) else {
                dbg.console.error("Syntax Error");
                return upd::ALL;
            };
            for _ in 0..n {
                if let Err(e) = tl.step_via(m, |m| {
                    dbg.trace_n(m, 1);
                }) {
                    dbg.console.error(&e.to_string());
                    break;
                }
            }
            upd::ALL
        }
        _ => {
            let before = state_hash(m);
            let u = dbg.execute(m, line);
            if state_hash(m) != before {
                tl.note_external_change(m);
            }
            u
        }
    }
}
