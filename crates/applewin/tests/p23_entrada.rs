//! Rodada p23 do gate G0: issues do AppleWin de joystick, teclado e mouse (`docs/conformance/results/
//! p23-entrada.csv`). Cada teste cita o C++ do oráculo (AppleWin a8f0d1d) que transcreve ou compara.
//!
//! Arquivos do oráculo (`source/` do AppleWin a8f0d1d): `Joystick.cpp`, `Keyboard.cpp`,
//! `MouseInterface.cpp`, `FourPlay.cpp`, `SNESMAX.cpp`, `CopyProtectionDongles.cpp`,
//! `CmdLine.cpp`, `Configuration/PageInput.cpp`, `Windows/WinFrame.cpp`, `Utilities.cpp`.
//! Os testes marcados com `#[ignore]` reproduzem um FAIL conhecido (a causa está no comentário).

use apple2_cards::snesmax::{buttons as sb, ButtonMap, HostPad};
use apple2_cards::{mouse_button, mouse_move, Dongle, DongleKind, MouseCard, SnesMax};
use apple2_config::settings::{Joystick0Kind, Joystick1Kind};
use apple2_config::{CardType, CmdLine, Settings, SlotCardSpec};
use apple2_core::hooks::GameIoDevice;
use apple2_core::{Machine, Model, NavKey};
use apple2_cpu::Bus;
use applewin_shell::actions::{action_for, action_for_with, Action};
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::{Command, Emulator};
use applewin_shell::factory::parse_controller_mapping_file;
use applewin_shell::input::{InputRouter, KeyCtx, Routed};
use applewin_shell::joy::{
    apply_trim, axis_to_paddle, square_circle, JoyConfig, JoyMode, JoySystem, DEFAULT_DEADZONE,
};
use applewin_shell::keymap::{char_to_ascii, HostKey, KeyEvent, KeyInput, KeyboardState, Mods};
use applewin_shell::settings::model::{joystick0_choices, joystick1_choices, set_model};

const NONE: Mods = Mods {
    shift: false,
    ctrl: false,
    alt: false,
};
const CTRL: Mods = Mods {
    shift: false,
    ctrl: true,
    alt: false,
};
const ALT: Mods = Mods {
    shift: false,
    ctrl: false,
    alt: true,
};
const ALTGR: Mods = Mods {
    shift: false,
    ctrl: true,
    alt: true,
};
const SHIFT: Mods = Mods {
    shift: true,
    ctrl: false,
    alt: false,
};

/// Todos os modelos com ROM (a //c é só um esqueleto no AppleWin).
const MODELS: [Model; 12] = [
    Model::Apple2,
    Model::Apple2Plus,
    Model::Apple2JPlus,
    Model::Apple2e,
    Model::Apple2eEnhanced,
    Model::Pravets8M,
    Model::Pravets82,
    Model::Base64A,
    Model::BadPravets82,
    Model::BadPravets8M,
    Model::Pravets8A,
    Model::Tk3000e,
];

// ------------------------------------------------------------------------------ infraestrutura

fn settings_for(model: Model, f: impl FnOnce(&mut Settings)) -> Settings {
    let mut s = Settings::default();
    set_model(&mut s, model);
    s.slots.0[6] = SlotCardSpec::Empty;
    f(&mut s);
    s
}

/// Emulador e roteador de entrada da mesma configuração, com o estado do joystick já enviado.
fn rig(model: Model, f: impl FnOnce(&mut Settings)) -> (Emulator, InputRouter) {
    let s = settings_for(model, f);
    let mut r = InputRouter::new(&s);
    let mut e = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
    sync(&mut e, &mut r, 0);
    (e, r)
}

fn sync(e: &mut Emulator, r: &mut InputRouter, now: u64) {
    for c in r.update(now) {
        e.apply(c);
    }
}

fn feed(e: &mut Emulator, routed: Vec<Routed>) {
    for r in routed {
        if let Routed::Cmd(c) = r {
            e.apply(c);
        }
    }
}

fn press(e: &mut Emulator, r: &mut InputRouter, input: KeyInput, mods: Mods) {
    let ctx = KeyCtx {
        mods,
        ..KeyCtx::default()
    };
    feed(e, r.key(input, true, false, ctx, 0));
}

fn release(e: &mut Emulator, r: &mut InputRouter, input: KeyInput, mods: Mods) {
    let ctx = KeyCtx {
        mods,
        ..KeyCtx::default()
    };
    feed(e, r.key(input, false, false, ctx, 0));
}

fn rd(e: &mut Emulator, a: u16) -> u8 {
    e.machine.board_mut().read(a)
}

/// Bit 7 de `$C061+n`.
fn pb(e: &mut Emulator, n: u16) -> bool {
    rd(e, 0xC061 + n) & 0x80 != 0
}

fn kb(c: char) -> KeyInput {
    KeyInput::new(HostKey::Char(c))
}

fn np(c: char) -> KeyInput {
    KeyInput::numpad(HostKey::Char(c))
}

fn parse(args: &[&str]) -> Settings {
    let c = CmdLine::parse(args).unwrap();
    let mut s = Settings::default();
    c.apply(&mut s);
    s
}

/// `IsApple2PlusOrClone` (Common.h:234): ][, ][+, ][J-Plus e clones ][.
fn cpp_is_apple2_plus_or_clone(m: Model) -> bool {
    m.value() & (0x10 | 0x20) == 0
}

// ================================================================================== #1234 e a SKM

/// #1234 (commit 6c579da): `-no-joystick1|2` chama `JoyUnplug` (AppleWin.cpp:740, Joystick.cpp:888),
/// que põe o joystick *emulado* em `DEVICE_NONE`. Na família ][ o PB0/PB1 flutuam em nível alto
/// (Joystick.cpp:629-630 e 641-642); na //e continuam baixos.
#[test]
fn gh1234_no_joystick_switch_unplugs_the_emulated_joystick() {
    let s = parse(&["-no-joystick1"]);
    assert_eq!(s.joystick.joystick0, Joystick0Kind::Disabled);
    assert_eq!(s.joystick.joystick1, Joystick1Kind::Disabled);
    let s = parse(&["-no-joystick2"]);
    assert_eq!(s.joystick.joystick1, Joystick1Kind::Disabled);
    assert_ne!(s.joystick.joystick0, Joystick0Kind::Disabled, "só o #2");
    // sem a chave o joystick 0 existe (cai no teclado numérico se não há gamepad)
    let (mut e, _r) = rig(Model::Apple2Plus, |_| {});
    assert!(!pb(&mut e, 0) && !pb(&mut e, 1), "joystick 0 presente");

    for model in MODELS {
        let (mut e, _r) = rig(model, |s| {
            let a = parse(&["-no-joystick1"]);
            s.joystick = a.joystick;
        });
        let want = cpp_is_apple2_plus_or_clone(model);
        assert_eq!(pb(&mut e, 0), want, "{model:?}: PB0 sem joystick 0");
        assert_eq!(pb(&mut e, 1), want, "{model:?}: PB1 sem joystick 0");
    }
}

/// #1234 / #99 / #121: sem joystick 2 o PB2 sobe (`pressed = 1`, Joystick.cpp:656) ou, com o
/// "SHIFT key mod", segue `!Shift` (:654); com joystick 2 volta a ser o botão (:660).
#[test]
fn gh1234_gh99_gh121_pb2_follows_the_shift_key_mod_only_without_joystick_2() {
    for model in MODELS {
        for skm in [false, true] {
            let (mut e, mut r) = rig(model, |s| s.joystick.shift_key_mod = skm);
            assert!(pb(&mut e, 2), "{model:?} skm={skm}: Shift solto, PB2 alto");
            r.joy.set_shift(true);
            sync(&mut e, &mut r, 1);
            assert_eq!(
                pb(&mut e, 2),
                !skm,
                "{model:?} skm={skm}: Shift apertado (:654)"
            );
            r.joy.set_shift(false);
            sync(&mut e, &mut r, 2);
            assert!(pb(&mut e, 2));
        }
    }
    // com o joystick 2 (gamepad 2) o PB2 é o botão e não flutua mais
    let (mut e, mut r) = rig(Model::Apple2Plus, |s| {
        s.joystick.joystick1 = Joystick1Kind::HostJoystick2;
        s.joystick.shift_key_mod = true;
    });
    r.joy.set_pads_connected(2);
    sync(&mut e, &mut r, 1);
    assert!(!pb(&mut e, 2), "joystick 2 presente: botão solto");
    r.joy.pads[1].button(gilrs::Button::South, true);
    sync(&mut e, &mut r, 2);
    assert!(pb(&mut e, 2), "botão do joystick 2 apertado");
}

/// #99 / #121 (PageConfig.cpp:161-167, PageInput.cpp:335-345): ao trocar o computador na
/// configuração a SKM passa ao padrão do modelo: ligada na família ][, desligada na //e e na //c.
#[test]
fn gh99_gh121_shift_key_mod_default_follows_the_model() {
    for model in MODELS {
        let mut s = Settings::default();
        s.machine.model = Model::Apple2eEnhanced;
        s.joystick.shift_key_mod = model.is_apple2(); // o contrário do que sai de `set_model`
        s.joystick.shift_key_mod = !s.joystick.shift_key_mod;
        let want = cpp_is_apple2_plus_or_clone(model); // kShiftKeyModForAppleII_Default = true
        if model != Model::Apple2eEnhanced {
            set_model(&mut s, model);
            assert_eq!(s.joystick.shift_key_mod, want, "{model:?}");
        }
    }
    // Apple ][+ com a SKM padrão: o PB2 cai com o Shift (Platinum //e: ligando a caixa)
    let mut s = Settings::default();
    set_model(&mut s, Model::Apple2Plus);
    assert!(s.joystick.shift_key_mod);
}

// ===================================================================== teclado como joystick

