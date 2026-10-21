//! Auditoria wave0-core, area 6 (HU-0401): importacao de save-state REAL do AppleWin.
//!
//! O repositorio nao traz nenhum `.aws.yaml` gerado pelo AppleWin (as fixtures dos testes sao
//! escritas a partir do codigo C++). Este teste le arquivos de um diretorio externo, indicado por
//! `APPLE2_REAL_AWS_DIR`, e e ignorado (BLOCKED) se a variavel nao existir.
//!
//! Proveniencia do arquivo usado na auditoria: anexo da issue AppleWin#1520 ("Merlin Pro v2.34
//! Disk 1 (ProDOS).aws.yaml", AppleWin 1.31.0.2, //e Enhanced, Saturn no slot 3).
//! SHA-256: 813f26612183cc4632761233fe2f05141b37f7922be58e86a25827fe2d42c284.

use apple2_core::language_card::LcKind;
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::image::DiskImage;

struct Rec(Vec<(String, String)>);

impl LoadHost for Rec {
    fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
        self.0
            .push((req.filename.to_string(), req.absolute_path.to_string()));
        Err("imagem nao disponivel".into())
    }
}

fn real_files() -> Vec<std::path::PathBuf> {
    let Ok(dir) = std::env::var("APPLE2_REAL_AWS_DIR") else {
        return Vec::new();
    };
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.to_string_lossy().ends_with(".aws.yaml"))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn real_applewin_states_load_and_resave_without_panic() {
    let files = real_files();
    if files.is_empty() {
        eprintln!("BLOCKED: defina APPLE2_REAL_AWS_DIR com save-states reais do AppleWin");
        return;
    }
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let mut host = Rec(Vec::new());
        let loaded = load_aws_yaml(&text, &mut host)
            .unwrap_or_else(|e| panic!("{}: erro ao importar: {e}", f.display()));
        eprintln!("{}: avisos {:?}", f.display(), loaded.warnings);
        // GH#1499: caminhos entre aspas com `\U...` (contrabarra simples) tem de chegar intactos.
        for want in text.lines().filter_map(|l| {
            let v = l.trim().strip_prefix("Absolute Path: ")?;
            let v = v.trim_matches('"');
            (!v.is_empty()).then(|| v.replace("\\\\", "\\"))
        }) {
            assert!(
                host.0.iter().any(|(_, abs)| *abs == want),
                "{}: caminho {want:?} nao chegou intacto: {:?}",
                f.display(),
                host.0
            );
        }
        let mut m = loaded.machine;
        // Limitado por ciclos: run_frame() nao termina com "Cycles This Frame" > 17030 (AUD-C-060).
        m.run_cycles(5 * 17030);
        let again = save_aws_yaml(&m, &SaveOptions::default()).unwrap();
        assert!(again.text.contains("Tag: AppleWin Save State"));
        // Saturn salvo com N bancos deve voltar com N bancos (AUD-C-001 reaparece aqui).
        let want = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("Num Saturn Banks: 0x"))
            .map(|v| u8::from_str_radix(&v[..2], 16).unwrap());
        if let Some(want) = want {
            let got = m.board().lc.infos().iter().find_map(|i| match i.kind {
                LcKind::Saturn { banks, .. } => Some(banks),
                _ => None,
            });
            assert_eq!(got, Some(want), "{}", f.display());
        }
    }
}
