//! Os cenários medidos. Todos são determinísticos (semente de RAM fixa, nenhuma entrada de
//! relógio do host): o que muda entre execuções é só o tempo de parede. Cada cenário devolve
//! também uma impressão digital (ciclos emulados, hash do último quadro, hash e pico do áudio)
//! que o modo `--check` compara com o baseline: uma otimização que mude timing, vídeo ou áudio
//! é pega ali, antes de qualquer conversa sobre velocidade.

use crate::sys;
use apple2_audio::AudioBus;
use apple2_config::card::DriveSpec;
use apple2_config::settings::{RefreshRate, VideoType};
use apple2_config::{Settings, SlotCardSpec};
use apple2_core::Machine;
use apple2_rust_shell::backend::{Backend, DefaultBackend};
use apple2_rust_shell::benchmark::setup_cpu_benchmark;
use apple2_rust_shell::emu::Emulator;
use std::path::Path;
use std::time::Instant;

pub struct Scenario {
    pub name: &'static str,
    pub title: &'static str,
    pub run: fn(&Path) -> Result<Sample, String>,
}

pub const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "cpu",
        title: "CPU pura: laço de instruções (programa do Benchmark do AppleWin)",
        run: cpu,
    },
    Scenario {
        name: "boot_text",
        title: "Boot //e Enhanced até o prompt e programa BASIC imprimindo texto",
        run: boot_text,
    },
    Scenario {
        name: "hgr",
        title: "HGR animando (página inteira reescrita todo quadro), cores idealizadas",
        run: hgr,
    },
    Scenario {
        name: "dhgr",
        title: "DHGR animando (main + aux reescritas todo quadro), cores idealizadas",
        run: dhgr,
    },
    Scenario {
        name: "ntsc_monitor",
        title: "HGR animando, renderer NTSC Color Monitor (60 Hz)",
        run: ntsc_monitor,
    },
    Scenario {
        name: "ntsc_tv",
        title: "HGR animando, renderer NTSC Color TV (60 Hz)",
        run: ntsc_tv,
    },
    Scenario {
        name: "pal",
        title: "HGR animando, renderer Color TV em PAL (50 Hz)",
        run: pal,
    },
    Scenario {
        name: "disk2_dos33",
        title: "Disk II lendo: boot do DOS 3.3 System Master (assets/disks)",
        run: disk2_dos33,
    },
    Scenario {
        name: "mockingboard",
        title: "Áudio: Mockingboard C (2x AY-3-8910) tocando, mixer em 48 kHz",
        run: mockingboard,
    },
];

/// Resultado de uma execução (um processo filho).
#[derive(Default)]
pub struct Sample {
    pub wall_s: f64,
    pub cpu_s: f64,
    pub frames: u64,
    pub cycles: u64,
    pub audio_frames: u64,
    pub fp_cycles: u64,
    pub fp_frame: u64,
    pub fp_audio: u64,
    pub audio_peak: f32,
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(FNV_PRIME);
    }
    h
}

fn base_settings() -> Settings {
    let mut s = Settings::default();
    // Sem isto o host sorteia a RAM de power-on pelo relógio e nada seria reprodutível.
    s.machine.memory_seed = Some(1);
    for slot in s.slots.0.iter_mut() {
        *slot = SlotCardSpec::Empty;
    }
    s
}

fn new_emulator(s: Settings) -> Result<Emulator, String> {
    Emulator::new(s, Box::new(DefaultBackend), 48_000)
}

