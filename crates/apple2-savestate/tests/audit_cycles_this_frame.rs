//! Auditoria wave0-core, area 6 (HU-0401): "Cycles This Frame" de um save-state real do AppleWin
//! passa de 17030 e o carregador o aceita sem normalizar.
//!
//! Evidencia real: anexo da issue AppleWin#1520 (AppleWin 1.31.0.2) traz `Cycles This Frame: 26190`.
//! Rust: apple2.rs:530 `st.cycles_in_frame = v.u32(..)`; machine.rs:336-340 so zera o contador
//! quando `cycles_in_frame == cycles_per_frame` (igualdade), logo o contador nunca fecha o quadro,
//! `frames` para de avancar e `run_frame()` roda ate o u32 estourar (4,29e9 ciclos: minutos de CPU
//! real; panic "attempt to add with overflow" em debug).
//! C++: Video.cpp:328 le o valor cru, mas o laco principal do AppleWin usa o contador so como
//! deslocamento dentro do quadro (`NTSC_VideoUpdateCycles`), nao como condicao de igualdade.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::image::DiskImage;

struct NoImages;

impl LoadHost for NoImages {
    fn open_image(&mut self, _req: &ImageRequest) -> Result<DiskImage, String> {
        Err("sem imagens".into())
    }
}

fn state_with_cycles_this_frame(v: u64) -> String {
    let m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let text = save_aws_yaml(
        &m,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .unwrap()
    .text;
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("Cycles This Frame:"))
        .expect("campo")
        .to_string();
    let indent = &line[..line.len() - line.trim_start().len()];
    text.replace(&line, &format!("{indent}Cycles This Frame: {v}"))
}

#[test]
#[ignore = "reproduz achado AUD-C-060; remover ignore quando corrigido"]
fn cycles_this_frame_above_the_frame_length_is_normalised_on_load() {
    for v in [17030u64, 26190, 1_000_000] {
        let doc = state_with_cycles_this_frame(v);
        let mut m = load_aws_yaml(&doc, &mut NoImages).unwrap().machine;
        let per_frame = if m.config().video_standard == apple2_core::VideoStandard::Ntsc {
            17030
        } else {
            20280
        };
        assert!(
            m.cycles_in_frame() < per_frame || v == 17030,
            "cycles_in_frame = {} apos carregar {v}",
            m.cycles_in_frame()
        );
        let before = m.frames();
        m.run_cycles(3 * per_frame as u64);
        assert!(
            m.frames() > before,
            "o contador de quadros nao avanca com Cycles This Frame = {v}"
        );
    }
}
