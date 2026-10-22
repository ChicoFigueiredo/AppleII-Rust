//! Prints raw printer bytes on a virtual Epson LX-310 and saves the pages as PDF.
//!
//! `cargo run -p apple2-printer --example imprimir -- <saida.pdf> [bytes.raw]`
//! Without `bytes.raw`, prints a built-in demonstration page (text modes and a bit-image band).

use apple2_printer::engine::PrinterEngine;
use apple2_printer::lx310::{Lx310, Lx310Config};
use apple2_printer::paper::RenderOptions;
use apple2_printer::pdf::{self, PageSelection, PdfMeta};

/// A three-row frame (`[top-left, horizontal, top-right, vertical, bottom-left, bottom-right]`)
/// around `text`, as raw PC437 bytes with CR LF at the end of every row.
fn frame(c: [u8; 6], text: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let edge = |l: u8, r: u8, out: &mut Vec<u8>| {
        out.push(l);
        out.extend(std::iter::repeat_n(c[1], text.len()));
        out.extend_from_slice(&[r, b'\r', b'\n']);
    };
    edge(c[0], c[2], &mut out);
    for _ in 0..2 {
        out.push(c[3]);
        out.extend_from_slice(text);
        out.extend_from_slice(&[c[3], b'\r', b'\n']);
    }
    edge(c[4], c[5], &mut out);
    out
}

fn demo() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"\x1b@");
    b.extend_from_slice(b"\x1bE\x1bW1APPLE II-RUST\x1bW0\x1bF\r\n\r\n");
    b.extend_from_slice(b"10 REM IMPRESSORA VIRTUAL EPSON LX-310\r\n");
    b.extend_from_slice(
        b"20 FOR I = 1 TO 10\r\n30 PRINT \"OLA, IMPRESSORA\"; I\r\n40 NEXT I\r\n\r\n",
    );
    b.extend_from_slice(
        b"Normal 10 CPI   \x1bE Negrito \x1bF   \x1b-1Sublinhado\x1b-0   \x1b4Italico\x1b5\r\n",
    );
    b.extend_from_slice(b"\x1bM12 CPI: The quick brown fox jumps over the lazy dog.\x1bP\r\n");
    b.extend_from_slice(
        b"\x0fCondensado: The quick brown fox jumps over the lazy dog 0123456789.\x12\r\n",
    );
    b.extend_from_slice(b"\x1bx1NLQ: Near Letter Quality - The quick brown fox.\x1bx0\r\n\r\n");
    // Frame in PC437 (single and double lines): vertical strokes continue between the lines.
    b.extend_from_slice(b"\x1bt\x01");
    b.extend(frame(
        [0xDA, 0xC4, 0xBF, 0xB3, 0xC0, 0xD9],
        b" Moldura PC437 ",
    ));
    b.extend(frame(
        [0xC9, 0xCD, 0xBB, 0xBA, 0xC8, 0xBC],
        b" Linha dupla   ",
    ));
    // PC850 assigned to slot 1 (ESC ( t), then selected (ESC t 1): accents in Portuguese.
    b.extend_from_slice(b"\x1b(t\x03\x00\x01\x03\x00\x1bt\x01");
    b.extend_from_slice(
        b"\r\nPC850: A\x87\xC6o, cora\x87\xC6o, \x93nibus, \xA1ndio, \x82 \xA3til, \xA8Quem? 1\xA6 2\xA7 25\xF8C \xF1 \xFD \xF6 \xFC \x9E \xE6 \xF5\r\n",
    );
    // Double width (ESC W 1) line.
    b.extend_from_slice(b"\x1bW1Largura dupla\x1bW0\r\n\r\n");
    // A bit-image band: a sine-like wave in 8-pin 120 dpi graphics (ESC L).
    let cols = 480u16;
    b.extend_from_slice(&[0x1b, b'L', (cols & 0xff) as u8, (cols >> 8) as u8]);
    for i in 0..cols {
        let y = ((i as f64 / 24.0).sin() * 3.5 + 3.5).round() as u32;
        b.push(0x80u8 >> y);
    }
    b.extend_from_slice(b"\r\n\x0c");
    b.extend_from_slice(b"Segunda pagina, depois do FF.\r\n");
    b
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(out) = args.first().filter(|a| !a.starts_with('-')) else {
        eprintln!("uso: imprimir <saida.pdf> [bytes.raw]");
        std::process::exit(2);
    };
    let bytes = match args.get(1) {
        Some(p) => std::fs::read(p).unwrap_or_else(|e| {
            eprintln!("{p}: {e}");
            std::process::exit(1)
        }),
        None => demo(),
    };
    let opts = RenderOptions::default();
    let mut lx = Lx310::new(Lx310Config::default());
    lx.begin(opts);
    lx.feed(0, &bytes);
    lx.finish();
    let doc = lx.document();
    for d in &doc.diagnostics {
        eprintln!("diagnóstico @{}: {} {:02X?}", d.offset, d.message, d.bytes);
    }
    let meta = PdfMeta {
        title: "Demonstração da Epson LX-310".into(),
        ..PdfMeta::default()
    };
    match pdf::export_to_path(out.as_ref(), doc, &opts, &meta, &PageSelection::All, true) {
        Ok(_) => println!("{} página(s) em {out}", doc.pages.len()),
        Err(e) => {
            eprintln!("{out}: {e}");
            std::process::exit(1)
        }
    }
}
