//! Regressões das issues da seção "Save-state" de docs/conformance/wave1-candidates.md que
//! pertencem ao crate apple2-savestate (uma por issue). As que exigem o oráculo executável
//! estão em `apple2-conformance/tests/oracle_roundtrip.rs` (#1262, #255, #591, #413), em
//! `applewin/tests/state_gh564.rs` (#564) e nos casos `savestate-*` (corpus real).

mod common;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::image::DiskImage;

struct NoImages;
impl LoadHost for NoImages {
    fn open_image(&mut self, _r: &ImageRequest) -> Result<DiskImage, String> {
        Err("sem imagens".into())
    }
}

fn saved(model: Model) -> String {
    let mut m = Machine::new(MachineConfig::for_model(model));
    m.run_cycles(20_000);
    save_aws_yaml(&m, &SaveOptions::default()).unwrap().text
}

/// GH#838: "Loading Save State = Parser Error (invalid trailing UTF-8 octet)". A file with a
/// BOM loads (libyaml accepts it); bytes that are not UTF-8 give an error, never a panic.
#[test]
fn gh838_bom_loads_and_invalid_utf8_is_an_error_not_a_panic() {
    let text = saved(Model::Apple2eEnhanced);
    let with_bom = format!("\u{FEFF}{text}");
    assert!(
        load_aws_yaml(&with_bom, &mut NoImages).is_ok(),
        "arquivo com BOM"
    );
    // The app reads the file as bytes and decodes it lossily or refuses it: both end here.
    let mut bytes = text.clone().into_bytes();
    let at = bytes.iter().position(|&b| b == b'#').unwrap();
    bytes.insert(at + 2, 0xC3); // a lone lead byte
    assert!(String::from_utf8(bytes.clone()).is_err());
    let lossy = String::from_utf8_lossy(&bytes);
    let r = std::panic::catch_unwind(|| load_aws_yaml(&lossy, &mut NoImages).map(|_| ()));
    assert!(r.is_ok(), "panic com UTF-8 inválido");
}

/// GH#486 (Lisa 2.5D + Uthernet II) and GH#1372 (SSI263/SC01) have dedicated tests; this checks
/// they are still there so the CSV evidence cannot rot silently.
#[test]
fn evidence_tests_for_other_issues_still_exist() {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let find = |file: &str, name: &str| {
        let t = std::fs::read_to_string(here.join(file)).unwrap();
        assert!(t.contains(name), "{file}: falta {name}");
    };
    find(
        "cards_audio.rs",
        "a_phoneme_playing_when_saved_restarts_and_completes_after_the_load",
    );
}