/// #23: {Mouse, Disabled} -> {Mouse, Teclado} tem de funcionar. `JoyProcessKey` manda o teclado
/// para o joystick 1 quando o joystick 0 não é teclado (Joystick.cpp:489) e o mouse fica com o 0.
#[test]
fn gh23_mouse_and_keyboard_can_be_the_two_joysticks() {
    for k1 in [
        Joystick1Kind::KeyboardNumpad,
        Joystick1Kind::KeyboardCursors,
    ] {
        let mut s = Settings::default();
        s.joystick.joystick0 = Joystick0Kind::Mouse;
        s.joystick.joystick1 = k1;
        let mut j = JoySystem::new(JoyConfig::from_settings(&s));
        assert_eq!(j.effective_kinds()[0], JoyMode::Mouse);
        j.set_mouse_position(10, 200);
        let key = if k1 == Joystick1Kind::KeyboardNumpad {
            np('4')
        } else {
            KeyInput::new(HostKey::Left)
        };
        j.process_key(key, true, false);
        let f = j.output(0);
        assert_eq!((f.out[0].x, f.out[0].y), (10, 200), "mouse: joystick 0");
        assert_eq!((f.out[1].x, f.out[1].y), (0, 127), "teclado: joystick 1");
        assert_eq!(f.attached, [true, true]);
    }
    // e o combo oferece as duas coisas ao mesmo tempo (PageInput.cpp:290-314)
    assert!(joystick1_choices(Joystick0Kind::Mouse, false).contains(&Joystick1Kind::KeyboardNumpad));
    assert!(joystick0_choices(Joystick1Kind::KeyboardNumpad, false).contains(&Joystick0Kind::Mouse));
}

/// Mesma regra que `InitJoystickChoices` (PageInput.cpp:272-314), reescrita sobre os índices
/// `JOY0CHOICE` / `JOY1CHOICE` (Joystick.h:6-7).
fn cpp_choices(joy1: bool, other: u32, mouse_card: bool) -> Vec<u32> {
    let (cursors, numpad, mouse, max) = (2u32, 3u32, 4u32, if joy1 { 6u32 } else { 5 });
    let mut v = vec![0u32, 1];
    for i in cursors..max {
        if (i == cursors || i == numpad) && (other == cursors || other == numpad) {
            continue;
        }
        if i == mouse && mouse_card {
            continue;
        }
        if other != i {
            v.push(i);
        }
    }
    v
}

const J0: [Joystick0Kind; 5] = [
    Joystick0Kind::Disabled,
    Joystick0Kind::HostJoystick1,
    Joystick0Kind::KeyboardCursors,
    Joystick0Kind::KeyboardNumpad,
    Joystick0Kind::Mouse,
];
const J1: [Joystick1Kind; 6] = [
    Joystick1Kind::Disabled,
    Joystick1Kind::HostJoystick2,
    Joystick1Kind::KeyboardCursors,
    Joystick1Kind::KeyboardNumpad,
    Joystick1Kind::Mouse,
    Joystick1Kind::HostJoystick1Thumbstick2,
];

/// #426: com a placa do mouse ausente, Joystick 1 = cursores e Joystick 2 = Mouse tem de poder ser
/// escolhido. Em todas as combinações a lista do port é a do `InitJoystickChoices`, e o combo do
/// port escolhe por valor (sem o deslocamento de índices do Win32).
#[test]
fn gh426_joystick_choices_match_pageinput() {
    for mouse_card in [false, true] {
        for (i, other) in J1.iter().enumerate() {
            let got: Vec<u32> = joystick0_choices(*other, mouse_card)
                .into_iter()
                .map(|k| k as u32)
                .collect();
            assert_eq!(
                got,
                cpp_choices(false, i as u32, mouse_card),
                "J0 / {other:?}"
            );
        }
        for (i, other) in J0.iter().enumerate() {
            let got: Vec<u32> = joystick1_choices(*other, mouse_card)
                .into_iter()
                .map(|k| k as u32)
                .collect();
            assert_eq!(
                got,
                cpp_choices(true, i as u32, mouse_card),
                "J1 / {other:?}"
            );
        }
    }
    let c = joystick1_choices(Joystick0Kind::KeyboardCursors, false);
    assert!(c.contains(&Joystick1Kind::Mouse), "o caso da issue");
}

/// #28 / #936: "Swap 0/1" vale para todas as fontes de botão (Joystick.cpp:617-637): as teclas
/// Alt (Open/Closed Apple), `0` e `.` do teclado numérico, o botão do mouse e os do gamepad.
#[test]
fn gh28_gh936_swap_buttons_applies_to_every_source() {
    type Act = fn(&mut Emulator, &mut InputRouter, bool);
    fn alt_left(e: &mut Emulator, r: &mut InputRouter, _: bool) {
        press(e, r, KeyInput::new(HostKey::AltLeft), ALT);
    }
    fn alt_right(e: &mut Emulator, r: &mut InputRouter, _: bool) {
        press(e, r, KeyInput::new(HostKey::AltRight), ALT);
    }
    fn num0(e: &mut Emulator, r: &mut InputRouter, _: bool) {
        press(e, r, np('0'), NONE);
    }
    fn numdot(e: &mut Emulator, r: &mut InputRouter, _: bool) {
        press(e, r, np('.'), NONE);
    }
    fn mouse0(e: &mut Emulator, r: &mut InputRouter, _: bool) {
        r.joy.set_mouse_button(0, true);
        sync(e, r, 1);
    }
    fn pad0(e: &mut Emulator, r: &mut InputRouter, _: bool) {
        r.joy.set_pads_connected(1);
        r.joy.pads[0].button(gilrs::Button::South, true);
        sync(e, r, 1);
    }
    fn pad_button2(e: &mut Emulator, r: &mut InputRouter, _: bool) {
        r.joy.set_pads_connected(1);
        r.joy.pads[0].button(gilrs::Button::East, true);
        sync(e, r, 1);
    }
    // (ação, botão que o AppleWin liga sem o swap)
    let table: [(Act, usize, Joystick0Kind); 7] = [
        (alt_left, 0, Joystick0Kind::HostJoystick1),
        (alt_right, 1, Joystick0Kind::HostJoystick1),
        (num0, 0, Joystick0Kind::KeyboardNumpad),
        (numdot, 1, Joystick0Kind::KeyboardNumpad),
        (mouse0, 0, Joystick0Kind::Mouse),
        (pad0, 0, Joystick0Kind::HostJoystick1),
        (pad_button2, 1, Joystick0Kind::HostJoystick1),
    ];
    for swap in [false, true] {
        for (act, button, kind) in table {
            let (mut e, mut r) = rig(Model::Apple2eEnhanced, |s| {
                s.joystick.joystick0 = kind;
                s.joystick.swap_buttons = swap;
            });
            act(&mut e, &mut r, swap);
            let want = if swap { 1 - button } else { button };
            assert!(pb(&mut e, want as u16), "swap={swap} botão {button}");
            assert!(
                !pb(&mut e, 1 - want as u16),
                "swap={swap} botão {button}: o outro solto"
            );
        }
    }
    // a opção -swap-buttons liga o mesmo
    assert!(parse(&["-swap-buttons"]).joystick.swap_buttons);
}

/// #743: `-left-alt-control-buttons` (botão 0 = Ctrl esquerdo, botão 1 = Alt esquerdo) e
/// `-right-alt-control-buttons` (botão 0 = Alt direito, botão 1 = Ctrl direito), CmdLine.cpp:556-565.
#[test]
fn gh743_alt_control_button_switches() {
    let alt_l = KeyInput::new(HostKey::AltLeft);
    let alt_r = KeyInput::new(HostKey::AltRight);
    let ctl_l = KeyInput::new(HostKey::ControlLeft);
    let ctl_r = KeyInput::new(HostKey::ControlRight);
    // (argumentos, [(tecla, botão esperado)])
    type Keys = [(KeyInput, Option<usize>); 4];
    let cases: [(&[&str], Keys); 3] = [
        (
            &[],
            [
                (alt_l, Some(0)),
                (alt_r, Some(1)),
                (ctl_l, None),
                (ctl_r, None),
            ],
        ),
        (
            &["-left-alt-control-buttons"],
            [
                (alt_l, Some(1)),
                (alt_r, None),
                (ctl_l, Some(0)),
                (ctl_r, None),
            ],
        ),
        (
            &["-right-alt-control-buttons"],
            [
                (alt_l, None),
                (alt_r, Some(0)),
                (ctl_l, None),
                (ctl_r, Some(1)),
            ],
        ),
    ];
    for (args, keys) in cases {
        for (key, want) in keys {
            let a = parse(args);
            let (mut e, mut r) = rig(Model::Apple2eEnhanced, |s| s.joystick = a.joystick);
            press(&mut e, &mut r, key, ALT);
            for b in 0..2u16 {
                assert_eq!(
                    pb(&mut e, b),
                    want == Some(b as usize),
                    "{args:?} {:?} botão {b}",
                    key.key
                );
            }
        }
    }
}

