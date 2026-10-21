//! Primeira medição de carga do corpus AppleWin-Test (HU-0002 / HU-0401, passo 1).
//!
//! Para cada `.aws.yaml` do índice `tests/conformance/inputs/applewin-test-index.csv`, carrega o
//! save-state, monta a Máquina e roda 1 quadro (17.030 ciclos). O resultado é IMPLEMENTED
//! ("carrega sem crash"), NUNCA PASS de fidelidade: PASS exige comparar o estado observável com o
//! Oráculo (o AppleWin no SHA congelado).
//!
//! O corpus não tem licença e fica fora do Git. Rode `tools/conformance/fetch-applewin-test.sh`
//! e depois, com `--ignored` (o teste exige o cache local e não deve rodar no `cargo test`
//! normal):
//!
//! `cargo test -p apple2-savestate --release -- --ignored applewin_test_corpus --nocapture`
//!
//! Escreve `tests/conformance/reports/applewin-test-load.md`.

mod common;

use apple2_audio::{AudioBus, MockingboardCard, SamCard, Variant};
use apple2_core::{Card, Model};
use apple2_hostio::network::MockNetwork;
use apple2_hostio::printer::{MemorySink, ParallelPrinterCard, PrinterOptions};
use apple2_hostio::ssc::SuperSerialCard;
use apple2_hostio::uthernet1::Uthernet1;
use apple2_hostio::uthernet2::Uthernet2;
use apple2_hostio::LoopbackSerial;
use apple2_savestate::{default_card, load_aws_yaml, Error, ImageRequest, LoadHost};
use apple2_storage::image::DiskImage;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const FRAME_CYCLES: u64 = 17_030;
/// Cache padrão, relativo à raiz do workspace; `APPLEWIN_TEST_CACHE` substitui.
const DEFAULT_CACHE: &str = "target/conformance-cache/applewin-test";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Ok,
    OkComAvisos,
    Erro,
    NaoSuportado,
    Panic,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Outcome::Ok => "OK",
            Outcome::OkComAvisos => "OK_COM_AVISOS",
            Outcome::Erro => "ERRO",
            Outcome::NaoSuportado => "NAO_SUPORTADO",
            Outcome::Panic => "PANIC",
        }
    }
}

/// Resolve imagens pelo nome-base, procurando do diretório do YAML até a raiz do cache.
/// Os cartões que dependem do hospedeiro usam saídas em memória (sem áudio, serial ou rede reais).
struct CorpusHost {
    dirs: Vec<PathBuf>,
    bus: AudioBus,
}

impl LoadHost for CorpusHost {
    fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
        // O nome pode ser um caminho do Windows (`F:\Apple][\x.dsk`): fica só o nome-base.
        let base = req
            .filename
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(req.filename);
        if base.is_empty() {
            return Err("nome de imagem vazio".into());
        }
        for d in &self.dirs {
            let p = d.join(base);
            if let Ok(bytes) = std::fs::read(&p) {
                let img = if req.hard_disk {
                    DiskImage::hard_disk_from_bytes(base, bytes)
                } else {
                    DiskImage::from_bytes(base, bytes)
                };
                return img.map_err(|e| e.to_string());
            }
        }
        Err(format!("imagem '{base}' não está no corpus"))
    }

    fn make_card(&mut self, name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
        let mb = |v| Some(Box::new(MockingboardCard::new(v, &self.bus)) as Box<dyn Card>);
        match name {
            "Super Serial Card" => Some(Box::new(SuperSerialCard::new(Box::new(
                LoopbackSerial::new().0,
            )))),
            "Generic Printer" => Some(Box::new(ParallelPrinterCard::new(
                Box::new(MemorySink::default()),
                PrinterOptions::default(),
            ))),
            "Uthernet" => Some(Box::new(Uthernet1::new(Box::new(MockNetwork::new().0)))),
            "Uthernet II" => Some(Box::new(Uthernet2::new(
                Box::new(MockNetwork::new().0),
                true,
            ))),
            "Mockingboard C" => mb(Variant::MockingboardC),
            "Phasor" => mb(Variant::Phasor),
            "MEGA Audio" => mb(Variant::MegaAudio),
            "SD Music" => mb(Variant::SdMusic),
            "SAM" => Some(Box::new(SamCard::new(&self.bus))),
            _ => default_card(name, slot, model),
        }
    }
}

struct Row {
    path: String,
    model: String,
    outcome: Outcome,
    detail: String,
}

/// Divide uma linha CSV com campos entre aspas.
fn csv_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

