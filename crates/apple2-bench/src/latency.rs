//! Latência de entrada: da tecla até a leitura de `$C000` pelo programa, sem janela.
//!
//! Três medidas, da mais interna para a mais externa:
//! 1. ciclos emulados entre `key_press` e o fim do `LDA $C000` que lê a tecla (determinístico);
//! 2. o mesmo intervalo em tempo real na thread de emulação (custo do caminho do host);
//! 3. tempo real entre `Command::Key` enviado à thread de emulação (`emu::spawn`, ritmo por
//!    timer a 1.0x, sem áudio) e o primeiro quadro publicado que mostra o efeito da tecla.
//!
//! O que não dá para medir sem GUI está em `NOT_MEASURABLE` e vai para o relatório.

use apple2_config::Settings;
use apple2_config::SlotCardSpec;
use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::{spawn, Command, Emulator};
use apple2_rust_shell::keymap::KeyEvent;
use apple2_rust_shell::video::Frame;
use std::time::{Duration, Instant};

pub const NOT_MEASURABLE: &[&str] = &[
    "Entrega do evento de teclado pelo SO e pelo winit até a thread de UI (pilha de entrada do SO, compositor).",
    "Apresentação do quadro: fila do wgpu/swapchain, vsync e varredura do monitor físico.",
    "Latência do dispositivo de áudio (buffer do cpal e do driver): só existe com placa de som real.",
    "Tempo de resposta do próprio teclado/USB (debounce, polling de 1 ms a 8 ms).",
];

fn settings() -> Settings {
    let mut s = Settings::default();
    s.machine.memory_seed = Some(1);
    for slot in s.slots.0.iter_mut() {
        *slot = SlotCardSpec::Empty;
    }
    s
}

/// Programa de espera: `LDA $C000; BPL; STA $2000; STA $C010; JMP` em tela HGR preta.
const POLL: [u8; 27] = [
    0xAD, 0x50, 0xC0, 0xAD, 0x52, 0xC0, 0xAD, 0x54, 0xC0, 0xAD, 0x57, 0xC0, // HGR p.1
    0xAD, 0x00, 0xC0, // 030C: LDA $C000
    0x10, 0xFB, // BPL 030C
    0x8D, 0x14, 0x20, // STA $2014 (a tecla vira pixels)
    0x8D, 0x10, 0xC0, // STA $C010
    0x4C, 0x0C, 0x03, // JMP 030C
    0xEA,
];

fn stats(v: &mut [f64]) -> serde_json::Value {
    v.sort_by(|a, b| a.total_cmp(b));
    let pick = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    serde_json::json!({
        "n": v.len(), "min": v[0], "median": pick(0.5), "p95": pick(0.95), "max": v[v.len() - 1],
    })
}

/// Medidas 1 e 2.
fn in_machine() -> Result<serde_json::Value, String> {
    let mut emu = Emulator::new(settings(), Box::new(DefaultBackend), 48_000)?;
    for _ in 0..120 {
        emu.run_frame();
    }
    for (i, b) in POLL.iter().enumerate() {
        emu.machine.poke_main(0x300 + i as u16, *b);
    }
    emu.machine.cpu_mut().pc = 0x300;
    let mut cycles = Vec::new();
    let mut nanos = Vec::new();
    let mut via_command = Vec::new();
    for trial in 0..400u32 {
        // varia a fase do laço de espera (7 ciclos) em relação ao momento da tecla
        for _ in 0..(trial % 7) + 3 {
            emu.machine.step();
        }
        let key = b'A' + (trial % 26) as u8;
        let t0 = Instant::now();
        let c0 = emu.machine.cycles();
        if trial % 2 == 0 {
            emu.machine.key_press(key);
        } else {
            emu.apply(Command::Key(KeyEvent::Ascii(key)));
        }
        // roda até o LDA $C000 que enxerga a tecla (bit 7 de A) terminar
        loop {
            let pc = emu.machine.cpu().pc;
            emu.machine.step();
            if pc == 0x30C && emu.machine.cpu().a & 0x80 != 0 {
                break;
            }
        }
        let dc = (emu.machine.cycles() - c0) as f64;
        let dt = t0.elapsed().as_nanos() as f64;
        if trial % 2 == 0 {
            cycles.push(dc);
            nanos.push(dt);
        } else {
            via_command.push(dt);
        }
        // consome o resto até o próximo laço
        for _ in 0..4 {
            emu.machine.step();
        }
    }
    let hz = emu.clock_hz();
    let mut us = cycles.iter().map(|c| c / hz * 1e6).collect::<Vec<_>>();
    Ok(serde_json::json!({
        "cycles": stats(&mut cycles),
        "emulated_microseconds": stats(&mut us),
        "host_ns_machine_only": stats(&mut nanos),
        "host_ns_via_emulator_apply": stats(&mut via_command),
    }))
}

