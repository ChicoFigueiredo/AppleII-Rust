//! T-027: 10,000 random ESC sequences with bounded limits: no panic, no loop, no unbounded memory,
//! and the same document for any split of the stream. A fixed-seed xorshift generator keeps the
//! run deterministic and dependency-free (no proptest needed).

mod lx310_common;
use apple2_printer::engine::PrinterEngine;
use apple2_printer::lx310::{Glyphs, Lx310, Lx310Config};
use apple2_printer::paper::{Paper, PaperKind, RenderOptions};
use lx310_common::hash;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn byte(&mut self) -> u8 {
        self.next() as u8
    }
}

const COMMANDS: &[u8] = b"@0123456789ACDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~ !#$%&'()*+,-./:;<=>?";

fn gen(rng: &mut Rng) -> Vec<u8> {
    let mut v = Vec::new();
    let n = 1 + rng.below(300) as usize;
    while v.len() < n {
        match rng.below(10) {
            0..=3 => {
                // ESC + a command byte + random parameters.
                v.push(0x1B);
                v.push(COMMANDS[rng.below(COMMANDS.len() as u64) as usize]);
                for _ in 0..rng.below(5) {
                    v.push(rng.byte());
                }
            }
            4 => {
                // Graphics with a declared count that may exceed the stream.
                v.extend([0x1B, b"KLYZ*^"[rng.below(6) as usize]]);
                if v[v.len() - 1] >= b'*' && v[v.len() - 1] != b'Z' {
                    v.push(rng.below(9) as u8);
                }
                v.push(rng.byte());
                v.push(if rng.below(3) == 0 { rng.byte() } else { 0 });
                for _ in 0..rng.below(40) {
                    v.push(rng.byte());
                }
            }
            5 => {
                v.extend([0x1B, b"DB("[rng.below(3) as usize]]);
                for _ in 0..rng.below(50) {
                    v.push(rng.byte());
                }
            }
            6 => v.push(0x1B),
            7 => v.push(
                [0x0A, 0x0C, 0x0D, 0x09, 0x0B, 0x0E, 0x0F, 0x12, 0x14, 0x08]
                    [rng.below(10) as usize],
            ),
            _ => v.push(rng.byte()),
        }
    }
    v
}

fn config() -> Lx310Config {
    Lx310Config {
        glyphs: Glyphs::FontWithPlaceholderFallback,
        max_pages: 40,
        max_dots: 100_000,
        max_diagnostics: 500,
        ..Lx310Config::default()
    }
}

fn run(stream: &[u8], chunks: &[usize]) -> apple2_printer::document::PageDocument {
    let mut e = Lx310::new(config());
    e.begin(RenderOptions {
        paper: Paper::preset(PaperKind::GreenBar),
        ..RenderOptions::default()
    });
    let mut off = 0usize;
    let mut i = 0;
    while off < stream.len() {
        let c = chunks[i % chunks.len()].max(1).min(stream.len() - off);
        e.feed(off as u64, &stream[off..off + c]);
        off += c;
        i += 1;
    }
    e.finish();
    e.document().clone()
}

#[test]
fn t027_ten_thousand_random_streams() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for case in 0..10_000 {
        let stream = gen(&mut rng);
        let whole = run(&stream, &[stream.len()]);
        let split = run(&stream, &[1 + (case % 7), 3, 1]);
        assert_eq!(
            hash(&whole),
            hash(&split),
            "case {case} stream {stream:02X?}"
        );
        assert!(whole.pages.len() <= 40);
        assert!(whole.diagnostics.len() <= 501);
        let dots: usize = whole.pages.iter().map(|p| p.dots.len()).sum();
        assert!(dots <= 100_000);
        for p in &whole.pages {
            for d in &p.dots {
                assert!(
                    d.x < p.width && d.y < p.height,
                    "dot out of the page in case {case}"
                );
            }
        }
    }
}

#[test]
fn t027_pathological_streams_stay_bounded() {
    // Declared maximum graphics with no data; endless ESCs; endless ESC D; FF flood; huge payload.
    let mut cases: Vec<Vec<u8>> = vec![
        vec![0x1B, b'K', 0xFF, 0xFF],
        vec![0x1B; 100_000],
        [vec![0x1B, b'D'], vec![1; 50_000]].concat(),
        vec![0x0C; 100_000],
        [vec![0x1B, b'(', b'U', 0xFF, 0xFF], vec![0xAA; 70_000]].concat(),
        [vec![0x1B, b'K', 0xFF, 0xFF], vec![0xFF; 70_000]].concat(),
        [vec![0x1B, b'^', 0xFF, 0xFF], vec![0xFF; 140_000]].concat(),
        b"A".repeat(200_000),
        [vec![0x1B, b'W', 1, 0x1B, b' ', 127], b"M".repeat(50_000)].concat(),
    ];
    // Many graphics lines to hit the dot cap.
    let mut many = Vec::new();
    for _ in 0..200 {
        many.extend([0x1B, b'K', 0xFF, 0x03]);
        many.extend(vec![0xFF; 1023]);
        many.push(0x0A);
    }
    cases.push(many);
    for (i, c) in cases.iter().enumerate() {
        let d = run(c, &[c.len()]);
        let dots: usize = d.pages.iter().map(|p| p.dots.len()).sum();
        assert!(dots <= 100_000, "case {i}");
        assert!(d.pages.len() <= 40, "case {i}");
        assert!(d.diagnostics.len() <= 501, "case {i}");
    }
}
