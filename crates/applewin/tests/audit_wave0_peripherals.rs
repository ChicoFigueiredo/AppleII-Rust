//! Auditoria wave0 (periféricos, entrada, debugger e UI do frontend).
//!
//! Cada teste cita o AUD-P-NNN de `docs/conformance/audit/wave0-peripherals.md`. Os testes que
//! passam documentam comportamento correto (PASS); os `#[ignore]` reproduzem um achado e falham
//! enquanto o código de produção não for corrigido (`cargo test -- --ignored`).
//! Oráculo: AppleWin a8f0d1d.

use apple2_config::card::{AyChip, SoundSockets};
use apple2_config::settings::{Joystick0Kind, Joystick1Kind};
use apple2_config::{CardType, Settings, SlotCardSpec};
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::{Command, Emulator};
use applewin_shell::joy::{JoyConfig, JoySystem};
use applewin_shell::keymap::{debug_key, DbgKey, DbgMods, DebugKey, HostKey, Mods};

fn base_settings() -> Settings {
    let mut s = Settings::default();
    s.slots.0[6] = SlotCardSpec::Empty;
    s
}

fn src(file: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(file);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

// ---------------------------------------------------------------- Mockingboard / Phasor

/// HU-0501. AppleWin tem um tipo de AY por soquete (`SetSocketAY891x`, Mockingboard.cpp:136-151;
/// `-sN ay-socketN`). O Rust só leva o soquete 0 para a carta (`factory::sockets` descarta 1-3).
#[test]
#[ignore = "reproduz achado AUD-P-006"]
fn aud_p_006_ay_type_of_each_socket_reaches_the_card() {
    let a = SoundSockets {
        ay: [AyChip::Ay8910; 4],
        ..Default::default()
    };
    let b = SoundSockets {
        ay: [AyChip::Ay8910, AyChip::Empty, AyChip::Ym2149, AyChip::Empty],
        ..Default::default()
    };
    assert_ne!(
        applewin_shell::factory::sockets(&a),
        applewin_shell::factory::sockets(&b),
        "soquetes 1, 2 e 3 do AY são ignorados"
    );
}

/// HU-0501. `-sN 6522a-bad` / `6522b-bad` (CmdLine.cpp:254-260 -> `SY6522::InitBadState`,
/// 6522.cpp:355: o DDRB lido perde o bit 0). O flag é lido pela linha de comando e descartado.
#[test]
#[ignore = "reproduz achado AUD-P-007"]
fn aud_p_007_bad_6522_flag_reaches_the_card() {
    let a = SoundSockets::default();
    let b = SoundSockets {
        bad_6522: [true, true],
        ..Default::default()
    };
    assert_ne!(
        applewin_shell::factory::sockets(&a),
        applewin_shell::factory::sockets(&b)
    );
}

// ---------------------------------------------------------------- SNES MAX / 4Play / ThunderClock

fn all_shell_sources() -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = String::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out += &std::fs::read_to_string(&p).unwrap();
            }
        }
    }
    out
}

/// HU-0602. A SNES MAX só lê controles pelo `SnesMax::set_controller` (SNESMAX.cpp:110-128 lê
/// `joyGetPosEx` direto). O frontend agora o chama (`emu.rs`, `Command::HostPads`); o
/// comportamento de ponta a ponta (gamepad -> carta -> programa) está em
/// `tests/hu0702_input.rs::aud_p_017_*`. Aqui só o resto do achado: os flags
/// `-snes-max-alt-joy*`/`-user-joy*` ficam em `SlotCardSpec::SnesMax` e a fábrica faz
/// `SnesMax { .. }` (ignora os dois) - a cargo do agente de factory.rs.
#[test]
#[ignore = "reproduz achado AUD-P-017 (flags da fábrica)"]
fn aud_p_017_snes_max_flags_reach_the_factory() {
    let all = all_shell_sources();
    assert!(
        all.contains("set_controller("),
        "nenhum gamepad do host alimenta a SNES MAX"
    );
    let factory = src("factory.rs");
    assert!(
        !factory.contains("SlotCardSpec::SnesMax { .. }"),
        "a fábrica descarta alt_controller e user_mapping"
    );
}

