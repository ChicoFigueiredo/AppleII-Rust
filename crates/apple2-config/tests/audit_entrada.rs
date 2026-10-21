//! #434 ("Choosing Thumbstick #2 causes crash in older AppleWin's"): o valor 5 de
//! `Joystick1 Emu Type` derrubava o AppleWin antigo (o máximo era 4). A correção do AppleWin
//! (1.26.3.0) passou a gravar em chaves novas (`... Emu Type v3`), lê as antigas só como
//! fallback (Utilities.cpp `LoadConfiguration`) e ignora valores fora da faixa (`JoySetJoyType`).
//! Oráculo: AppleWin a8f0d1d. Resultados em `docs/conformance/results/entrada-cartas.csv`.

use apple2_config::import::import_ini;
use apple2_config::registry_map::lookup;
use apple2_config::settings::{Joystick0Kind, Joystick1Kind};

#[test]
fn gh434_thumbstick2_is_stored_under_the_v3_key() {
    let s = import_ini("[Configuration]\nJoystick1 Emu Type v3=5\n");
    assert_eq!(
        s.joystick.joystick1,
        Joystick1Kind::HostJoystick1Thumbstick2
    );
    // as duas chaves existem no mapa do registro; só a v3 é a corrente
    let new = lookup("Configuration", "Joystick1 Emu Type v3").unwrap();
    let old = lookup("Configuration", "Joystick1 Emu Type").unwrap();
    assert!(!new.note.contains("deprecated"));
    assert!(old.note.contains("deprecated GH#434"));
}

#[test]
fn gh434_old_key_is_only_a_fallback_and_v3_wins() {
    let s = import_ini("[Configuration]\nJoystick1 Emu Type=4\n");
    assert_eq!(s.joystick.joystick1, Joystick1Kind::Mouse);
    let s = import_ini("[Configuration]\nJoystick1 Emu Type=4\nJoystick1 Emu Type v3=2\n");
    assert_eq!(s.joystick.joystick1, Joystick1Kind::KeyboardCursors);
    let s = import_ini("[Configuration]\nJoystick0 Emu Type=2\n");
    assert_eq!(s.joystick.joystick0, Joystick0Kind::KeyboardCursors);
}

#[test]
fn gh434_out_of_range_types_are_ignored_not_fatal() {
    // JoySetJoyType: joystick 0 tem 5 tipos (0-4), o 1 tem 6 (0-5); fora disso o valor é ignorado
    for ini in [
        "[Configuration]\nJoystick0 Emu Type v3=5\n",
        "[Configuration]\nJoystick0 Emu Type v3=4294967295\n",
        "[Configuration]\nJoystick1 Emu Type v3=6\n",
        "[Configuration]\nJoystick1 Emu Type v3=-1\n",
        "[Configuration]\nJoystick1 Emu Type=7\n",
    ] {
        let s = import_ini(ini);
        let d = apple2_config::Settings::default();
        assert_eq!(s.joystick.joystick0, d.joystick.joystick0, "{ini:?}");
        assert_eq!(s.joystick.joystick1, d.joystick.joystick1, "{ini:?}");
    }
}
