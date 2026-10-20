//! Parallel printer card on a real Machine: BASIC `PR#1` output reaches the sink.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::printer::CYCLES_PER_IDLE_SECOND;
use apple2_hostio::{MemorySink, ParallelPrinterCard, PrinterOptions};

const IO: u16 = 0xC080 + 16; // slot 1

fn machine(opts: PrinterOptions) -> (Machine, MemorySink) {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let sink = MemorySink::new();
    assert!(m.insert_card(1, Box::new(ParallelPrinterCard::new(Box::new(sink.clone()), opts))));
    (m, sink)
}

fn run_frames(m: &mut Machine, n: u32) {
    for _ in 0..n {
        m.run_frame();
    }
}

#[test]
fn pr_number_1_prints_basic_output() {
    let (mut m, sink) = machine(PrinterOptions::default());
    m.reset();
    run_frames(&mut m, 200);
    assert!(sink.text().is_empty() && !sink.is_open());
    assert!(m.paste("PR#1\r"));
    run_frames(&mut m, 30);
    assert!(m.paste("PRINT \"HELLO PRINTER\"\r"));
    run_frames(&mut m, 60);
    assert!(sink.text().contains("HELLO PRINTER"), "printed {:?}", sink.text());
    assert!(sink.is_open());
}

#[test]
fn reads_return_ff_and_only_port_zero_prints() {
    let (mut m, sink) = machine(PrinterOptions::default());
    assert_eq!(m.board_mut().read(IO + 3), 0xFF);
    m.board_mut().write(IO + 1, b'x');
    m.board_mut().write(IO, b'A' | 0x80); // bit 7 is stripped
    assert_eq!(sink.text(), "A");
}

#[test]
fn idle_printer_closes_the_job_and_next_print_starts_a_new_one() {
    let (mut m, sink) = machine(PrinterOptions { idle_limit: 1, ..Default::default() });
    m.board_mut().write(IO, b'a');
    assert!(sink.is_open());
    m.run_cycles(CYCLES_PER_IDLE_SECOND + 1000);
    assert!(!sink.is_open());
    m.board_mut().write(IO, b'b');
    assert_eq!((sink.text().as_str(), sink.jobs()), ("b", 2), "overwrite mode starts fresh");
}

#[test]
fn append_mode_keeps_earlier_jobs() {
    let (mut m, sink) = machine(PrinterOptions { idle_limit: 1, append: true, ..Default::default() });
    m.board_mut().write(IO, b'a');
    m.run_cycles(CYCLES_PER_IDLE_SECOND + 1000);
    m.board_mut().write(IO, b'b');
    assert_eq!(sink.text(), "ab");
}

#[test]
fn filter_unprintable_keeps_cr_and_lf_only() {
    let (mut m, sink) = machine(PrinterOptions { filter_unprintable: true, ..Default::default() });
    for c in [7u8, b'h', 13, 10, 27, b'i'] {
        m.board_mut().write(IO, c);
    }
    assert_eq!(sink.contents(), b"h\r\ni");
}

#[test]
fn ctrl_reset_closes_the_job() {
    let (mut m, sink) = machine(PrinterOptions::default());
    m.board_mut().write(IO, b'a');
    m.reset();
    assert!(!sink.is_open());
}

#[test]
fn slot_rom_is_the_parallel_card_firmware() {
    let (mut m, _) = machine(PrinterOptions::default());
    // INTCXROM is off at power-on, so $C100 is the card's ROM.
    assert_eq!(m.board_mut().read(0xC100), apple2_roms::PARALLEL[0]);
    assert_eq!(m.board_mut().read(0xC1FF), apple2_roms::PARALLEL[255]);
}