/// HU-0602/0702. A 4Play só recebe estado por `FourPlay::set_stick`; o frontend agora o chama
/// (`emu.rs`). O teste de ponta a ponta está em `tests/hu0702_input.rs::aud_p_018_*`.
#[test]
fn aud_p_018_four_play_gets_sticks() {
    assert!(all_shell_sources().contains("set_stick("));
}

/// HU-0602. O ThunderClock existe em `apple2-cards`, mas não há `CardType` para ele. No AppleWin
/// o ThunderClock também não é selecionável (Card.h:15-42 não tem `CT_ThunderClock`;
/// Peripheral_Clock_ThunderClockPlus.cpp é esqueleto comentado): paridade com o oráculo.
#[test]
fn aud_p_015_thunderclock_has_no_card_type_like_applewin() {
    let names: Vec<String> = (0..64)
        .filter_map(CardType::from_value)
        .map(|t| format!("{t:?}").to_lowercase())
        .collect();
    assert!(!names.iter().any(|n| n.contains("thunder")));
    assert_eq!(names.len(), 26, "Card.h tem 26 tipos incluindo Empty");
}

/// HU-0602. `Emulator::new` aceita configurações que o diálogo recusa (`settings::model::validate`):
/// dois cartões de mouse, Uthernet + Uthernet II. Sem erro nem aviso.
#[test]
#[ignore = "reproduz achado AUD-P-019"]
fn aud_p_019_emulator_rejects_conflicting_slot_configuration() {
    let mut s = base_settings();
    s.slots.0[5] = SlotCardSpec::new(CardType::MouseInterface);
    s.slots.0[7] = SlotCardSpec::new(CardType::MouseInterface);
    assert!(
        Emulator::new(s, Box::new(DefaultBackend), 48_000).is_err(),
        "dois Mouse Interface devem ser recusados"
    );
}

/// HU-0602. Cartão que não é Language Card no slot 0 do ][+ some em silêncio.
#[test]
#[ignore = "reproduz achado AUD-P-020"]
fn aud_p_020_invalid_card_in_slot_0_is_reported() {
    let mut s = base_settings();
    s.slots.0[0] = SlotCardSpec::new(CardType::Ssc);
    let r = Emulator::new(s, Box::new(DefaultBackend), 48_000);
    let e = match r {
        Ok(e) => e,
        Err(_) => return, // recusar também é aceitável
    };
    let built = e.machine.board().slots.card(0).map(|c| c.name());
    assert!(
        built.is_some() || e.status().message.to_lowercase().contains("slot 0"),
        "o SSC do slot 0 sumiu sem aviso"
    );
}

// ---------------------------------------------------------------- Entrada

fn joy(f: impl FnOnce(&mut Settings)) -> JoySystem {
    let mut s = Settings::default();
    f(&mut s);
    JoySystem::new(JoyConfig::from_settings(&s))
}

/// HU-0702, #1522: com "Mouse" como joystick, o primeiro PREAD não pode dar $FF (o AppleWin
/// 1.32.1.0 centra em 127 no início).
#[test]
fn aud_p_025_mouse_joystick_starts_centred_gh1522() {
    let mut j = joy(|s| s.joystick.joystick0 = Joystick0Kind::Mouse);
    let f = j.output(0);
    assert_eq!((f.out[0].x, f.out[0].y), (127, 127));
}

/// HU-0702, #1234: II/II+ com PB0/PB1 flutuando alto sem joystick 1; PB2 flutua alto em todos.
#[test]
fn aud_p_026_disconnected_buttons_float_high_gh1234() {
    use apple2_core::game_port::GamePort;
    let mut g = GamePort::default();
    g.attached = [false, false];
    for n in 0..3 {
        assert!(g.button(n, true), "][+: PB{n} flutua alto sem joystick");
    }
    assert!(
        !g.button(0, false) && !g.button(1, false),
        "//e: PB0/PB1 baixos"
    );
    assert!(g.button(2, false), "//e: PB2 flutua alto");
    g.attached = [true, true];
    assert!(!g.button(0, true) && !g.button(2, true));
}

