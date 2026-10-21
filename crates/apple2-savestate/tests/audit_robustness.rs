//! Auditoria wave0-core, area 6 (HU-0401 / HU-0402): robustez do carregador `.aws.yaml` e
//! round-trip de textos. Um panic ao ler um arquivo ruim e P0 (HU-0401: "erro descritivo e nao
//! crash/corrupcao").
//!
//! Oraculo: YamlHelper.cpp / YamlSaveHelper (C++, SHA a8f0d1d) e a regra do plano.

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::yaml::{self, Map, Node, Writer};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use common::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn rich_machine(image_name: &str) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut card = Disk2Card::new();
    let img = DiskImage::from_bytes(image_name, disk(DOS33)).unwrap();
    card.insert_disk(0, img);
    assert!(m.insert_card(6, Box::new(card)));
    m.power_cycle();
    for _ in 0..3 {
        m.run_frame();
    }
    m
}

fn saved_text() -> String {
    let m = rich_machine("DOS 3.3 System Master - 680-0210-A.dsk");
    save_aws_yaml(&m, &SaveOptions::default()).unwrap().text
}

/// True if loading `text` panicked.
fn load_panics(text: &str) -> bool {
    let mut host = Rec::default(); // nao le disco: so registra o pedido
    catch_unwind(AssertUnwindSafe(|| {
        let _ = load_aws_yaml(text, &mut host);
    }))
    .is_err()
}

#[test]
fn truncated_yaml_gives_error_or_machine_never_panic() {
    let text = saved_text();
    let len = text.len();
    // Cortes densos no cabecalho e nas primeiras unidades, esparsos no resto.
    let mut cuts: Vec<usize> = (0..4000.min(len)).step_by(11).collect();
    cuts.extend((4000..len).step_by((len / 120).max(1)));
    cuts.push(len - 1);
    let mut panics = Vec::new();
    for c in cuts {
        let mut c = c;
        while !text.is_char_boundary(c) {
            c -= 1;
        }
        if load_panics(&text[..c]) {
            panics.push(c);
        }
    }
    assert!(
        panics.is_empty(),
        "panic ao carregar YAML truncado em {} cortes, ex.: {:?}",
        panics.len(),
        &panics[..panics.len().min(10)]
    );
}

#[test]
fn wrong_typed_or_hostile_values_never_panic() {
    let text = saved_text();
    // Linhas "chave: valor" que nao sao blocos de memoria (hex longo).
    let lines: Vec<&str> = text.lines().collect();
    let candidates: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim();
            !t.starts_with('#')
                && t.contains(": ")
                && t.len() < 90
                && !t
                    .split(": ")
                    .next()
                    .unwrap()
                    .chars()
                    .all(|c| c.is_ascii_hexdigit())
        })
        .map(|(i, _)| i)
        .collect();
    let values = ["abc", "-1", "\"", "0xFFFFFFFF", "ção"];
    let mut panics = Vec::new();
    let mut tried = 0;
    for &i in candidates.iter().step_by(5) {
        let (key, _) = lines[i].split_once(": ").unwrap();
        if key.trim() == "Length" {
            continue; // AUD-C-010: teste proprio abaixo
        }
        for v in values {
            let mut mutated: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            mutated[i] = format!("{key}: {v}");
            let doc = mutated.join("\n");
            tried += 1;
            if load_panics(&doc) {
                panics.push(format!("{} => {v}", lines[i].trim()));
            }
        }
    }
    assert!(
        tried > 100 || candidates.len() < 100,
        "poucas mutacoes ({tried})"
    );
    assert!(
        panics.is_empty(),
        "{} panics em {tried} mutacoes, ex.: {:?}",
        panics.len(),
        &panics[..panics.len().min(10)]
    );
}

#[test]
#[ignore = "reproduz achado AUD-C-010; remover ignore quando corrigido"]
fn extension_length_field_with_hostile_value_does_not_panic_or_allocate_unbounded() {
    // ext.rs:66: `vec![0u8; len.div_ceil(8) * 8]` com `len` vindo do arquivo (u64 sem teto).
    // "-1" vira u64::MAX (strtoul), a multiplicacao estoura (panic em debug, alocacao
    // gigantesca/abort em release). C++: YamlHelper limita por tamanho do buffer real.
    let text = saved_text();
    let doc: Vec<String> = text
        .lines()
        .map(|l| match l.trim_start().strip_prefix("Length: ") {
            Some(_) => format!("{}Length: -1", &l[..l.len() - l.trim_start().len()]),
            None => l.to_string(),
        })
        .collect();
    let doc = doc.join("\n");
    assert_ne!(doc, text, "campo Length nao encontrado");
    // Obs.: 0xFFFFFFFFFFFF abortaria o processo (alocacao de 281 TB), nao e capturavel.
    assert!(!load_panics(&doc), "panic com Length: -1");
}