/// Roda `frames` quadros de vídeo com renderização do quadro e mixagem do áudio (o mesmo
/// trabalho da thread de emulação), medindo só esse trecho.
fn timed_frames(emu: &mut Emulator, frames: u64) -> Sample {
    // Só para criação de perfil (gdb/amostragem): alonga a execução; invalida o baseline.
    let frames = frames
        * std::env::var("BENCH_FRAMES_X")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1);
    let c0 = emu.machine.cycles();
    let mut audio_frames = 0u64;
    let mut audio_hash = FNV_OFFSET;
    let mut peak = 0f32;
    let cpu0 = sys::thread_cpu_secs();
    let t0 = Instant::now();
    for _ in 0..frames {
        let (_, audio) = emu.run_frame();
        audio_frames += (audio.len() / 2) as u64;
        // Soma de verificação barata (um xor-rotate por amostra), longe de pesar no quadro.
        let mut h = 0u32;
        for s in &audio {
            h = h.rotate_left(5) ^ s.to_bits();
            peak = peak.max(s.abs());
        }
        audio_hash = fnv(audio_hash, &h.to_le_bytes());
        emu.render();
    }
    let wall_s = t0.elapsed().as_secs_f64();
    let cpu_s = sys::thread_cpu_secs() - cpu0;
    let frame_hash = fnv(FNV_OFFSET, &emu.frame().rgba);
    Sample {
        wall_s,
        cpu_s,
        frames,
        cycles: emu.machine.cycles() - c0,
        audio_frames,
        fp_cycles: emu.machine.cycles(),
        fp_frame: frame_hash,
        fp_audio: audio_hash,
        audio_peak: peak,
    }
}

fn load_program(m: &mut Machine, org: u16, code: &[u8]) {
    for (i, b) in code.iter().enumerate() {
        m.poke_main(org + i as u16, *b);
    }
    m.cpu_mut().pc = org;
}

/// Sub-rotina em `$0330`: preenche `$2000-$3FFF` com o contador de `$FB`.
const FILL: [u8; 25] = [
    0xA9, 0x20, 0x85, 0xFD, 0xA9, 0x00, 0x85, 0xFC, 0xA0, 0x00, // ptr = $2000, Y = 0
    0xA5, 0xFB, 0x91, 0xFC, 0xC8, 0xD0, 0xF9, // LDA $FB; STA (ptr),Y; INY; BNE
    0xE6, 0xFD, 0xA5, 0xFD, 0xC9, 0x40, 0xD0, 0xF1, // INC hi; CMP #$40; BNE
];

fn with_fill(mut main: Vec<u8>) -> Vec<u8> {
    assert!(main.len() <= 0x30);
    main.resize(0x30, 0xEA);
    main.extend_from_slice(&FILL);
    main.push(0x60); // RTS
    main
}

fn hgr_program() -> Vec<u8> {
    with_fill(vec![
        0xAD, 0x50, 0xC0, // LDA $C050 gráficos
        0xAD, 0x52, 0xC0, // LDA $C052 tela cheia
        0xAD, 0x54, 0xC0, // LDA $C054 página 1
        0xAD, 0x57, 0xC0, // LDA $C057 HGR
        0xE6, 0xFB, // loop: INC $FB
        0x20, 0x30, 0x03, // JSR fill
        0x4C, 0x0C, 0x03, // JMP loop
    ])
}

fn dhgr_program() -> Vec<u8> {
    with_fill(vec![
        0x8D, 0x0D, 0xC0, // STA $C00D 80COL on
        0x8D, 0x01, 0xC0, // STA $C001 80STORE on
        0xAD, 0x50, 0xC0, // gráficos
        0xAD, 0x52, 0xC0, // tela cheia
        0xAD, 0x57, 0xC0, // HGR
        0xAD, 0x5E, 0xC0, // DHIRES on
        0xE6, 0xFB, // loop (0x12): INC $FB
        0xAD, 0x55, 0xC0, // PAGE2: escritas vão para a aux
        0x20, 0x30, 0x03, // JSR fill
        0xAD, 0x54, 0xC0, // PAGE1: main
        0x20, 0x30, 0x03, // JSR fill
        0x4C, 0x12, 0x03, // JMP loop
    ])
}

pub fn cpu(_root: &Path) -> Result<Sample, String> {
    let s = base_settings();
    let bus = AudioBus::new(48_000, s.to_machine_config().video_standard);
    let mut m = DefaultBackend.build_machine(&s, &bus)?;
    setup_cpu_benchmark(&mut m);
    m.run_cycles(200_000);
    let c0 = m.cycles();
    let cpu0 = sys::thread_cpu_secs();
    let t0 = Instant::now();
    for _ in 0..200 {
        m.run_cycles(100_000);
    }
    let wall_s = t0.elapsed().as_secs_f64();
    let cpu_s = sys::thread_cpu_secs() - cpu0;
    let cycles = m.cycles() - c0;
    if !(0x300..=0x400).contains(&m.cpu().pc) {
        return Err("o PC saiu do código do benchmark".into());
    }
    Ok(Sample {
        wall_s,
        cpu_s,
        frames: cycles / u64::from(s.to_machine_config().video_standard.cycles_per_frame()),
        cycles,
        fp_cycles: m.cycles(),
        ..Sample::default()
    })
}

