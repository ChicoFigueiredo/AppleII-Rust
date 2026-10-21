//! W5100 (Uthernet II): register model, host TCP/UDP sockets against local std sockets, virtual
//! DNS, and RAW sockets over a mock Ethernet backend. Driven through the Machine bus like a
//! 6502 driver would.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::network::{create_eth2_frame, MockNetwork, MockNetworkHandle, NullNetwork};
use apple2_hostio::uthernet2::{
    SR_CLOSED, SR_ESTABLISHED, SR_INIT, SR_IPRAW, SR_LISTEN, SR_MACRAW, SR_UDP,
};
use apple2_hostio::Uthernet2;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::time::{Duration, Instant};

const IO: u16 = 0xC080 + 4 * 16; // slot 4
const MODE: u16 = IO;
const ADDR_HI: u16 = IO + 1;
const ADDR_LO: u16 = IO + 2;
const DATA: u16 = IO + 3;

const S0: u16 = 0x0400;
const TX0: u16 = 0x4000;
const RX0: u16 = 0x6000;

struct Chip {
    m: Machine,
}

impl Chip {
    fn new(card: Uthernet2) -> Chip {
        let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
        assert!(m.insert_card(4, Box::new(card)));
        let mut c = Chip { m };
        c.m.board_mut().write(MODE, 0x02); // auto-increment
        c
    }

    fn with_null() -> Chip {
        Chip::new(Uthernet2::new(Box::new(NullNetwork), true))
    }

