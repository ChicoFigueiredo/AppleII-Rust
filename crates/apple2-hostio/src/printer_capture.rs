//! Delivery of the virtual printer's capture events (fase A) from the emulation thread to the
//! spool service, without ever blocking the CPU.
//!
//! Policy (explicit, no silent loss): the queue is bounded by *bytes*, not by events. Consecutive
//! `Bytes` events of the same job that nobody has consumed yet are merged, so a stalled consumer
//! costs memory proportional to the bytes printed, not one allocation per flush. The default
//! limit ([`DEFAULT_LIMIT_BYTES`], 256 MiB) is far above any realistic print job. If it is ever
//! reached, the surplus bytes are dropped, counted ([`CaptureReceiver::dropped_bytes`]) and
//! reported once through [`crate::diagnostics`]; `Open` and `Close` events are never dropped.
//! The producer only takes a short mutex (never held across I/O) and never waits for the consumer.

use apple2_printer::capture::{CaptureEvent, CloseReason, JobOrigin, Transport};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Bytes the queue holds before it starts dropping (and complaining).
pub const DEFAULT_LIMIT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Default)]
struct Queue {
    events: VecDeque<CaptureEvent>,
    queued_bytes: usize,
    dropped_bytes: u64,
    reported: bool,
}

/// Pending close requests kept (jobs never taken, e.g. already closed, are forgotten oldest first).
const MAX_CLOSE_REQUESTS: usize = 64;

struct Shared {
    queue: Mutex<Queue>,
    ready: Condvar,
    next_job: AtomicU64,
    /// Jobs the user asked to close, one entry per job (several slots can have a request pending
    /// at once). Read by each card on its next tick; bounded, oldest forgotten first.
    close_requests: Mutex<Vec<u64>>,
    limit: usize,
}

fn lock(m: &Mutex<Queue>) -> std::sync::MutexGuard<'_, Queue> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Creates a queue with the default limit.
pub fn capture_channel() -> (CaptureSender, CaptureReceiver) {
    capture_channel_with_limit(DEFAULT_LIMIT_BYTES)
}

/// Creates a queue that holds at most `limit` unconsumed payload bytes.
pub fn capture_channel_with_limit(limit: usize) -> (CaptureSender, CaptureReceiver) {
    let shared = Arc::new(Shared {
        queue: Mutex::default(),
        ready: Condvar::new(),
        next_job: AtomicU64::new(1),
        close_requests: Mutex::default(),
        limit,
    });
    (CaptureSender(shared.clone()), CaptureReceiver(shared))
}

/// Producer side; cloneable, shared by every printer card built for one emulator. Job ids come
/// from a counter shared by all clones, so ids are never reused across card rebuilds.
#[derive(Clone)]
pub struct CaptureSender(Arc<Shared>);

impl CaptureSender {
    /// Starts a job and returns its id.
    pub fn open(&self, origin: JobOrigin, transport: Transport) -> u64 {
        let job = self.0.next_job.fetch_add(1, Ordering::Relaxed);
        self.push(CaptureEvent::Open {
            job,
            origin,
            transport,
        });
        job
    }

    /// Appends a block; `offset` = bytes of the job before it.
    pub fn bytes(&self, job: u64, offset: u64, data: Vec<u8>) {
        if !data.is_empty() {
            self.push(CaptureEvent::Bytes { job, offset, data });
        }
    }

    /// True once if the user asked to close `job` (the card then closes it with `User`).
    pub fn take_close_request(&self, job: u64) -> bool {
        let mut r = self
            .0
            .close_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match r.iter().position(|&j| j == job) {
            Some(i) => {
                r.remove(i);
                true
            }
            None => false,
        }
    }

    pub fn close(&self, job: u64, reason: CloseReason) {
        self.push(CaptureEvent::Close { job, reason });
    }

    fn push(&self, ev: CaptureEvent) {
        let mut q = lock(&self.0.queue);
        let mut ev = ev;
        if let CaptureEvent::Bytes { job, offset, data } = &mut ev {
            let room = self.0.limit.saturating_sub(q.queued_bytes);
            if data.len() > room {
                let lost = data.len() - room;
                data.truncate(room);
                q.dropped_bytes += lost as u64;
                if !q.reported {
                    q.reported = true;
                    crate::diagnostics::report(
                        "printer-capture",
                        format!(
                            "Virtual printer: the capture queue is full ({} MiB) and the consumer is not draining it; printed bytes are being dropped",
                            self.0.limit / (1024 * 1024)
                        ),
                    );
                }
                if data.is_empty() {
                    return;
                }
            }
            q.queued_bytes += data.len();
            // Merge with the previous unconsumed block of the same job when contiguous.
            if let Some(CaptureEvent::Bytes {
                job: pj,
                offset: po,
                data: pd,
            }) = q.events.back_mut()
            {
                if *pj == *job && *po + pd.len() as u64 == *offset {
                    pd.extend_from_slice(data);
                    drop(q);
                    self.0.ready.notify_one();
                    return;
                }
            }
        }
        q.events.push_back(ev);
        drop(q);
        self.0.ready.notify_one();
    }
}

