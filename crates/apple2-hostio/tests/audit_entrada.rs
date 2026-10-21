//! #70 ("1.14.2 crashes with USCD Pascal", P0): o AppleWin 1.14.2 caía (0xC0000409, estouro de
//! buffer) com o SSC na configuração "Serial Port: Disabled"; o desenvolvedor reproduziu o
//! defeito só depois de um estado de COM inválido e o corrigiu no 1.16.1 (a solução alternativa
//! era escolher COM1 e voltar a Disabled). O mecanismo provável é o SSC acessado enquanto a
//! porta serial não existe. Sem a imagem do UCSD Pascal 1.2 (BLOCKED), este teste exercita o
//! mecanismo: o SSC com um backend que nunca abre, com todos os registradores, o firmware do
//! Pascal/BASIC (`PR#2`, `IN#2`) e resets no meio.
//! Resultados em `docs/conformance/results/entrada-cartas.csv`.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;
use apple2_hostio::serial::{LineConfig, ModemStatus, SerialBackend};
use apple2_hostio::SuperSerialCard;
use std::collections::VecDeque;

/// "Serial Port: Disabled" / porta que não abre: nada recebe, nada envia.
struct NoPort {
    opens: u32,
}

impl SerialBackend for NoPort {
    fn open(&mut self) -> bool {
        self.opens += 1;
        false
    }
    fn is_active(&self) -> bool {
        false
    }
    fn close(&mut self) {}
    fn poll_rx(&mut self, _out: &mut VecDeque<u8>) {}
    fn write(&mut self, _byte: u8) -> bool {
        false
    }
    fn can_send(&self) -> bool {
        false
    }
    fn modem_status(&mut self) -> ModemStatus {
        ModemStatus::default()
    }
    fn configure(&mut self, _line: &LineConfig) {}
}

fn machine(model: Model) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(model));
    assert!(m.insert_card(
        2,
        Box::new(SuperSerialCard::new(Box::new(NoPort { opens: 0 })))
    ));
    m
}

#[test]
fn ssc_without_a_serial_port_survives_every_register_access() {
    for model in [
        Model::Apple2,
        Model::Apple2Plus,
        Model::Apple2e,
        Model::Apple2eEnhanced,
    ] {
        let mut m = machine(model);
        let mut x: u32 = 0x1234_5678;
        for i in 0..200_000u32 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let reg = 0xC0A0 + (x & 0xF) as u16; // $C0A0-$C0AF: o slot 2 inteiro
            if x & 0x10 != 0 {
                m.board_mut().write(reg, (x >> 8) as u8);
            } else {
                m.board_mut().read(reg);
            }
            if i % 7919 == 0 {
                m.run_cycles(300);
            }
            if i % 50_000 == 0 {
                m.reset();
            }
        }
        m.power_cycle();
        m.run_frame();
    }
}

#[test]
fn pascal_and_basic_serial_firmware_run_without_a_port() {
    for model in [Model::Apple2Plus, Model::Apple2e] {
        let mut m = machine(model);
        m.reset();
        for _ in 0..200 {
            m.run_frame();
        }
        for text in ["PR#2\r", "PRINT \"UCSD\"\r", "PR#0\r", "IN#2\r"] {
            let _ = m.paste(text); // recusado se a fila anterior ainda não esvaziou
            for _ in 0..60 {
                m.run_frame();
            }
        }
        // Pascal 1.1 firmware entry points ($Cn0D INIT, $Cn0E READ, $Cn0F WRITE, $Cn10 STATUS
        // are offsets in $C20D-$C20F...): chama cada um com A = caractere
        for off in [0x0Du16, 0x0E, 0x0F, 0x10] {
            let entry = 0xC200 + u16::from(m.board_mut().read(0xC200 + off));
            let code = [
                0xA2u8,
                0x20,
                0xA0,
                0x02,
                0xA9,
                0x41,
                0x20,
                entry as u8,
                (entry >> 8) as u8,
                0x4C,
                0x09,
                0x03,
            ];
            for (i, b) in code.iter().enumerate() {
                m.poke(0x300 + i as u16, *b);
            }
            m.cpu_mut().pc = 0x300;
            for _ in 0..200_000 {
                if m.cpu().pc == 0x309 {
                    break;
                }
                m.step();
            }
        }
    }
}