/// #31: o trim também vale para um joystick de verdade (CheckJoystick0, Joystick.cpp:157-158:
/// "if (xpos == 127 || xpos == 128) xpos += trim") e para o teclado (`JoySetTrim`, :979-997). Os
/// quatro tempos de paddle saem iguais aos do `JoyResetPosition` (:717-780).
#[test]
fn gh31_trim_applies_to_the_gamepad_centre_and_the_keyboard() {
    const INTERVAL: f64 = 2816.0 / 255.0;
    // JoyResetPosition para uma posição de joystick `analog` (círculo -> quadrado) ou não
    fn cpp_count(x: i32, y: i32, pdl: usize, analog: bool) -> u64 {
        let mut pos = if pdl & 1 == 0 { x } else { y } as f64;
        if analog {
            let (xx, yy) = (f64::from(x) / 127.5 - 1.0, f64::from(y) / 127.5 - 1.0);
            let mut axis = if pdl & 1 == 0 { xx } else { yy };
            if xx * yy != 0.0 {
                let r2 = (yy * yy) / (xx * xx);
                axis *= (1.0 + r2.min(1.0 / r2)).sqrt();
            }
            pos = ((axis.clamp(-1.0, 1.0) + 1.0) * 127.5) as i32 as f64;
        }
        let mut p = pos as u32;
        if p >= 255 {
            p = 287;
        }
        (f64::from(p) * INTERVAL) as u64
    }
    for tx in [-127i16, -50, -1, 0, 1, 25, 60, 127] {
        for ty in [-100i16, 0, 33, 127] {
            // gamepad: o stick parado dá 127 (zona morta) e o trim entra
            let mut s = Settings::default();
            s.joystick.joystick0 = Joystick0Kind::HostJoystick1;
            s.joystick.pdl_x_trim = tx;
            s.joystick.pdl_y_trim = ty;
            let mut j = JoySystem::new(JoyConfig::from_settings(&s));
            j.set_pads_connected(1);
            let f = j.output(0);
            let mut g = apple2_core::game_port::GamePort::default();
            g.paddles = [f.out[0].x, f.out[0].y, 127, 127];
            g.trigger(0);
            let got = g.inactive_cycles();
            for (pdl, got) in got.iter().enumerate().take(2) {
                let want = cpp_count(127 + i32::from(tx), 127 + i32::from(ty), pdl, true);
                assert_eq!(*got, want, "gamepad trim ({tx},{ty}) paddle {pdl}");
            }
            // teclado numérico parado: centro + trim (sem círculo)
            let mut s = Settings::default();
            s.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
            s.joystick.pdl_x_trim = tx;
            s.joystick.pdl_y_trim = ty;
            let mut j = JoySystem::new(JoyConfig::from_settings(&s));
            let f = j.output(0);
            let mut g = apple2_core::game_port::GamePort::default();
            g.paddles = [f.out[0].x, f.out[0].y, 127, 127];
            g.trigger(0);
            let got = g.inactive_cycles();
            for (pdl, got) in got.iter().enumerate().take(2) {
                let want = cpp_count(127 + i32::from(tx), 127 + i32::from(ty), pdl, false);
                assert_eq!(*got, want, "teclado trim ({tx},{ty}) paddle {pdl}");
            }
        }
    }
    // só o centro recebe o trim (:155-156); fora dele a posição é a do stick
    assert_eq!(apply_trim(127, 10), 137);
    assert_eq!(apply_trim(128, 10), 138);
    assert_eq!(apply_trim(200, 10), 200);
}

/// #156: um joystick USB escolhido em "PC Joystick #1" move os paddles e aperta os botões. Os
/// eventos do gamepad são sintéticos (gilrs); a enumeração do dispositivo físico não é testada.
/// `CheckJoystick0` (:136-160): faixa de 16 bits `>> 8` => 0..=255, centro 127; botão 1/2 => PB0/PB1.
#[test]
fn gh156_pc_joystick_deflection_reaches_the_paddles_and_buttons() {
    let (mut e, mut r) = rig(Model::Apple2eEnhanced, |s| {
        s.joystick.joystick0 = Joystick0Kind::HostJoystick1;
    });
    r.joy.set_pads_connected(1);
    for v in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
        r.joy.pads[0].axis(gilrs::Axis::LeftStickX, v);
        sync(&mut e, &mut r, 1);
        // `CheckJoystick0` dá xpos = (v+1)/2*255 e ypos = 127; `JoyResetPosition` achata o círculo
        let raw = axis_to_paddle(v, DEFAULT_DEADZONE);
        assert!(raw as f32 >= ((v + 1.0) * 127.5).floor() - 1.0 || v.abs() < DEFAULT_DEADZONE);
        let want = cpp_square_the_circle(raw, 127, 0).min(255) as u8;
        assert_eq!(e.machine.board().game.paddles[0], want, "eixo X = {v}");
    }
    let extremes = [(-1.0f32, 0u8), (1.0, 255)];
    for (v, want) in extremes {
        r.joy.pads[0].axis(gilrs::Axis::LeftStickX, v);
        sync(&mut e, &mut r, 1);
        assert_eq!(e.machine.board().game.paddles[0], want, "extremo {v}");
    }
    r.joy.pads[0].button(gilrs::Button::South, true);
    r.joy.pads[0].button(gilrs::Button::East, true);
    sync(&mut e, &mut r, 2);
    assert!(pb(&mut e, 0) && pb(&mut e, 1));
    // sem gamepad o Joystick 1 cai no teclado numérico (JoyInitialize, :248-274)
    r.joy.set_pads_connected(0);
    assert_eq!(r.joy.effective_kinds()[0], JoyMode::KeyboardNumpad);
}

/// `JoyResetPosition` transcrito (igual ao de `audit_entrada.rs`): posição do paddle `pdl`.
fn cpp_square_the_circle(xpos: u8, ypos: u8, pdl: usize) -> u32 {
    let scalar = 0.5 * 255.0;
    let x = f64::from(xpos) / scalar - 1.0;
    let y = f64::from(ypos) / scalar - 1.0;
    let mut axis = if pdl & 1 == 0 { x } else { y };
    if x * y != 0.0 {
        let ratio2 = (y * y) / (x * x);
        axis *= (1.0 + ratio2.min(1.0 / ratio2)).sqrt();
    }
    let mut p = ((axis.clamp(-1.0, 1.0) + 1.0) * scalar) as u32;
    if p >= 255 {
        p = 287;
    }
    p
}

/// #1237: com um analógico circular (X e Y a 0,707) o AppleWin alcança os cantos (GH#429). Do evento
/// do stick até o tempo do timer do paddle, o port dá o mesmo que `CheckJoystick0` + `JoyResetPosition`.
#[test]
fn gh1237_circular_stick_corners_end_to_end() {
    const INTERVAL: f64 = 2816.0 / 255.0;
    let mut checked = 0;
    for ix in -10i32..=10 {
        for iy in -10i32..=10 {
            let (vx, vy) = (ix as f32 / 10.0, iy as f32 / 10.0);
            if (vx != 0.0 && vx.abs() < DEFAULT_DEADZONE)
                || (vy != 0.0 && vy.abs() < DEFAULT_DEADZONE)
            {
                continue; // a zona morta é do port (o stick do AppleWin é cru)
            }
            let mut s = Settings::default();
            s.joystick.joystick0 = Joystick0Kind::HostJoystick1;
            let mut j = JoySystem::new(JoyConfig::from_settings(&s));
            j.set_pads_connected(1);
            j.pads[0].axis(gilrs::Axis::LeftStickX, vx);
            j.pads[0].axis(gilrs::Axis::LeftStickY, -vy); // gilrs: +Y é para cima
            let f = j.output(0);
            let (rx, ry) = (
                axis_to_paddle(vx, DEFAULT_DEADZONE),
                axis_to_paddle(vy, DEFAULT_DEADZONE),
            );
            let mut g = apple2_core::game_port::GamePort::default();
            g.paddles = [f.out[0].x, f.out[0].y, 127, 127];
            g.trigger(0);
            let got = g.inactive_cycles();
            for (pdl, got) in got.iter().enumerate().take(2) {
                let want = (f64::from(cpp_square_the_circle(rx, ry, pdl)) * INTERVAL) as u64;
                assert_eq!(*got, want, "stick ({vx},{vy}) paddle {pdl}");
            }
            checked += 1;
        }
    }
    assert!(checked > 300);
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let (x, y) = square_circle(
        axis_to_paddle(d, DEFAULT_DEADZONE),
        axis_to_paddle(-d, DEFAULT_DEADZONE),
    );
    assert!(x >= 253 && y <= 2, "canto superior direito: ({x},{y})");
}

/// #1522 (xpos/ypos começam em `PDL_CENTRAL`, Joystick.cpp:97-100): com o mouse como joystick, o
/// primeiro `PREAD` do monitor ($FB1E) não pode dar $FF antes de o mouse se mexer.
#[test]
fn gh1522_first_pread_with_the_mouse_as_joystick_is_not_ff() {
    let (mut e, _r) = rig(Model::Apple2eEnhanced, |s| {
        s.joystick.joystick0 = Joystick0Kind::Mouse;
    });
    for (i, b) in [0x20, 0x1E, 0xFB, 0x4C, 0x03, 0x03].iter().enumerate() {
        e.machine.poke(0x300 + i as u16, *b); // JSR $FB1E ; JMP $0303
    }
    e.machine.cpu_mut().pc = 0x300;
    e.machine.cpu_mut().x = 0;
    for _ in 0..200_000 {
        if e.machine.cpu().pc == 0x303 {
            break;
        }
        e.machine.step();
    }
    assert_eq!(e.machine.cpu().pc, 0x303);
    let y = e.machine.cpu().y;
    assert!(
        (120..=134).contains(&y),
        "PDL(0) com o mouse parado: {y} (esperado perto de 127, nunca 255)"
    );
}

// ===================================================================== SNES MAX e 4Play

/// `SNESMAXCard::GetControllerButtons` (SNESMAX.cpp:146-169) transcrito. `map` é a tabela de
/// botões (`m_mainControllerButtons` ou a alternativa); bits do cartão: B Y Sl St U D L R A X LB RB.
fn cpp_controller_buttons(pad: &HostPad, map: &ButtonMap) -> u32 {
    let (x, y, pov) = ((pad.x >> 8) & 0xFF, (pad.y >> 8) & 0xFF, pad.pov);
    let mut b = 0u32;
    b |= u32::from(y < 103 || pov == 0 || pov == 4500 || pov == 31500) << 4;
    b |= u32::from(y > 153 || (13500..=22500).contains(&pov)) << 5;
    b |= u32::from(x < 103 || (22500..=31500).contains(&pov)) << 6;
    b |= u32::from(x > 153 || (4500..=13500).contains(&pov)) << 7;
    for (i, m) in map.0.iter().enumerate() {
        if pad.buttons & (1 << i) != 0 {
            if let Some(bit) = m {
                b |= 1 << bit;
            }
        }
    }
    b | 0x10000 // controlador ligado
}

