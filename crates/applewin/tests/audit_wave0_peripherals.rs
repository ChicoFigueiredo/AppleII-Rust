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
use applewin_shell::keymap::{debug_key, DebugKey, HostKey, Mods};

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

/// HU-0602. Parte da fábrica do AUD-P-017: os flags `-snes-max-alt-joy*`/`-user-joy*` ficam em
/// `SlotCardSpec::SnesMax` e a fábrica os aplica à carta (antes fazia `SnesMax { .. }`). O
/// comportamento fino está em `tests/factory_slots.rs`.
#[test]
fn aud_p_017_factory_applies_snes_max_flags() {
    let factory = src("factory.rs");
    assert!(
        !factory.contains("SlotCardSpec::SnesMax { .. }"),
        "a fábrica descarta alt_controller e user_mapping"
    );
    let mut s = base_settings();
    s.slots.0[5] = SlotCardSpec::SnesMax {
        alt_controller: [true, false],
        user_mapping: Default::default(),
    };
    let (m, _) = applewin_shell::factory::build_machine_report(
        &s,
        &apple2_audio::AudioBus::new(48_000, apple2_core::VideoStandard::Ntsc),
    )
    .unwrap();
    let card = m.card::<apple2_cards::SnesMax>(5).unwrap();
    assert!(card.alt_controller(0) && !card.alt_controller(1));
}

/// HU-0602. A outra metade do AUD-P-017: o gamepad do host (gilrs) chamar `SnesMax::set_host_pad`
/// ou `set_controller`. É do agente do frontend (app.rs/joy.rs); segue pendente aqui.
#[test]
#[ignore = "reproduz achado AUD-P-017 (parte do gamepad, fora da fábrica)"]
fn aud_p_017_snes_max_gets_controllers() {
    let all = all_shell_sources();
    assert!(
        all.contains("set_controller(") || all.contains("set_host_pad("),
        "nenhum gamepad do host alimenta a SNES MAX"
    );
}

/// HU-0602/0702. A 4Play só recebe estado por `FourPlay::set_stick`; o frontend nunca o chama.
#[test]
#[ignore = "reproduz achado AUD-P-018"]
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

/// HU-0602. No AppleWin, `CardManager::InsertInternal` deixa vazio o slot da segunda Mouse
/// Interface (o oráculo vence): a Máquina inicia, o slot 7 fica vazio e há aviso visível.
#[test]
fn aud_p_019_emulator_ignores_duplicate_card_with_warning_like_applewin() {
    let mut s = base_settings();
    s.slots.0[5] = SlotCardSpec::new(CardType::MouseInterface);
    s.slots.0[7] = SlotCardSpec::new(CardType::MouseInterface);
    let e = Emulator::new(s, Box::new(DefaultBackend), 48_000).expect("a Máquina inicia");
    assert!(e.machine.board().slots.card(5).is_some());
    assert!(e.machine.board().slots.card(7).is_none());
    assert!(e.status().message.contains("MouseInterface"));
}

/// HU-0602. Cartão que não é Language Card no slot 0 do ][+ some em silêncio.
#[test]
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
#[ignore = "reproduz achado AUD-P-027"]
fn aud_p_027_shift_key_mod_does_not_attach_joystick_1() {
    let mut j = joy(|s| {
        s.joystick.joystick1 = Joystick1Kind::Disabled;
        s.joystick.shift_key_mod = true;
    });
    assert!(!j.output(0).attached[1]);
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

/// HU-0801. `DebuggerProcessKey` (Debug.cpp:9770): Espaço com a linha de comando vazia = Trace
/// (`CmdTrace`). O Rust transforma o Espaço em caractere do console.
#[test]
#[ignore = "reproduz achado AUD-P-035"]
fn aud_p_035_space_traces_one_instruction_in_the_debugger() {
    let mut e = dbg_emulator();
    for _ in 0..120 {
        e.run_frame();
    }
    e.apply(Command::ToggleDebugger);
    let pc = e.machine.cpu().pc;
    e.apply(Command::Debug(DebugKey::Char(' ')));
    assert_ne!(e.machine.cpu().pc, pc, "Espaço deveria dar um passo");
}

/// HU-0801. Teclas de navegação do depurador sem tradução no frontend (`keymap::debug_key`).
#[test]
#[ignore = "reproduz achado AUD-P-035"]
fn aud_p_035_navigation_keys_are_translated() {
    let none = Mods::default();
    for k in [
        HostKey::Tab,
        HostKey::Home,
        HostKey::End,
        HostKey::Left,
        HostKey::Right,
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
}

/// HU-0801. Documenta o que existe: Enter, Backspace, Escape, PgUp/PgDn e histórico em Up/Down.
#[test]
fn aud_p_036_existing_debug_keys() {
    let none = Mods::default();
    assert_eq!(debug_key(HostKey::Enter, none), Some(DebugKey::Enter));
    assert_eq!(debug_key(HostKey::Escape, none), Some(DebugKey::ClearInput));
    assert_eq!(debug_key(HostKey::PageUp, none), Some(DebugKey::ScrollUp));
    // No AppleWin, Up/Down (sem texto digitado) movem o cursor do disassembly (Debug.cpp:9990-10028).
    assert_eq!(debug_key(HostKey::Up, none), Some(DebugKey::HistoryPrev));
}

// ---------------------------------------------------------------- UI

/// HU-0802. Ctrl+Print Screen: AppleWin copia 40 ou 80 colunas (aux+principal) e a página 2
/// (`Util_GetTextScreen`, Debug.cpp:5271). O Rust usa `apple2_core::text_screen`, só 40 colunas
/// da página 1, embora `apple2_video::screen_text` já trate 80 colunas e página 2.
#[test]
#[ignore = "reproduz achado AUD-P-040"]
fn aud_p_040_ctrl_print_screen_copies_80_columns() {
    let mut s = base_settings();
    s.machine.model = apple2_core::Model::Apple2eEnhanced;
    let mut e = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    e.machine.board_mut().video.col80 = true;
    e.machine.poke_main(0x400, 0xC2);
    e.machine.poke_aux(0x400, 0xC1);
    e.apply(Command::CopyScreenText);
    let rows = apple2_core::text_screen::text_screen(&e.machine);
    assert!(
        rows[0].starts_with("AB"),
        "linha 0 em 80 colunas: {:?}",
        rows[0]
    );
}

/// HU-0802. A preferência "Alt+Enter toggles full-screen" (`-alt-enter=...`) existe nas
/// configurações e na página Advanced, mas nenhum código do frontend a consulta: Alt+Enter alterna
/// a tela cheia sempre (`actions::action_for`).
#[test]
#[ignore = "reproduz achado AUD-P-046"]
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