#[test]
fn garbage_documents_never_panic() {
    for doc in [
        "",
        "\u{feff}",
        "---",
        "File_hdr:",
        "File_hdr:\n  Tag: AppleWin Save State\n  Version: 2\n",
        "File_hdr:\n  Tag: AppleWin Save State\n  Version: 99\n",
        "Unit:\n  Type: Apple2\n  Version: 9\n  State:\n",
        "\0\0\0",
        "a: b\n  c: d\n e: f",
        "\t: \"",
        "\"\": \"\"",
        "- a\n- b",
        "Unit:\n  Type: Slots\n  Version: 1\n  State:\n    99:\n      Card: Disk II\n",
    ] {
        assert!(!load_panics(doc), "panic com {doc:?}");
    }
}

#[test]
fn schema_version_unknown_gives_descriptive_error() {
    let text = saved_text().replace("  Version: 2\n", "  Version: 3\n");
    let mut host = TestHost::default();
    let r = load_aws_yaml(&text, &mut host);
    assert!(r.is_err(), "File_hdr Version 3 deveria ser recusada");
}

// ---- GH#1499 / GH#1510: textos com contrabarra, espaco, #, Unicode. ----

fn roundtrip_string(v: &str) -> String {
    let mut w = Writer::new("", "");
    w.map("M", |w| w.string("P", v));
    let doc = w.finish();
    let top = yaml::parse(&doc).unwrap_or_else(|e| panic!("{v:?}: {e}\n{doc}"));
    let Node::Map(e) = &top.iter().find(|(k, _)| k == "M").unwrap().1 else {
        panic!()
    };
    Map {
        name: "M",
        entries: e,
    }
    .string("P")
    .unwrap()
}

#[test]
fn strings_with_spaces_hash_and_unicode_round_trip() {
    for v in [
        r"C:\Users\Zoë Müller\Games we Play #1\día 日本.dsk",
        "/home/chico/Área de Trabalho/disco #2.dsk",
        r"C:\Sub1\Printer.txt",
        "tab\there",
    ] {
        assert_eq!(roundtrip_string(v), v);
    }
}

#[test]
#[ignore = "reproduz achado AUD-C-020; remover ignore quando corrigido"]
fn strings_with_a_double_backslash_round_trip() {
    // yaml.rs:394 so dobra as contrabarras se a string NAO contem "\\\\" e yaml.rs:108 desdobra
    // todo "\\\\": um caminho UNC (\\servidor\share) ou misto perde uma contrabarra por ciclo.
    for v in [r"\\servidor\compartilhado\disco.dsk", r"C:\a\\b\c.dsk"] {
        assert_eq!(roundtrip_string(v), v, "caminho corrompido");
    }
}

/// Resolver que guarda o que o save-state pediu.
#[derive(Default)]
struct Rec {
    asked: Vec<(String, String)>,
}

impl LoadHost for Rec {
    fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
        self.asked
            .push((req.filename.to_string(), req.absolute_path.to_string()));
        Err("so registra".into())
    }
}

#[test]
fn disk_image_path_with_unicode_space_hash_and_backslash_survives_save_load() {
    let name = r"C:\Users\Zoë Müller\Games we Play #1\día 日本.dsk";
    let m = rich_machine(name);
    let text = save_aws_yaml(&m, &SaveOptions::default()).unwrap().text;
    let mut host = Rec::default();
    let _ = load_aws_yaml(&text, &mut host);
    assert!(
        host.asked.iter().any(|(_, abs)| abs == name),
        "caminho absoluto pedido ao host: {:?}",
        host.asked
    );
}

#[test]
fn saving_twice_does_not_grow_backslashes_gh1510() {
    let name = r"C:\Sub1\Printer #1.dsk";
    let m = rich_machine(name);
    let a = save_aws_yaml(&m, &SaveOptions::default()).unwrap().text;
    let mut host = Rec::default();
    let _ = load_aws_yaml(&a, &mut host);
    let (_, abs) = host.asked.first().cloned().unwrap();
    assert_eq!(abs, name);
}