fn load_and_run(path: &Path, dirs: Vec<PathBuf>) -> Result<Vec<String>, Error> {
    let bytes = std::fs::read(path).map_err(|e| Error::Format(format!("leitura: {e}")))?;
    let text = String::from_utf8_lossy(&bytes);
    let mut host = CorpusHost {
        dirs,
        bus: AudioBus::new(44_100, apple2_core::VideoStandard::Ntsc),
    };
    let mut loaded = load_aws_yaml(&text, &mut host)?;
    loaded.machine.run_cycles(FRAME_CYCLES);
    Ok(loaded.warnings)
}

#[test]
#[ignore = "exige o corpus em cache; rode com --ignored (ver cabeçalho)"]
fn applewin_test_corpus() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../..");
    let cache = std::env::var("APPLEWIN_TEST_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join(DEFAULT_CACHE));
    let index = root.join("tests/conformance/inputs/applewin-test-index.csv");
    if !cache.join("v2").is_dir() || !index.is_file() {
        println!("BLOCKED: corpus ausente; rode tools/conformance/fetch-applewin-test.sh");
        return;
    }

    // Registra a localização do panic (arquivo:linha) sem poluir a saída.
    static PANIC_AT: Mutex<String> = Mutex::new(String::new());
    std::panic::set_hook(Box::new(|info| {
        let at = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        *PANIC_AT.lock().unwrap() = at;
    }));

    let csv = std::fs::read_to_string(&index).expect("índice");
    let mut rows: Vec<Row> = Vec::new();
    for line in csv.lines().skip(1) {
        let f = csv_fields(line);
        if f.len() < 9 || f[1] != "aws.yaml" {
            continue;
        }
        let rel = &f[0];
        let full = cache.join(rel);
        let mut dirs = Vec::new();
        let mut d = full.parent().map(Path::to_path_buf);
        while let Some(p) = d {
            dirs.push(p.clone());
            if p == cache {
                break;
            }
            d = p.parent().map(Path::to_path_buf);
        }
        PANIC_AT.lock().unwrap().clear();
        let res = catch_unwind(AssertUnwindSafe(|| load_and_run(&full, dirs)));
        let (outcome, detail) = match res {
            Ok(Ok(w)) if w.is_empty() => (Outcome::Ok, String::new()),
            Ok(Ok(w)) => (Outcome::OkComAvisos, format!("avisos: {}", w.join("; "))),
            Ok(Err(e @ Error::Unsupported(_))) => (Outcome::NaoSuportado, e.to_string()),
            Ok(Err(e)) => (Outcome::Erro, e.to_string()),
            Err(p) => {
                let msg = p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "(sem mensagem)".into());
                let at = PANIC_AT.lock().unwrap().clone();
                (Outcome::Panic, format!("{msg} @ {at}"))
            }
        };
        println!("{:<14} {}  {}", outcome.label(), rel, detail);
        rows.push(Row {
            path: rel.clone(),
            model: f[4].clone(),
            outcome,
            detail,
        });
    }
    let _ = std::panic::take_hook();

    let mut totals: BTreeMap<Outcome, usize> = BTreeMap::new();
    for r in &rows {
        *totals.entry(r.outcome).or_default() += 1;
    }
    let mut md = String::new();
    md.push_str("# Carga do corpus AppleWin-Test (1 quadro)\n\n");
    md.push_str("Gerado por `crates/apple2-savestate/tests/applewin_test_corpus.rs`. ");
    md.push_str("SHA do AppleWin-Test: `4b113ce8cff1426bda5595cac9cb04776adfbb5d`.\n\n");
    md.push_str(
        "**Isto NÃO é PASS de fidelidade.** OK significa apenas IMPLEMENTED: o arquivo carrega, \
         a Máquina é montada e roda 1 quadro (17.030 ciclos) sem erro nem panic. PASS exige \
         comparar o estado observável (CPU, memória, vídeo, cartas) com o Oráculo, o AppleWin \
         no SHA congelado, e isso ainda não foi feito. OK_COM_AVISOS carregou, mas perdeu \
         algo (cartão sem equivalente ou imagem de disco ausente do corpus). Cartões que \
         dependem do hospedeiro usam saídas em memória.\n\n",
    );
    md.push_str("## Total por resultado\n\n| resultado | arquivos |\n|---|---|\n");
    for (o, n) in &totals {
        md.push_str(&format!("| {} | {} |\n", o.label(), n));
    }
    md.push_str(&format!("| **total** | {} |\n\n", rows.len()));
    md.push_str("## Por fixture\n\n| fixture | modelo | resultado | erro |\n|---|---|---|---|\n");
    for r in &rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            md_cell(&r.path),
            md_cell(&r.model),
            r.outcome.label(),
            md_cell(&r.detail)
        ));
    }
    let out = root.join("tests/conformance/reports/applewin-test-load.md");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, md).unwrap();
    println!("relatório: {}", out.display());
    for (o, n) in &totals {
        println!("{}: {}", o.label(), n);
    }
}
