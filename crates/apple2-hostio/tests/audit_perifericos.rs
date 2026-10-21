//! Regressões de serial, rede e impressora (P23, gate G0). Cada teste cita o trecho do C++ do
//! AppleWin a8f0d1d que transcreve. Resultados em `docs/conformance/results/p23-perifericos.csv`.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::network::{MockNetwork, NullNetwork};
use apple2_hostio::printer::{MemorySink, ParallelPrinterCard, PrinterEncoding, PrinterOptions};
use apple2_hostio::serial::{LoopbackSerial, ModemStatus, TcpServerSerial};
use apple2_hostio::ssc::{ST_DCD, ST_DSR, ST_IRQ, ST_RX_FULL, ST_TX_EMPTY};
use apple2_hostio::{SuperSerialCard, Uthernet1, Uthernet2};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const SSC: u16 = 0xC080 + 2 * 16; // slot 2
const DATA: u16 = SSC + 8;
const STATUS: u16 = SSC + 9;
const COMMAND: u16 = SSC + 0xA;
const CONTROL: u16 = SSC + 0xB;
const DIPSW2: u16 = SSC + 2;

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

fn machine() -> Machine {
    Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced))
}

/// SSC em TCP num porto livre: o primeiro acesso a um registrador abre o listener (`CheckComm`,
/// SerialComms.cpp:212), pois o reset do cartão ao ser inserido fecha o anterior.
fn tcp_ssc() -> (Machine, u16) {
    let port = TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut m = machine();
    assert!(m.insert_card(
        2,
        Box::new(SuperSerialCard::new(Box::new(TcpServerSerial::new(port))))
    ));
    let _ = rd(&mut m, STATUS);
    (m, port)
}