/// Lê as 19 primeiras posições da cadeia do SNES MAX no slot 4 como o AppleWin
/// (`$C0C0` trava, lê `$C0C0`, `$C0C1` avança) e devolve (bit 7, bit 6) de cada leitura.
fn read_snes_chain(m: &mut Machine) -> Vec<(bool, bool)> {
    m.board_mut().write(0xC0C0, 0); // latch
    let mut v = Vec::new();
    for _ in 0..19 {
        let b = m.board_mut().read(0xC0C0);
        v.push((b & 0x80 != 0, b & 0x40 != 0));
        m.board_mut().write(0xC0C1, 0); // clock
    }
    v
}

/// O que `IOWrite` (latch/clock, SNESMAX.cpp:98-140) deixa na cadeia para os dois controles.
fn cpp_snes_chain(c1: Option<u32>, c2: Option<u32>) -> Vec<(bool, bool)> {
    let mut a = !c1.unwrap_or(0);
    let mut b = !c2.unwrap_or(0);
    let mut index = 0;
    let mut v = Vec::new();
    for _ in 0..19 {
        v.push((a & 1 != 0, b & 1 != 0));
        if index <= 16 {
            index += 1;
            a >>= 1;
            b >>= 1;
        }
    }
    v
}

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 33) as u32
}

fn random_pad(seed: &mut u64) -> HostPad {
    let pov = [0xFFFF, 0, 4500, 9000, 13500, 18000, 22500, 27000, 31500][lcg(seed) as usize % 9];
    let axis = |s: &mut u64| match lcg(s) % 4 {
        0 => 0x8000,
        1 => (lcg(s) % 0x10000) & 0xFFFF,
        2 => 0,
        _ => 0xFFFF,
    };
    HostPad {
        x: axis(seed),
        y: axis(seed),
        pov,
        buttons: lcg(seed) & 0xFFFF,
    }
}

fn snes_machine() -> Machine {
    let mut m = Machine::new(apple2_core::MachineConfig::for_model(Model::Apple2Plus));
    assert!(m.insert_card(4, Box::new(SnesMax::new())));
    m
}

/// #972 (cartão SNES MAX, lançado na 1.30.5.0): o protocolo latch/clock/leitura e a tradução do
/// estado de um controle para os botões saem iguais ao C++ para 400 estados sorteados, com a tabela
/// principal (DS4/DualSense) e a alternativa (8BitDo NES30 PRO), um e dois controles, e desligados.
#[test]
fn gh972_snes_max_protocol_matches_cpp() {
    let mut seed = 0x5EED_u64;
    for round in 0..400 {
        let mut m = snes_machine();
        let (p1, p2) = (random_pad(&mut seed), random_pad(&mut seed));
        let alt = [round % 2 == 0, round % 3 == 0];
        let plugged = [round % 5 != 0, round % 7 != 0];
        m.with_card(4, |c, _| {
            let sm = c.as_any_mut().unwrap().downcast_mut::<SnesMax>().unwrap();
            for (n, p) in [p1, p2].into_iter().enumerate() {
                sm.set_alt_controller(n, alt[n]);
                sm.set_host_pad(n, plugged[n].then_some(p));
            }
        });
        let map = |n: usize| {
            if alt[n] {
                ButtonMap::ALT_DEFAULT
            } else {
                ButtonMap::MAIN
            }
        };
        let c1 = plugged[0].then(|| cpp_controller_buttons(&p1, &map(0)));
        let c2 = plugged[1].then(|| cpp_controller_buttons(&p2, &map(1)));
        assert_eq!(
            read_snes_chain(&mut m),
            cpp_snes_chain(c1, c2),
            "rodada {round}"
        );
    }
}