    fn with_mock() -> (Chip, MockNetworkHandle) {
        let (n, h) = MockNetwork::new();
        (Chip::new(Uthernet2::new(Box::new(n), true)), h)
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

    fn rd16(&mut self, a: u16) -> u16 {
        u16::from_be_bytes([self.rd(a), self.rd(a + 1)])
    }

    fn wr16(&mut self, a: u16, v: u16) {
        self.wr(a, (v >> 8) as u8);
        self.wr(a + 1, v as u8);
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

    fn sock(s: u16) -> u16 {
        S0 + s * 0x100
    }

    fn sr(&mut self, s: u16) -> u8 {
        self.rd(Self::sock(s) + 3)
    }

    fn command(&mut self, s: u16, cr: u8) {
        self.wr(Self::sock(s) + 1, cr);
    }

    fn open(&mut self, s: u16, mode: u8) {
        self.wr(Self::sock(s), mode);
        self.command(s, 0x01);
    }

    fn set_dest(&mut self, s: u16, ip: [u8; 4], port: u16) {
        self.write_block(Self::sock(s) + 0x0C, &ip);
        self.wr16(Self::sock(s) + 0x10, port);
    }

    /// Writes `data` at the socket's TX write pointer, advances it and issues SEND.
    fn send(&mut self, s: u16, data: &[u8]) {
        let wr = self.rd16(Self::sock(s) + 0x24);
        let base = TX0 + s * 0x800;
        for (k, &b) in data.iter().enumerate() {
            self.wr(base + ((wr + k as u16) & 0x7FF), b);
        }
        self.wr16(Self::sock(s) + 0x24, wr + data.len() as u16);
        self.command(s, 0x20);
    }

    /// Received bytes available (reading RSR fetches from the host), polling up to two seconds.
    fn wait_rx(&mut self, s: u16, at_least: u16) -> u16 {
        let t = Instant::now();
        loop {
            let n = self.rd16(Self::sock(s) + 0x26);
            if n >= at_least || t.elapsed() > Duration::from_secs(2) {
                return n;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Reads and consumes `n` bytes of the socket's RX ring.
    fn recv(&mut self, s: u16, n: usize) -> Vec<u8> {
        let rd = self.rd16(Self::sock(s) + 0x28);
        let base = RX0 + s * 0x800;
        let out = (0..n)
            .map(|k| self.rd(base + ((rd + k as u16) & 0x7FF)))
            .collect();
        self.wr16(Self::sock(s) + 0x28, rd + n as u16);
        self.command(s, 0x40);
        out
    }

    fn wait_status(&mut self, s: u16, want: u8) -> u8 {
        let t = Instant::now();
        loop {
            let sr = self.sr(s);
            // Windows retries a refused localhost connect for ~2 s before failing.
            if sr == want || t.elapsed() > Duration::from_secs(10) {
                return sr;
            }
            self.m.run_cycles(2000);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

#[test]
fn power_on_register_values_and_buffer_sizes() {
    let mut c = Chip::with_null();
    assert_eq!(c.rd(0x0017), 0x07);
    assert_eq!(c.rd(0x0018), 0xD0);
    assert_eq!(c.rd(0x0019), 0x08);
    assert_eq!(c.rd(0x001A), 0x55);
    assert_eq!(c.rd(0x0028), 0, "PTIMER is 0 when virtual DNS is supported");
    assert_eq!(c.rd(S0 + 0x16), 0x80, "TTL");
    assert_eq!(c.rd(S0 + 0x06), 0xFF, "DHAR");
    // 4 x 2K: free TX space of an idle socket is 2K.
    assert_eq!(c.rd16(S0 + 0x20), 0x0800);
    assert_eq!(c.rd16(Chip::sock(3) + 0x20), 0x0800);
}

#[test]
fn ptimer_is_0x28_without_virtual_dns() {
    let mut c = Chip::new(Uthernet2::new(Box::new(NullNetwork), false));
    assert_eq!(c.rd(0x0028), 0x28);
}

#[test]
fn address_registers_and_auto_increment_with_wraparound() {
    let mut c = Chip::with_null();
    assert_eq!(c.m.board_mut().read(MODE), 0x02);
    c.write_block(0x0009, &[1, 2, 3, 4, 5, 6]); // SHAR
    assert_eq!(c.read_block(0x0009, 6), [1, 2, 3, 4, 5, 6]);
    // TX memory wraps from $5FFF back to $4000, RX from $7FFF to $6000 (Uthernet II p.12).
    c.wr(0x5FFF, 0xAA);
    c.wr(0x6000, 0xBB);
    c.set_addr(0x5FFF);
    assert_eq!(c.m.board_mut().read(DATA), 0xAA);
    assert_eq!(c.m.board_mut().read(DATA), 0x00, "wrapped to $4000");
    c.set_addr(0x7FFF);
    c.m.board_mut().read(DATA);
    assert_eq!(c.m.board_mut().read(DATA), 0xBB, "wrapped to $6000");
}

#[test]
fn memory_size_registers_resize_the_buffers() {
    let mut c = Chip::with_null();
    c.wr(0x001B, 0x03); // socket 0 TX = 8K, the rest 1K
    assert_eq!(c.rd16(S0 + 0x20), 0x2000);
    assert_eq!(
        c.rd16(Chip::sock(1) + 0x20),
        0x0000,
        "no memory left for the others"
    );
}

#[test]
fn mode_register_reset_bit_clears_the_chip_but_keeps_the_data_address() {
    let mut c = Chip::with_null();
    c.wr(0x0001, 0x7F);
    c.set_addr(0x1234);
    c.m.board_mut().write(MODE, 0x80);
    assert_eq!(c.m.board_mut().read(MODE), 0);
    assert_eq!(
        (c.m.board_mut().read(ADDR_HI), c.m.board_mut().read(ADDR_LO)),
        (0x12, 0x34)
    );
    c.m.board_mut().write(MODE, 0x02);
    assert_eq!(c.rd(0x0001), 0);
}

#[test]
fn tcp_echo_round_trip_against_a_local_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 64];
        let n = s.read(&mut buf).unwrap();
        s.write_all(&buf[..n]).unwrap();
        std::thread::sleep(Duration::from_millis(100));
    });

    let mut c = Chip::with_null();
    c.open(0, 0x01);
    assert_eq!(c.sr(0), SR_INIT);
    c.set_dest(0, [127, 0, 0, 1], port);
    c.command(0, 0x04); // CONNECT
    assert_eq!(c.wait_status(0, SR_ESTABLISHED), SR_ESTABLISHED);

    c.send(0, b"hello w5100");
    assert_eq!(c.rd16(S0 + 0x20), 0x0800, "SEND consumed the TX data");
    assert_eq!(c.wait_rx(0, 11), 11);
    assert_eq!(c.recv(0, 11), b"hello w5100");
    assert_eq!(c.rd16(S0 + 0x26), 0, "receive size drops after RECV");
    server.join().unwrap();
}

#[test]
fn tcp_remote_close_closes_the_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || drop(listener.accept().unwrap()));
    let mut c = Chip::with_null();
    c.open(0, 0x01);
    c.set_dest(0, [127, 0, 0, 1], port);
    c.command(0, 0x04);
    assert_eq!(c.wait_status(0, SR_ESTABLISHED), SR_ESTABLISHED);
    server.join().unwrap();
    let t = Instant::now();
    while c.sr(0) != SR_CLOSED && t.elapsed() < Duration::from_secs(2) {
        c.rd16(S0 + 0x26); // reading RSR notices the EOF
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(c.sr(0), SR_CLOSED);
}

#[test]
fn tcp_connect_failure_closes_the_socket() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port(); // now closed
    let mut c = Chip::with_null();
    c.open(0, 0x01);
    c.set_dest(0, [127, 0, 0, 1], port);
    c.command(0, 0x04);
    assert_eq!(c.wait_status(0, SR_CLOSED), SR_CLOSED);
}

#[test]
fn tcp_listen_accepts_a_host_client() {
    let mut c = Chip::with_null();
    // Pick a free port first, then ask the chip to listen on it.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    c.wr16(S0 + 4, port);
    c.open(0, 0x01);
    c.command(0, 0x02); // LISTEN
    assert_eq!(c.sr(0), SR_LISTEN);
    let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
    assert_eq!(c.wait_status(0, SR_ESTABLISHED), SR_ESTABLISHED);
    client.write_all(b"ping").unwrap();
    assert_eq!(c.wait_rx(0, 4), 4);
    assert_eq!(c.recv(0, 4), b"ping");
}

#[test]
fn udp_send_and_receive_with_header() {
    let host = UdpSocket::bind("127.0.0.1:0").unwrap();
    host.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let host_port = host.local_addr().unwrap().port();

    let mut c = Chip::with_null();
    c.open(0, 0x02);
    assert_eq!(c.sr(0), SR_UDP);
    c.set_dest(0, [127, 0, 0, 1], host_port);
    c.send(0, b"datagram");
    let mut buf = [0u8; 64];
    let (n, from) = host.recv_from(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"datagram");

    host.send_to(b"reply", from).unwrap();
    // 8-byte header: source IP, source port, length; then the data.
    assert_eq!(c.wait_rx(0, 13), 13);
    let got = c.recv(0, 13);
    assert_eq!(&got[0..4], &[127, 0, 0, 1]);
    assert_eq!(u16::from_be_bytes([got[4], got[5]]), host_port);
    assert_eq!(u16::from_be_bytes([got[6], got[7]]), 5);
    assert_eq!(&got[8..], b"reply");
}

#[test]
fn virtual_dns_resolves_names_into_the_destination_ip() {
    let mut c = Chip::with_null();
    let name = b"127.0.0.1";
    c.wr(S0 + 0x2A, name.len() as u8);
    c.write_block(S0 + 0x2B, name);
    c.open(0, 0x09); // TCP + virtual DNS
    assert_eq!(c.read_block(S0 + 0x0C, 4), [127, 0, 0, 1]);
    assert_eq!(c.sr(0), SR_INIT);

    let bad = b"no-such-host.invalid";
    c.wr(Chip::sock(1) + 0x2A, bad.len() as u8);
    c.write_block(Chip::sock(1) + 0x2B, bad);
    c.open(1, 0x0A); // UDP + virtual DNS
    assert_eq!(
        c.read_block(Chip::sock(1) + 0x0C, 4),
        [0, 0, 0, 0],
        "0.0.0.0 means failure"
    );
}

#[test]
fn virtual_dns_mode_does_nothing_when_disabled() {
    let mut c = Chip::new(Uthernet2::new(Box::new(NullNetwork), false));
    c.open(0, 0x09);
    assert_eq!(c.sr(0), SR_CLOSED);
}

#[test]
fn mac_raw_socket_receives_frames_with_a_length_prefix_and_sends_raw() {
    let (mut c, h) = Chip::with_mock();
    c.write_block(0x0009, &[2, 0, 0, 0, 0, 1]);
    c.open(0, 0x04);
    assert_eq!(c.sr(0), SR_MACRAW);

    let mut frame = vec![0xFF; 6];
    frame.extend_from_slice(&[2, 0, 0, 0, 0, 9, 0x08, 0x06]);
    frame.resize(60, 7);
    h.push_rx(&frame);
    assert_eq!(c.wait_rx(0, 62), 62);
    let got = c.recv(0, 62);
    assert_eq!(
        u16::from_be_bytes([got[0], got[1]]),
        62,
        "length includes the 2-byte header"
    );
    assert_eq!(&got[2..], &frame[..]);

    c.send(0, &frame);
    assert_eq!(h.take_tx(), vec![frame]);
}

#[test]
fn mac_raw_filter_bit_drops_frames_for_other_hosts() {
    let (mut c, h) = Chip::with_mock();
    c.write_block(0x0009, &[2, 0, 0, 0, 0, 1]);
    c.open(0, 0x04 | 0x40); // MAC filter
    let mut other = vec![2, 0, 0, 0, 0, 5, 2, 0, 0, 0, 0, 9, 0x08, 0x00];
    other.resize(60, 0);
    h.push_rx(&other);
    assert_eq!(c.rd16(S0 + 0x26), 0);
    c.command(0, 0x10);
    c.open(0, 0x04); // no filter: promiscuous
    h.push_rx(&other);
    assert_eq!(c.wait_rx(0, 62), 62);
}

#[test]
fn ip_raw_socket_exchanges_payloads_by_protocol() {
    let (mut c, h) = Chip::with_mock();
    let my_mac = [2, 0, 0, 0, 0, 1];
    c.write_block(0x0009, &my_mac);
    c.write_block(0x000F, &[10, 0, 0, 2]); // SIPR
    c.write_block(0x0005, &[255, 255, 255, 0]); // SUBR
    c.wr(Chip::sock(1) + 0x14, 1); // protocol: ICMP
    c.open(1, 0x03);
    assert_eq!(c.sr(1), SR_IPRAW);

    // Incoming ICMP to our MAC.
    let icmp = [8, 0, 0xF7, 0xFF, 0, 0, 0, 0];
    let f = create_eth2_frame(
        &icmp,
        [2, 0, 0, 0, 0, 9],
        my_mac,
        64,
        0,
        1,
        [10, 0, 0, 1],
        [10, 0, 0, 2],
    );
    h.push_rx(&f);
    assert_eq!(c.wait_rx(1, 14), 14);
    let got = c.recv(1, 14);
    assert_eq!(&got[0..4], &[10, 0, 0, 1]);
    assert_eq!(u16::from_be_bytes([got[4], got[5]]), 8);
    assert_eq!(&got[6..], &icmp);

    // Outgoing: the MAC comes from the backend's ARP, once, then the cache.
    h.add_arp([10, 0, 0, 1], [2, 0, 0, 0, 0, 9]);
    c.set_dest(1, [10, 0, 0, 1], 0);
    c.send(1, &icmp);
    let tx = h.take_tx();
    assert_eq!(tx.len(), 1);
    assert_eq!(
        tx[0],
        create_eth2_frame(
            &icmp,
            my_mac,
            [2, 0, 0, 0, 0, 9],
            0x80,
            0,
            1,
            [10, 0, 0, 2],
            [10, 0, 0, 1]
        )
    );
}

#[test]
fn unknown_or_off_net_destinations_go_to_broadcast_or_gateway() {
    let (mut c, h) = Chip::with_mock();
    c.write_block(0x0009, &[2, 0, 0, 0, 0, 1]);
    c.write_block(0x000F, &[10, 0, 0, 2]);
    c.write_block(0x0005, &[255, 255, 255, 0]);
    c.write_block(0x0001, &[10, 0, 0, 254]); // GAR
    h.add_arp([10, 0, 0, 254], [2, 0, 0, 0, 0, 0xFE]);
    c.wr(Chip::sock(0) + 0x14, 17);
    c.open(0, 0x03);
    c.set_dest(0, [8, 8, 8, 8], 0); // off-net: via the gateway's MAC
    c.send(0, b"x");
    assert_eq!(&h.take_tx()[0][..6], &[2, 0, 0, 0, 0, 0xFE]);
    c.set_dest(0, [255, 255, 255, 255], 0);
    c.send(0, b"x");
    assert_eq!(&h.take_tx()[0][..6], &[0xFF; 6]);
}

#[test]
fn closed_socket_ignores_send_and_receive() {
    let mut c = Chip::with_null();
    c.send(0, b"nothing");
    assert_eq!(c.rd16(S0 + 0x26), 0);
    assert_eq!(c.sr(0), SR_CLOSED);
}

#[test]
fn save_state_keeps_registers_and_raw_sockets() {
    let (mut c, _) = Chip::with_mock();
    c.write_block(0x0009, &[2, 0, 0, 0, 0, 1]);
    c.open(0, 0x04);
    let state = c.m.board().slots.save();
    c.m.power_cycle();
    assert_eq!(c.rd(0x0009), 0);
    assert!(c.m.board_mut().slots.load(&state));
    assert_eq!(c.read_block(0x0009, 6), [2, 0, 0, 0, 0, 1]);
    assert_eq!(c.sr(0), SR_MACRAW);
}

// ---------------------------------------------------------------------------------------------
// AUD-P-024: local, deterministic network tests (no Internet, no fixed ports: port 0 everywhere)
// and AUD-P-021: modes AppleWin does not have.

/// Sn_PORT of socket `s`.
fn port_of(c: &mut Chip, s: u16) -> u16 {
    c.rd16(Chip::sock(s) + 0x04)
}

/// Waits (bounded) until `pred` holds, running the card in between.
fn wait_until(c: &mut Chip, what: &str, mut pred: impl FnMut(&mut Chip) -> bool) {
    let t = Instant::now();
    while !pred(c) {
        assert!(
            t.elapsed() < Duration::from_secs(5),
            "timeout waiting for {what}"
        );
        c.m.run_cycles(2000);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn four_tcp_sockets_talk_to_the_host_at_the_same_time() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    // One echo thread per accepted connection.
    let server = std::thread::spawn(move || {
        let mut workers = Vec::new();
        for _ in 0..4 {
            let (mut s, _) = listener.accept().unwrap();
            workers.push(std::thread::spawn(move || {
                let mut buf = [0u8; 64];
                let n = s.read(&mut buf).unwrap();
                s.write_all(&buf[..n]).unwrap();
                // Hold the connection until the card closes it.
                let _ = s.read(&mut buf);
            }));
        }
        workers
    });

    let mut c = Chip::with_null();
    for s in 0..4 {
        c.open(s, 0x01);
        c.set_dest(s, [127, 0, 0, 1], port);
        c.command(s, 0x04);
    }
    for s in 0..4 {
        assert_eq!(
            c.wait_status(s, SR_ESTABLISHED),
            SR_ESTABLISHED,
            "socket {s}"
        );
    }
    // Send in an order unrelated to the connection order.
    for s in [2u16, 0, 3, 1] {
        c.send(s, format!("socket-{s}-payload").as_bytes());
    }
    for s in 0..4u16 {
        let want = format!("socket-{s}-payload");
        assert_eq!(
            c.wait_rx(s, want.len() as u16),
            want.len() as u16,
            "socket {s}"
        );
        assert_eq!(c.recv(s, want.len()), want.as_bytes(), "socket {s}");
    }
    drop(c); // closes the host sockets, releasing the echo threads
    for w in server.join().unwrap() {
        w.join().unwrap();
    }
}

#[test]
fn tcp_listen_with_port_zero_reports_the_chosen_port_and_serves_two_sockets() {
    let mut c = Chip::with_null();
    for s in 0..2 {
        c.open(s, 0x01);
        c.command(s, 0x02); // LISTEN on Sn_PORT = 0: the host picks
        assert_eq!(c.sr(s), SR_LISTEN);
    }
    let p0 = port_of(&mut c, 0);
    let p1 = port_of(&mut c, 1);
    assert!(p0 != 0 && p1 != 0 && p0 != p1, "{p0} {p1}");

    let mut a = TcpStream::connect(("127.0.0.1", p0)).unwrap();
    let mut b = TcpStream::connect(("127.0.0.1", p1)).unwrap();
    assert_eq!(c.wait_status(0, SR_ESTABLISHED), SR_ESTABLISHED);
    assert_eq!(c.wait_status(1, SR_ESTABLISHED), SR_ESTABLISHED);

    a.write_all(b"from-a").unwrap();
    b.write_all(b"from-b!").unwrap();
    assert_eq!(c.wait_rx(0, 6), 6);
    assert_eq!(c.recv(0, 6), b"from-a");
    assert_eq!(c.wait_rx(1, 7), 7);
    assert_eq!(c.recv(1, 7), b"from-b!");

    // The card answers, then the clients go away: the sockets close one at a time.
    c.send(0, b"pong");
    let mut buf = [0u8; 4];
    a.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    a.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"pong");
    drop(a);
    wait_until(&mut c, "socket 0 to close", |c| {
        c.rd16(S0 + 0x26); // reading RSR notices the EOF
        c.sr(0) == SR_CLOSED
    });
    assert_eq!(c.sr(1), SR_ESTABLISHED);
    drop(b);
    wait_until(&mut c, "socket 1 to close", |c| {
        c.rd16(Chip::sock(1) + 0x26);
        c.sr(1) == SR_CLOSED
    });
}

#[test]
fn tcp_peer_that_sends_and_disconnects_still_delivers_its_data() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.write_all(b"last words").unwrap();
    }); // dropped: FIN right after the data
    let mut c = Chip::with_null();
    c.open(0, 0x01);
    c.set_dest(0, [127, 0, 0, 1], port);
    c.command(0, 0x04);
    server.join().unwrap();
    assert_eq!(c.wait_status(0, SR_ESTABLISHED), SR_ESTABLISHED);
    assert_eq!(c.wait_rx(0, 10), 10);
    assert_eq!(c.recv(0, 10), b"last words");
    wait_until(&mut c, "EOF", |c| {
        c.rd16(S0 + 0x26);
        c.sr(0) == SR_CLOSED
    });
    // A closed socket ignores further traffic and commands.
    c.send(0, b"too late");
    assert_eq!(c.rd16(S0 + 0x26), 0);
}

