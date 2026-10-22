//! Virtual printer capture path (fase A): bytes reach the CaptureEvent queue independently of the
//! legacy `Printer.txt` sink.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::printer::{PrinterEncoding, CYCLES_PER_IDLE_SECOND};
use apple2_hostio::{
    capture_channel, capture_channel_with_limit, CaptureReceiver, MemorySink, ParallelPrinterCard,
    PrinterOptions,
};
use apple2_printer::capture::{CaptureEvent, CloseReason, JobOrigin, Transport};

const IO: u16 = 0xC080 + 16; // slot 1

fn origin() -> JobOrigin {
    JobOrigin {
        machine: "Apple //e Enhanced".into(),
        slot: 1,
    }
}

fn machine(opts: PrinterOptions, transport: Transport) -> (Machine, MemorySink, CaptureReceiver) {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let sink = MemorySink::new();
    let (tx, rx) = capture_channel();
    let card = ParallelPrinterCard::new(Box::new(sink.clone()), opts).with_virtual(
        tx,
        origin(),
        transport,
    );
    assert!(m.insert_card(1, Box::new(card)));
    (m, sink, rx)
}

fn run_frames(m: &mut Machine, n: u32) {
    for _ in 0..n {
        m.run_frame();
    }
}

/// (job, reason) of each Close, and every Bytes payload concatenated per job.
fn flatten(evs: &[CaptureEvent]) -> Vec<u8> {
    evs.iter()
        .filter_map(|e| match e {
            CaptureEvent::Bytes { data, .. } => Some(data.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn fmt_opts() -> PrinterOptions {
    PrinterOptions {
        filter_unprintable: true,
        convert_encoding: Some(PrinterEncoding::Pravets82),
        ..Default::default()
    }
}

#[test]
fn t001_basic_print_emits_open_bytes_close_and_legacy_is_unchanged() {
    let (mut m, sink, rx) = machine(PrinterOptions::default(), Transport::EightBit);
    m.reset();
    run_frames(&mut m, 200);
    assert!(rx.drain().is_empty(), "nothing before the first write");
    assert!(m.paste("PR#1\r"));
    run_frames(&mut m, 30);
    assert!(m.paste("PRINT \"HELLO PRINTER\"\r"));
    run_frames(&mut m, 60);
    assert!(sink.text().contains("HELLO PRINTER"));
    m.reset();
    let evs = rx.drain();
    let (job, o, t) = match &evs[0] {
        CaptureEvent::Open {
            job,
            origin,
            transport,
        } => (*job, origin.clone(), *transport),
        e => panic!("{e:?}"),
    };
    assert_eq!((o.slot, o.machine.as_str()), (1, "Apple //e Enhanced"));
    assert_eq!(t, Transport::EightBit);
    // The Apple II sends characters with bit 7 set: the virtual printer sees them whole.
    let data = flatten(&evs);
    assert!(data.iter().any(|&c| c >= 0x80), "{data:?}");
    let seven: Vec<u8> = data.iter().map(|c| c & 0x7F).collect();
    assert!(String::from_utf8_lossy(&seven).contains("HELLO PRINTER"));
    // The legacy file got the same text through the 7-bit rule of AppleWin.
    assert_eq!(seven, sink.contents());
    assert!(matches!(
        evs.last(),
        Some(CaptureEvent::Close { job: j, reason: CloseReason::Reset }) if *j == job
    ));
    // Offsets are contiguous.
    let mut next = 0;
    for e in &evs {
        if let CaptureEvent::Bytes { offset, data, .. } = e {
            assert_eq!(*offset, next);
            next += data.len() as u64;
        }
    }
}

#[test]
fn initial_task_esc_at_and_ff_reach_the_virtual_printer_intact() {
    let (mut m, sink, rx) = machine(fmt_opts(), Transport::EightBit);
    for c in [0x1B, 0x40, b'A', 0x0C, 0x07, b'\r', b'\n'] {
        m.board_mut().write(IO, c);
    }
    m.reset();
    assert_eq!(
        flatten(&rx.drain()),
        [0x1B, 0x40, b'A', 0x0C, 0x07, b'\r', b'\n']
    );
    // The legacy file is filtered/converted as before: ESC, FF and BEL are dropped, `@` is a
    // printable (and the Pravets 82 table turns it into 0xDE).
    assert_eq!(sink.contents(), [0xDE, b'A', b'\r', b'\n']);
}

#[test]
fn t012_all_256_values_arrive_identical_in_eight_bits() {
    let (mut m, _sink, rx) = machine(PrinterOptions::default(), Transport::EightBit);
    let all: Vec<u8> = (0..=255u8).collect();
    for &c in &all {
        m.board_mut().write(IO, c);
    }
    m.reset();
    assert_eq!(flatten(&rx.drain()), all);
}

#[test]
fn t013_seven_bit_transport_clears_bit_7() {
    let (mut m, _sink, rx) = machine(PrinterOptions::default(), Transport::SevenBitAppleWin);
    let all: Vec<u8> = (0..=255u8).collect();
    for &c in &all {
        m.board_mut().write(IO, c);
    }
    m.reset();
    let evs = rx.drain();
    assert!(matches!(
        &evs[0],
        CaptureEvent::Open {
            transport: Transport::SevenBitAppleWin,
            ..
        }
    ));
    let want: Vec<u8> = all.iter().map(|c| c & 0x7F).collect();
    assert_eq!(flatten(&evs), want);
    for (v, w) in [
        (0x00u8, 0x00u8),
        (0x0A, 0x0A),
        (0x0C, 0x0C),
        (0x0D, 0x0D),
        (0x1B, 0x1B),
        (0x7F, 0x7F),
        (0x80, 0x00),
        (0xFF, 0x7F),
    ] {
        assert_eq!(want[v as usize], w);
    }
}

#[test]
fn explicit_edge_values_in_eight_bits() {
    let (mut m, _sink, rx) = machine(fmt_opts(), Transport::EightBit);
    let edge = [0x00u8, 0x0A, 0x0C, 0x0D, 0x1B, 0x7F, 0x80, 0xFF];
    for c in edge {
        m.board_mut().write(IO, c);
    }
    m.reset();
    assert_eq!(flatten(&rx.drain()), edge);
}

#[test]
fn t014_legacy_filtered_and_virtual_raw_in_the_same_job() {
    let (mut m, sink, rx) = machine(
        PrinterOptions {
            filter_unprintable: true,
            ..Default::default()
        },
        Transport::EightBit,
    );
    for c in [b'h', 0x1B, 0x40, b'i', 0x0C] {
        m.board_mut().write(IO, c);
    }
    m.reset();
    assert_eq!(sink.contents(), b"h@i"); // ESC and FF dropped, `@` is printable
    assert_eq!(flatten(&rx.drain()), [b'h', 0x1B, 0x40, b'i', 0x0C]);
}

#[test]
fn ca02_reading_the_card_never_opens_a_virtual_job() {
    let (mut m, sink, rx) = machine(PrinterOptions::default(), Transport::EightBit);
    for lo in 0..16u16 {
        assert_eq!(m.board_mut().read(IO + lo), 0xFF);
    }
    m.board_mut().write(IO + 1, b'x'); // not port 0: not a byte either
    m.run_cycles(1000);
    assert!(rx.drain().is_empty());
    assert!(
        sink.is_open(),
        "the legacy sink still opens on a read, as in AppleWin"
    );
}

#[test]
fn ca06_reset_idle_and_load_close_with_the_right_reason_without_duplicates() {
    let (mut m, _sink, rx) = machine(
        PrinterOptions {
            idle_limit: 1,
            ..Default::default()
        },
        Transport::EightBit,
    );
    // Job 1: closed by idle.
    m.board_mut().write(IO, b'a');
    m.board_mut().write(IO, b'b');
    m.run_cycles(CYCLES_PER_IDLE_SECOND + 1000);
    // Job 2: closed by reset.
    m.board_mut().write(IO, b'c');
    m.reset();
    // Job 3: closed by a state load.
    m.board_mut().write(IO, b'd');
    m.card_mut::<ParallelPrinterCard>(1)
        .unwrap()
        .close_virtual(CloseReason::StateLoaded);
    // Job 4: closed when the machine is dropped.
    m.board_mut().write(IO, b'e');
    drop(m);
    let evs = rx.drain();
    let summary: Vec<String> = evs
        .iter()
        .map(|e| match e {
            CaptureEvent::Open { job, .. } => format!("open{job}"),
            CaptureEvent::Bytes { job, offset, data } => {
                format!("bytes{job}@{offset}:{}", String::from_utf8_lossy(data))
            }
            CaptureEvent::Close { job, reason } => format!("close{job}:{reason:?}"),
        })
        .collect();
    assert_eq!(
        summary,
        [
            "open1",
            "bytes1@0:ab",
            "close1:Idle",
            "open2",
            "bytes2@0:c",
            "close2:Reset",
            "open3",
            "bytes3@0:d",
            "close3:StateLoaded",
            "open4",
            "bytes4@0:e",
            "close4:Shutdown",
        ]
    );
}

#[test]
fn bytes_are_delivered_in_blocks_while_the_job_runs() {
    let (mut m, _sink, rx) = machine(PrinterOptions::default(), Transport::EightBit);
    for _ in 0..10 {
        m.board_mut().write(IO, b'x');
    }
    assert!(
        rx.try_recv()
            .is_some_and(|e| matches!(e, CaptureEvent::Open { .. })),
        "Open at the first byte"
    );
    assert!(rx.try_recv().is_none(), "bytes wait for the flush");
    m.run_cycles(20_000);
    assert!(matches!(
        rx.try_recv(),
        Some(CaptureEvent::Bytes { offset: 0, ref data, .. }) if data.len() == 10
    ));
    // Second block continues the offset.
    m.board_mut().write(IO, b'y');
    m.run_cycles(20_000);
    assert!(matches!(
        rx.try_recv(),
        Some(CaptureEvent::Bytes { offset: 10, ref data, .. }) if data == b"y"
    ));
}

#[test]
fn a_full_queue_with_a_stalled_consumer_loses_no_byte() {
    let (mut m, _s, rx) = machine(PrinterOptions::default(), Transport::EightBit);
    let mut want = Vec::new();
    for i in 0..200_000u32 {
        let c = (i % 251) as u8;
        want.push(c);
        m.board_mut().write(IO, c);
        if i % 100 == 0 {
            m.run_cycles(20_000);
        }
    }
    m.reset();
    assert_eq!(rx.dropped_bytes(), 0);
    assert_eq!(flatten(&rx.drain()), want);
}

#[test]
fn a_tiny_limit_drops_visibly_not_silently() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let (tx, rx) = capture_channel_with_limit(16);
    let card = ParallelPrinterCard::virtual_only(PrinterOptions::default()).with_virtual(
        tx,
        origin(),
        Transport::EightBit,
    );
    assert!(m.insert_card(1, Box::new(card)));
    for i in 0..100u8 {
        m.board_mut().write(IO, i);
    }
    m.reset();
    assert_eq!(rx.dropped_bytes(), 84);
    assert_eq!(flatten(&rx.drain()).len(), 16);
}

#[test]
fn virtual_only_card_has_no_legacy_output() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let (tx, rx) = capture_channel();
    let card = ParallelPrinterCard::virtual_only(PrinterOptions::default()).with_virtual(
        tx,
        origin(),
        Transport::EightBit,
    );
    assert!(m.insert_card(1, Box::new(card)));
    assert_eq!(m.board_mut().read(IO), 0xFF);
    m.board_mut().write(IO, b'z');
    m.reset();
    assert_eq!(flatten(&rx.drain()), b"z");
    assert!(!m.card::<ParallelPrinterCard>(1).unwrap().is_open());
}

/// Transport::Auto (ADR 0008): text through the card's firmware (PR#1 + PRINT) loses bit 7, so
/// the LX-310 sees plain ASCII and real ESC codes; the legacy file is unchanged.
#[test]
fn auto_transport_clears_bit_7_on_firmware_text() {
    let (mut m, sink, rx) = machine(PrinterOptions::default(), Transport::Auto);
    m.reset();
    run_frames(&mut m, 200);
    assert!(m.paste("PR#1\r"));
    run_frames(&mut m, 30);
    assert!(m.paste("PRINT CHR$(27);\"E\";\"HELLO PRINTER\"\r"));
    run_frames(&mut m, 60);
    m.reset();
    let data = flatten(&rx.drain());
    assert!(data.iter().all(|&c| c < 0x80), "{data:?}");
    assert!(data.windows(2).any(|w| w == [0x1B, b'E']), "{data:?}");
    assert!(String::from_utf8_lossy(&data).contains("HELLO PRINTER"));
    assert_eq!(data, sink.contents());
}

/// Transport::Auto: a program that writes the port itself (a graphics driver) sends all 8 bits.
#[test]
fn auto_transport_keeps_8_bits_from_programs_writing_the_port() {
    let (mut m, _sink, rx) = machine(PrinterOptions::default(), Transport::Auto);
    m.reset();
    run_frames(&mut m, 200);
    // $0300: LDA #$C1 / STA $C090 / LDA #$FF / STA $C090 / RTS
    let prog = [
        0xA9, 0xC1, 0x8D, 0x90, 0xC0, 0xA9, 0xFF, 0x8D, 0x90, 0xC0, 0x60,
    ];
    for (i, b) in prog.iter().enumerate() {
        m.poke(0x0300 + i as u16, *b);
    }
    assert!(m.paste("CALL 768\r"));
    run_frames(&mut m, 30);
    m.reset();
    let data = flatten(&rx.drain());
    assert_eq!(data, vec![0xC1, 0xFF]);
}

/// "Concluir trabalho" in the Central: the request reaches the card, which closes the job with
/// `User`; the next byte opens a new job (instead of being "ignored" by the spool).
#[test]
fn a_user_close_request_closes_the_job_on_the_card_and_the_next_byte_opens_a_new_one() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let (tx, rx) = capture_channel();
    let card = ParallelPrinterCard::virtual_only(PrinterOptions::default()).with_virtual(
        tx,
        origin(),
        Transport::EightBit,
    );
    assert!(m.insert_card(1, Box::new(card)));
    m.board_mut().write(IO, b'a');
    let first = m
        .card::<ParallelPrinterCard>(1)
        .unwrap()
        .virtual_job()
        .unwrap();
    // A request for another job (stale) does nothing.
    rx.request_close(first + 99);
    run_frames(&mut m, 2);
    assert_eq!(
        m.card::<ParallelPrinterCard>(1).unwrap().virtual_job(),
        Some(first)
    );
    rx.request_close(first);
    run_frames(&mut m, 2);
    assert_eq!(
        m.card::<ParallelPrinterCard>(1).unwrap().virtual_job(),
        None
    );
    m.board_mut().write(IO, b'b');
    let second = m
        .card::<ParallelPrinterCard>(1)
        .unwrap()
        .virtual_job()
        .unwrap();
    assert_ne!(first, second);
    m.reset();
    let evs = rx.drain();
    let closes: Vec<(u64, CloseReason)> = evs
        .iter()
        .filter_map(|e| match e {
            CaptureEvent::Close { job, reason } => Some((*job, *reason)),
            _ => None,
        })
        .collect();
    assert_eq!(
        closes,
        vec![(first, CloseReason::User), (second, CloseReason::Reset)]
    );
    // The bytes before the close were delivered first, in order.
    assert_eq!(flatten(&evs), b"ab");
}

/// Two "Concluir" in a row for jobs of different slots, before the cards run again: both reach
/// their card (a single stored request lost the first one).
#[test]
fn two_close_requests_before_the_next_tick_both_reach_their_jobs() {
    let (tx, rx) = capture_channel();
    rx.request_close(7);
    rx.request_close(9);
    assert!(tx.take_close_request(9));
    assert!(tx.take_close_request(7));
    assert!(!tx.take_close_request(7), "taken once");
    assert!(!tx.take_close_request(9));
    // A request for a job that never opens does not pile up without bound.
    for j in 100..1000 {
        rx.request_close(j);
    }
    assert!(
        !tx.take_close_request(100),
        "the oldest stale request was forgotten"
    );
    assert!(tx.take_close_request(999));
}