fn tmp_file(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("applewin-p23-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

/// Arquivo de mapeamento de botões do AppleWin (`snesmax/controller_*.yaml`, a correção das
/// issues 1141-1150) lido pelo port, aplicado a um cartão e conferido botão a botão: o botão
/// `n` (1..=16) do dispositivo deve apertar exatamente `want[n]` no SNES MAX.
fn check_controller_file(yaml: &str, want: &[(u32, u32)]) {
    let map = parse_controller_mapping_file(&tmp_file("controller.yaml", yaml)).unwrap();
    for n in 1..=16u32 {
        let mut m = snes_machine();
        m.with_card(4, |c, _| {
            let sm = c.as_any_mut().unwrap().downcast_mut::<SnesMax>().unwrap();
            sm.set_alt_controller(0, true);
            sm.set_alt_map(0, map);
            sm.set_host_pad(
                0,
                Some(HostPad {
                    buttons: 1 << (n - 1),
                    ..HostPad::CENTRED
                }),
            );
        });
        let chain = read_snes_chain(&mut m);
        let pressed: u32 = (0..16)
            .filter(|i| !chain[*i as usize].0)
            .fold(0, |a, i| a | 1 << i);
        let expect = want
            .iter()
            .find(|(b, _)| *b == n)
            .map_or(0, |(_, bit)| 1 << bit.trailing_zeros());
        assert_eq!(pressed, expect, "botão {n} do dispositivo");
        assert!(!chain[16].0, "controle 1 ligado (bit 16 baixo)");
    }
}

const F310_XMODE: &str = "---
File_hdr:
  Tag: AppleWin Controller Button Remapping
  Version: 1

Unit:
    HID: VID_046D&PID_C21D
    Description: Logitech F310
    1: B
    2: A
    3: Y
    4: X
    5: LB
    6: RB
    7: SELECT
    8: START
    9: \"\"
    10: \"\"
...
";

/// #1141: F310 (modo X), botões 1-8 = A,B,X,Y,Lb,Rb,Back,Start; o mapeamento certo é
/// B,A,Y,X,Lb,Rb,Select,Start.
#[test]
fn gh1141_logitech_f310_mapping_file() {
    check_controller_file(
        F310_XMODE,
        &[
            (1, sb::B),
            (2, sb::A),
            (3, sb::Y),
            (4, sb::X),
            (5, sb::L),
            (6, sb::R),
            (7, sb::SELECT),
            (8, sb::START),
        ],
    );
}

const RAPHNET_DUAL_SNES: &str = "---
File_hdr:
  Tag: AppleWin Controller Button Remapping
  Version: 1

Unit:
    HID: VID_289B&PID_0057
    Description: Raphnet Dual SNES adapter V2
    1: Y
    2: B
    3: SELECT
    4: START
    5: A
    6: X
    7: LB
    8: RB
    9: \"\"
    10: \"\"
    11: \"\"
    12: \"\"
...
";

const RAPHNET_DUAL_NES: &str = "---
File_hdr:
  Tag: AppleWin Controller Button Remapping
  Version: 1

Unit:
    HID: VID_289B&PID_0042
    Description: Raphnet Dual NES adapter V2
    1: Y
    2: B
    3: SELECT
    4: START
    5: \"\"
    6: \"\"
    7: \"\"
    8: \"\"
...
";

/// #1142: adaptadores raphnet. SNES: 1=Y 2=B 3=Select 4=Start 5=A 6=X 7=Lb 8=Rb; NES: 1=B 2=A
/// 3=Select 4=Start (o B do NES é o Y do SNES e o A do NES é o B do SNES).
#[test]
fn gh1142_raphnet_adapters_mapping_files() {
    check_controller_file(
        RAPHNET_DUAL_SNES,
        &[
            (1, sb::Y),
            (2, sb::B),
            (3, sb::SELECT),
            (4, sb::START),
            (5, sb::A),
            (6, sb::X),
            (7, sb::L),
            (8, sb::R),
        ],
    );
    check_controller_file(
        RAPHNET_DUAL_NES,
        &[(1, sb::Y), (2, sb::B), (3, sb::SELECT), (4, sb::START)],
    );
}

const SAITEK_P2500: &str = "---
File_hdr:
  Tag: AppleWin Controller Button Remapping
  Version: 1

Unit:
    HID: VID_06A3&PID_FF0C
    Description: Saitek P2500 Cyborg Force Rumble Pad
    1: Y
    2: X
    3: B
    4: A
    5: LB
    6: RB
    7: LB
    8: RB
    9: SELECT
    10: START
    11: START
    12: SELECT
...
";

/// #1148: Saitek P2500, 3a sugestão da issue: 1=Y 2=X 3=B 4=A 5=L 6=R 7=L 8=R 9=Select 10=Start.
#[test]
fn gh1148_saitek_p2500_mapping_file() {
    check_controller_file(
        SAITEK_P2500,
        &[
            (1, sb::Y),
            (2, sb::X),
            (3, sb::B),
            (4, sb::A),
            (5, sb::L),
            (6, sb::R),
            (7, sb::L),
            (8, sb::R),
            (9, sb::SELECT),
            (10, sb::START),
            (11, sb::START),
            (12, sb::SELECT),
        ],
    );
}

const SAITEK_P990: &str = "---
File_hdr:
  Tag: AppleWin Controller Button Remapping
  Version: 1

Unit:
    HID: VID_06A3&PID_040B
    Description: Saitek P990 Dual Analog Pad
    1: Y
    2: B
    3: A
    4: X
    5: LB
    6: RB
    7: SELECT
    8: START
    9: RB
    10: LB
    11: SELECT
    12: START
    13: \"\"
    14: \"\"
...
";

/// #1149: Saitek P990 / P2900, 2a sugestão da issue: 1=Y 2=B 3=A 4=X 5=L 6=R 7=Select 8=Start
/// 9=R 10=L.
#[test]
fn gh1149_saitek_p990_p2900_mapping_file() {
    check_controller_file(
        SAITEK_P990,
        &[
            (1, sb::Y),
            (2, sb::B),
            (3, sb::A),
            (4, sb::X),
            (5, sb::L),
            (6, sb::R),
            (7, sb::SELECT),
            (8, sb::START),
            (9, sb::R),
            (10, sb::L),
            (11, sb::SELECT),
            (12, sb::START),
        ],
    );
}

const LIKSANG: &str = "---
File_hdr:
  Tag: AppleWin Controller Button Remapping
  Version: 1

Unit:
    HID: VID_6666&PID_0667
    Description: Lik-Sang Super Smartjoy adapter
    1: X
    2: A
    3: B
    4: Y
    5: \"\"
    6: \"\"
    7: LB
    8: RB
    9: SELECT
    10: \"\"
    11: \"\"
    12: START
    13: \"\"
    14: \"\"
    15: \"\"
    16: \"\"
...
";

/// #1150: Lik-Sang Super Smartjoy: X=1 A=2 B=3 Y=4 L=7 R=8 Select=9 Start=12 (os botões 5, 6,
/// 10, 11 espelham outros e 13-16 espelham o D-pad, que já vem pelos eixos).
#[test]
fn gh1150_liksang_super_smartjoy_mapping_file() {
    check_controller_file(
        LIKSANG,
        &[
            (1, sb::X),
            (2, sb::A),
            (3, sb::B),
            (4, sb::Y),
            (7, sb::L),
            (8, sb::R),
            (9, sb::SELECT),
            (12, sb::START),
        ],
    );
}

/// #946: o 4Play (lançado na 1.30.5.0). `MakeByte` (FourPlay.cpp:61-72): cima=bit 0, baixo=1,
/// esquerda=2, direita=3, bit 4 sempre 0, bit 5 sempre 1, gatilho 2=bit 6, gatilho 1=bit 7. Os
/// joysticks 3 e 4 são as teclas E/D/S/F + Z/X e I/K/J/L + N/M (:115-132). Todas as 2^6 combinações.
#[test]
fn gh946_four_play_keys_match_makebyte() {
    fn make_byte(up: bool, down: bool, left: bool, right: bool, t1: bool, t2: bool) -> u8 {
        u8::from(up)
            | u8::from(down) << 1
            | u8::from(left) << 2
            | u8::from(right) << 3
            | 1 << 5
            | u8::from(t2) << 6
            | u8::from(t1) << 7
    }
    // (cima, baixo, esquerda, direita, gatilho 1, gatilho 2)
    let ports: [[char; 6]; 2] = [
        ['e', 'd', 's', 'f', 'z', 'x'],
        ['i', 'k', 'j', 'l', 'n', 'm'],
    ];
    for combo in 0u32..64 {
        let (mut e, mut r) = rig(Model::Apple2eEnhanced, |s| {
            s.slots.0[4] = SlotCardSpec::new(CardType::FourPlay);
        });
        for (port, keys) in ports.iter().enumerate() {
            for (i, k) in keys.iter().enumerate() {
                if combo >> i & 1 != 0 {
                    press(&mut e, &mut r, kb(*k), NONE);
                }
            }
            let bits: Vec<bool> = (0..6).map(|i| combo >> i & 1 != 0).collect();
            let want = make_byte(bits[0], bits[1], bits[2], bits[3], bits[4], bits[5]);
            assert_eq!(
                rd(&mut e, 0xC0C2 + port as u16),
                want,
                "porta {} combo {combo:06b}",
                port + 3
            );
            for (i, k) in keys.iter().enumerate() {
                if combo >> i & 1 != 0 {
                    release(&mut e, &mut r, kb(*k), NONE);
                }
            }
            assert_eq!(rd(&mut e, 0xC0C2 + port as u16), 0x20, "solto");
        }
    }
}

// ===================================================================== dongles do game port

fn ann(mask: u8) -> [bool; 4] {
    [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0]
}

/// #1153 (SpeedStar, PR #1154) e #1247 (Robocom BitStik): as tabelas e a porta NAND do
/// `CopyProtectionDongles.cpp` (SdsSpeedStar :87-90, PDL :124-169) para as 16 combinações de AN0-AN3.
#[test]
fn gh1153_gh1247_dongles_match_the_cpp_tables() {
    let speedstar = Dongle::new(DongleKind::SdsSpeedStar);
    let hayden = Dongle::new(DongleKind::HaydenCompiler);
    let r500 = Dongle::new(DongleKind::Robocom500);
    let r1000 = Dongle::new(DongleKind::Robocom1000);
    let r1500 = Dongle::new(DongleKind::Robocom1500);
    for mask in 0..16u8 {
        let a = ann(mask);
        // SdsSpeedStar(): !AN0 || !(AN1 && AN2)
        assert_eq!(
            speedstar.button(2, &a),
            Some(!a[0] || !(a[1] && a[2])),
            "{mask:04b}"
        );
        assert_eq!(speedstar.button(0, &a), None);
        assert_eq!(speedstar.button(1, &a), None);
        for n in 0..4 {
            assert_eq!(speedstar.paddle(n, &a), None);
        }
        // Hayden: PB1 a GND; PDL3 = {FF,96,96,50}[AN2<<1 | AN0]
        assert_eq!(hayden.button(1, &a), Some(false));
        let h = [0xFFu32, 0x96, 0x96, 0x50][usize::from(a[2]) << 1 | usize::from(a[0])];
        assert_eq!(hayden.paddle(3, &a), Some(h));
        assert_eq!(hayden.paddle(2, &a), None);
        // Robocom: o módulo só responde com AN3 desligado e só no paddle 3
        let mode = usize::from(a[2]) << 2 | usize::from(a[1]) << 1 | usize::from(a[0]);
        let lo = [0x3Fu32, 0x2E, 0x54, 0x54, 0x2E, 0x22, 0x72, 0x17];
        let hi = [0x6Fu32, 0x54, 0x94, 0x94, 0x54, 0x40, 0xC4, 0x2E];
        let t1000 = [34u32, 151, 48, 64, 113, 113, 64, 85];
        let t1500 = [153u32, 34, 64, 34, 48, 86, 114, 48];
        for (dev, want) in [
            (&r500, (lo[mode] + hi[mode] - 1) / 2),
            (&r1000, t1000[mode]),
            (&r1500, t1500[mode]),
        ] {
            assert_eq!(
                dev.paddle(3, &a),
                if a[3] { None } else { Some(want) },
                "{mask:04b}"
            );
            for n in 0..3 {
                assert_eq!(dev.paddle(n, &a), None);
            }
            assert_eq!(dev.button(2, &a), None);
        }
    }
    // cadeia completa pela máquina: SpeedStar no $C063 com os annunciators ($C058-$C05F)
    let mut m = Machine::new(apple2_core::MachineConfig::for_model(Model::Apple2Plus));
    m.set_joystick_attached(1, true);
    m.set_game_io(Some(Box::new(Dongle::new(DongleKind::SdsSpeedStar))));
    for mask in 0..8u8 {
        for (n, base) in [0xC058u16, 0xC05A, 0xC05C].into_iter().enumerate() {
            m.board_mut().read(base + u16::from(mask >> n & 1));
        }
        let want = mask & 1 == 0 || (mask & 6) != 6;
        assert_eq!(
            m.board_mut().read(0xC063) & 0x80 != 0,
            want,
            "AN2..0={mask:03b}"
        );
    }
}

/// CodeWriter: o registrador de 7 bits anda na borda de descida do AN2 e o AN3 o reinicia, com
/// precedência (`DongleControl`, CopyProtectionDongles.cpp:68-83).
#[test]
fn gh1153_codewriter_lfsr_follows_dongle_control() {
    let mut m = Machine::new(apple2_core::MachineConfig::for_model(Model::Apple2Plus));
    m.set_game_io(Some(Box::new(Dongle::new(DongleKind::CodeWriter))));
    let mut lfsr = 0x6Bu8;
    let mut an = [false; 4];
    let mut seed = 7u64;
    for _ in 0..500 {
        let n = lcg(&mut seed) as usize % 4;
        let on = lcg(&mut seed).is_multiple_of(2);
        m.board_mut().read(0xC058 + (n as u16) * 2 + u16::from(on));
        // DongleControl(address)
        if (n == 3 && on) || an[3] {
            lfsr = 0x6B;
        } else if n == 2 && !on && an[2] {
            let bit = ((lfsr >> 1) ^ lfsr) & 1;
            lfsr = (lfsr >> 1) | (bit << 6);
        }
        an[n] = on;
        assert_eq!(m.board_mut().read(0xC063) & 0x80 != 0, lfsr & 1 != 0);
    }
}

// ===================================================================== mouse (firmware real)

const SLOT: u8 = 4;

/// Chama uma entrada do firmware do AppleMouse (`entry`: $12 SETMOUSE ... $19 INITMOUSE) com A=`a`.
fn call(m: &mut Machine, entry: u8, a: u8) {
    let code = [
        0xA2, 0xC4, 0xA0, 0x40, 0xA9, a, 0x20, 0x20, 0x03, 0x4C, 0x09, 0x03,
    ];
    for (i, b) in code.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    let sub = [
        0x48, 0xAD, entry, 0xC4, 0x8D, 0x29, 0x03, 0x68, 0x4C, 0x00, 0xC4,
    ];
    for (i, b) in sub.iter().enumerate() {
        m.poke(0x320 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    for _ in 0..200_000 {
        if m.cpu().pc == 0x309 {
            break;
        }
        m.step();
    }
    assert_eq!(m.cpu().pc, 0x309, "o firmware voltou");
}

fn hole(m: &Machine, base: u16) -> u8 {
    m.peek(base + u16::from(SLOT))
}

fn mouse_machine(model: Model) -> Machine {
    let mut m = Machine::new(apple2_core::MachineConfig::for_model(model));
    assert!(m.insert_card(SLOT, Box::new(MouseCard::new())));
    m
}

fn mouse_pos(m: &mut Machine) -> (i32, i32) {
    m.with_card(SLOT, |c, _| {
        c.as_any_mut()
            .unwrap()
            .downcast_mut::<MouseCard>()
            .unwrap()
            .position()
    })
    .unwrap()
}

/// X e Y que o READMOUSE deixou nos buracos de tela.
fn read_mouse(m: &mut Machine) -> (u16, u16, u8) {
    call(m, 0x14, 0);
    let x = u16::from(hole(m, 0x578)) << 8 | u16::from(hole(m, 0x478));
    let y = u16::from(hole(m, 0x5F8)) << 8 | u16::from(hole(m, 0x4F8));
    (x, y, hole(m, 0x778))
}

fn clamp(m: &mut Machine, axis: u8, min: u16, max: u16) {
    m.poke_main(0x478, min as u8);
    m.poke_main(0x4F8, max as u8);
    m.poke_main(0x578, (min >> 8) as u8);
    m.poke_main(0x5F8, (max >> 8) as u8);
    call(m, 0x17, axis);
}

/// #266: INITMOUSE, SETMOUSE(1), CLAMPMOUSE X 1..40 e Y 3..21, HOMEMOUSE, READMOUSE. O AppleWin
/// põe a posição em (0,0) no HOMEMOUSE (MouseInterface.cpp:391-394, `SetPositionAbs(0,0)`), sem
/// reaplicar o limite mínimo; o READMOUSE devolve 0/0 (era o relato da issue, depois fechada).
#[test]
fn gh266_homemouse_then_readmouse_matches_the_cpp() {
    for model in [Model::Apple2Plus, Model::Apple2e, Model::Apple2eEnhanced] {
        let mut m = mouse_machine(model);
        call(&mut m, 0x19, 0); // INITMOUSE
        call(&mut m, 0x12, 0x01); // SETMOUSE: ligado, sem interrupções
        clamp(&mut m, 0, 1, 40);
        clamp(&mut m, 1, 3, 21);
        // `SetClampX/Y` já empurra a posição atual para dentro do limite (:557-558, :577)
        assert_eq!(mouse_pos(&mut m), (1, 3), "{model:?}");
        call(&mut m, 0x18, 0); // HOMEMOUSE
        assert_eq!(
            mouse_pos(&mut m),
            (0, 0),
            "{model:?}: HOMEMOUSE = SetPositionAbs(0,0)"
        );
        let (x, y, _) = read_mouse(&mut m);
        assert_eq!((x, y), (0, 0), "{model:?}");
    }
}

/// #154 (CopyII+ 9.1) e #182 (ProTERM 3.1), corrigidos na 1.24.0: com o mouse ligado e sem
/// interrupções, o movimento fica guardado para o próximo READMOUSE no bit 5 do status
/// (OnMouseEvent, MouseInterface.cpp:458-464 "[TC] Used by CopyII+9.1 and ProTERM3.1"), que
/// o próprio READMOUSE limpa depois de entregar (:356). A posição relida acompanha o movimento.
#[test]
fn gh154_gh182_movement_is_reported_to_readmouse_without_interrupts() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = mouse_machine(model);
        call(&mut m, 0x19, 0);
        call(&mut m, 0x12, 0x01);
        assert!(mouse_move(&mut m, SLOT, 12, 0));
        assert!(mouse_move(&mut m, SLOT, 3, 5));
        let (x, y, st) = read_mouse(&mut m);
        assert_eq!((x, y), (15, 5), "{model:?}");
        assert_eq!(
            st & 0x20,
            0x20,
            "{model:?}: movimento desde o último READMOUSE"
        );
        let (_, _, st) = read_mouse(&mut m);
        assert_eq!(
            st & 0x20,
            0,
            "{model:?}: o segundo READMOUSE não vê movimento"
        );
        // com o mouse desligado (modo 0) o movimento não é anotado (:458)
        call(&mut m, 0x12, 0x00);
        mouse_move(&mut m, SLOT, 1, 1);
        let (_, _, st) = read_mouse(&mut m);
        assert_eq!(st & 0x20, 0, "{model:?}: mouse desligado");
    }
}

/// #151: o AppleMouse II numa //e (BeagleWrite / MultiScribe, 1.21.1): o cartão aparece com a
/// assinatura Pascal 1.1 ($Cn05=$38, $Cn07=$18, $Cn0B=$01, $Cn0C=$20) e o byte do AppleMouse
/// ($CnFB=$D6), e a rotina de inicialização do firmware roda na //e e na //e aprimorada.
#[test]
fn gh151_applemouse_card_identifies_and_runs_on_the_iie() {
    for model in [Model::Apple2e, Model::Apple2eEnhanced] {
        let mut m = mouse_machine(model);
        for (a, v) in [
            (0xC405, 0x38),
            (0xC407, 0x18),
            (0xC40B, 0x01),
            (0xC40C, 0x20),
            (0xC4FB, 0xD6),
        ] {
            assert_eq!(m.board_mut().read(a), v, "{model:?} ${a:04X}");
        }
        call(&mut m, 0x19, 0);
        call(&mut m, 0x12, 0x01);
        mouse_move(&mut m, SLOT, 100, 50);
        mouse_button(&mut m, SLOT, 0, true);
        let (x, y, st) = read_mouse(&mut m);
        assert_eq!((x, y), (100, 50), "{model:?}");
        assert_eq!(st & 0x80, 0x80, "{model:?}: botão 0");
    }
}

/// #267: a placa do mouse recebe movimento e botões também com o depurador em MODE_STEPPING (o
/// comando `g`), não só em MODE_RUNNING (Frame.cpp tratava só o segundo; corrigido em 779b86b).
#[test]
fn gh267_mouse_events_reach_the_card_while_stepping() {
    use apple2_debugger::RunMode;
    use applewin_shell::keymap::{DbgKey, DebugKey};
    let (mut e, _r) = rig(Model::Apple2eEnhanced, |s| {
        s.slots.0[4] = SlotCardSpec::new(CardType::MouseInterface);
    });
    // liga o mouse pelo firmware (A=1: SETMOUSE ligado)
    call(&mut e.machine, 0x19, 0);
    call(&mut e.machine, 0x12, 0x01);
    // F7 e `g`: o depurador deixa a máquina rodando em MODE_STEPPING
    e.apply(Command::ToggleDebugger);
    for c in "g".chars() {
        e.apply(Command::Debug(DebugKey::Char(c)));
    }
    e.apply(Command::Debug(DebugKey::plain(DbgKey::Enter)));
    assert_eq!(e.debugger().mode, RunMode::Stepping);
    e.apply(Command::MouseMove { dx: 7, dy: 9 });
    e.apply(Command::MouseButton {
        button: 0,
        down: true,
    });
    e.run_frame();
    assert_eq!(mouse_pos(&mut e.machine), (7, 9));
    let (x, y, st) = read_mouse(&mut e.machine);
    assert_eq!((x, y), (7, 9));
    assert_eq!(st & 0x80, 0x80, "botão 0 apertado");
}

/// #514: depois de usar o GEOS o mouse ficava com uma interrupção de VBL pendente e o reinício não
/// zerava a placa (o IRQ ia para $0000, um BRK, e travava). Aqui o cartão é reiniciado no F2 e na
/// troca de hardware (que reconstrói a máquina): sem IRQ pendente e sem modo de interrupção.
#[test]
fn gh514_restart_resets_the_mouse_card_and_its_pending_irq() {
    let make = || {
        let (mut e, _r) = rig(Model::Apple2eEnhanced, |s| {
            s.slots.0[4] = SlotCardSpec::new(CardType::MouseInterface);
        });
        call(&mut e.machine, 0x19, 0);
        call(&mut e.machine, 0x12, 0x09); // ligado + interrupção de VBL
        e.machine.run_frame();
        e.machine.run_frame();
        assert!(
            e.machine.board().interrupts.irq_line(),
            "VBL do mouse pendente (GEOS)"
        );
        e
    };
    // F2: ResetMachineState -> GetCardMgr().Reset(true)
    let mut e = make();
    e.apply(Command::PowerCycle);
    assert!(
        !e.machine.board().interrupts.irq_line(),
        "IRQ pendente desfeito"
    );
    let mode = e.machine.with_card(SLOT, |c, _| {
        c.as_any_mut()
            .unwrap()
            .downcast_mut::<MouseCard>()
            .unwrap()
            .mode()
    });
    assert_eq!(mode, Some(0), "modo zerado");
    for _ in 0..3 {
        e.machine.run_frame();
    }
    assert!(
        !e.machine.board().interrupts.irq_line(),
        "sem VBL novo: o modo é 0"
    );
    // habilitar o HDD (a troca de hardware do relato) reconstrói a máquina
    let mut e = make();
    let mut s = e.settings.clone();
    s.slots.0[7] = SlotCardSpec::new(CardType::GenericHdd);
    e.apply(Command::ApplySettings(Box::new(s)));
    for _ in 0..3 {
        e.machine.run_frame();
    }
    assert!(!e.machine.board().interrupts.irq_line());
    assert!(
        e.machine.card::<MouseCard>(SLOT).is_some(),
        "a placa do mouse continua no slot 4"
    );
}

/// #993: a caixa "Mouse interface in slot 4" tem de sobreviver ao reinício e ao arquivo de
/// configuração.
#[test]
fn gh993_mouse_card_survives_the_config_round_trip_and_the_restart() {
    let mut s = Settings::default();
    s.slots.0[4] = SlotCardSpec::new(CardType::MouseInterface);
    let text = s.to_toml().unwrap();
    let back = Settings::from_toml(&text).unwrap();
    assert_eq!(back.slots.0[4].card_type(), CardType::MouseInterface);
    let mut e = Emulator::new(back.clone(), Box::new(DefaultBackend), 48_000).unwrap();
    assert!(e.machine.card::<MouseCard>(4).is_some());
    e.apply(Command::ApplySettings(Box::new(back)));
    assert!(
        e.machine.card::<MouseCard>(4).is_some(),
        "depois do reinício"
    );
    assert!(e.status().mouse_available);
}

// ===================================================================== teclado

/// `asciicode[3][10]` (Keyboard.cpp:39-44): VK_LEFT/UP/RIGHT/DOWN/SELECT, PRINT/EXECUTE/SNAPSHOT/
/// INSERT/DELETE por tipo (Apple ][ / //e / Base 64A), escolhido por `IsCopamBase64A ? 2 :
/// IS_APPLE2 ? 0 : 1` (:225).
fn cpp_nav(model: Model, key: NavKey) -> u8 {
    const ASCII: [[u8; 10]; 3] = [
        [0x08, 0x00, 0x15, 0x00, 0x00, 0, 0, 0, 0, 0x00],
        [0x08, 0x0B, 0x15, 0x0A, 0x00, 0, 0, 0, 0, 0x7F],
        [0x08, 0x00, 0x15, 0x00, 0x00, 0, 0, 0, 0, 0x7F],
    ];
    let row = if model == Model::Base64A {
        2
    } else if model.is_apple2() {
        0
    } else {
        1
    };
    let col = match key {
        NavKey::Left => 0,
        NavKey::Up => 1,
        NavKey::Right => 2,
        NavKey::Down => 3,
        NavKey::Delete => 9,
    };
    ASCII[row][col]
}

/// #353, #617 e #912: as setas para cima e para baixo não existem na família ][ (nem nos clones,
/// que são `IS_APPLE2`); a tecla Delete só existe na //e e na Base 64A (a "F2" dela, #912).
#[test]
fn gh353_gh617_gh912_navigation_keys_follow_the_asciicode_table() {
    for model in MODELS {
        for key in [
            NavKey::Left,
            NavKey::Up,
            NavKey::Right,
            NavKey::Down,
            NavKey::Delete,
        ] {
            let mut m = Machine::new(apple2_core::MachineConfig::for_model(model));
            let code = cpp_nav(model, key);
            let typed = m.key_press_nav(key);
            assert_eq!(typed, code != 0, "{model:?} {key:?}");
            if typed {
                assert_eq!(m.board_mut().read(0xC000), 0x80 | code, "{model:?} {key:?}");
            } else {
                assert_eq!(m.board_mut().read(0xC000) & 0x80, 0, "sem strobe");
            }
        }
    }
    // pelo teclado do frontend: Up / Down / Delete viram Nav, sem caractere
    let mut k = KeyboardState::new();
    for (host, nav) in [
        (HostKey::Up, NavKey::Up),
        (HostKey::Down, NavKey::Down),
        (HostKey::Delete, NavKey::Delete),
    ] {
        let ev = k.handle(KeyInput::new(host), true, false, NONE);
        assert_eq!(ev[0], KeyEvent::Nav(nav));
        k.handle(KeyInput::new(host), false, false, NONE);
    }
}

/// #305 / #1156: o AppleWin devolve $08 para a tecla Backspace (WM_CHAR 8) e $7F para a tecla
/// Delete (VK_DELETE) e para Ctrl+Backspace (WM_CHAR 0x7F do Windows); a //e é a única com o
/// Delete. Sem opção para o Backspace virar $7F: o port repete isso.
#[test]
fn gh305_gh1156_backspace_is_08_delete_and_ctrl_backspace_are_7f() {
    let mut k = KeyboardState::new();
    let bs = KeyInput::new(HostKey::Backspace);
    assert_eq!(k.handle(bs, true, false, NONE)[0], KeyEvent::Ascii(0x08));
    k.handle(bs, false, false, NONE);
    assert_eq!(k.handle(bs, true, false, CTRL)[0], KeyEvent::Ascii(0x7F));
    k.handle(bs, false, false, CTRL);
    let (mut e, mut r) = rig(Model::Apple2eEnhanced, |_| {});
    press(&mut e, &mut r, KeyInput::new(HostKey::Delete), NONE);
    assert_eq!(rd(&mut e, 0xC000), 0xFF);
}

/// #92: as teclas de função do PC são atalhos do emulador (não chegam ao Apple, que não tem Fn):
/// nenhuma F1-F12, com ou sem Ctrl/Alt, vira tecla do Apple.
#[test]
fn gh92_function_keys_never_reach_the_apple_keyboard() {
    let mut k = KeyboardState::new();
    for n in 1..=12u8 {
        for mods in [NONE, CTRL, ALT, ALTGR, SHIFT] {
            let ev = k.handle(KeyInput::new(HostKey::F(n)), true, false, mods);
            assert!(ev.is_empty(), "F{n} {mods:?}: {ev:?}");
        }
    }
}

/// #1140 e #625 (§ e £): no AppleWin só os clones aceitam caracteres acima de $7F no WM_CHAR
/// (`!IS_CLONE() && key > 0x7F` descarta, Keyboard.cpp:139-140), então "éèçà" não digita numa //e
/// real do AppleWin. O port faz o mesmo; nos clones o caractere entra mascarado a 7 bits (:159-161).
#[test]
fn gh1140_accented_characters_are_not_typed_on_non_clones() {
    for c in ['é', 'è', 'ç', 'à', '§', '£'] {
        let code = char_to_ascii(c, NONE).unwrap();
        assert!(code > 0x7F);
        for model in [
            Model::Apple2e,
            Model::Apple2eEnhanced,
            Model::Apple2Plus,
            Model::Apple2,
        ] {
            let mut m = Machine::new(apple2_core::MachineConfig::for_model(model));
            assert!(!m.key_press(code), "{model:?} {c}");
            assert_eq!(m.board_mut().read(0xC000) & 0x80, 0);
        }
        let mut m = Machine::new(apple2_core::MachineConfig::for_model(Model::Pravets82));
        assert!(m.key_press(code), "clone {c}");
    }
}

/// #367 (item 1, a tecla |Mode| do TK3000 //e no Scroll Lock): com a tecla ligada os caracteres
/// acentuados são remapeados (Keyboard.cpp:166-192) e sem ela (> $7F) são descartados (:133-134).
#[test]
fn gh367_tk3000_mode_key_remaps_accented_characters() {
    fn cpp(key: u8, mode: bool, caps: bool) -> Option<u8> {
        let mut key = key;
        if key > 0x7F && !mode {
            return None;
        }
        // //e: Caps Lock em minúscula (:142-145); clone: keycode &= 0x7F (:159-161)
        let caps_key = if caps && key.is_ascii_lowercase() {
            key - 32
        } else {
            key
        };
        let mut keycode = caps_key & 0x7F;
        if mode {
            if (0xC0..=0xDA).contains(&key) {
                key += 0x20;
            }
            key = match key {
                0xE0 => b'_',
                0xE1 => b'@',
                0xE2 => b'\\',
                0xE3 => b'[',
                0xE7 => b']',
                0xE9 => b'`',
                0xEA => b'&',
                0xED => b'{',
                0xF3 => b'~',
                0xF4 => b'}',
                0xF5 => b'#',
                0xFA => b'|',
                k => k,
            };
            if key > 0x7F {
                return None;
            }
            keycode = if key.is_ascii_lowercase() && caps {
                key - 32
            } else {
                key
            };
        }
        Some(keycode)
    }
    for mode in [false, true] {
        for caps in [false, true] {
            for key in 0x20u8..=0xFF {
                let mut m = Machine::new(apple2_core::MachineConfig::for_model(Model::Tk3000e));
                m.set_caps_lock(caps);
                m.set_tk3000_mode(mode);
                // o port filtra antes o que o AppleWin deixa passar com o bit 7 mascarado
                let want = cpp(key, mode, caps);
                let got = m.key_press(key).then(|| m.board_mut().read(0xC000) & 0x7F);
                assert_eq!(got, want, "key {key:#04x} mode={mode} caps={caps}");
            }
        }
    }
    // o Scroll Lock liga e desliga o modo
    let mut k = KeyboardState::new();
    let sl = KeyInput::new(HostKey::ScrollLock);
    assert_eq!(
        k.handle(sl, true, false, NONE),
        vec![KeyEvent::ScrollLock(true)]
    );
}

/// #639 e #1367: F2 (`ResetMachineState`), Ctrl+F2 / Ctrl+Break (`CtrlReset`) e a troca de
/// hardware (`RepeatInitialization`) chamam `KeybReset()` (Keyboard.cpp:73-77): keycode = 0 e
/// sem strobe.
#[test]
fn gh639_gh1367_every_restart_clears_the_keyboard() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        // F2 e Ctrl+F2
        for power in [true, false] {
            let mut m = Machine::new(apple2_core::MachineConfig::for_model(model));
            m.key_press(b'a');
            assert_eq!(m.board_mut().read(0xC000), 0xC1);
            if power {
                m.power_cycle();
            } else {
                m.reset();
            }
            assert_eq!(m.board_mut().read(0xC000), 0x00, "{model:?} power={power}");
        }
        // troca de hardware: a máquina é reconstruída
        let (mut e, _r) = rig(model, |_| {});
        e.machine.key_press(b'a');
        assert_eq!(rd(&mut e, 0xC000), 0xC1);
        let s = e.settings.clone();
        e.apply(Command::ApplySettings(Box::new(s)));
        assert_eq!(rd(&mut e, 0xC000), 0x00, "{model:?}: reconstrução");
    }
}

