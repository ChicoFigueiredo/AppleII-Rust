//! Super Serial Card running on a real Machine: direct ACIA access and the SSC firmware.

use apple2_core::text_screen::text_screen;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::serial::{LoopbackHandle, LoopbackSerial, ModemStatus};
use apple2_hostio::ssc::{ST_DCD, ST_DSR, ST_IRQ, ST_RX_FULL, ST_TX_EMPTY};
use apple2_hostio::SuperSerialCard;

const SLOT: u16 = 0xC080 + 2 * 16; // slot 2
const DATA: u16 = SLOT + 8;
const STATUS: u16 = SLOT + 9;
const COMMAND: u16 = SLOT + 0xA;
const CONTROL: u16 = SLOT + 0xB;

fn machine_with_ssc(echo: bool) -> (Machine, LoopbackHandle) {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let (b, h) = if echo {
        LoopbackSerial::echo()
    } else {
        LoopbackSerial::new()
    };
    assert!(m.insert_card(2, Box::new(SuperSerialCard::new(Box::new(b)))));
    (m, h)
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

#[test]
fn dip_switches_read_as_9600_8n1_comms_mode() {
    let (mut m, _) = machine_with_ssc(false);
    assert_eq!(rd(&mut m, SLOT + 1), 0xE0); // baud index $0E, firmware mode CIC
                                            // SW2: 1 stop(0), 8 bits(0), no parity(0), no LF(1), CTS asserted(0)
    assert_eq!(rd(&mut m, SLOT + 2), 0b0000_0010);
}

#[test]
fn hardware_reset_state_of_status_and_registers() {
    let (mut m, h) = machine_with_ssc(false);
    // Modem lines are active low: DSR/DCD asserted => bits clear.
    assert_eq!(rd(&mut m, STATUS), ST_TX_EMPTY);
    assert_eq!(rd(&mut m, COMMAND), 0);
    assert_eq!(rd(&mut m, CONTROL), 0);
    h.set_modem(ModemStatus::default());
    assert_eq!(rd(&mut m, STATUS), ST_TX_EMPTY | ST_DSR | ST_DCD);
}

#[test]
fn transmit_takes_one_character_time_and_reaches_the_backend() {
    let (mut m, h) = machine_with_ssc(false);
    wr(&mut m, CONTROL, 0x1E); // 9600, 8-N-1, internal clock
    wr(&mut m, COMMAND, 0x0B);
    wr(&mut m, DATA, b'A');
    assert_eq!(h.take_tx(), b"A");
    assert_eq!(rd(&mut m, STATUS) & ST_TX_EMPTY, 0, "shifting out");
    m.run_cycles(1000); // 10 bits at 9600 baud = 1063 cycles
    assert_eq!(rd(&mut m, STATUS) & ST_TX_EMPTY, 0);
    m.run_cycles(100);
    assert_eq!(rd(&mut m, STATUS) & ST_TX_EMPTY, ST_TX_EMPTY);
    let line = h.line().unwrap();
    assert_eq!((line.baud, line.data_bits), (9600, 8));
}

#[test]
fn transmitter_disabled_discards_data() {
    let (mut m, h) = machine_with_ssc(false);
    wr(&mut m, COMMAND, 0x01); // DTR only: transmit control = 00
    wr(&mut m, DATA, b'A');
    assert!(h.take_tx().is_empty());
}

#[test]
fn receive_interrupt_is_raised_and_cleared_by_status_read() {
    let (mut m, h) = machine_with_ssc(false);
    wr(&mut m, COMMAND, 0x09); // DTR, receiver IRQ enabled, transmitter on
    assert!(!m.board().interrupts.irq_line());
    h.push_rx(b"hi");
    m.run_cycles(2 * 1000 + 10);
    assert!(
        m.board().interrupts.irq_line(),
        "IRQ asserted by the poll event"
    );
    let st = rd(&mut m, STATUS);
    assert_eq!(st & (ST_IRQ | ST_RX_FULL), ST_IRQ | ST_RX_FULL);
    assert!(
        !m.board().interrupts.irq_line(),
        "status read clears the line"
    );
    assert_eq!(rd(&mut m, STATUS) & ST_IRQ, 0, "IRQ bit reported once");
    assert_eq!(rd(&mut m, DATA), b'h');
    assert!(
        m.board().interrupts.irq_line(),
        "more data waiting: IRQ again"
    );
    assert_eq!(rd(&mut m, DATA), b'i');
    assert_eq!(rd(&mut m, STATUS) & ST_RX_FULL, 0);
}

#[test]
fn no_interrupt_when_receiver_irq_disabled() {
    let (mut m, h) = machine_with_ssc(false);
    wr(&mut m, COMMAND, 0x0B); // bit 1 set: receiver IRQ disabled
    h.push_rx(b"x");
    m.run_cycles(3000);
    assert!(!m.board().interrupts.irq_line());
    assert_eq!(rd(&mut m, STATUS) & (ST_IRQ | ST_RX_FULL), ST_RX_FULL);
}

#[test]
fn dtr_low_blocks_receive() {
    let (mut m, h) = machine_with_ssc(false);
    wr(&mut m, COMMAND, 0x00);
    h.push_rx(b"x");
    m.run_cycles(3000);
    assert_eq!(rd(&mut m, DATA), 0);
    wr(&mut m, COMMAND, 0x0B);
    assert_eq!(rd(&mut m, DATA), b'x');
}

#[test]
fn programmed_reset_keeps_parity_bits_only() {
    let (mut m, _) = machine_with_ssc(false);
    wr(&mut m, COMMAND, 0b1110_1011);
    wr(&mut m, STATUS, 0);
    assert_eq!(rd(&mut m, COMMAND), 0b1110_0000);
}

fn screen_has(m: &Machine, text: &str) -> bool {
    text_screen(m).iter().any(|r| r.contains(text))
}

fn run_frames(m: &mut Machine, n: u32) {
    for _ in 0..n {
        m.run_frame();
    }
}

fn booted_with_ssc(echo: bool) -> (Machine, LoopbackHandle) {
    let (mut m, h) = machine_with_ssc(echo);
    m.reset();
    run_frames(&mut m, 200);
    (m, h)
}

#[test]
fn pr_number_2_sends_basic_output_through_the_firmware() {
    let (mut m, h) = booted_with_ssc(false);
    assert!(m.paste("PR#2\r"));
    run_frames(&mut m, 30);
    assert!(m.paste("PRINT \"HELLO\"\r"));
    run_frames(&mut m, 120);
    // Apple text goes out with bit 7 set (the firmware does not strip it at 8 data bits).
    let tx: String = h.take_tx().iter().map(|&b| (b & 0x7F) as char).collect();
    assert!(tx.contains("HELLO"), "sent {tx:?}");
}

#[test]
fn in_number_2_reads_basic_input_through_the_firmware() {
    let (mut m, h) = booted_with_ssc(false);
    assert!(m.paste("IN#2\r"));
    run_frames(&mut m, 30);
    h.push_rx(b"PRINT 6*7\r");
    run_frames(&mut m, 200);
    assert!(screen_has(&m, "42"), "{}", text_screen(&m).join("\n"));
}
