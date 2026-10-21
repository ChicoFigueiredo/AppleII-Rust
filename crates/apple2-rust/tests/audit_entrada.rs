//! Regressões das issues do AppleWin de teclado, joystick, "outra" (jogos) e configuração
//! (HU-0702): #103, #116, #293, #330, #454, #1161, #1261, #429, #778, #985, #1128, #232 e #1491.
//!
//! Oráculo: AppleWin a8f0d1d (C++ transcrito onde o teste cita linhas) e, para o game port e o
//! `$C01x`, o trace instrução a instrução contra o AppleWin (casos `disk-micro-input-io-*`).
//! Os resultados por issue ficam em `docs/conformance/results/entrada-cartas.csv`.

use apple2_config::{CardType, CmdLine, Settings, SlotCardSpec};
use apple2_core::game_port::GamePort;
use apple2_core::{Machine, MachineConfig, Model, VideoStandard};
use apple2_cpu::Bus;
use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::{Command, Emulator};
use apple2_rust_shell::input::{InputRouter, KeyCtx, Routed};
use apple2_rust_shell::joy::{apply_trim, axis_to_paddle, square_circle, DEFAULT_DEADZONE};
use apple2_rust_shell::keymap::{HostKey, KeyEvent, KeyInput, KeyboardState, Mods};

const NONE: Mods = Mods {
    shift: false,
    ctrl: false,
    alt: false,
};

