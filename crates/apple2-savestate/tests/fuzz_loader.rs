//! HU-0401 / AUD-C-010 / AUD-C-011: fuzzing rápido do carregador `.aws.yaml`, sem dependência nova
//! (o lock não tem proptest): um gerador xorshift com semente fixa produz arquivos truncados,
//! campos com tipo trocado, números enormes ou negativos e estados de máquina com valores extremos.
//! Nenhum caso pode dar panic, travar ou alocar sem limite. Rode em debug (`cargo test`) e em
//! release (`cargo test --release`): em debug a aritmética estourada vira panic.

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::ext::{MAX_COMPRESSED_LEN, MAX_JSON_LEN};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::image::DiskImage;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

struct NoImages;
impl LoadHost for NoImages {
    fn open_image(&mut self, _r: &ImageRequest) -> Result<DiskImage, String> {
        Err("sem imagens".into())
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const HOSTILE: &[&str] = &[
    "-1",
    "0",
    "0xFFFFFFFFFFFFFFFF",
    "0xFFFFFFFF",
    "99999999999999999999999",
    "-99999999999999999999999",
    "4294967296",
    "18446744073709551615",
    "abc",
    "",
    "true",
    "\"\"",
    "~",
    "0x",
    "1e400",
    "[1,2]",
    "{a: b}",
];

fn base_text(extension: bool) -> String {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    m.run_cycles(50_000);
    save_aws_yaml(
        &m,
        &SaveOptions {
            extension,
            ..Default::default()
        },
    )
    .unwrap()
    .text
}

/// Loads `text`, which must neither panic nor take long; then runs the machine briefly.
fn load_and_run(text: &str) -> Result<(), String> {
    let t0 = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| {
        if let Ok(mut l) = load_aws_yaml(text, &mut NoImages) {
            l.machine.run_cycles(40_000);
            l.machine.run_frame();
        }
    }));
    if r.is_err() {
        return Err("panic".into());
    }
    if t0.elapsed() > Duration::from_secs(20) {
        return Err(format!("lento: {:?}", t0.elapsed()));
    }
    Ok(())
}

fn mutate(text: &str, rng: &mut Rng) -> String {
    let lines: Vec<&str> = text.lines().collect();
    match rng.below(6) {
        // truncated
        0 => {
            let mut c = rng.below(text.len());
            while !text.is_char_boundary(c) {
                c -= 1;
            }
            text[..c].to_string()
        }
        // value replaced by a hostile one (type swapped, huge, negative)
        1 | 2 | 3 => {
            let mut v: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            for _ in 0..1 + rng.below(3) {
                let i = rng.below(v.len());
                if let Some((k, _)) = v[i].split_once(':') {
                    if !k.trim().is_empty() && !k.trim().chars().all(|c| c.is_ascii_hexdigit()) {
                        v[i] = format!("{k}: {}", HOSTILE[rng.below(HOSTILE.len())]);
                    }
                }
            }
            v.join("\n")
        }
        // line deleted or duplicated
        4 => {
            let mut v: Vec<&str> = lines.clone();
            let i = rng.below(v.len());
            if rng.below(2) == 0 {
                v.remove(i);
            } else {
                v.insert(i, lines[i]);
            }
            v.join("\n")
        }
        // a byte flipped (kept valid UTF-8 by going through lossy)
        _ => {
            let mut b = text.as_bytes().to_vec();
            let i = rng.below(b.len());
            b[i] = rng.next() as u8;
            String::from_utf8_lossy(&b).into_owned()
        }
    }
}

#[test]
fn mutated_documents_never_panic_or_hang() {
    for extension in [false, true] {
        let text = base_text(extension);
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ extension as u64);
        let mut bad = Vec::new();
        for n in 0..(if cfg!(debug_assertions) { 80 } else { 250 }) {
            let doc = mutate(&text, &mut rng);
            if let Err(e) = load_and_run(&doc) {
                bad.push(format!("#{n}: {e}"));
            }
        }
        assert!(bad.is_empty(), "extensão={extension}: {bad:?}");
    }
}

/// Replaces the `Length` of the extension block with `v` and returns the document.
fn with_length(text: &str, v: &str) -> String {
    let mut out = Vec::new();
    for l in text.lines() {
        if l.trim_start().starts_with("Length:") {
            let ind = &l[..l.len() - l.trim_start().len()];
            out.push(format!("{ind}Length: {v}"));
        } else {
            out.push(l.to_string());
        }
    }
    out.join("\n")
}

#[test]
fn hostile_extension_length_is_refused_with_a_descriptive_error() {
    let text = base_text(true);
    assert!(text.contains("AppleII-Rust Extension"));
    for v in [
        "-1",
        "0xFFFFFFFFFFFF",
        "18446744073709551615",
        "99999999999999999999999",
        &(MAX_COMPRESSED_LEN + 1).to_string(),
        "1000000000",
    ] {
        let doc = with_length(&text, v);
        let r = catch_unwind(AssertUnwindSafe(|| load_aws_yaml(&doc, &mut NoImages)));
        let r = r.unwrap_or_else(|_| panic!("panic com Length {v}"));
        let err = r.err().unwrap_or_else(|| panic!("Length {v} foi aceito"));
        // A value that overflows 64 bits wraps (strtoul) to something small: an error all the same.
        if !v.starts_with("9999") {
            assert!(
                err.to_string().contains("AppleII-Rust Extension"),
                "Length {v}: {err}"
            );
        }
    }
    // Length 0 and 1: too short for a zlib stream, also an error, never a panic.
    for v in ["0", "1", "7"] {
        let doc = with_length(&text, v);
        assert!(catch_unwind(AssertUnwindSafe(|| load_aws_yaml(&doc, &mut NoImages))).is_ok());
    }
}