fn boot_text(_root: &Path) -> Result<Sample, String> {
    let mut emu = new_emulator(base_settings())?;
    // O boot entra no cronômetro: é o que o usuário espera de "ligar a máquina".
    let mut a = timed_frames(&mut emu, 150);
    emu.apply(apple2_rust_shell::emu::Command::Paste(
        "10 FOR I=1 TO 200:PRINT I;\" APPLE II RUST BENCHMARK\":NEXT I\rRUN\r".into(),
    ));
    let b = timed_frames(&mut emu, 450);
    a.wall_s += b.wall_s;
    a.cpu_s += b.cpu_s;
    a.frames += b.frames;
    a.cycles += b.cycles;
    a.audio_frames += b.audio_frames;
    a.fp_cycles = b.fp_cycles;
    a.fp_frame = b.fp_frame;
    a.fp_audio = fnv(a.fp_audio, &b.fp_audio.to_le_bytes());
    a.audio_peak = a.audio_peak.max(b.audio_peak);
    Ok(a)
}

/// Cenário de máquina completa: configuração, quadros de aquecimento (fora do cronômetro),
/// programa a injetar e quadros cronometrados. Também alimenta a decomposição de custo.
pub struct Spec {
    pub settings: Settings,
    pub warm: u64,
    pub program: Option<(u16, Vec<u8>)>,
    pub frames: u64,
}

fn animated(mode: VideoType, hz50: bool, program: Vec<u8>) -> Spec {
    let mut settings = base_settings();
    settings.video.mode = mode;
    settings.video.half_scanlines = false;
    if hz50 {
        settings.machine.refresh_rate = RefreshRate::Hz50;
    }
    Spec {
        settings,
        warm: 130,
        program: Some((0x300, program)),
        frames: 300,
    }
}

/// Os cenários que rodam a máquina inteira (todos menos `cpu` e `boot_text`).
pub fn spec(name: &str, root: &Path) -> Result<Option<Spec>, String> {
    Ok(Some(match name {
        "hgr" => animated(VideoType::ColorIdealized, false, hgr_program()),
        "dhgr" => animated(VideoType::ColorIdealized, false, dhgr_program()),
        "ntsc_monitor" => animated(VideoType::ColorMonitorNtsc, false, hgr_program()),
        "ntsc_tv" => animated(VideoType::ColorTv, false, hgr_program()),
        "pal" => animated(VideoType::ColorTv, true, hgr_program()),
        "disk2_dos33" => {
            let image = root.join("assets/disks/DOS 3.3 System Master - 680-0210-A.dsk");
            if !image.is_file() {
                return Err(format!("disco ausente: {}", image.display()));
            }
            let mut settings = base_settings();
            settings.slots.0[6] = SlotCardSpec::Disk2 {
                firmware_13_sector: false,
                drives: [
                    DriveSpec {
                        image: Some(image),
                        ..DriveSpec::default()
                    },
                    DriveSpec::default(),
                ],
            };
            Spec {
                settings,
                warm: 0,
                program: None,
                frames: 600,
            }
        }
        "mockingboard" => {
            let mut settings = base_settings();
            settings.slots.0[4] = SlotCardSpec::MockingboardC(Default::default());
            Spec {
                settings,
                warm: 130,
                program: Some((MB_ORG, mockingboard_program())),
                frames: 400,
            }
        }
        _ => return Ok(None),
    }))
}

