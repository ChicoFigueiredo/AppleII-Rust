//! Printer Center: GPU-free behaviour seen through the public API.

use apple2_rust_shell::actions::{action_for, Action};
use apple2_rust_shell::keymap::{HostKey, Mods};
use apple2_rust_shell::printer_window::{route, PrinterWindow, Route};
use apple2_rust_shell::toolbar::Button;

#[test]
fn the_printer_button_has_no_function_key() {
    // F9 stays the video mode (AppleWin); the button is reachable by mouse only.
    assert_eq!(Button::Printer.fkey(), None);
    assert_eq!(Button::Printer.action(), Action::PrinterCenter);
    for n in 1..=12 {
        assert_ne!(
            action_for(HostKey::F(n), Mods::default()),
            Some(Action::PrinterCenter)
        );
    }
}

#[test]
fn keys_of_the_secondary_window_never_reach_the_emulator() {
    let (main, printer) = (10u64, 20u64);
    for id in [main, printer, 99] {
        let r = route(id, Some(main), Some(printer));
        assert_eq!(r.reaches_emulator(), id == main);
    }
    // once the Central is closed its old id is unknown, not the main window
    assert_eq!(route(printer, Some(main), None), Route::Unknown);
}

#[test]
fn opening_and_closing_never_needs_a_gpu() {
    let mut w = PrinterWindow::default();
    w.open(true);
    assert!(w.is_open());
    w.close();
    assert!(!w.is_open());
}