/// #556 / #448: Alt+Enter alterna a tela cheia por padrão (WM_SYSKEYDOWN come a tecla,
/// WinFrame.cpp:1850-1855); com `-alt-enter=open-apple-enter` o Return chega ao Apple com o Open
/// Apple (Alt esquerdo) apertado ($C061) e o par Open Apple + Return é lido pelo programa.
#[test]
fn gh556_gh448_open_apple_return_reaches_the_apple_with_the_switch() {
    let on = parse(&["-alt-enter=toggle-full-screen"]);
    let off = parse(&["-alt-enter=open-apple-enter"]);
    assert!(on.video.alt_enter_toggles_fullscreen);
    assert!(!off.video.alt_enter_toggles_fullscreen);
    let enter = HostKey::Enter;
    assert_eq!(
        action_for_with(enter, ALT, true),
        Some(Action::ToggleFullscreen)
    );
    assert_eq!(action_for_with(enter, ALT, false), None);
    let (mut e, mut r) = rig(Model::Apple2eEnhanced, |_| {});
    press(&mut e, &mut r, KeyInput::new(HostKey::AltLeft), ALT);
    press(&mut e, &mut r, KeyInput::new(enter), ALT);
    assert_eq!(rd(&mut e, 0xC000), 0x8D, "Return com strobe");
    assert!(pb(&mut e, 0), "Open Apple apertado junto");
    assert!(!pb(&mut e, 1));
    // o teclado numérico como alternativa (#448): `0` e `.` são Open/Closed Apple
    let (mut e, mut r) = rig(Model::Apple2eEnhanced, |s| {
        s.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
    });
    press(&mut e, &mut r, np('0'), NONE);
    press(&mut e, &mut r, kb('\r'), NONE);
    assert!(pb(&mut e, 0));
}

