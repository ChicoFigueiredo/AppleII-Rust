//! Synchronous event scheduler: port of AppleWin `SynchronousEventManager`
//! (SynchronousEventManager.cpp/.h).
//!
//! Events form a delta list: the Nth event expires `rem[1] + ... + rem[N]` cycles from now,
//! so only the head needs updating. Handlers are not stored here; `update` takes a dispatch
//! closure keyed by [`EventId`], so later slices can route ids to cards/devices without this
//! module knowing about them.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncEvent {
    pub id: EventId,
    /// While queued: cycles relative to the previous event in the list.
    pub cycles_remaining: i32,
    /// An expiry exactly on the last cycle of an opcode defers the IRQ by one opcode.
    pub can_assert_irq: bool,
}

impl SyncEvent {
    pub fn new(id: EventId, cycles: i32) -> Self {
        SyncEvent {
            id,
            cycles_remaining: cycles,
            can_assert_irq: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Scheduler {
    events: Vec<SyncEvent>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// The delta list, head first (save-states).
    pub fn events(&self) -> &[SyncEvent] {
        &self.events
    }

    pub fn set_events(&mut self, events: Vec<SyncEvent>) {
        self.events = events;
    }

    pub fn reset(&mut self) {
        self.events.clear();
    }

    pub fn is_active(&self, id: EventId) -> bool {
        self.events.iter().any(|e| e.id == id)
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// `Insert` (cpp:40-93): `cycles_remaining` is relative to now. Events due at the same
    /// cycle fire in insertion order.
    pub fn insert(&mut self, mut ev: SyncEvent) {
        let mut extra = ev.cycles_remaining;
        let mut pos = 0;
        while pos < self.events.len() {
            let cur = self.events[pos].cycles_remaining;
            if extra < cur {
                self.events[pos].cycles_remaining -= extra;
                break;
            }
            extra -= cur;
            pos += 1;
        }
        // Appended at the end of the list keeps its delta; empty list keeps it unchanged.
        ev.cycles_remaining = extra;
        self.events.insert(pos, ev);
    }

    /// Sets `can_assert_irq` of a queued event; false if it is not queued.
    pub fn set_can_assert_irq(&mut self, id: EventId, flag: bool) -> bool {
        match self.events.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                e.can_assert_irq = flag;
                true
            }
            None => false,
        }
    }

    /// `Remove` (cpp:95-135): the following event inherits the removed delta.
    pub fn remove(&mut self, id: EventId) -> bool {
        match self.events.iter().position(|e| e.id == id) {
            Some(i) => {
                let ev = self.events.remove(i);
                if let Some(next) = self.events.get_mut(i) {
                    next.cycles_remaining += ev.cycles_remaining;
                }
                true
            }
            None => false,
        }
    }

    /// True if advancing by `cycles` expires the head event.
    #[inline]
    pub fn head_due(&self, cycles: i32) -> bool {
        self.events
            .first()
            .is_some_and(|e| e.cycles_remaining - cycles <= 0)
    }

    /// [`Scheduler::update`] for the case where [`Scheduler::head_due`] is false.
    #[inline]
    pub fn count_down(&mut self, cycles: i32) {
        if let Some(e) = self.events.first_mut() {
            e.cycles_remaining -= cycles;
        }
    }

    /// `Update` (cpp:137-169). Advances time by `cycles`, calling `handler(id, cycles)` for each
    /// expired event; the handler returns the cycles until it re-arms (0 = do not re-arm).
    /// Returns true if some `can_assert_irq` event expired exactly on the last cycle.
    pub fn update(&mut self, cycles: i32, mut handler: impl FnMut(EventId, i32) -> i32) -> bool {
        let mut irq_on_last_cycle = false;
        let mut rearm = Vec::new();
        let mut c = cycles;
        while let Some(head) = self.events.first_mut() {
            head.cycles_remaining -= c;
            if head.cycles_remaining > 0 {
                break;
            }
            if head.cycles_remaining == 0 && head.can_assert_irq {
                irq_on_last_cycle = true;
            }
            let underflow = -head.cycles_remaining;
            let mut ev = self.events.remove(0);
            ev.cycles_remaining = handler(ev.id, c);
            rearm.push(ev);
            // Next event may also be due, even with zero underflow.
            c = underflow;
        }
        // The C++ recursion re-inserts the innermost event first.
        for ev in rearm.into_iter().rev() {
            if ev.cycles_remaining != 0 {
                self.insert(ev);
            }
        }
        irq_on_last_cycle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Scheduler, cycles: i32, log: &mut Vec<(u32, i32)>) -> bool {
        s.update(cycles, |id, c| {
            log.push((id.0, c));
            0
        })
    }

    #[test]
    fn fires_in_time_order_regardless_of_insertion_order() {
        let mut s = Scheduler::new();
        s.insert(SyncEvent::new(EventId(2), 30));
        s.insert(SyncEvent::new(EventId(1), 10));
        s.insert(SyncEvent::new(EventId(3), 20));
        let mut log = vec![];
        run(&mut s, 9, &mut log);
        assert!(log.is_empty());
        run(&mut s, 1, &mut log);
        assert_eq!(log, [(1, 1)]);
        run(&mut s, 25, &mut log);
        assert_eq!(log.iter().map(|e| e.0).collect::<Vec<_>>(), [1, 3, 2]);
        assert!(s.is_empty());
    }

    #[test]
    fn simultaneous_events_all_fire_in_insertion_order() {
        let mut s = Scheduler::new();
        s.insert(SyncEvent::new(EventId(1), 5));
        s.insert(SyncEvent::new(EventId(2), 5));
        s.insert(SyncEvent::new(EventId(3), 5));
        let mut log = vec![];
        run(&mut s, 5, &mut log);
        assert_eq!(log.iter().map(|e| e.0).collect::<Vec<_>>(), [1, 2, 3]);
    }

    #[test]
    fn handler_return_rearms_relative_to_now() {
        let mut s = Scheduler::new();
        s.insert(SyncEvent::new(EventId(1), 4));
        let mut fired = 0;
        for _ in 0..12 {
            s.update(1, |_, _| {
                fired += 1;
                4
            });
        }
        assert_eq!(fired, 3);
        assert!(s.is_active(EventId(1)));
    }

    #[test]
    fn remove_gives_delta_to_next_event() {
        let mut s = Scheduler::new();
        s.insert(SyncEvent::new(EventId(1), 10));
        s.insert(SyncEvent::new(EventId(2), 25));
        assert!(s.remove(EventId(1)));
        assert!(!s.remove(EventId(1)));
        let mut log = vec![];
        run(&mut s, 24, &mut log);
        assert!(log.is_empty());
        run(&mut s, 1, &mut log);
        assert_eq!(log, [(2, 1)]);
    }

    #[test]
    fn overshoot_passes_underflow_to_next_event() {
        let mut s = Scheduler::new();
        s.insert(SyncEvent::new(EventId(1), 3));
        s.insert(SyncEvent::new(EventId(2), 5));
        let mut log = vec![];
        run(&mut s, 7, &mut log);
        assert_eq!(log, [(1, 7), (2, 4)]);
    }

    #[test]
    fn irq_on_last_cycle_only_when_expiring_exactly() {
        let mut s = Scheduler::new();
        s.insert(SyncEvent::new(EventId(1), 4));
        assert!(!s.update(5, |_, _| 0));
        s.insert(SyncEvent::new(EventId(1), 4));
        assert!(s.update(4, |_, _| 0));
        let mut ev = SyncEvent::new(EventId(1), 4);
        ev.can_assert_irq = false;
        s.insert(ev);
        assert!(!s.update(4, |_, _| 0));
    }
}