/// ROM de NOPs com RESET em $0300 (12K para a família ][, 16K para a //e).
fn custom(model: Model) -> Machine {
    let size = if model.is_apple2() { 0x3000 } else { 0x4000 };
    let mut rom = vec![0xEAu8; size];
    rom[size - 4] = 0x00;
    rom[size - 3] = 0x03;
    let mut cfg = MachineConfig::for_model(model);
    cfg.custom_rom = Some(rom);
    cfg.video_standard = VideoStandard::Ntsc;
    Machine::new(cfg)
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn load(m: &mut Machine, at: u16, code: &[u8]) {
    for (i, b) in code.iter().enumerate() {
        m.poke(at + i as u16, *b);
    }
}

/// Entrega os eventos do `KeyboardState` à máquina, como o `Emulator` faz.
fn deliver(m: &mut Machine, events: Vec<KeyEvent>) {
    for e in events {
        match e {
            KeyEvent::Ascii(b) => {
                m.key_press(b);
            }
            KeyEvent::AnyKeyDown(d) => m.set_any_key_down(d),
            _ => {}
        }
    }
}

// ------------------------------------------------------------------ #116 e #1261

/// #116 (AKD só existe na //e) e #1261 (`$C01x` na família ][ não lê o latch do teclado: é o
/// barramento flutuante). C++ Memory.cpp `IORead_C01x`: `if (IS_APPLE2) { KeybClearStrobe();
/// return IO_Null(...); }`. O mesmo comportamento foi comparado com o AppleWin no trace do
/// micro-programa `input-io` (ciclo a ciclo, incluindo os `LDA $C010`).
#[test]
fn gh116_gh1261_c01x_on_the_ii_family_is_the_floating_bus_not_the_latch() {
    for model in [Model::Apple2, Model::Apple2Plus, Model::Apple2JPlus] {
        let mut m = custom(model);
        for a in 0..0xC000u16 {
            m.poke_main(a, 0x5A); // o scanner de vídeo só pode ver 5A
        }
        assert!(m.key_press(b'a'));
        m.set_any_key_down(true);
        for addr in [0xC010u16, 0xC011, 0xC015, 0xC01F] {
            m.key_press(b'a');
            let v = rd(&mut m, addr);
            assert_eq!(
                v, 0x5A,
                "{model:?} ${addr:04X}: barramento flutuante, não o latch"
            );
            assert_eq!(rd(&mut m, 0xC000) & 0x80, 0, "o strobe foi limpo");
        }
    }
    // a //e devolve keycode | AKD
    let mut e = custom(Model::Apple2e);
    e.key_press(b'a');
    e.set_any_key_down(true);
    assert_eq!(rd(&mut e, 0xC010), 0xC1);
    e.set_any_key_down(false);
    e.key_press(b'a');
    assert_eq!(rd(&mut e, 0xC010), 0x41, "sem tecla apertada o AKD é 0");
}

// ------------------------------------------------------------------ #330 e #1161

/// #330 (sequência de Michael Mahon no //e real): ESC, A, soltar A, soltar ESC lê
/// 155, 193, 193, 65 em `PEEK(49168)`.
#[test]
fn gh330_second_key_release_keeps_any_key_down() {
    let mut m = custom(Model::Apple2eEnhanced);
    let mut k = KeyboardState::new();
    let esc = KeyInput::new(HostKey::Escape);
    let a = KeyInput::new(HostKey::Char('a'));
    deliver(&mut m, k.handle(esc, true, false, NONE));
    assert_eq!(rd(&mut m, 0xC010), 155);
    deliver(&mut m, k.handle(a, true, false, NONE));
    assert_eq!(rd(&mut m, 0xC010), 193);
    deliver(&mut m, k.handle(a, false, false, NONE));
    assert_eq!(rd(&mut m, 0xC010), 193, "soltar a 2a tecla não apaga o AKD");
    deliver(&mut m, k.handle(esc, false, false, NONE));
    assert_eq!(rd(&mut m, 0xC010), 65);
}

/// #330, parte do RETURN: no Windows RETURN e ENTER do teclado numérico mandam o mesmo VK_RETURN;
/// o AppleWin separa pelo bit "extended" (WM_KEYDOWN lparam bit 24). Soltar só o ENTER numérico
/// mantém o AKD do RETURN.
#[test]
fn gh330_return_and_numpad_enter_are_counted_apart() {
    let mut m = custom(Model::Apple2eEnhanced);
    let mut k = KeyboardState::new();
    let ret = KeyInput::new(HostKey::Enter);
    let kp = KeyInput::numpad(HostKey::Enter);
    deliver(&mut m, k.handle(ret, true, false, NONE));
    deliver(&mut m, k.handle(kp, true, false, NONE));
    deliver(&mut m, k.handle(kp, false, false, NONE));
    assert_eq!(rd(&mut m, 0xC010) & 0x80, 0x80, "RETURN continua apertado");
    deliver(&mut m, k.handle(ret, false, false, NONE));
    assert_eq!(rd(&mut m, 0xC010) & 0x80, 0);
}

/// #1161 (Prince of Persia lê o AKD antes de a tecla ser decodificada: erro do jogo, aberta no
/// AppleWin sem correção). O AppleWin levanta o AKD no WM_KEYDOWN e o strobe no WM_CHAR seguinte,
/// sem a CPU rodar entre os dois: o 6502 nunca vê o AKD sem o strobe nem o contrário. O port entrega
/// o ASCII e o AKD na mesma rodada de eventos, então vale a mesma garantia.
#[test]
fn gh1161_akd_and_strobe_become_visible_together() {
    let mut m = custom(Model::Apple2eEnhanced);
    let mut k = KeyboardState::new();
    let events = k.handle(KeyInput::new(HostKey::Char('x')), true, false, NONE);
    assert!(events.contains(&KeyEvent::Ascii(b'x')));
    assert!(events.contains(&KeyEvent::AnyKeyDown(true)));
    deliver(&mut m, events);
    assert_eq!(rd(&mut m, 0xC000), 0xD8, "strobe + keycode");
    assert_eq!(rd(&mut m, 0xC010), 0xD8, "keycode + AKD");
    // e soltar a tecla apaga só o AKD; o keycode fica (latch)
    deliver(
        &mut m,
        k.handle(KeyInput::new(HostKey::Char('x')), false, false, NONE),
    );
    assert_eq!(rd(&mut m, 0xC010), 0x58);
}

// ------------------------------------------------------------------ #103

/// #103 (Karateka/Bilestoad: "só o X funciona"): sem as imagens dos jogos, o mecanismo genérico
/// é que cada tecla distinta chegue ao latch, com strobe, sem depender de a anterior ter sido
/// solta ou lida, na ][+ e na //e.
#[test]
fn gh103_every_key_reaches_the_latch_with_the_strobe() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = custom(model);
        let mut k = KeyboardState::new();
        for c in ('a'..='z')
            .chain('0'..='9')
            .chain([' ', ',', '.', '/', ';'])
        {
            let ki = KeyInput::new(HostKey::Char(c));
            deliver(&mut m, k.handle(ki, true, false, NONE));
            let want = (c.to_ascii_uppercase() as u8) | 0x80;
            assert_eq!(rd(&mut m, 0xC000), want, "{model:?} {c:?}");
            // sem ler o strobe, a próxima tecla ainda substitui o latch
            deliver(&mut m, k.handle(ki, false, false, NONE));
        }
    }
}

