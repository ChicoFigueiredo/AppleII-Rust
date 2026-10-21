//! AUD-P-033 (and AUD-P-023/030 by the same path): a host I/O failure inside a card reaches the
//! frontend's status line while the emulation goes on. AppleWin only logs these.

use apple2_config::card::PrinterSpec;
use apple2_config::{Settings, SlotCardSpec};
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::Emulator;

#[test]
fn a_printer_file_that_cannot_be_opened_shows_a_message_and_emulation_continues() {
    let dir = std::env::temp_dir().join(format!("applewin_hostio_err_{}", std::process::id()));
    let bad = dir.join("no_such_dir").join("Printer.txt");
    let mut s = Settings::default();
    s.slots.0[6] = SlotCardSpec::Empty;
    s.slots.0[1] = SlotCardSpec::GenericPrinter(PrinterSpec {
        filename: Some(bad.clone()),
        ..PrinterSpec::default()
    });
    let mut emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();

    emu.run_frame();
    assert_eq!(emu.status().message_seq, 0, "nothing to say before the card is used");

    // A byte to the printer card: the file does not open.
    emu.machine.with_card(1, |c, ctx| c.io_write(0, b'X', ctx));
    let (cycles, _) = emu.run_frame(); // the frame itself is not affected
    assert!(cycles > 0);
    emu.run_frame();
    let st = emu.status();
    assert!(st.message_seq > 0, "the failure was announced");
    assert!(
        st.message.starts_with("Printer: cannot open printer file")
            && st.message.contains("Printer.txt"),
        "{}",
        st.message
    );
}