/// #752: Ctrl+Alt+Break reinicia com o Open Apple apertado (Open Apple + Ctrl + Reset). Ctrl+Break
/// (`VK_CANCEL` com Ctrl) chama `CtrlReset` (Keyboard.cpp:198-203) sem olhar o Alt, e o Alt
/// continua sendo o botão 0 durante o reset.
#[test]
fn gh752_ctrl_alt_break_resets_with_open_apple_down() {
    let (mut e, mut r) = rig(Model::Apple2eEnhanced, |_| {});
    for _ in 0..30 {
        e.machine.run_frame();
    }
    press(&mut e, &mut r, KeyInput::new(HostKey::AltLeft), ALT);
    let cycles = e.machine.cycles();
    let mods = Mods {
        shift: false,
        ctrl: true,
        alt: true,
    };
    let ctx = KeyCtx {
        mods,
        ..KeyCtx::default()
    };
    let routed = r.key(KeyInput::new(HostKey::Pause), true, false, ctx, 0);
    assert!(
        routed
            .iter()
            .any(|r| matches!(r, Routed::Cmd(Command::Key(KeyEvent::Reset)))),
        "Ctrl+Alt+Break é o reset: {routed:?}"
    );
    feed(&mut e, routed);
    assert!(pb(&mut e, 0), "Open Apple continua apertado");
    assert_eq!(
        e.machine.cycles(),
        cycles,
        "reset suave: o contador de ciclos segue"
    );
    let rv = u16::from(rd(&mut e, 0xFFFD)) << 8 | u16::from(rd(&mut e, 0xFFFC));
    assert_eq!(e.machine.cpu().pc, rv, "a CPU foi ao vetor de RESET");
    // sem o Alt é o mesmo reset (Ctrl+Break), e a pausa sozinha não reinicia
    assert_eq!(action_for(HostKey::Pause, CTRL), None);
    assert_eq!(action_for(HostKey::Pause, NONE), Some(Action::TogglePause));
}