#[test]
fn tcp_silent_peer_is_not_a_timeout_and_close_reaches_the_peer() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut buf = [0u8; 8];
        s.read(&mut buf).unwrap() // 0 = the card closed the connection
    });
    let mut c = Chip::with_null();
    c.open(0, 0x01);
    c.set_dest(0, [127, 0, 0, 1], port);
    c.command(0, 0x04);
    assert_eq!(c.wait_status(0, SR_ESTABLISHED), SR_ESTABLISHED);
    // Nothing arrives: polling for a while neither fabricates data nor drops the connection.
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(100) {
        assert_eq!(c.rd16(S0 + 0x26), 0);
        c.m.run_cycles(1000);
    }
    assert_eq!(c.sr(0), SR_ESTABLISHED);
    c.command(0, 0x10); // CLOSE
    assert_eq!(c.sr(0), SR_CLOSED);
    assert_eq!(server.join().unwrap(), 0, "the peer saw EOF");
}

#[test]
fn udp_oversized_and_empty_datagrams_stay_inside_the_ring() {
    let mut c = Chip::with_null();
    c.open(0, 0x02);
    // Learn the card's (ephemeral) address from a datagram it sends.
    let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    c.set_dest(0, [127, 0, 0, 1], peer.local_addr().unwrap().port());
    c.send(0, b"probe");
    let mut b = [0u8; 16];
    let (_, card) = peer.recv_from(&mut b).unwrap();

    // 4000 bytes into a 2 KB ring: cut by the read, never stored beyond the ring.
    peer.send_to(&[0xAA; 4000], card).unwrap();
    let n = c.wait_rx(0, 9);
    assert!((9..0x800).contains(&n), "ring size {n}");
    let rd = c.rd16(S0 + 0x28);
    let header = c.read_block(RX0 + (rd & 0x7FF), 8);
    let len = u16::from_be_bytes([header[6], header[7]]);
    assert_eq!(len + 8, n, "header length matches what was stored");
    c.wr16(S0 + 0x28, rd.wrapping_add(n));
    c.command(0, 0x40);
    assert_eq!(c.rd16(S0 + 0x26), 0);

    // An empty datagram is only a header.
    peer.send_to(&[], card).unwrap();
    assert_eq!(c.wait_rx(0, 8), 8);
    let h = c.recv(0, 8);
    assert_eq!(u16::from_be_bytes([h[6], h[7]]), 0);
}