#[test]
fn a_zlib_bomb_in_the_extension_is_bounded() {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    // 300 MiB of zeros deflate to ~300 KiB: small Length, huge expansion.
    let mut z = ZlibEncoder::new(Vec::new(), Compression::best());
    let zeros = vec![0u8; 1 << 20];
    for _ in 0..(MAX_JSON_LEN / (1 << 20) + 44) {
        z.write_all(&zeros).unwrap();
    }
    let data = z.finish().unwrap();
    assert!(data.len() < MAX_COMPRESSED_LEN as usize);
    let text = base_text(true);
    // Splice the bomb in place of the real data.
    let at = text.find("      Data:").or_else(|| text.find("Data:")).unwrap();
    let head_end = text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let _ = head_end;
    let mut doc = String::new();
    let mut in_data = false;
    let mut data_indent = 0;
    for l in text.lines() {
        let ind = l.len() - l.trim_start().len();
        if in_data {
            if ind > data_indent {
                continue; // old data line
            }
            in_data = false;
        }
        if l.trim_start().starts_with("Length:") {
            doc.push_str(&format!("{}Length: {}\n", &l[..ind], data.len()));
        } else if l.trim_start() == "Data:" {
            doc.push_str(l);
            doc.push('\n');
            data_indent = ind;
            in_data = true;
            let mut padded = data.clone();
            padded.resize(padded.len().div_ceil(8) * 8, 0);
            for (i, chunk) in padded.chunks(64).enumerate() {
                let hex: String = chunk.iter().map(|b| format!("{b:02X}")).collect();
                doc.push_str(&format!("{}{:08X}: {hex}\n", " ".repeat(ind + 2), i * 64));
            }
        } else {
            doc.push_str(l);
            doc.push('\n');
        }
    }
    let r = catch_unwind(AssertUnwindSafe(|| load_aws_yaml(&doc, &mut NoImages)));
    let err = r.expect("panic").err().expect("a bomba foi aceita");
    assert!(err.to_string().contains("limit"), "{err}");
}

/// Numbers inside the machine state replaced by extreme values: the machine must survive.
#[test]
fn extreme_values_in_the_machine_state_never_overflow() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    m.run_cycles(30_000);
    let json = serde_json::to_value(m.save_state()).unwrap();
    let mut rng = Rng(0xDEAD_BEEF_1234_5678);
    let extremes: [serde_json::Value; 6] = [
        u64::MAX.into(),
        u32::MAX.into(),
        (u32::MAX as u64 - 1).into(),
        0.into(),
        65_535.into(),
        (i64::MIN).into(),
    ];
    let mut bad = Vec::new();
    let mut applied = 0;
    for n in 0..300 {
        let mut v = json.clone();
        let mut leaves = Vec::new();
        collect_numbers(&v, &mut Vec::new(), &mut leaves);
        for _ in 0..1 + rng.below(3) {
            let path = leaves[rng.below(leaves.len())].clone();
            set_path(&mut v, &path, extremes[rng.below(extremes.len())].clone());
        }
        let Ok(st) = serde_json::from_value::<apple2_core::MachineState>(v) else {
            continue; // wrong type for the field: refused by the decoder
        };
        applied += 1;
        let r = catch_unwind(AssertUnwindSafe(|| {
            let mut m2 = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
            if m2.load_state(&st).is_ok() {
                m2.run_cycles(40_000);
                m2.run_frame();
            }
        }));
        if r.is_err() {
            bad.push(n);
        }
    }
    assert!(applied > 20, "poucos casos aplicados: {applied}");
    assert!(bad.is_empty(), "panic nos casos {bad:?}");
}

type Path = Vec<Result<String, usize>>;

fn collect_numbers(v: &serde_json::Value, p: &mut Path, out: &mut Vec<Path>) {
    use serde_json::Value::*;
    match v {
        Number(_) => out.push(p.clone()),
        Array(a) => {
            // Large byte arrays (RAM) are sampled, not enumerated.
            for (i, x) in a.iter().enumerate().take(64) {
                p.push(Err(i));
                collect_numbers(x, p, out);
                p.pop();
            }
        }
        Object(o) => {
            for (k, x) in o {
                p.push(Ok(k.clone()));
                collect_numbers(x, p, out);
                p.pop();
            }
        }
        _ => {}
    }
}

fn set_path(v: &mut serde_json::Value, p: &Path, new: serde_json::Value) {
    let mut cur = v;
    for step in p {
        cur = match step {
            Ok(k) => &mut cur[k.as_str()],
            Err(i) => &mut cur[*i],
        };
    }
    *cur = new;
}
