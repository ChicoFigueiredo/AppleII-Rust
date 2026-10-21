//! AUD-P-021: modes the Uthernet II does not support. SPI, PPPoE (MR = 5) and interrupts do not
//! exist in AppleWin's Uthernet2.cpp either, so they stay NOT_APPLICABLE (parity with the
//! oracle). What matters is that the chip fails in a controlled way: OPEN on an unknown protocol
//! leaves the socket CLOSED (`openSocket` has cases only for IPRAW, MACRAW, TCP and UDP; the
//! default is a `U2_LOG_UNKNOWN` log line) and never panics.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::network::NullNetwork;
use apple2_hostio::uthernet2::{SR_CLOSED, SR_INIT};
use apple2_hostio::Uthernet2;

const IO: u16 = 0xC080 + 4 * 16; // slot 4
const MODE: u16 = IO;
const ADDR_HI: u16 = IO + 1;
const ADDR_LO: u16 = IO + 2;
const DATA: u16 = IO + 3;
const S0: u16 = 0x0400;

struct Chip {
    m: Machine,
}

impl Chip {
    fn new(virtual_dns: bool) -> Chip {
        let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
        assert!(m.insert_card(
            4,
            Box::new(Uthernet2::new(Box::new(NullNetwork), virtual_dns))
        ));
        let mut c = Chip { m };
        c.m.board_mut().write(MODE, 0x02); // auto-increment
        c
    }

    fn set_addr(&mut self, a: u16) {
        self.m.board_mut().write(ADDR_HI, (a >> 8) as u8);
        self.m.board_mut().write(ADDR_LO, a as u8);
    }

    fn rd(&mut self, a: u16) -> u8 {
        self.set_addr(a);
        self.m.board_mut().read(DATA)
    }

    fn wr(&mut self, a: u16, v: u8) {
        self.set_addr(a);
        self.m.board_mut().write(DATA, v);
    }

    fn write_block(&mut self, a: u16, data: &[u8]) {
        self.set_addr(a);
        for &b in data {
            self.m.board_mut().write(DATA, b);
        }
    }

    fn read_block(&mut self, a: u16, n: usize) -> Vec<u8> {
        self.set_addr(a);
        (0..n).map(|_| self.m.board_mut().read(DATA)).collect()
    }

    fn sr(&mut self) -> u8 {
        self.rd(S0 + 3)
    }

    fn command(&mut self, cr: u8) {
        self.wr(S0 + 1, cr);
    }

    fn open(&mut self, mode: u8) {
        self.wr(S0, mode);
        self.command(0x01);
    }
}

#[test]
fn unsupported_socket_modes_leave_the_socket_closed() {
    // 0x05 PPPoE, 0x06/0x07 reserved, 0x08 closed+DNS, 0x0C MACRAW+DNS, 0x0D PPPoE+DNS,
    // 0x0E/0x0F reserved, each alone and with the MF bit.
    for virtual_dns in [false, true] {
        for base in [0x05u8, 0x06, 0x07, 0x08, 0x0C, 0x0D, 0x0E, 0x0F] {
            for mode in [base, base | 0x40] {
                let mut c = Chip::new(virtual_dns);
                c.open(mode);
                assert_eq!(
                    c.sr(),
                    SR_CLOSED,
                    "MR={mode:#04x} (virtual DNS {virtual_dns}) must stay closed"
                );
            }
        }
    }
}

#[test]
fn unsupported_modes_with_the_dns_bit_do_not_resolve_names() {
    // Only TCP, UDP and IP RAW have a *_DNS variant: for the others the destination IP stays 0.
    for mode in [0x08u8, 0x0C, 0x0D] {
        let mut c = Chip::new(true);
        let name = b"127.0.0.1";
        c.wr(S0 + 0x2A, name.len() as u8);
        c.write_block(S0 + 0x2B, name);
        c.open(mode);
        assert_eq!(c.read_block(S0 + 0x0C, 4), [0, 0, 0, 0], "MR={mode:#04x}");
    }
}

#[test]
fn commands_on_a_closed_unsupported_socket_do_not_panic() {
    // CONNECT, LISTEN, SEND, RECV, DISCON, CLOSE and undefined commands on a PPPoE socket.
    let mut c = Chip::new(true);
    c.open(0x05);
    for cr in [0x04u8, 0x02, 0x20, 0x40, 0x08, 0x10, 0x30, 0xFF, 0x00] {
        c.command(cr);
        assert_eq!(c.sr(), SR_CLOSED, "CR={cr:#04x}");
    }
}

#[test]
fn ppoe_bit_of_the_common_mode_register_is_stored_without_effect() {
    // MR bit 3 (PPPoE enable) is accepted but no PPPoE machine exists (AppleWin has none either):
    // sockets keep working.
    let mut c = Chip::new(true);
    c.wr(0x0000, 0x08 | 0x02);
    c.open(0x01);
    assert_eq!(c.sr(), SR_INIT);
}