#[test]
fn register_lengths_out_of_range_never_panic_or_overflow() {
    let (mut c, h) = Chip::with_mock();
    // TX and RX pointers far beyond the 2 KB ring on a closed socket: ignored.
    c.wr16(S0 + 0x24, 0xFFFF);
    c.command(0, 0x20);
    c.wr16(S0 + 0x28, 0xFFFF);
    c.command(0, 0x40);
    // `updateRSR` is the ring distance between Sn_RX_WR (0) and the masked read pointer.
    assert!(c.rd16(S0 + 0x26) < 0x800);

    // MACRAW: write pointer wrapped around the ring, read pointer past the data.
    c.open(0, 0x04);
    c.wr16(S0 + 0x24, 0xFFFF);
    c.command(0, 0x20);
    assert_eq!(h.take_tx().len(), 1);
    assert_eq!(
        c.rd16(S0 + 0x22) & 0x7FF,
        0x7FF,
        "read pointer follows the masked write pointer"
    );
    c.wr16(S0 + 0x28, 0xFFFF);
    c.command(0, 0x40); // RECV with Sn_RX_RD beyond the data
    let rsr = c.rd16(S0 + 0x26);
    assert!(rsr < 0x800, "RSR stays inside the ring: {rsr:#x}");

    // A DNS name length past the register page is a failure (0.0.0.0), not an overrun.
    c.wr(Chip::sock(1) + 0x2A, 0xFF);
    c.open(1, 0x09);
    assert_eq!(c.read_block(Chip::sock(1) + 0x0C, 4), [0, 0, 0, 0]);
}

