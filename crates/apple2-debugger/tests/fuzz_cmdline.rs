//! HU-0801 / issue #34: fuzz da linha de comando do depurador. O Cargo.lock não tem proptest, então
//! o gerador é um xorshift com semente fixa (reproduzível). Nenhuma entrada pode dar panic nem
//! travar; o que importa é voltar do `execute`.

use std::time::{Duration, Instant};

use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::cmdtable::COMMANDS;
use apple2_debugger::host::MemHost;
use apple2_debugger::{Debugger, RunMode};

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
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[self.below(v.len())]
    }
}

const TOKENS: &[&str] = &[
    "0", "1", "300", "$300", "#10", "FF", "FFFF", "FFFFFFFF", "ffffffffffffffffffffffffffffffff",
    "99999999999999999999999999", "-1", "-99999999999999999999", "0x10", "1e9", "300:310",
    "300,10", "300,0", "0:FFFF", "FFFF,FFFF", "FFFF:0", "0,10000", "1:2000", "s6/300", "L1/300",
    "ROM/D000", "$$", "((((((", "))))", "(1+", "1+", "+", "*", "@", "@@@", "#", "<", ">", "<<",
    ">>", "==", "!=", "=", "!", "&", "|", "^", "~", "/", "%", ":", "::", ",", ",,", ";", "'", "\"",
    "\"unterminated", "\"ok\"", "'a'", "\"\"", "\"x\",300", "é", "日本語", "😀", "\u{0}", "\u{200b}",
    "\u{202e}", "ÿÿ", "ǅ", "A2", "PC", "SP", "A", "X", "Y", "P", "RESET", "LOAD", "SAVE", "ON",
    "OFF", "ALL", "CLEAR", "LIST", "SYM", "MEM", "R", "W", "RW", "COLOR", "MONO", "BW", "NAME",
    "BASIC", "USER", "ASSEM", "x_y", "a.b", "..", "../..", "/", "C:\\x", "con", "asdf.txt",
    "Debugger.cfg", "?", "*?", "1..2", "..3", "9:9:9", "1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16",
];

const SEPS: &[&str] = &[" ", " ", " ", ",", "  ", "\t", ":", " = "];

fn gen_line(r: &mut Rng) -> String {
    match r.below(10) {
        // totalmente aleatório: bytes ASCII imprimíveis e alguns pontos de código Unicode
        0 => {
            let n = r.below(200);
            (0..n)
                .map(|_| match r.below(8) {
                    0 => char::from_u32(0x80 + r.below(0x2000) as u32).unwrap_or('?'),
                    1 => char::from_u32(0x1F300 + r.below(200) as u32).unwrap_or('?'),
                    _ => (0x20 + r.below(0x5F) as u8) as char,
                })
                .collect()
        }
        // linha muito longa
        1 => {
            let n = 80 + r.below(6000);
            let c = r.pick(&["A", "9", "(", "é", "😀", " ", "\"", "+"]);
            c.repeat(n)
        }
        _ => {
            let e = &COMMANDS[r.below(COMMANDS.len())];
            let mut name = e.name.to_string();
            match r.below(6) {
                0 => name = name.to_ascii_lowercase(),
                1 => name.truncate(name.len().div_ceil(2)),
                2 => name.push_str(r.pick(TOKENS)),
                _ => {}
            }
            let mut s = name;
            for _ in 0..r.below(7) {
                s.push_str(r.pick(SEPS));
                s.push_str(r.pick(TOKENS));
            }
            s
        }
    }
}

/// Comandos que rodam a CPU de forma síncrona (como no AppleWin: `P` repete o passo
/// `count` vezes, cada JSR até 0xFFFFF passos) ou dependem de volume; com argumentos enormes eles
/// são lentos por natureza (P 0:FFFF leva ~15 s), não travados. Ficam fora do fuzz de tempo.
fn heavy(line: &str) -> bool {
    let first = line
        .split(|c: char| !c.is_ascii_alphanumeric())
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(
        first.as_str(),
        "BENCH" | "BENCHMARK" | "STARTUP" | "G" | "GG" | "RUN" | "SOURCE" | "P" | "RTS"
    ) && line.len() > 3
}

fn fresh() -> (Machine, Debugger) {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut d = Debugger::new();
    d.set_host(Box::new(MemHost::default()));
    d.initialize(&mut m);
    d.enter(&mut m);
    (m, d)
}

fn run_fuzz(seed: u64, count: usize) -> usize {
    let mut r = Rng(seed);
    let (mut m, mut d) = fresh();
    let mut done = 0;
    for i in 0..count {
        let line = gen_line(&mut r);
        if heavy(&line) {
            continue;
        }
        let t = Instant::now();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            d.execute(&mut m, &line);
        }));
        assert!(res.is_ok(), "panic na linha #{i} (semente {seed}): {line:?}");
        assert!(
            t.elapsed() < Duration::from_secs(5),
            "linha #{i} demorou {:?} (semente {seed}): {line:?}",
            t.elapsed()
        );
        done += 1;
        d.mode = RunMode::Debug;
        d.steps = 0;
        d.asm_active = false;
        d.console.clear();
        if i % 2000 == 1999 {
            (m, d) = fresh();
        }
    }
    done
}

#[test]
fn fuzz_command_line_never_panics_or_hangs() {
    let mut total = 0;
    // FUZZ_N=<linhas por semente> para rodadas mais longas fora do CI.
    let n = std::env::var("FUZZ_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);
    for seed in [0x9E37_79B9_7F4A_7C15u64, 0xDEAD_BEEF_CAFE_F00D, 0x1234_5678_9ABC_DEF1] {
        total += run_fuzz(seed, n);
    }
    assert!(total > 40_000, "{total}");
    println!("fuzz: {total} linhas executadas");
}

/// Teclas direto no console (entrada de caracteres, backspace, Enter) com lixo Unicode.
#[test]
fn fuzz_console_input_edit_never_panics() {
    let mut r = Rng(0xABCD_EF01_2345_6789);
    let (mut m, mut d) = fresh();
    for _ in 0..3000 {
        for _ in 0..r.below(300) {
            match r.below(10) {
                0 => {
                    d.console.backspace();
                }
                1 => {
                    d.console.history_prev();
                }
                2 => {
                    d.console.history_next();
                }
                _ => {
                    let c = char::from_u32(r.below(0x3000) as u32).unwrap_or('x');
                    d.console.input_char(c);
                }
            }
        }
        if !heavy(&d.console.input()) {
            d.input_enter(&mut m);
        } else {
            d.console.clear_input();
        }
        d.mode = RunMode::Debug;
        d.steps = 0;
        d.asm_active = false;
    }
}