/// #749: AltGr+3 do teclado francês é Ctrl+Alt+3 no Windows e não pode ligar a velocidade máxima
/// (Ctrl-3). WinFrame.cpp:1356-1358: `KeybGetCtrlStatus() && !KeybGetAltStatus()`.
#[test]
fn gh749_altgr_digit_does_not_trigger_the_speed_hotkeys() {
    for c in '0'..='9' {
        let a = action_for(HostKey::Char(c), ALTGR);
        assert_eq!(a, None, "Ctrl+Alt+{c}");
        let ctrl_only = action_for(HostKey::Char(c), CTRL);
        let hot = matches!(c, '0' | '1' | '3');
        assert_eq!(ctrl_only.is_some(), hot, "Ctrl+{c}");
    }
    // e o '#' (AltGr+3) chega ao Apple com o caractere do layout
    let (mut e, mut r) = rig(Model::Apple2eEnhanced, |_| {});
    press(&mut e, &mut r, KeyInput::new(HostKey::AltRight), ALTGR);
    press(&mut e, &mut r, kb('#'), ALTGR);
    assert_eq!(rd(&mut e, 0xC000), 0xA3, "'#' com strobe");
}

/// #583 e #625: caracteres que exigem o AltGr (`[`, `{`, `@`, `#` do teclado italiano/francês/
/// belga) chegam ao Apple, e o Alt direito (Closed Apple, botão 1) vem junto; com `-no-hook-alt`
/// (CmdLine.cpp:552, Joystick.cpp:373) o Alt deixa de ser botão e fica só o caractere.
#[test]
fn gh583_gh625_altgr_characters_type_and_no_hook_alt_frees_the_button() {
    for (c, code) in [
        ('[', 0x5B),
        (']', 0x5D),
        ('{', 0x7B),
        ('}', 0x7D),
        ('@', 0x40),
        ('#', 0x23),
    ] {
        let (mut e, mut r) = rig(Model::Apple2eEnhanced, |_| {});
        press(&mut e, &mut r, KeyInput::new(HostKey::AltRight), ALTGR);
        press(&mut e, &mut r, kb(c), ALTGR);
        assert_eq!(rd(&mut e, 0xC000), 0x80 | code, "AltGr+{c}");
        assert!(pb(&mut e, 1), "AltGr é também o Closed Apple");
        let a = parse(&["-no-hook-alt"]);
        assert!(!a.joystick.hook_alt_keys);
        let (mut e, mut r) = rig(Model::Apple2eEnhanced, |s| s.joystick = a.joystick);
        press(&mut e, &mut r, KeyInput::new(HostKey::AltRight), ALTGR);
        press(&mut e, &mut r, kb(c), ALTGR);
        assert_eq!(rd(&mut e, 0xC000), 0x80 | code, "-no-hook-alt: AltGr+{c}");
        assert!(!pb(&mut e, 1), "-no-hook-alt: sem Closed Apple");
    }
}

/// #558: Closed Apple (Alt direito) + tecla entrega a tecla ao $C000. É o caminho que o AppleWin
/// liga com `-altgr-sends-wmchar` (Keyboard.cpp:231-252: a letra vira minúscula sem Shift e sem
/// Caps Lock, senão fica maiúscula; depois :142-145 maiúscula com Caps Lock na //e).
#[test]
fn gh558_closed_apple_plus_key_delivers_the_key() {
    for caps in [false, true] {
        for shift in [false, true] {
            for letter in 'a'..='z' {
                let upper = letter.to_ascii_uppercase();
                // C++: newKey
                let mut key = upper as u8;
                if !shift && !caps {
                    key += b'a' - b'A';
                }
                let want = if caps && key.is_ascii_lowercase() {
                    key - 32
                } else {
                    key
                };
                let (mut e, mut r) = rig(Model::Apple2eEnhanced, |_| {});
                e.machine.set_caps_lock(caps);
                press(&mut e, &mut r, KeyInput::new(HostKey::AltRight), ALT);
                let mods = Mods {
                    shift,
                    ctrl: false,
                    alt: true,
                };
                let c = if shift { upper } else { letter };
                press(&mut e, &mut r, kb(c), mods);
                assert_eq!(
                    rd(&mut e, 0xC000),
                    0x80 | want,
                    "caps={caps} shift={shift} {letter}"
                );
                assert!(pb(&mut e, 1), "Closed Apple apertado");
            }
        }
    }
}

/// #817: Ctrl+F3 / Ctrl+F4 abrem o menu do drive (WinFrame.cpp:1426-1434) sem o mouse.
#[test]
fn gh817_ctrl_f3_f4_open_the_drive_menus() {
    assert_eq!(action_for(HostKey::F(3), CTRL), Some(Action::DriveMenu(0)));
    assert_eq!(action_for(HostKey::F(4), CTRL), Some(Action::DriveMenu(1)));
    assert_eq!(
        action_for(HostKey::F(3), NONE),
        Some(Action::DriveSelect(0))
    );
    assert_eq!(
        action_for(HostKey::F(4), NONE),
        Some(Action::DriveSelect(1))
    );
}

/// #1396 (aberta no AppleWin): segurar o Alt esquerdo e apertar F2 não entrega o Open Apple ao
/// firmware. O F2 do AppleWin é `ResetMachineState`, que chama `JoyReset()` (Utilities.cpp:555) e
/// apaga `keydown[JK_OPENAPPLE]`; Ctrl+F2 (`CtrlReset`) não. O port deixa o Open Apple apertado
/// depois do F2 (o botão 0 segue nos botões da porta de jogos), então o Alt+F2 funciona aqui e
/// não no AppleWin: FAIL por divergência (a favor do usuário), sem correção no port por ser uma
/// escolha de produto (registrar como Divergência intencional ou replicar o defeito).
#[test]
#[ignore = "FAIL conhecido #1396: o AppleWin zera o Open Apple no F2 (JoyReset), o port não"]
fn gh1396_alt_f2_clears_open_apple_like_joyreset() {
    let (mut e, mut r) = rig(Model::Apple2eEnhanced, |_| {});
    press(&mut e, &mut r, KeyInput::new(HostKey::AltLeft), ALT);
    assert!(pb(&mut e, 0));
    e.apply(Command::PowerCycle); // F2
    assert!(!pb(&mut e, 0), "AppleWin: JoyReset() apagou o Open Apple");
}