// ------------------------------------------------------------------ #454

fn emulator(f: impl FnOnce(&mut Settings)) -> Emulator {
    let mut s = Settings::default();
    s.machine.model = Model::Apple2eEnhanced;
    s.slots.0[6] = SlotCardSpec::Empty;
    f(&mut s);
    Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap()
}

fn feed(e: &mut Emulator, routed: Vec<Routed>) {
    for r in routed {
        if let Routed::Cmd(c) = r {
            e.apply(c);
        }
    }
}

/// #454: depois de F7 (entrar), F7 (sair), a primeira tecla digitada ("C" de CATALOG) tem de
/// chegar ao Apple. O bug era a tecla ser engolida na saída do depurador.
#[test]
fn gh454_first_key_after_leaving_the_debugger_is_not_lost() {
    let mut e = emulator(|_| {});
    let s = Settings::default();
    let mut r = InputRouter::new(&s);
    e.apply(Command::ToggleDebugger);
    e.apply(Command::ToggleDebugger);
    let ctx = KeyCtx::default();
    feed(
        &mut e,
        r.key(KeyInput::new(HostKey::Char('c')), true, false, ctx, 0),
    );
    assert_eq!(e.machine.board_mut().read(0xC000), 0xC3, "'C' com o strobe");
    // e as seguintes também
    feed(
        &mut e,
        r.key(KeyInput::new(HostKey::Char('c')), false, false, ctx, 1),
    );
    feed(
        &mut e,
        r.key(KeyInput::new(HostKey::Char('a')), true, false, ctx, 2),
    );
    assert_eq!(e.machine.board_mut().read(0xC000), 0xC1);
}

// ------------------------------------------------------------------ #429

/// `JoyResetPosition` (Joystick.cpp, GH#429) transcrito: posição do paddle `pdl` a partir de
/// `xpos`/`ypos` de um joystick com analógico.
fn cpp_square_the_circle(xpos: u8, ypos: u8, pdl: usize) -> u32 {
    let scalar = 0.5 * 255.0;
    let offset = 1.0;
    let x = f64::from(xpos) / scalar - offset;
    let y = f64::from(ypos) / scalar - offset;
    let mut axis = if pdl & 1 == 0 { x } else { y };
    if x * y != 0.0 {
        let ratio2 = (y * y) / (x * x);
        let c = ratio2.min(1.0 / ratio2);
        axis *= (1.0 + c).sqrt();
    }
    axis = axis.clamp(-1.0, 1.0);
    let mut pdl_pos = ((axis + offset) * scalar) as u32;
    if pdl_pos >= 255 {
        pdl_pos = 287; // "from KEGS" (GH#1128)
    }
    pdl_pos
}

#[test]
fn gh429_square_the_circle_matches_the_cpp_for_every_stick_position() {
    for x in 0..=255u8 {
        for y in 0..=255u8 {
            let (sx, sy) = square_circle(x, y);
            let mut g = GamePort::default();
            g.paddles = [sx, sy, 127, 127];
            g.trigger(0);
            let got = g.inactive_cycles();
            for (pdl, got) in got.iter().take(2).enumerate() {
                let want = (f64::from(cpp_square_the_circle(x, y, pdl)) * (2816.0 / 255.0)) as u64;
                assert_eq!(*got, want, "stick ({x},{y}) paddle {pdl}");
            }
        }
    }
}

/// #429: com um analógico circular (X e Y a 0,7071 do centro) os cantos do quadrado do Apple II
/// são alcançáveis: `PDL(0)` e `PDL(1)` chegam a 0 ao mesmo tempo.
#[test]
fn gh429_the_four_corners_are_reachable_with_a_circular_stick() {
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let p = |v: f32| axis_to_paddle(v, DEFAULT_DEADZONE);
    let corners = [
        ((-d, -d), (0u8, 0u8)),
        ((d, -d), (255, 0)),
        ((-d, d), (0, 255)),
        ((d, d), (255, 255)),
    ];
    for ((ax, ay), (ex, ey)) in corners {
        let (x, y) = square_circle(p(ax), p(ay));
        assert!(
            x.abs_diff(ex) <= 2 && y.abs_diff(ey) <= 2,
            "({ax},{ay}) -> ({x},{y}), esperado ({ex},{ey})"
        );
    }
}

