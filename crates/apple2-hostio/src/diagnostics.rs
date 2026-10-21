//! User-visible host I/O errors (AUD-P-023, AUD-P-030, AUD-P-033).
//!
//! AppleWin swallows these failures: a pcap adapter that does not open, a COM port that is busy
//! or a print file that cannot be created leave the card "disconnected" and, at most, write a line
//! to its log file. We keep the emulation going exactly the same way, but also queue a
//! descriptive message here. The frontend drains the queue every frame ([`drain`]) and shows the
//! messages in its status line and on stderr.
//!
//! The queue is process-wide on purpose: cards are built by the shell and run on the emulation
//! thread, with no channel back to the window. A failing source is reported once and again only
//! if its message changes; [`resolved`] re-arms it after a success.

use std::collections::BTreeMap;
use std::sync::Mutex;

/// Messages kept before the oldest are dropped.
const CAPACITY: usize = 32;

#[derive(Default)]
struct State {
    queue: Vec<String>,
    /// Last message per source, to avoid repeating the same failure on every access.
    last: BTreeMap<String, String>,
}

static STATE: Mutex<State> = Mutex::new(State {
    queue: Vec::new(),
    last: BTreeMap::new(),
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Queues `message` for `source` (for instance `"printer:/tmp/x"`). Returns false when it
/// repeats the previous message of that source and was therefore not queued.
pub fn report(source: &str, message: impl Into<String>) -> bool {
    let message = message.into();
    let mut s = state();
    if s.last.get(source) == Some(&message) {
        return false;
    }
    s.last.insert(source.to_string(), message.clone());
    if s.queue.len() >= CAPACITY {
        s.queue.remove(0);
    }
    s.queue.push(message);
    true
}

/// The source works again: a later failure is reported afresh.
pub fn resolved(source: &str) {
    state().last.remove(source);
}

/// Takes every queued message, oldest first.
pub fn drain() -> Vec<String> {
    std::mem::take(&mut state().queue)
}

/// The queued messages, left in place (for tests that must not race a concurrent [`drain`]).
pub fn peek() -> Vec<String> {
    state().queue.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_failures_are_reported_once_until_resolved() {
        let src = "diagnostics-test-source";
        assert!(report(src, "diagnostics-test: boom"));
        assert!(!report(src, "diagnostics-test: boom"));
        assert!(report(src, "diagnostics-test: other"));
        resolved(src);
        assert!(report(src, "diagnostics-test: other"));
        let mine: Vec<_> = peek()
            .into_iter()
            .filter(|m| m.starts_with("diagnostics-test"))
            .collect();
        assert_eq!(mine.len(), 3);
    }
}
