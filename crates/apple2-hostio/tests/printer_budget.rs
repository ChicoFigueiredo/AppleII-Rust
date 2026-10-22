//! HU-IMP-080: the CPU never waits for the virtual printer, and the cost of `io_write` with the
//! virtual path stays within 2x the legacy file-only path (spec, desempenho). The measurement is
//! `#[ignore]`: run it with
//! `cargo test -p apple2-hostio --release --test printer_budget -- --ignored --nocapture`.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::{
    capture_channel, capture_channel_with_limit, FileSink, ParallelPrinterCard, PrinterOptions,
};
use apple2_printer::capture::{CaptureEvent, CloseReason, JobOrigin, Transport};
use std::time::{Duration, Instant};

const IO: u16 = 0xC080 + 16; // slot 1

fn origin() -> JobOrigin {
    JobOrigin {
        machine: "Apple //e Enhanced".into(),
        slot: 1,
    }
}

/// A stalled consumer (UI blocked, spool slow): the producer never waits, every drop is counted
/// and the Open/Close events survive.
#[test]
fn stalled_consumer_never_makes_the_cpu_wait_and_drops_are_counted() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let limit = 64 * 1024;
    let (tx, rx) = capture_channel_with_limit(limit);
    let card = ParallelPrinterCard::virtual_only(PrinterOptions::default()).with_virtual(
        tx,
        origin(),
        Transport::EightBit,
    );
    assert!(m.insert_card(1, Box::new(card)));
    let total = 2 * 1024 * 1024usize;
    let t = Instant::now();
    for i in 0..total {
        m.board_mut().write(IO, (i % 251) as u8);
        if i % 4096 == 0 {
            m.run_cycles(20_000); // card ticks: blocks are handed to the queue
        }
    }
    m.reset();
    let el = t.elapsed();
    assert!(
        el < Duration::from_secs(20),
        "the CPU side waited for a consumer: {el:?}"
    );
    assert_eq!(rx.dropped_bytes(), (total - limit) as u64);
    assert!(rx.pending_bytes() <= limit);
    let evs = rx.drain();
    assert!(matches!(evs.first(), Some(CaptureEvent::Open { .. })));
    assert!(matches!(
        evs.last(),
        Some(CaptureEvent::Close {
            reason: CloseReason::Reset,
            ..
        })
    ));
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// ns per byte of each batch.
fn measure(mut m: Machine, drain: impl Fn(), batches: usize, batch: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(batches);
    let mut c = 0u8;
    // Warm-up.
    for _ in 0..batch {
        m.board_mut().write(IO, b'a');
    }
    for _ in 0..batches {
        let t = Instant::now();
        for _ in 0..batch {
            c = c.wrapping_add(1) | 0x20;
            m.board_mut().write(IO, c);
        }
        out.push(t.elapsed().as_nanos() as f64 / batch as f64);
        m.run_cycles(20_000); // outside the timed region: flushes pending blocks
        drain();
    }
    out
}

#[test]
#[ignore = "measurement; see docs/printer/desempenho.md"]
fn io_write_budget_virtual_plus_file_is_within_2x_of_legacy() {
    let dir = std::env::temp_dir().join(format!("apple2-printer-budget-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let opts = PrinterOptions::default();
    let (batches, batch) = (400, 4096);
    let new_machine = || Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));

    let mut m = new_machine();
    m.insert_card(
        1,
        Box::new(ParallelPrinterCard::new(
            Box::new(FileSink::new(dir.join("legacy.txt"))),
            opts,
        )),
    );
    let mut legacy = measure(m, || {}, batches, batch);

    let mut m = new_machine();
    let (tx, rx) = capture_channel();
    m.insert_card(
        1,
        Box::new(
            ParallelPrinterCard::virtual_only(opts).with_virtual(
                tx,
                origin(),
                Transport::EightBit,
            ),
        ),
    );
    let mut virt = measure(m, || drop(rx.drain()), batches, batch);

    let mut m = new_machine();
    let (tx, rx) = capture_channel();
    m.insert_card(
        1,
        Box::new(
            ParallelPrinterCard::new(Box::new(FileSink::new(dir.join("both.txt"))), opts)
                .with_virtual(tx, origin(), Transport::EightBit),
        ),
    );
    let mut both = measure(m, || drop(rx.drain()), batches, batch);

    let (l50, l95) = (percentile(&mut legacy, 0.5), percentile(&mut legacy, 0.95));
    let (v50, v95) = (percentile(&mut virt, 0.5), percentile(&mut virt, 0.95));
    let (b50, b95) = (percentile(&mut both, 0.5), percentile(&mut both, 0.95));
    println!("io_write ns/byte (lote de {batch} bytes, {batches} lotes)");
    println!("legado (arquivo):   P50 {l50:.1}  P95 {l95:.1}");
    println!("virtual (so fila):  P50 {v50:.1}  P95 {v95:.1}");
    println!("virtual + arquivo:  P50 {b50:.1}  P95 {b95:.1}");
    println!("razao virtual+arquivo / legado: P50 {:.2}x  P95 {:.2}x", b50 / l50, b95 / l95);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(b50 <= 2.0 * l50, "P50 {b50:.1} > 2x {l50:.1}");
    assert!(b95 <= 2.0 * l95, "P95 {b95:.1} > 2x {l95:.1}");
}