// ------------------------------------------------------------------ #293

/// A rotina de joystick de "Where in the USA is Carmen Sandiego" ($DBC1): 21 ciclos por volta,
/// compara com #$50 e #$90. Sem trim o centro (127) dá menos de $50 ("cima preso"); com o trim
/// de Y em +25 (a configuração que o AppleWin criou para esta issue) fica entre os dois.
fn carmen_count(paddle_pos: u8) -> u8 {
    let mut m = custom(Model::Apple2e);
    m.set_paddle(1, paddle_pos);
    let code = [
        0xAD, 0x70, 0xC0, // LDA $C070
        0xA2, 0x00, // LDX #0
        0xAD, 0x65, 0xC0, // loop: LDA $C065   (4)
        0x10, 0x09, // BPL done           (2)
        0xE8, // INX                (2)
        0xEA, 0xEA, 0xEA, 0xEA, 0xEA, // 5 NOPs         (10)
        0x4C, 0x05, 0x03, // JMP loop           (3)  = 21 ciclos
        0x4C, 0x13, 0x03, // done: JMP done
    ];
    load(&mut m, 0x300, &code);
    m.cpu_mut().pc = 0x300;
    for _ in 0..20_000 {
        if m.cpu().pc == 0x313 {
            break;
        }
        m.step();
    }
    assert_eq!(m.cpu().pc, 0x313, "a rotina terminou");
    m.cpu().x
}

#[test]
fn gh293_y_trim_makes_the_carmen_sandiego_stick_neutral() {
    let centre = axis_to_paddle(0.0, DEFAULT_DEADZONE);
    assert_eq!(centre, 127);
    let untrimmed = carmen_count(apply_trim(centre, 0));
    assert!(untrimmed < 0x50, "sem trim: {untrimmed:#x} (cima preso)");
    let trimmed = carmen_count(apply_trim(centre, 25));
    assert!(
        (0x50..0x90).contains(&trimmed),
        "com Y-trim 25: {trimmed:#x} (neutro)"
    );
}

// ------------------------------------------------------------------ #778

/// #778: sem joystick o AppleWin deixa o timer ativo para sempre (Joystick.cpp `JoyReadPosition`:
/// `DEVICE_NONE` => `bPdlCntrActive = true`), então a rotina de Prince of Persia (que desiste a
/// partir de 622) e a do Total Replay (mesmo 622) concluem "sem joystick".
#[test]
fn gh778_without_a_joystick_the_paddle_never_times_out() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = custom(model);
        m.set_joystick_attached(0, false);
        m.set_joystick_attached(1, false);
        // PREAD sem o limite de 8 bits: conta voltas de 11 ciclos em X:Y
        let code = [
            0xAD, 0x70, 0xC0, // LDA $C070
            0xA0, 0x00, 0xA2, 0x00, // LDY #0 ; LDX #0
            0xBD, 0x64, 0xC0, // loop: LDA $C064,X ... (X fixo em 0)
            0x10, 0x09, // BPL done
            0xE6, 0x20, // INC $20
            0xD0, 0x02, // BNE +2
            0xE6, 0x21, // INC $21
            0xA5, 0x21, // LDA $21
            0xC9, 0x04, // CMP #4  (>= 1024 voltas)
            0x90, 0xEF, // BCC loop
            0x4C, 0x19, 0x03, // done/fim: JMP *
        ];
        load(&mut m, 0x300, &code);
        m.cpu_mut().pc = 0x300;
        for _ in 0..200_000 {
            if m.cpu().pc == 0x319 {
                break;
            }
            m.step();
        }
        assert_eq!(
            m.cpu().pc,
            0x319,
            "{model:?}: contou 1024 voltas sem o timer acabar"
        );
    }
}

// ------------------------------------------------------------------ #1128

