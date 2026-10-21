//! CS8900A (Uthernet I) register and PacketPage model through the Machine bus, with a mock backend.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::network::{MockNetwork, MockNetworkHandle};
use apple2_hostio::uthernet1::{
    PP_CC_RXCTL, PP_MAC_ADDR, PP_PRODUCTID, PP_RXLENGTH, PP_SE_BUSST, PP_SE_RXEVENT,
};
use apple2_hostio::Uthernet1;

const IO: u16 = 0xC080 + 3 * 16; // slot 3
const RXTXDATA: u16 = IO;
const TXCMD: u16 = IO + 4;
const TXLENGTH: u16 = IO + 6;
const PP_PTR: u16 = IO + 0xA;
const PP_DATA: u16 = IO + 0xC;

fn setup() -> (Machine, MockNetworkHandle) {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let (net, h) = MockNetwork::new();
    assert!(m.insert_card(3, Box::new(Uthernet1::new(Box::new(net)))));
    (m, h)
}

fn word_w(m: &mut Machine, port: u16, v: u16) {
    m.board_mut().write(port, v as u8);
    m.board_mut().write(port + 1, (v >> 8) as u8);
}

fn word_r(m: &mut Machine, port: u16) -> u16 {
    let lo = m.board_mut().read(port);
    let hi = m.board_mut().read(port + 1);
    u16::from_le_bytes([lo, hi])
}

fn word_r_hi_first(m: &mut Machine, port: u16) -> u16 {
    let hi = m.board_mut().read(port + 1);
    let lo = m.board_mut().read(port);
    u16::from_le_bytes([lo, hi])
}

fn pp_write(m: &mut Machine, addr: u16, v: u16) {
    word_w(m, PP_PTR, addr);
    word_w(m, PP_DATA, v);
}

fn pp_read(m: &mut Machine, addr: u16) -> u16 {
    word_w(m, PP_PTR, addr);
    word_r(m, PP_DATA)
}

/// High byte of RxEvent (RxOK = bit 0, multicast = 1, correct IA = 2, broadcast = 3). A read of
/// either byte of the register fetches the next frame, so a single read per frame is used: the
/// low byte is 0x04 for every case tested here (AppleWin behaves the same).
fn rx_event_hi(m: &mut Machine) -> u8 {
    word_w(m, PP_PTR, PP_SE_RXEVENT);
    m.board_mut().read(PP_DATA + 1)
}

const MAC: [u8; 6] = [0x00, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E];

fn program_mac(m: &mut Machine) {
    for i in 0..3 {
        pp_write(
            m,
            PP_MAC_ADDR + 2 * i,
            u16::from_le_bytes([MAC[2 * i as usize], MAC[2 * i as usize + 1]]),
        );
    }
}

fn frame_to(dest: [u8; 6], len: usize) -> Vec<u8> {
    let mut f = dest.to_vec();
    f.extend_from_slice(&[0x02, 0, 0, 0, 0, 1]);
    f.resize(len, 0);
    for (i, b) in f.iter_mut().enumerate().skip(12) {
        *b = i as u8;
    }
    f
}

#[test]
fn product_id_and_power_on_registers() {
    let (mut m, _) = setup();
    assert_eq!(pp_read(&mut m, PP_PRODUCTID), 0x630E);
    assert_eq!(pp_read(&mut m, PP_PRODUCTID + 2), 0x0700);
    assert_eq!(pp_read(&mut m, PP_CC_RXCTL), 0x0005);
    assert_eq!(pp_read(&mut m, 0x0102), 0x0003); // RxCFG
}

#[test]
fn packetpage_pointer_corrects_odd_addresses() {
    let (mut m, _) = setup();
    word_w(&mut m, PP_PTR, 0x0021);
    assert_eq!(word_r(&mut m, PP_DATA), pp_read(&mut m, 0x0020));
}

#[test]
fn programmed_mac_is_readable_back() {
    let (mut m, _) = setup();
    program_mac(&mut m);
    assert_eq!(pp_read(&mut m, PP_MAC_ADDR), 0x0A00);
    assert_eq!(pp_read(&mut m, PP_MAC_ADDR + 4), 0x0E0D);
}

#[test]
fn transmit_collects_words_until_the_announced_length_then_sends() {
    let (mut m, h) = setup();
    word_w(&mut m, TXCMD, 0x00C0);
    word_w(&mut m, TXLENGTH, 60);
    assert_eq!(
        pp_read(&mut m, PP_SE_BUSST) & 0x180,
        0x100,
        "ready for the frame"
    );
    let frame: Vec<u8> = (0..60).collect();
    for w in frame.chunks(2) {
        word_w(&mut m, RXTXDATA, u16::from_le_bytes([w[0], w[1]]));
        if w[0] < 58 {
            assert!(h.take_tx().is_empty(), "still collecting");
        }
    }
    assert_eq!(h.take_tx(), vec![frame]);
    assert_eq!(
        pp_read(&mut m, 0x0108),
        0x00C0,
        "TxCMD mirrored to the status register"
    );
}