/// HU-0702. "SHIFT key mod" sem joystick 1: no AppleWin `joyinfo[joytype[1]]` continua
/// DEVICE_NONE, então os paddles 2/3 seguem "sempre ativos" (Joystick.cpp:695-697) e só o PB2
/// segue o Shift (:648-653). O Rust marca `attached[1] = true` (joy.rs:644) e o timer dos paddles
/// 2/3 passa a contar como se houvesse joystick.
#[test]
fn aud_p_027_shift_key_mod_does_not_attach_joystick_1() {
    let mut j = joy(|s| {
        s.joystick.joystick1 = Joystick1Kind::Disabled;
        s.joystick.shift_key_mod = true;
    });
    let f = j.output(0);
    assert!(!f.attached[1]);
    assert_eq!(
        f.pb2_shift,
        Some(true),
        "PB2 segue o Shift (flutua alto sem ele)"
    );
}

/// HU-0702, AUD-P-027 de ponta a ponta: sem joystick 1 os timers dos paddles 2/3 seguem
/// "sempre ativos" e o PB2 segue o Shift.
#[test]
fn aud_p_027_paddle_timers_stay_always_active_with_shift_key_mod() {
    let mut s = base_settings();
    s.joystick.shift_key_mod = true;
    s.joystick.joystick1 = Joystick1Kind::Disabled;
    let mut e = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    let mut j = joy(|s| {
        s.joystick.joystick1 = Joystick1Kind::Disabled;
        s.joystick.shift_key_mod = true;
    });
    let f = j.output(0);
    e.apply(Command::JoyAttached {
        index: 1,
        attached: f.attached[1],
    });
    e.apply(Command::JoyPb2Shift(f.pb2_shift));
    let g = &e.machine.board().game;
    assert!(!g.attached[1], "joystick 1 continua ausente");
    // paddle 2 ativo para sempre, mesmo depois do strobe e de muito tempo
    assert!(g.paddle_active(2, 10_000_000));
    assert!(g.button(2, false), "sem Shift o PB2 flutua alto");
    j.set_shift(true);
    e.apply(Command::JoyPb2Shift(j.output(0).pb2_shift));
    assert!(
        !e.machine.board().game.button(2, false),
        "Shift puxa o PB2 para baixo"
    );
}

// ---------------------------------------------------------------- Serial

#[cfg(feature = "serialport")]
#[test]
fn aud_p_030_missing_serial_device_does_not_crash() {
    use apple2_hostio::serial::{HostSerialPort, SerialBackend};
    let mut p = HostSerialPort::new("/dev/ttyUSB-inexistente-wave0");
    assert!(!p.open());
    assert!(!p.is_active());
    assert!(!p.write(0x41));
}

// ---------------------------------------------------------------- Debugger (teclas)

fn dbg_emulator() -> Emulator {
    Emulator::new(base_settings(), Box::new(DefaultBackend), 48_000).unwrap()
}

fn press(e: &mut Emulator, k: DbgKey, m: DbgMods) {
    e.apply(Command::Debug(DebugKey::Press(k, m)));
}

fn plain(e: &mut Emulator, k: DbgKey) {
    e.apply(Command::Debug(DebugKey::plain(k)));
}

fn dbg_running() -> Emulator {
    let mut e = dbg_emulator();
    for _ in 0..120 {
        e.run_frame();
    }
    e.apply(Command::ToggleDebugger);
    e
}