#[test]
fn malformed_frames_from_the_backend_are_dropped_not_delivered() {
    let (mut c, h) = Chip::with_mock();
    let my_mac = [2, 0, 0, 0, 0, 1];
    c.write_block(0x0009, &my_mac);
    c.write_block(0x000F, &[10, 0, 0, 2]);
    c.wr(Chip::sock(1) + 0x14, 1);
    c.open(1, 0x03); // IP RAW, ICMP

    let good = create_eth2_frame(
        &[8, 0, 0, 0, 0, 0, 0, 0],
        [2, 0, 0, 0, 0, 9],
        my_mac,
        64,
        0,
        1,
        [10, 0, 0, 1],
        [10, 0, 0, 2],
    );
    let mut bad_total = good.clone();
    bad_total[16] = 0xFF; // IP total length beyond the frame
    let mut bad_version = good.clone();
    bad_version[14] = 0x65; // IPv6 nibble under an IPv4 ethertype
    let runt = &good[..10]; // shorter than an Ethernet header

    for f in [&bad_total[..], &bad_version[..], runt, &[][..]] {
        h.push_rx(f);
    }
    assert_eq!(
        c.rd16(Chip::sock(1) + 0x26),
        0,
        "no IP RAW delivery for malformed packets"
    );

    h.push_rx(&good);
    assert_eq!(
        c.wait_rx(1, 14),
        14,
        "a well-formed packet still gets through afterwards"
    );
}