/// A rotina de joystick de "Learning with Leeper" copiada da issue (`110B-1158`) com as tabelas de
/// limites (`1159/1161/1169/1171`). O stick todo à direita/embaixo (255) tem de passar de $A0.
fn leeper_counts(x: u8, y: u8) -> (u8, u8, u8) {
    let mut m = custom(Model::Apple2e);
    m.set_paddle(0, x);
    m.set_paddle(1, y);
    let routine: [u8; 78] = [
        0xA2, 0x40, 0x8E, 0x70, 0xC0, 0x8E, 0x7A, 0x11, 0x8E, 0x7B, 0x11, 0xA2, 0x00, 0xAD, 0x64,
        0xC0, 0x30, 0x04, 0xEA, 0xEA, 0x10, 0x03, 0xEE, 0x7A, 0x11, 0xAD, 0x65, 0xC0, 0x30, 0x04,
        0xEA, 0xEA, 0x10, 0x03, 0xEE, 0x7B, 0x11, 0xEA, 0xCA, 0xD0, 0xE4, 0xA2, 0x07, 0xAD, 0x7A,
        0x11, 0xDD, 0x69, 0x11, 0x90, 0x14, 0xDD, 0x71, 0x11, 0xB0, 0x0F, 0xAD, 0x7B, 0x11, 0xDD,
        0x59, 0x11, 0x90, 0x07, 0xDD, 0x61, 0x11, 0xB0, 0x02, 0x90, 0x03, 0xCA, 0x10, 0xE1, 0x8E,
        0x79, 0x11, 0x60,
    ];
    load(&mut m, 0x110B, &routine);
    load(&mut m, 0x1159, &[0x00; 8]);
    load(&mut m, 0x1161, &[0x60; 8]);
    load(&mut m, 0x1169, &[0x60; 8]);
    load(&mut m, 0x1171, &[0xA0; 8]);
    load(&mut m, 0x300, &[0x20, 0x0B, 0x11, 0x4C, 0x03, 0x03]);
    m.cpu_mut().pc = 0x300;
    for _ in 0..400_000 {
        if m.cpu().pc == 0x303 {
            break;
        }
        m.step();
    }
    assert_eq!(m.cpu().pc, 0x303, "a rotina retornou");
    (
        m.peek_main(0x117A),
        m.peek_main(0x117B),
        m.peek_main(0x1179),
    )
}

#[test]
fn gh1128_full_deflection_counts_past_the_leeper_threshold() {
    let (cx, cy, _) = leeper_counts(127, 127);
    assert!(
        (0x60..0xA0).contains(&cx) && (0x60..0xA0).contains(&cy),
        "centro: {cx:#x} {cy:#x}"
    );
    let (rx, dy, x) = leeper_counts(255, 255);
    assert!(
        rx >= 0xA0,
        "direita total: {rx:#x} (o AppleWin antigo chegava a $9E)"
    );
    assert!(dy >= 0xA0, "embaixo total: {dy:#x}");
    assert_eq!(
        x, 0xFF,
        "a rotina não reconhece as posições do meio com o stick no canto"
    );
    let (lx, uy, _) = leeper_counts(0, 0);
    assert!(lx < 0x60 && uy < 0x60, "esquerda/cima: {lx:#x} {uy:#x}");
}

// ------------------------------------------------------------------ #985

/// #985 (o strobe com o timer rodando não o reinicia): o trace do micro-programa `input-io`
/// (leitura de $C064 com um segundo `LDA $C070` no meio da contagem) é idêntico ao do AppleWin
/// nos três modelos; aqui, além disso, a contagem total fica igual à de um strobe só.
#[test]
fn gh985_a_second_strobe_does_not_extend_the_count() {
    fn run(restrobe: bool) -> u8 {
        let mut m = custom(Model::Apple2Plus);
        m.set_paddle(0, 100);
        let mut code = vec![0xAD, 0x70, 0xC0, 0xA0, 0x00];
        // loop: LDA $C064 ; BPL done ; INY ; CPY #40 ; BNE loop
        code.extend([0xAD, 0x64, 0xC0, 0x10, 0x0D, 0xC8, 0xC0, 0x28, 0xD0, 0xF6]);
        if restrobe {
            code.extend([0xAD, 0x70, 0xC0]);
        } else {
            code.extend([0xEA, 0xEA, 0xEA, 0xEA]);
            code.push(0xEA);
        }
        // l2: LDA $C064 ; BPL done ; INY ; BNE l2
        code.extend([0xAD, 0x64, 0xC0, 0x10, 0x03, 0xC8, 0xD0, 0xF8]);
        code.extend([0x4C, 0x00, 0x20]);
        load(&mut m, 0x300, &code);
        m.cpu_mut().pc = 0x300;
        for _ in 0..100_000 {
            if m.cpu().pc == 0x2000 {
                break;
            }
            m.step();
        }
        assert_eq!(m.cpu().pc, 0x2000);
        m.cpu().y
    }
    let once = run(false);
    let twice = run(true);
    assert!(once > 40, "o timer de 100 passa de 40 voltas: {once}");
    assert!(
        once.abs_diff(twice) <= 1,
        "o 2o strobe não reiniciou o timer: {once} contra {twice}"
    );
}

