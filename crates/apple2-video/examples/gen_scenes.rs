//! Writes `tests/conformance/inputs/video/*.scene` (see `tools/scenes.rs`).
//! `cargo run -p apple2-video --example gen_scenes`

#[path = "../tools/scenes.rs"]
mod scenes;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/inputs/video");
    std::fs::create_dir_all(&dir).unwrap();
    for s in scenes::all() {
        std::fs::write(dir.join(format!("{}.scene", s.name)), s.to_text()).unwrap();
    }
}