/// HU-0801. `DebuggerProcessKey` (Debug.cpp:9848-9867): Espaço com a linha de comando vazia =
/// Trace (`CmdTrace`), Shift+Espaço = `CmdStepOut`, Ctrl+Espaço = `CmdStepOver`.
#[test]
fn aud_p_035_space_traces_one_instruction_in_the_debugger() {
    let mut e = dbg_running();
    let pc = e.machine.cpu().pc;
    e.apply(Command::Debug(DebugKey::Char(' ')));
    assert_ne!(e.machine.cpu().pc, pc, "Espaço deveria dar um passo");
    // via tradução do teclado (Espaço do winit chega como HostKey::Char(' '))
    let pc = e.machine.cpu().pc;
    let k = debug_key(HostKey::Char(' '), Mods::default()).expect("Espaço");
    e.apply(Command::Debug(k));
    assert_ne!(e.machine.cpu().pc, pc);
    // com texto digitado o Espaço é caractere, não passo
    e.apply(Command::Debug(DebugKey::Char('R')));
    let pc = e.machine.cpu().pc;
    e.apply(Command::Debug(DebugKey::Char(' ')));
    assert_eq!(e.machine.cpu().pc, pc);
}

/// HU-0801. Teclas de navegação do depurador têm tradução no frontend (`keymap::debug_key`).
#[test]
fn aud_p_035_navigation_keys_are_translated() {
    let none = Mods::default();
    for k in [
        HostKey::Tab,
        HostKey::Home,
        HostKey::End,
        HostKey::Left,
        HostKey::Right,
        HostKey::Up,
        HostKey::Down,
        HostKey::PageUp,
        HostKey::PageDown,
        HostKey::Enter,
        HostKey::Backspace,
        HostKey::Escape,
    ] {
        assert!(debug_key(k, none).is_some(), "{k:?} sem ação no depurador");
    }
    let ctrl = Mods {
        ctrl: true,
        ..Default::default()
    };
    assert!(
        debug_key(HostKey::Right, ctrl).is_some(),
        "Ctrl-Right (PC := cursor)"
    );
    for d in '0'..='9' {
        assert_eq!(
            debug_key(HostKey::Char(d), ctrl),
            Some(DebugKey::Press(
                DbgKey::Char(d),
                DbgMods {
                    ctrl: true,
                    ..Default::default()
                }
            )),
            "Ctrl+{d}"
        );
    }
    // Ctrl+Shift+1 chega como '!' num teclado americano; o AppleWin olha o código da tecla
    let cs = Mods {
        ctrl: true,
        shift: true,
        alt: false,
    };
    assert!(matches!(
        debug_key(HostKey::Char('!'), cs),
        Some(DebugKey::Press(DbgKey::Char('1'), _))
    ));
    // Ctrl+~ alterna o console em tela cheia
    assert!(debug_key(HostKey::Char('~'), ctrl).is_some());
}

/// HU-0801. Setas, Tab e Home/End mexem no cursor do disassembly (Debug.cpp:9830-10066).
#[test]
fn aud_p_035_navigation_keys_move_the_cursor() {
    let mut e = dbg_running();
    let pc = e.machine.cpu().pc;
    assert_eq!(e.debugger().disasm_cur, pc);
    plain(&mut e, DbgKey::Home);
    assert_eq!(e.debugger().disasm_cur, 0x0000);
    plain(&mut e, DbgKey::End);
    assert_eq!(e.debugger().disasm_cur, 0x8000);
    plain(&mut e, DbgKey::Tab);
    assert_eq!(e.debugger().disasm_cur, pc);
    // Shift+Direita: topo; Alt+Direita: centro
    plain(&mut e, DbgKey::Home);
    press(
        &mut e,
        DbgKey::Right,
        DbgMods {
            shift: true,
            ..Default::default()
        },
    );
    assert_eq!(e.debugger().disasm_cur, pc);
    assert_eq!(e.debugger().disasm_cur_line, 0);
    plain(&mut e, DbgKey::Home);
    press(
        &mut e,
        DbgKey::Right,
        DbgMods {
            alt: true,
            ..Default::default()
        },
    );
    assert_eq!(e.debugger().disasm_cur, pc);
    assert!(e.debugger().disasm_cur_line > 0);
    // Ctrl+Direita: PC := cursor
    plain(&mut e, DbgKey::Down);
    let cur = e.debugger().disasm_cur;
    assert_ne!(cur, pc);
    press(
        &mut e,
        DbgKey::Right,
        DbgMods {
            ctrl: true,
            ..Default::default()
        },
    );
    assert_eq!(e.machine.cpu().pc, cur);
}