#[test]
fn oversized_transmit_flags_an_error_and_sends_nothing() {
    let (mut m, h) = setup();
    word_w(&mut m, TXCMD, 0);
    word_w(&mut m, TXLENGTH, 1600);
    assert_eq!(pp_read(&mut m, PP_SE_BUSST) & 0x180, 0x080);
    for _ in 0..800 {
        word_w(&mut m, RXTXDATA, 0);
    }
    assert!(h.take_tx().is_empty());
}

#[test]
fn receive_delivers_status_length_and_data_words() {
    let (mut m, h) = setup();
    program_mac(&mut m);
    pp_write(&mut m, PP_CC_RXCTL, 0x0400 | 0x0100); // accept our IA
    let f = frame_to(MAC, 64);
    h.push_rx(&f);
    // RxEvent register: RxOK | correct IA | (0x0004 base)
    assert_eq!(rx_event_hi(&mut m), 0x05);
    assert_eq!(pp_read(&mut m, PP_RXLENGTH), 64);

    // Now read through the RxTxData port like the driver: status, length, then the frame.
    // The first two words are read high byte first (the odd access advances the buffer).
    assert_eq!(word_r_hi_first(&mut m, RXTXDATA), 0x0504);
    assert_eq!(word_r_hi_first(&mut m, RXTXDATA), 64);
    let mut got = Vec::new();
    for _ in 0..32 {
        got.extend_from_slice(&word_r(&mut m, RXTXDATA).to_le_bytes());
    }
    assert_eq!(got, f);
}

#[test]
fn frames_not_matching_the_filter_are_skipped() {
    let (mut m, h) = setup();
    program_mac(&mut m);
    pp_write(&mut m, PP_CC_RXCTL, 0x0400); // only our IA
    h.push_rx(&frame_to([0x02, 9, 9, 9, 9, 9], 64));
    h.push_rx(&frame_to([0xFF; 6], 64));
    assert_eq!(rx_event_hi(&mut m), 0x00, "nothing acceptable");
    pp_write(&mut m, PP_CC_RXCTL, 0x0800); // broadcast
    h.push_rx(&frame_to([0xFF; 6], 64));
    assert_eq!(rx_event_hi(&mut m), 0x09);
}

#[test]
fn promiscuous_mode_accepts_everything() {
    let (mut m, h) = setup();
    pp_write(&mut m, PP_CC_RXCTL, 0x0080);
    h.push_rx(&frame_to([0x02, 9, 9, 9, 9, 9], 64));
    assert_eq!(rx_event_hi(&mut m) & 1, 1);
}

#[test]
fn odd_sized_frames_are_padded_to_even_length() {
    let (mut m, h) = setup();
    pp_write(&mut m, PP_CC_RXCTL, 0x0080);
    h.push_rx(&frame_to([0xFF; 6], 63));
    rx_event_hi(&mut m);
    assert_eq!(pp_read(&mut m, PP_RXLENGTH), 64);
}

#[test]
fn multicast_matching_the_hash_filter_is_flagged() {
    let (mut m, h) = setup();
    // AppleWin (VICE) tests bit 7 of the first octet for "multicast" (Uthernet1.cpp:845).
    let dest = [0x81, 0x00, 0x5E, 0x00, 0x00, 0x01];
    let idx = ((!apple2_hostio::network::crc32(&dest)) >> 26) & 0x3F;
    for w in 0..4u16 {
        let mut v = 0u16;
        if idx / 16 == w as u32 {
            v = 1 << (idx % 16);
        }
        pp_write(&mut m, 0x0150 + 2 * w, v);
    }
    pp_write(&mut m, PP_CC_RXCTL, 0x0200); // multicast through the hash
    h.push_rx(&frame_to(dest, 64));
    assert_eq!(rx_event_hi(&mut m), 0x03);
}

#[test]
fn power_cycle_restores_defaults_but_ctrl_reset_keeps_state() {
    let (mut m, _) = setup();
    program_mac(&mut m);
    m.reset();
    assert_eq!(pp_read(&mut m, PP_MAC_ADDR), 0x0A00);
    m.power_cycle();
    assert_eq!(pp_read(&mut m, PP_MAC_ADDR), 0);
}
