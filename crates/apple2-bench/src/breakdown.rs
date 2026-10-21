//! Decomposição do custo de um quadro (`--breakdown`), no lugar do `perf`, que o kernel desta
//! máquina bloqueia (`perf_event_paranoid=4`). Cada cenário roda quatro vezes sobre máquinas
//! idênticas e determinísticas, acrescentando uma etapa por vez:
//!
//! 0. só a máquina (`Machine::run_frame`: CPU, vídeo-scanner, memória, cartões);
//! 1. + o renderer alimentado a cada instrução (`FrameSource::run_frame`, o `NTSC_VideoUpdateCycles`);
//! 2. + a conversão do quadro para RGBA (`FrameSource::render`);
//! 3. + a mixagem do áudio (`Mixer::produce_machine`).
//!
//! A diferença entre etapas consecutivas é o custo da etapa. Mede-se tempo de CPU da thread, e
//! vale o mínimo de 3 execuções (o mínimo é o que menos sofre com a máquina ocupada).

use crate::scenarios::spec;
use crate::sys;
use apple2_audio::{AudioBus, Mixer};
use apple2_rust_shell::backend::{Backend, DefaultBackend};
use apple2_rust_shell::video::Frame;
use serde_json::{json, Value};
use std::path::Path;

const NAMES: [&str; 7] = [
    "hgr",
    "dhgr",
    "ntsc_monitor",
    "ntsc_tv",
    "pal",
    "disk2_dos33",
    "mockingboard",
];

fn one(name: &str, root: &Path, variant: u8) -> Result<f64, String> {
    let sp = spec(name, root)?.ok_or("sem Spec")?;
    let bus = AudioBus::new(48_000, sp.settings.to_machine_config().video_standard);
    let mut m = DefaultBackend.build_machine(&sp.settings, &bus)?;
    let mut video = DefaultBackend.frame_source(&sp.settings);
    let mut mixer = Mixer::new(&bus);
    let mut frame = Frame::new(0, 0);
    for _ in 0..sp.warm {
        video.run_frame(&mut m);
        mixer.produce_machine(&mut m);
    }
    if let Some((org, code)) = &sp.program {
        for (i, b) in code.iter().enumerate() {
            m.poke_main(org + i as u16, *b);
        }
        m.cpu_mut().pc = *org;
    }
    for _ in 0..10 {
        video.run_frame(&mut m);
        video.render(&m, &mut frame);
        mixer.produce_machine(&mut m);
    }
    let cpu0 = sys::thread_cpu_secs();
    for _ in 0..sp.frames {
        match variant {
            0 => {
                m.run_frame();
            }
            1 => {
                video.run_frame(&mut m);
            }
            2 => {
                video.run_frame(&mut m);
                video.render(&m, &mut frame);
            }
            _ => {
                video.run_frame(&mut m);
                video.render(&m, &mut frame);
                mixer.produce_machine(&mut m);
            }
        }
    }
    Ok((sys::thread_cpu_secs() - cpu0) / sp.frames as f64 * 1e3)
}

pub fn run(root: &Path) -> Result<Value, String> {
    let mut out = serde_json::Map::new();
    for name in NAMES {
        let mut ms = [0f64; 4];
        for (v, slot) in ms.iter_mut().enumerate() {
            let mut best = f64::MAX;
            for _ in 0..3 {
                best = best.min(one(name, root, v as u8)?);
            }
            *slot = best;
        }
        // O mínimo de cada etapa pode, por ruído, ficar abaixo da etapa anterior: trunca em 0.
        let step = |i: usize| (ms[i] - if i == 0 { 0.0 } else { ms[i - 1] }).max(0.0);
        out.insert(
            name.to_string(),
            json!({
                "ms_per_frame_total": ms[3],
                "machine_ms": step(0),
                "renderer_update_ms": step(1),
                "render_to_rgba_ms": step(2),
                "audio_mix_ms": step(3),
                "cumulative_ms": ms,
            }),
        );
        eprintln!("breakdown {name}: {ms:.3?} ms/quadro");
    }
    Ok(Value::Object(out))
}