/// Consumer side (the spool service); cloneable, all clones share the same queue.
#[derive(Clone)]
pub struct CaptureReceiver(Arc<Shared>);

impl CaptureReceiver {
    fn take(&self, q: &mut Queue) -> Option<CaptureEvent> {
        let ev = q.events.pop_front()?;
        if let CaptureEvent::Bytes { data, .. } = &ev {
            q.queued_bytes -= data.len();
            if q.queued_bytes == 0 {
                q.reported = false;
            }
        }
        Some(ev)
    }

    /// The next event, if any; never blocks.
    pub fn try_recv(&self) -> Option<CaptureEvent> {
        let mut q = lock(&self.0.queue);
        self.take(&mut q)
    }

    /// Waits up to `timeout` for an event.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<CaptureEvent> {
        let mut q = lock(&self.0.queue);
        if q.events.is_empty() {
            q = self
                .0
                .ready
                .wait_timeout(q, timeout)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        self.take(&mut q)
    }

    /// Asks the card to close `job` as soon as it runs again ("Concluir trabalho" in the Central).
    /// The card answers with a `Close { reason: User }` event, so the spool never sees bytes of a
    /// job it already closed. A request for a job that is no longer open is simply never taken.
    pub fn request_close(&self, job: u64) {
        let mut r = self
            .0
            .close_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !r.contains(&job) {
            if r.len() >= MAX_CLOSE_REQUESTS {
                r.remove(0);
            }
            r.push(job);
        }
    }

    /// Everything queued so far, in order.
    pub fn drain(&self) -> Vec<CaptureEvent> {
        let mut q = lock(&self.0.queue);
        q.queued_bytes = 0;
        q.reported = false;
        q.events.drain(..).collect()
    }

    /// Bytes waiting in the queue.
    pub fn pending_bytes(&self) -> usize {
        lock(&self.0.queue).queued_bytes
    }

    /// Bytes dropped because the queue limit was reached (0 in any sane run).
    pub fn dropped_bytes(&self) -> u64 {
        lock(&self.0.queue).dropped_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> JobOrigin {
        JobOrigin {
            machine: "test".into(),
            slot: 1,
        }
    }

    #[test]
    fn a_stalled_consumer_loses_nothing_and_blocks_nobody() {
        let (tx, rx) = capture_channel();
        let job = tx.open(origin(), Transport::EightBit);
        let mut offset = 0u64;
        for i in 0..100_000u32 {
            tx.bytes(job, offset, vec![i as u8; 3]);
            offset += 3;
        }
        tx.close(job, CloseReason::Idle);
        let evs = rx.drain();
        assert_eq!(evs.len(), 3, "merged into one Bytes block: {}", evs.len());
        match &evs[1] {
            CaptureEvent::Bytes {
                offset: 0, data, ..
            } => assert_eq!(data.len(), 300_000),
            e => panic!("{e:?}"),
        }
        assert_eq!(rx.dropped_bytes(), 0);
    }

    #[test]
    fn over_the_limit_bytes_are_counted_and_reported() {
        let (tx, rx) = capture_channel_with_limit(10);
        let job = tx.open(origin(), Transport::EightBit);
        tx.bytes(job, 0, vec![1; 8]);
        tx.bytes(job, 8, vec![2; 8]);
        tx.close(job, CloseReason::Reset);
        assert_eq!(rx.dropped_bytes(), 6);
        let evs = rx.drain();
        assert!(matches!(evs.last(), Some(CaptureEvent::Close { .. })));
        assert!(crate::diagnostics::peek()
            .iter()
            .any(|m| m.contains("capture queue is full")));
    }

    #[test]
    fn job_ids_are_unique_across_clones() {
        let (tx, _rx) = capture_channel();
        let tx2 = tx.clone();
        let a = tx.open(origin(), Transport::EightBit);
        let b = tx2.open(origin(), Transport::EightBit);
        assert_ne!(a, b);
    }

    #[test]
    fn recv_timeout_wakes_on_push() {
        let (tx, rx) = capture_channel();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            tx.open(origin(), Transport::EightBit)
        });
        let ev = rx.recv_timeout(Duration::from_secs(5)).expect("event");
        assert!(matches!(ev, CaptureEvent::Open { .. }));
        t.join().unwrap();
        assert!(rx.try_recv().is_none());
    }
}