fn frame_hash(f: &Frame) -> u64 {
    f.rgba.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Medida 3: a thread de emulação real, sem janela nem áudio.
fn through_thread() -> Result<serde_json::Value, String> {
    let mut h = spawn(
        settings(),
        Box::new(|| Box::new(DefaultBackend)),
        None,
        48_000,
        Box::new(|| {}),
    )?;
    // Programa de espera digitado pelo caminho real do teclado (colar texto = uma tecla por
    // strobe liberado): HGR2 (tela cheia, sem cursor) e o laço em $0300 que escreve em $4000.
    // (antes de o firmware chegar ao prompt, o boot apagaria o que fosse digitado)
    let boot = Instant::now();
    while h.shared.status().frames < 150 {
        if boot.elapsed() > Duration::from_secs(120) {
            return Err("a thread de emulação não terminou o boot em 120 s".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    h.send(Command::Paste(
        "HGR2\rCALL -151\r300:AD 00 C0 10 FB 8D 14 40 8D 10 C0 4C 00 03\r300G\r".into(),
    ));
    // Espera por tempo emulado, não de parede: com a máquina ocupada a thread atrasa.
    let t0 = Instant::now();
    while h.shared.status().frames < 150 + 300 {
        if t0.elapsed() > Duration::from_secs(120) {
            return Err("a thread de emulação não chegou a 450 quadros em 120 s".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut frame = Frame::new(0, 0);
    let wait_change = |h: &apple2_rust_shell::emu::EmuHandle, frame: &mut Frame, from: u64| {
        let t0 = Instant::now();
        loop {
            h.shared.copy_frame(frame);
            if frame_hash(frame) != from {
                return Some(t0.elapsed());
            }
            if t0.elapsed() > Duration::from_secs(10) {
                return None;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    };
    h.shared.copy_frame(&mut frame);
    let mut last = frame_hash(&frame);
    // sonda de aquecimento: confirma que o laço de espera está rodando antes de medir
    h.send(Command::Key(KeyEvent::Ascii(b'Z')));
    if wait_change(&h, &mut frame, last).is_none() {
        return Err(
            "nenhum quadro mudou depois da tecla de sondagem (o programa de espera não rodou?)"
                .into(),
        );
    }
    last = frame_hash(&frame);
    let mut ms = Vec::new();
    for i in 0..40u32 {
        // espalha o instante da tecla pela fase do quadro de 16,7 ms
        std::thread::sleep(Duration::from_micros(300_000 + u64::from(i) * 1_300));
        let key = b'A' + (i % 26) as u8;
        let t0 = Instant::now();
        h.send(Command::Key(KeyEvent::Ascii(key)));
        let Some(_) = wait_change(&h, &mut frame, last) else {
            return Err(
                "nenhum quadro mudou depois da tecla (programa de espera não rodou?)".into(),
            );
        };
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        last = frame_hash(&frame);
    }
    h.shutdown();
    Ok(stats(&mut ms))
}

pub fn measure() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "in_machine": in_machine()?,
        "key_to_published_frame_ms": through_thread()?,
        "not_measurable_without_gui": NOT_MEASURABLE,
    }))
}