/// Roda a máquina até `cond` ficar verdadeira (a thread do teste e o `accept` não são síncronos).
fn run_until(m: &mut Machine, what: &str, mut cond: impl FnMut(&mut Machine) -> bool) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(3) {
        m.run_cycles(2_000);
        if cond(m) {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("timeout: {what}");
}

// ------------------------------------------------------------------ #10, #308

/// #10 e #308: com a SSC em TCP (porta 1977 no AppleWin), habilitar a IRQ de recepção no registro
/// de comando levanta a IRQ e o bit 7 do status (ST_IRQ) vem junto. O fix do AppleWin foi
/// `m_vbRxIrqPending = true` em `CommTcpSerialReceive` (SerialComms.cpp:393); o status só devolve
/// ST_IRQ na primeira leitura (:856-857).
#[test]
fn gh10_gh308_tcp_receive_irq_sets_status_bit7_once() {
    let (mut m, port) = tcp_ssc();
    wr(&mut m, COMMAND, 0b0000_0101); // DTR, IRQ de RX ligada (bit 1 = 0), TX sem IRQ
    let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
    c.write_all(b"A").unwrap();
    run_until(&mut m, "IRQ de recepcao", |m| {
        m.board().interrupts.irq_line()
    });
    let st = rd(&mut m, STATUS);
    assert_eq!(st & (ST_IRQ | ST_RX_FULL), ST_IRQ | ST_RX_FULL, "{st:#x}");
    assert!(!m.board().interrupts.irq_line(), "ler o status limpa a IRQ");
    assert_eq!(
        rd(&mut m, STATUS) & ST_IRQ,
        0,
        "ST_IRQ so na primeira leitura"
    );
    assert_eq!(rd(&mut m, DATA), b'A');
}

// ------------------------------------------------------------------ #522

/// #522: IRQ de "TX register empty". Com o comando `%0000_0111` (DTR, IRQ de RX desligada,
/// transmissor com IRQ) a IRQ sobe quando o caractere termina de sair (`TransmitDone`,
/// SerialComms.cpp:724-733) e o status a informa uma vez; sem a IRQ de TX nada sobe.
#[test]
fn gh522_tx_empty_irq_is_raised_when_the_character_is_done() {
    let (b, h) = LoopbackSerial::new();
    let mut m = machine();
    assert!(m.insert_card(2, Box::new(SuperSerialCard::new(Box::new(b)))));
    wr(&mut m, CONTROL, 0x1E); // 9600 bps, 8-N-1
    wr(&mut m, COMMAND, 0b0000_0111);
    wr(&mut m, DATA, b'Z');
    assert_eq!(h.take_tx(), b"Z");
    assert!(!m.board().interrupts.irq_line(), "ainda transmitindo");
    m.run_cycles(1200); // 1063 ciclos por caractere a 9600 bps
    assert!(m.board().interrupts.irq_line(), "TX vazio levanta a IRQ");
    let st = rd(&mut m, STATUS);
    assert_eq!(st & (ST_IRQ | ST_TX_EMPTY), ST_IRQ | ST_TX_EMPTY);
    assert_eq!(rd(&mut m, STATUS) & ST_IRQ, 0, "so a primeira leitura");

    // Sem a IRQ de TX (comando %0000_1011: TX sem IRQ) nada sobe.
    wr(&mut m, COMMAND, 0b0000_1011);
    wr(&mut m, DATA, b'Y');
    m.run_cycles(1200);
    assert!(!m.board().interrupts.irq_line());
    assert_eq!(rd(&mut m, STATUS) & ST_IRQ, 0);
}

// ------------------------------------------------------------------ #386

/// #386: o DTR do registro de comando chega ao backend (`m_uDTR`, SerialComms.cpp:550-551) e o RTS
/// segue os bits de controle do transmissor (:521-536); DSR e DCD do status são ativos em nível
/// baixo (:863-864).
#[test]
fn gh386_command_register_dtr_and_rts_reach_the_backend() {
    let (b, h) = LoopbackSerial::new();
    let mut m = machine();
    assert!(m.insert_card(2, Box::new(SuperSerialCard::new(Box::new(b)))));
    wr(&mut m, COMMAND, 0b0000_0001); // DTR, transmissor desligado (RTS alto)
    let l = h.line().unwrap();
    assert!(l.dtr && !l.rts, "{l:?}");
    wr(&mut m, COMMAND, 0b0000_1011); // DTR, TX sem IRQ com RTS baixo
    let l = h.line().unwrap();
    assert!(l.dtr && l.rts, "{l:?}");
    wr(&mut m, COMMAND, 0b0000_1010); // DTR desligado
    let l = h.line().unwrap();
    assert!(!l.dtr && l.rts, "{l:?}");
    // ativos em nivel baixo: com DSR e DCD ligados os bits do status ficam em 0
    h.set_modem(ModemStatus {
        dsr: true,
        dcd: true,
        cts: true,
    });
    assert_eq!(rd(&mut m, STATUS) & (ST_DSR | ST_DCD), 0);
    h.set_modem(ModemStatus::default());
    assert_eq!(rd(&mut m, STATUS) & (ST_DSR | ST_DCD), ST_DSR | ST_DCD);
}

// ------------------------------------------------------------------ #553

/// #553: o ZLINK espera `(status & $30) == $10`, isto é DCD baixo (portadora presente) e TX vazio
/// (`2DE9: LDA ($F8) / AND #$30 / CMP #$10 / BNE`). Com o DCD do modem ligado o laço termina;
/// com o DCD desligado ele fica preso, como no AppleWin.
#[test]
fn gh553_zlink_status_loop_needs_dcd_asserted() {
    let (b, h) = LoopbackSerial::new();
    let mut m = machine();
    assert!(m.insert_card(2, Box::new(SuperSerialCard::new(Box::new(b)))));
    let zlink_ok = |m: &mut Machine| rd(m, STATUS) & 0x30 == 0x10;
    h.set_modem(ModemStatus {
        dsr: true,
        dcd: true,
        cts: true,
    });
    assert!(zlink_ok(&mut m), "DCD ligado");
    h.set_modem(ModemStatus {
        dsr: true,
        dcd: false,
        cts: true,
    });
    assert!(!zlink_ok(&mut m), "DCD desligado prende o laco");
}

// ------------------------------------------------------------------ #311

/// #311: com uma conexão TCP estabelecida DSR, DCD e CTS ficam ativos (`MS_RLSD_ON | MS_DSR_ON |
/// MS_CTS_ON`, SerialComms.cpp:832-835); sem cliente (ou depois de cair) ficam inativos. O CTS é
/// lido no bit 0 do DIP SW2 (:944-946).
#[test]
fn gh311_tcp_connection_state_drives_dsr_dcd_cts() {
    let (mut m, port) = tcp_ssc();
    let down = rd(&mut m, STATUS);
    assert_eq!(down & (ST_DSR | ST_DCD), ST_DSR | ST_DCD, "sem cliente");
    assert_eq!(rd(&mut m, DIPSW2) & 1, 1, "CTS inativo");
    let c = TcpStream::connect(("127.0.0.1", port)).unwrap();
    run_until(&mut m, "cliente aceito", |m| {
        rd(m, STATUS) & (ST_DSR | ST_DCD) == 0
    });
    assert_eq!(rd(&mut m, DIPSW2) & 1, 0, "CTS ativo");
    drop(c);
    run_until(&mut m, "cliente caiu", |m| {
        rd(m, STATUS) & (ST_DSR | ST_DCD) == ST_DSR | ST_DCD
    });
    assert_eq!(rd(&mut m, DIPSW2) & 1, 1, "CTS inativo de novo");
}

// ------------------------------------------------------------------ #1408

/// #1408 (aberta no AppleWin): o bit de overrun do status nunca é ligado na SSC em TCP.
/// `ST_OVERRUN_ERR` só existe no enum (SerialComms.cpp:811) e `CommStatus` nunca o devolve; o
/// port repete: lidos a conta-gotas sem pressa, os bytes saem em ordem e o bit 2 fica em 0.
#[test]
fn gh1408_overrun_bit_is_never_set_like_applewin() {
    let (mut m, port) = tcp_ssc();
    wr(&mut m, COMMAND, 0b0000_1011);
    let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
    c.write_all(b"abcdef").unwrap();
    run_until(&mut m, "seis bytes", |m| {
        m.run_cycles(20_000);
        rd(m, STATUS) & ST_RX_FULL != 0
    });
    m.run_cycles(200_000); // demora entre as leituras, como o player do relato
    let mut got = Vec::new();
    for _ in 0..6 {
        let st = rd(&mut m, STATUS);
        assert_eq!(st & 0b100, 0, "overrun nunca ligado");
        got.push(rd(&mut m, DATA));
        m.run_cycles(50_000);
    }
    assert_eq!(got, b"abcdef");
}

// ------------------------------------------------------------------ #1516

/// #1516: o porto TCP da SSC deixa de ser fixo (`-ssc-tcp-port`, CmdLine.cpp:824-834). O
/// backend escuta exatamente no porto pedido.
#[test]
fn gh1516_tcp_backend_listens_on_the_requested_port() {
    let free = TcpListener::bind(("0.0.0.0", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut b = TcpServerSerial::new(free);
    assert!(apple2_hostio::serial::SerialBackend::open(&mut b));
    assert_eq!(b.local_port(), Some(free));
}

// ------------------------------------------------------------------ #1075

/// #1075 (aberta no AppleWin): a ROM de expansão da SSC em `$C800` só é desligada por um acesso a
/// `$CFFF` (Memory.cpp:1012-1018), não por qualquer `$CFxx`. O port repete isso.
#[test]
fn gh1075_ssc_expansion_rom_is_released_only_by_cfff() {
    let (b, _h) = LoopbackSerial::new();
    let mut m = machine();
    assert!(m.insert_card(2, Box::new(SuperSerialCard::new(Box::new(b)))));
    let _ = rd(&mut m, 0xC200); // seleciona a ROM do slot 2
    let seen = |m: &mut Machine| (0..16).map(|i| rd(m, 0xC800 + i)).collect::<Vec<u8>>();
    let exp = seen(&mut m);
    assert!(exp.iter().any(|&b| b != 0xFF) && exp != vec![0; 16]);
    let _ = rd(&mut m, 0xCF00);
    assert_eq!(seen(&mut m), exp, "$CF00 nao libera a ROM");
    let _ = rd(&mut m, 0xCFFF);
    assert_ne!(seen(&mut m), exp, "$CFFF libera a ROM");
}

// ------------------------------------------------------------------ #1113

/// #1113: os quatro registradores do W5100 se repetem em todo o espaço DEVICE SELECT
/// (`U2_C0X_MASK 0x03`, W5100.h:6; `IO_C0`, Uthernet2.cpp:1419).
#[test]
fn gh1113_uthernet2_registers_repeat_every_four_addresses() {
    let mut m = machine();
    assert!(m.insert_card(4, Box::new(Uthernet2::new(Box::new(NullNetwork), true))));
    let io = 0xC080 + 4 * 16;
    for base in [0u16, 4, 8, 12] {
        wr(&mut m, io + base + 1, 0x04);
        wr(&mut m, io + base + 2, 0x16);
        // os dois ultimos registradores lidos de outro alias devolvem o mesmo endereco
        for alias in [0u16, 4, 8, 12] {
            assert_eq!(
                rd(&mut m, io + alias + 1),
                0x04,
                "base {base} alias {alias}"
            );
            assert_eq!(rd(&mut m, io + alias + 2), 0x16);
        }
    }
    // o registrador de modo tambem: escrever num alias e ler em outro
    wr(&mut m, io + 8, 0x03);
    assert_eq!(rd(&mut m, io), 0x03);
    assert_eq!(rd(&mut m, io + 12), 0x03);
}

// ------------------------------------------------------------------ #1046

/// #1046 (aberta no AppleWin: dois quadros no mesmo milissegundo perdem o segundo): o Uthernet I
/// do port entrega dois quadros que chegam juntos, na ordem, sem perder o segundo.
#[test]
fn gh1046_two_frames_arriving_together_are_both_delivered() {
    let (net, h) = MockNetwork::new();
    let mut m = machine();
    assert!(m.insert_card(3, Box::new(Uthernet1::new(Box::new(net)))));
    let io = 0xC080 + 3 * 16;
    let (pp_ptr, pp_data) = (io + 0xA, io + 0xC);
    let word_w = |m: &mut Machine, p: u16, v: u16| {
        wr(m, p, v as u8);
        wr(m, p + 1, (v >> 8) as u8);
    };
    // RxCTL: promíscuo
    word_w(&mut m, pp_ptr, 0x0104);
    word_w(&mut m, pp_data, 0x0080);
    let frame = |tag: u8, len: usize| {
        let mut f = vec![0x02, 9, 9, 9, 9, 9, 0x02, 0, 0, 0, 0, 1];
        f.resize(len, tag);
        f
    };
    h.push_rx(&frame(0xA1, 64));
    h.push_rx(&frame(0xB2, 60));
    let mut lengths = Vec::new();
    for _ in 0..2 {
        word_w(&mut m, pp_ptr, 0x0124); // RxEvent: busca o proximo quadro
        let _ = rd(&mut m, pp_data + 1);
        word_w(&mut m, pp_ptr, 0x0402); // RxLength
        let len = u16::from(rd(&mut m, pp_data)) | u16::from(rd(&mut m, pp_data + 1)) << 8;
        lengths.push(len);
    }
    assert_eq!(lengths, [64, 60], "os dois quadros chegam, em ordem");
}

// ------------------------------------------------------------------ #1017, #54

/// #1017: só `$C090` (porta 0) imprime; o strobe `$C092` e as outras portas não repetem o byte
/// (ParallelPrinter.cpp:140-141). #54: o arquivo de saída acumula (append) e é escolhido pelo
/// usuário; aqui, o modo append mantém os trabalhos anteriores no mesmo destino.
#[test]
fn gh1017_gh54_only_port_zero_prints_and_append_keeps_earlier_jobs() {
    let sink = MemorySink::new();
    let mut m = machine();
    let opts = PrinterOptions {
        append: true,
        filter_unprintable: false,
        ..PrinterOptions::default()
    };
    assert!(m.insert_card(
        1,
        Box::new(ParallelPrinterCard::new(Box::new(sink.clone()), opts))
    ));
    let io = 0xC080 + 16;
    for byte in b"AB" {
        wr(&mut m, io, *byte); // dado
        wr(&mut m, io + 2, 0x00); // strobe, como o Print Shop
    }
    assert_eq!(sink.contents(), b"AB", "cada byte uma vez so");
}

// ------------------------------------------------------------------ #916

/// #916: a conversão latim -> cirílico da impressora Pravets. O C++ a8f0d1d guarda as tabelas em
/// UTF-8 (Pravets.cpp:210-214: `Ã\u{80}Ã\u{81}`...), então o laço de 31/34 bytes troca por bytes
/// soltos; o port usa CP-1251 (o byte único de cada letra) como o manual da impressora mostra.
#[test]
fn gh916_pravets_conversion_maps_latin_to_cp1251_cyrillic() {
    // Pravets 82/M: 'a' -> A (U+0410 = 0xC0 em CP-1251), 'b' -> 0xC1, '[' (cirilico) fica '['.
    assert_eq!(PrinterEncoding::Pravets82.convert(b'a'), 0xC0);
    assert_eq!(PrinterEncoding::Pravets82.convert(b'b'), 0xC1);
    assert_eq!(PrinterEncoding::Pravets82.convert(b'c'), 0xD6); // 'c' -> Ц
                                                                // Pravets 8A: minúsculas (91..127) -> minúsculas cirílicas, maiúsculas (65..90) -> +32
    assert_eq!(PrinterEncoding::Pravets8A.convert(b'A'), b'a');
    assert_eq!(PrinterEncoding::Pravets8A.convert(b'a'), 0xE0);
}