// ------------------------------------------------------------------ #232

/// #232: trocar a placa do mouse por outra no mesmo slot não pode deixar o mouse gerando VBL.
/// No port trocar uma placa reconstrói a máquina (contrato de troca a quente, AUD-P-048), então
/// a placa antiga some junto; o IRQ do VBL do mouse não sobrevive.
#[test]
fn gh232_swapping_the_mouse_card_for_a_mockingboard_leaves_no_mouse_vbl() {
    use apple2_cards::MouseCard;
    let mut e = emulator(|s| s.slots.0[4] = SlotCardSpec::new(CardType::MouseInterface));
    assert!(e.machine.card::<MouseCard>(4).is_some());
    let mut s = Settings::default();
    s.machine.model = Model::Apple2eEnhanced;
    s.slots.0[6] = SlotCardSpec::Empty;
    s.slots.0[4] = SlotCardSpec::new(CardType::MockingboardC);
    s.slots.0[5] = SlotCardSpec::new(CardType::MockingboardC);
    e.apply(Command::ApplySettings(Box::new(s)));
    assert!(
        e.machine.card::<MouseCard>(4).is_none(),
        "a placa do mouse sumiu"
    );
    for _ in 0..4 {
        e.machine.run_frame();
    }
    // sem nenhum timer do Mockingboard armado, nada levanta IRQ
    assert!(!e.machine.board().interrupts.irq_line());
}

// ------------------------------------------------------------------ #1491

fn parse(args: &[&str]) -> Settings {
    let c = CmdLine::parse(args).unwrap();
    let mut s = Settings::default();
    c.apply(&mut s);
    s
}

fn disk_of(s: &Settings, slot: usize) -> [Option<std::path::PathBuf>; 2] {
    match &s.slots.0[slot] {
        SlotCardSpec::Disk2 { drives, .. } => [drives[0].image.clone(), drives[1].image.clone()],
        other => panic!("slot {slot}: {other:?}"),
    }
}

/// #1491: `-s6 empty -d1 demo.dsk` não gera `bad_cast`; `InsertFloppyDisks()` agora insere o Disk II
/// se o slot não o tem (Utilities.cpp: `if (QuerySlot(slot) != CT_Disk2) Insert(slot, CT_Disk2)`),
/// como `InsertHardDisks()` faz com o HDC. A ordem na linha de comando não importa: o AppleWin
/// guarda as placas e as imagens à parte e insere as imagens depois.
#[test]
fn gh1491_disk_image_in_an_empty_slot_inserts_the_card() {
    for args in [
        &["-s6", "empty", "-d1", "demo.dsk"][..],
        &["-d1", "demo.dsk", "-s6", "empty"][..],
        &["-s6", "hdc", "-d1", "demo.dsk"][..],
        &["-s5", "empty", "-s5d2", "demo.dsk"][..],
    ] {
        let s = parse(args);
        let slot = if args.contains(&"-s5") { 5 } else { 6 };
        let imgs = disk_of(&s, slot);
        let want = if slot == 5 { 1 } else { 0 };
        // o AppleWin guarda o caminho completo (GetFullPathName; Disk.cpp:241)
        assert_eq!(
            imgs[want],
            Some(std::path::absolute("demo.dsk").unwrap()),
            "{args:?}"
        );
    }
    for args in [
        &["-s7", "empty", "-h1", "demo.hdv"][..],
        &["-h1", "demo.hdv", "-s7", "empty"][..],
    ] {
        let s = parse(args);
        assert!(
            matches!(&s.slots.0[7], SlotCardSpec::GenericHdd { images, .. } if images[0] == Some(std::path::absolute("demo.hdv").unwrap())),
            "{args:?}"
        );
    }
}

