//! Acceptance of the Transports with ESC K bit-image. An Applesoft program loads a 14-byte
//! machine-code loop (`LDX #0 / LDA $0320,X / JSR $FDED / INX / CPX #n / BNE / RTS`) and the data
//! `ESC K 8 0 <8 columns> CR LF`, then calls it with `PR#1` active, exactly like a driver would.
//! With bit 7 kept the top pin of each column prints; the 7-bit Transport (the AppleWin Generic
//! Printer rule) loses it.

mod printer_aceite_common;

use apple2_core::Model;
use apple2_printer::capture::Transport;
use apple2_printer::paper::RenderOptions;
use printer_aceite_common::*;

const COLUMNS: [u8; 8] = [0xFF, 0x80, 0xAA, 0x55, 0x81, 0xFF, 0x01, 0x7F];

fn run(transport: Transport, direct: bool) -> Rig {
    let mut r = Rig::new(Model::Apple2eEnhanced, transport);
    // Loop: 14 bytes at $0300 (768); data at $0320 (800): ESC K nL nH, columns, CR LF.
    // Through the firmware: JSR $FDED (COUT). Direct: STA $C090 (the card's data port, slot 1).
    let code = if direct {
        "162,0,189,32,3,141,144,192,232,224,14,208,245,96"
    } else {
        "162,0,189,32,3,32,237,253,232,224,14,208,245,96"
    };
    r.type_line(&format!("10 DATA {code}"));
    r.type_line("20 FOR I=0 TO 13:READ A:POKE 768+I,A:NEXT");
    let data = "27,75,8,0,255,128,170,85,129,255,1,127,13,10";
    r.type_line(&format!("30 DATA {data}"));
    r.type_line("40 FOR I=0 TO 13:READ A:POKE 800+I,A:NEXT");
    // CTRL-I 255N turns the card firmware's line formatting off; without it the firmware pads
    // the stream with spaces to match the monitor cursor column ($24) and corrupts the graphics.
    if direct {
        // A driver writing to the port does not go through the firmware: no PR#1, no Ctrl-I.
        r.type_line("50 CALL 768");
    } else {
        r.type_line("50 PR#1:PRINT CHR$(9);\"255N\";:CALL 768:PR#0");
    }
    r.type_line("RUN");
    r.run_until_closed(1500);
    r
}

fn dots_of(r: &Rig) -> Vec<(u32, u32)> {
    let doc = render_raw(&r.raw(), &lx310_factory());
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    assert_eq!(doc.pages.len(), 1);
    doc.pages[0].dots.iter().map(|d| (d.x, d.y)).collect()
}

#[test]
fn auto_direct_driver_keeps_bit_7_firmware_path_loses_it() {
    // Auto: a driver writing straight to $C090 arrives whole (all 8 bits); the same bytes sent
    // through the card firmware (COUT) have bit 7 cleared.
    let r8 = run(Transport::Auto, true);
    let r7 = run(Transport::Auto, false);
    let fixed8 = run(Transport::EightBit, false);
    let fixed7 = run(Transport::SevenBitAppleWin, true);
    assert_eq!(
        fixed8.raw()[..12],
        r8.raw()[..12],
        "8 bits fixo = Auto direto"
    );
    assert_eq!(
        fixed7.raw()[..12],
        r7.raw()[..12],
        "7 bits fixo = Auto via firmware"
    );

    let mut want8 = vec![0x1B, 0x4B, 0x08, 0x00];
    want8.extend_from_slice(&COLUMNS);
    let raw8 = r8.raw();
    assert!(raw8.starts_with(&want8), "8-bit stream: {raw8:02x?}");
    let want7: Vec<u8> = want8.iter().map(|b| b & 0x7F).collect();
    assert!(
        r7.raw().starts_with(&want7),
        "7-bit stream: {:02x?}",
        r7.raw()
    );
    golden("aceite_escK_8bit.raw", &raw8);
    golden("aceite_escK_7bit.raw", &r7.raw());

    let d8 = dots_of(&r8);
    let d7 = dots_of(&r7);
    let ones = |cols: &[u8]| cols.iter().map(|c| c.count_ones() as usize).sum::<usize>();
    assert_eq!(d8.len(), ones(&COLUMNS));
    let masked: Vec<u8> = COLUMNS.iter().map(|c| c & 0x7F).collect();
    assert_eq!(d7.len(), ones(&masked));
    // Five columns had bit 7 set (0xFF, 0x80, 0xAA, 0x81, 0xFF): five top-pin dots are lost.
    assert_eq!(d8.len() - d7.len(), 5);
    // The top pin is the highest dot row: it exists only in the 8-bit page.
    let top8 = d8.iter().map(|d| d.1).min().unwrap();
    let top7 = d7.iter().map(|d| d.1).min().unwrap();
    assert_eq!(top7 - top8, 30, "uma agulha (1/72\") abaixo");

    // Goldens of the logical documents.
    golden(
        "aceite_escK_8bit.doc.txt",
        summary(&render_raw(&raw8, &placeholder_factory())).as_bytes(),
    );
    golden(
        "aceite_escK_7bit.doc.txt",
        summary(&render_raw(&r7.raw(), &placeholder_factory())).as_bytes(),
    );

    // Same through the Spool and the archive (the real pipeline), 8-bit.
    let dir = tempfile::tempdir().unwrap();
    let mut spool = spool_events(dir.path(), &r8.events);
    let snap = spool.snapshot_handle().get();
    assert_eq!(snap.jobs.len(), 1);
    let doc = spool
        .render(&snap.jobs[0].id, RenderOptions::default())
        .unwrap();
    assert_eq!(doc.pages[0].dots.len(), ones(&COLUMNS));
}