#[test]
fn unsupported_modes_leave_the_socket_closed_like_applewin() {
    // AUD-P-021: W5100.h:51 defines PPPoE (MR=5); Uthernet2.cpp:405-440 and :947-1001 handle
    // nothing for it or for the undefined values: OPEN leaves Sn_SR at 0 (CLOSED) with the
    // pointers reset. Same here, and the socket works afterwards.
    let (mut c, h) = Chip::with_mock();
    for mr in [0x05u8, 0x06, 0x07, 0x0C, 0x0D, 0x0E, 0x0F, 0x00] {
        c.wr(S0 + 0x25, 0x12); // dirty TX_WR, to see the pointers reset
        c.open(0, mr);
        assert_eq!(c.sr(0), SR_CLOSED, "MR={mr:#04x}");
        assert_eq!(
            c.rd16(S0 + 0x24),
            0,
            "MR={mr:#04x}: OPEN resets the pointers"
        );
        assert_eq!(c.rd16(S0 + 0x26), 0);
    }
    // Commands that do not exist on this card (PCON/PDISCON/PCR... $23-$27) are ignored.
    c.open(0, 0x02);
    assert_eq!(c.sr(0), SR_UDP);
    for cr in [0x23u8, 0x24, 0x25, 0x26, 0x27, 0x80, 0xFF] {
        c.command(0, cr);
        assert_eq!(c.sr(0), SR_UDP, "CR={cr:#04x}");
    }
    assert!(h.take_tx().is_empty());
}

#[test]
fn there_are_no_interrupts_and_the_pppoe_mode_bit_is_inert() {
    // IMR is not implemented (writes ignored, as in AppleWin) and IR never rises, and MR bit 3 (PPPoE enable on the
    // chip) changes nothing: sockets still open and the indirect bus still works.
    let mut c = Chip::with_null();
    c.wr(0x0016, 0xFF); // IMR: not implemented in AppleWin either, the write is ignored
    assert_eq!(c.rd(0x0016), 0);
    c.m.board_mut().write(MODE, 0x0B); // PPPoE bit | auto-increment | indirect
    c.open(0, 0x02);
    assert_eq!(c.sr(0), SR_UDP);
    c.set_dest(0, [127, 0, 0, 1], 9);
    c.send(0, b"x");
    assert_eq!(c.rd(0x0015), 0, "IR (common) stays clear");
    assert_eq!(c.rd(S0 + 0x02), 0, "Sn_IR stays clear");
}