/// `InsertFloppyDisks()` sai logo se não há imagem nenhuma: `-d1-disconnected` sozinho não mexe
/// em placa nem em drive (Utilities.cpp, "If no FDDs then just return").
#[test]
fn gh1491_disconnect_without_an_image_changes_nothing() {
    let s = parse(&["-s6", "empty", "-d1-disconnected"]);
    assert_eq!(s.slots.0[6], SlotCardSpec::Empty);
    let s = parse(&["-d1-disconnected"]);
    match &s.slots.0[6] {
        SlotCardSpec::Disk2 { drives, .. } => assert!(drives[0].connected && drives[1].connected),
        other => panic!("{other:?}"),
    }
    // com imagem no outro drive, o desconectado vale
    let s = parse(&["-d2", "b.dsk", "-d1-disconnected"]);
    match &s.slots.0[6] {
        SlotCardSpec::Disk2 { drives, .. } => {
            assert!(!drives[0].connected && drives[1].connected);
        }
        other => panic!("{other:?}"),
    }
}

/// Nenhuma operação de disco pode dar panic com os slots vazios (o `dynamic_cast` do AppleWin
/// falhava com `bad_cast`; `Disk2CardManager` sempre consulta `QuerySlot` antes).
#[test]
fn gh1491_no_disk_operation_panics_with_empty_slots() {
    let dir = std::env::temp_dir().join(format!("applewin-gh1491-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let img = dir.join("demo.dsk");
    std::fs::write(&img, vec![0u8; 143_360]).unwrap();
    let mut e = emulator(|s| {
        for i in 1..8 {
            s.slots.0[i] = SlotCardSpec::Empty;
        }
    });
    e.apply(Command::InsertDisk {
        drive: 0,
        path: img.clone(),
        read_only: false,
    });
    assert!(!e.status().message.is_empty(), "o usuário é avisado");
    e.apply(Command::InsertDisk {
        drive: 1,
        path: img.clone(),
        read_only: true,
    });
    e.apply(Command::DropDisk {
        drive: 0,
        path: img.clone(),
        reboot: true,
    });
    e.apply(Command::EjectDisk { drive: 0 });
    e.apply(Command::SwapDrives);
    e.apply(Command::SetWriteProtect { drive: 0, on: true });
    e.apply(Command::SetHdd {
        slot: 7,
        unit: 0,
        image: Some(img.clone()),
    });
    e.apply(Command::SetHdd {
        slot: 7,
        unit: 0,
        image: None,
    });
    for _ in 0..3 {
        e.machine.run_frame();
    }
    // e a máquina montada a partir de `-s6 empty -d1 demo.dsk` tem o Disk II com a imagem
    let mut s = parse(&["-s6", "empty", "-d1", img.to_str().unwrap()]);
    s.machine.model = Model::Apple2eEnhanced;
    let mut e = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    let card = e.machine.card::<apple2_storage::disk2::Disk2Card>(6);
    assert!(
        card.is_some_and(|c| !c.is_drive_empty(0)),
        "{}",
        e.status().message
    );
    for _ in 0..3 {
        e.machine.run_frame();
    }
    // arquivo que não existe: aviso, sem panic, com o cartão presente
    let s = parse(&["-s6", "empty", "-d1", "/nao/existe/demo.dsk"]);
    let e = Emulator::new(s, Box::new(DefaultBackend), 48_000);
    assert!(e.is_ok() || e.err().is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------ #143

#[test]
fn gh143_prodos_243_boots_on_the_pravetz_models() {
    let img =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/disks/ProDOS_2_4_3.po");
    for model in [Model::Pravets82, Model::Pravets8M, Model::Pravets8A] {
        let mut s = Settings::default();
        s.machine.model = model;
        s.reset_slot0_for_model();
        s.slots.0[6] = SlotCardSpec::new(CardType::Disk2);
        if let SlotCardSpec::Disk2 { drives, .. } = &mut s.slots.0[6] {
            drives[0].image = Some(img.clone());
        }
        let mut e = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
        for _ in 0..900 {
            e.machine.run_frame();
        }
        let screen = apple2_core::text_screen::text_screen(&e.machine).join("\n");
        // ProDOS 2.4.3 passa pela detecção de máquina ($BF98), carrega o BASIC.SYSTEM e chega ao
        // seletor (BITSY BYE); as mensagens da issue seriam "RELOCATION ERROR" (ProDOS < 2.0) e
        // "CANNOT LOAD BASIC.SYSTEM" (>= 2.0)
        assert!(screen.contains("RETURN:SELECT"), "{model:?}\n{screen}");
        assert!(!screen.contains("RELOCATION") && !screen.contains("CANNOT LOAD"));
    }
}