/// HU-0801. Ctrl+Shift+n cria o marcador n no cursor, Ctrl+n vai até ele (`CmdBookmarkAdd/Goto`).
#[test]
fn aud_p_035_ctrl_digits_are_bookmarks() {
    let mut e = dbg_running();
    let pc = e.machine.cpu().pc;
    let ctrl = DbgMods {
        ctrl: true,
        ..Default::default()
    };
    let cs = DbgMods {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    press(&mut e, DbgKey::Char('3'), cs);
    plain(&mut e, DbgKey::Home);
    press(&mut e, DbgKey::Char('3'), ctrl);
    assert_eq!(e.debugger().disasm_cur, pc);
}

/// HU-0801 / AUD-P-036. Up/Down (linha vazia) movem o cursor do disassembly, como no AppleWin
/// (Debug.cpp:9989-10047); PgUp/PgDn também. Com texto digitado só o console rola.
#[test]
fn aud_p_036_existing_debug_keys() {
    let none = Mods::default();
    assert_eq!(
        debug_key(HostKey::Enter, none),
        Some(DebugKey::plain(DbgKey::Enter))
    );
    assert_eq!(
        debug_key(HostKey::Escape, none),
        Some(DebugKey::plain(DbgKey::Escape))
    );
    assert_eq!(
        debug_key(HostKey::PageUp, none),
        Some(DebugKey::plain(DbgKey::PageUp))
    );
    assert_eq!(
        debug_key(HostKey::Up, none),
        Some(DebugKey::plain(DbgKey::Up))
    );
    let mut e = dbg_running();
    let cur = e.debugger().disasm_cur;
    plain(&mut e, DbgKey::Down);
    let down = e.debugger().disasm_cur;
    assert!(down > cur, "Baixo desce uma instrução");
    plain(&mut e, DbgKey::Up);
    assert_eq!(e.debugger().disasm_cur, cur, "Cima volta");
    plain(&mut e, DbgKey::PageDown);
    assert_ne!(e.debugger().disasm_cur, cur, "PgDn move o cursor");
    // com texto, Cima/Baixo não mexem no cursor (o AppleWin só rola o console com Shift)
    e.apply(Command::Debug(DebugKey::Char('R')));
    let cur = e.debugger().disasm_cur;
    plain(&mut e, DbgKey::Down);
    plain(&mut e, DbgKey::Up);
    assert_eq!(e.debugger().disasm_cur, cur);
}

/// HU-0801. Enter com a linha vazia alterna o console em tela cheia (Debug.cpp:9788-9799);
/// Esc limpa a linha.
#[test]
fn aud_p_035_empty_enter_toggles_the_full_screen_console() {
    use apple2_debugger::debugger::Window;
    let mut e = dbg_running();
    assert_eq!(e.debugger().window, Window::Code);
    plain(&mut e, DbgKey::Enter);
    assert_eq!(e.debugger().window, Window::Console);
    plain(&mut e, DbgKey::Enter);
    assert_eq!(e.debugger().window, Window::Code);
    e.apply(Command::Debug(DebugKey::Char('R')));
    plain(&mut e, DbgKey::Escape);
    assert!(e.debugger().console.input().is_empty());
}

/// HU-0801. A roda do mouse sobre o depurador é Cima/Baixo com os modificadores (WinFrame.cpp:1647).
#[test]
fn wheel_is_up_and_down_with_modifiers() {
    use applewin_shell::keymap::wheel_debug_key;
    let shift = Mods {
        shift: true,
        ..Default::default()
    };
    assert_eq!(
        wheel_debug_key(1.0, shift),
        Some(DebugKey::Press(
            DbgKey::Up,
            DbgMods {
                shift: true,
                ..Default::default()
            }
        ))
    );
    assert_eq!(
        wheel_debug_key(-3.0, Mods::default()),
        Some(DebugKey::plain(DbgKey::Down))
    );
    assert_eq!(wheel_debug_key(0.0, Mods::default()), None);
}

/// HU-0801. Com o depurador na tela, Ctrl+0/1/3 são marcadores, não atalhos de velocidade.
#[test]
fn ctrl_digits_reach_the_debugger_not_the_speed_shortcuts() {
    use applewin_shell::actions::action_for_mode;
    let ctrl = Mods {
        ctrl: true,
        ..Default::default()
    };
    for d in ['0', '1', '3'] {
        assert!(action_for_mode(HostKey::Char(d), ctrl, false).is_some());
        assert!(action_for_mode(HostKey::Char(d), ctrl, true).is_none());
    }
}

// ---------------------------------------------------------------- UI

/// HU-0802. Ctrl+Print Screen: AppleWin copia 40 ou 80 colunas (aux+principal) e a página 2
/// (`Util_GetTextScreen`, Debug.cpp:5271). O comando usa `apple2_video::screen_text`.
#[test]
fn aud_p_040_ctrl_print_screen_copies_80_columns() {
    let mut s = base_settings();
    s.machine.model = apple2_core::Model::Apple2eEnhanced;
    let mut e = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    e.machine.board_mut().video.col80 = true;
    e.machine.poke_main(0x400, 0xC2);
    e.machine.poke_aux(0x400, 0xC1);
    e.apply(Command::CopyScreenText);
    let st = e.status();
    assert_eq!(st.copied_seq, 1);
    let first = st.copied_text.lines().next().unwrap();
    assert!(first.starts_with("AB"), "linha 0 em 80 colunas: {first:?}");
    assert_eq!(first.chars().count(), 80);
    assert_eq!(st.copied_text.lines().count(), 24);
}

/// HU-0802, AUD-P-040: 40 colunas continuam 40, e a página 2 (sem 80STORE) é a copiada.
#[test]
fn aud_p_040_ctrl_print_screen_40_columns_and_page_2() {
    use apple2_core::memory::mf;
    let mut s = base_settings();
    s.machine.model = apple2_core::Model::Apple2eEnhanced;
    let mut e = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    e.machine.poke_main(0x400, 0xC1); // página 1: 'A'
    e.machine.poke_main(0x800, 0xC2); // página 2: 'B'
    e.apply(Command::CopyScreenText);
    let first = e.status().copied_text.lines().next().unwrap().to_string();
    assert!(
        first.starts_with('A') && first.chars().count() == 40,
        "{first:?}"
    );
    // PAGE2 ligado ($C055) sem 80STORE: a página 2 é a que aparece
    let mode = e.machine.memmode();
    e.machine.board_mut().memory.set_memmode(mode | mf::PAGE2);
    e.apply(Command::CopyScreenText);
    let first = e.status().copied_text.lines().next().unwrap().to_string();
    assert!(first.starts_with('B'), "página 2: {first:?}");
}

/// HU-0802. A preferência "Alt+Enter toggles full-screen" (`-alt-enter=...`) existe nas
/// configurações e na página Advanced, mas nenhum código do frontend a consulta: Alt+Enter alterna
/// a tela cheia sempre (`actions::action_for`).
#[test]
fn aud_p_046_alt_enter_setting_is_honoured() {
    let used = all_shell_sources()
        .matches("alt_enter_toggles_fullscreen")
        .count();
    // Só a definição da caixa de seleção (pages.rs) a menciona.
    assert!(
        used > 1,
        "a preferência nunca é lida pelo app (usos: {used})"
    );
}

/// HU-0802. Os atalhos F1-F12 listados na documentação têm ação.
#[test]
fn aud_p_049_listed_shortcuts_have_actions() {
    use applewin_shell::actions::action_for;
    let none = Mods::default();
    for n in 1..=12u8 {
        assert!(action_for(HostKey::F(n), none).is_some(), "F{n}");
    }
    assert!(action_for(HostKey::PrintScreen, none).is_some());
    assert!(action_for(HostKey::Pause, none).is_some());
}