fn run_spec(name: &str, root: &Path) -> Result<Sample, String> {
    let sp = spec(name, root)?.ok_or("cenário sem Spec")?;
    let mut emu = new_emulator(sp.settings)?;
    for _ in 0..sp.warm {
        emu.run_frame();
    }
    if let Some((org, code)) = &sp.program {
        load_program(&mut emu.machine, *org, code);
    }
    // Aquece o renderer e as caches antes de cronometrar.
    for _ in 0..10 {
        emu.run_frame();
        emu.render();
    }
    let a = timed_frames(&mut emu, sp.frames);
    if name == "mockingboard" && a.audio_peak < 0.001 {
        return Err("o Mockingboard ficou mudo: o programa de teste não funcionou".into());
    }
    Ok(a)
}

fn hgr(root: &Path) -> Result<Sample, String> {
    run_spec("hgr", root)
}
fn dhgr(root: &Path) -> Result<Sample, String> {
    run_spec("dhgr", root)
}
fn ntsc_monitor(root: &Path) -> Result<Sample, String> {
    run_spec("ntsc_monitor", root)
}
fn ntsc_tv(root: &Path) -> Result<Sample, String> {
    run_spec("ntsc_tv", root)
}
fn pal(root: &Path) -> Result<Sample, String> {
    run_spec("pal", root)
}
fn disk2_dos33(root: &Path) -> Result<Sample, String> {
    run_spec("disk2_dos33", root)
}
fn mockingboard(root: &Path) -> Result<Sample, String> {
    run_spec("mockingboard", root)
}

/// Emite `STA reg`/`valor` num AY pelo VIA em `via` (protocolo do Mockingboard: ORA = dado,
/// ORB = 7 trava o endereço, 6 escreve, 4 inativo).
fn ay_write(code: &mut Vec<u8>, via: u16, reg: u8, val: Option<u8>) {
    let (orb, ora) = (via, via + 1);
    let sta = |code: &mut Vec<u8>, a: u16| code.extend_from_slice(&[0x8D, a as u8, (a >> 8) as u8]);
    let lda = |code: &mut Vec<u8>, v: u8| code.extend_from_slice(&[0xA9, v]);
    lda(code, reg);
    sta(code, ora);
    lda(code, 7);
    sta(code, orb);
    lda(code, 4);
    sta(code, orb);
    match val {
        Some(v) => lda(code, v),
        None => code.extend_from_slice(&[0xA5, 0xFB]), // LDA $FB
    }
    sta(code, ora);
    lda(code, 6);
    sta(code, orb);
    lda(code, 4);
    sta(code, orb);
}

/// Fora de `$0300` porque as 18 escritas de AY não cabem em uma página.
const MB_ORG: u16 = 0x0800;

fn mockingboard_program() -> Vec<u8> {
    // O firmware da //e deixa a ROM interna em $C100-$CFFF; STA $C006 (SLOTCXROM) devolve a
    // janela do slot, onde fica o Mockingboard.
    let mut c = vec![0x8D, 0x06, 0xC0];
    for via in [0xC400u16, 0xC480] {
        for ddr in [via + 2, via + 3] {
            c.extend_from_slice(&[0xA9, 0xFF, 0x8D, ddr as u8, (ddr >> 8) as u8]);
        }
        // reset do AY (ORB bit 2 em 0) e depois inativo
        c.extend_from_slice(&[0xA9, 0x00, 0x8D, via as u8, (via >> 8) as u8]);
        c.extend_from_slice(&[0xA9, 0x04, 0x8D, via as u8, (via >> 8) as u8]);
        ay_write(&mut c, via, 7, Some(0x38)); // tons ligados nos 3 canais
        for (r, v) in [(8u8, 0x0Fu8), (9, 0x0C), (10, 0x0A), (1, 0), (3, 1), (5, 2)] {
            ay_write(&mut c, via, r, Some(v));
        }
        ay_write(&mut c, via, 2, Some(0x90));
        ay_write(&mut c, via, 4, Some(0x50));
    }
    let main = c.len() as u16 + MB_ORG;
    c.extend_from_slice(&[0xE6, 0xFB]); // INC $FB
    for via in [0xC400u16, 0xC480] {
        ay_write(&mut c, via, 0, None); // período fino do canal A varia: glissando
    }
    // atraso curto
    c.extend_from_slice(&[0xA2, 0xFF, 0xCA, 0xD0, 0xFD]);
    c.extend_from_slice(&[0x4C, main as u8, (main >> 8) as u8]);
    assert!(c.len() < 0x400);
    c
}
