//! The committed video scenes (inputs of the AppleWin differential) are exactly what
//! `tools/scenes.rs` generates, and every scene fits the assumptions of the loaders.

#[path = "../tools/scenes.rs"]
mod scenes;

#[test]
fn committed_scene_files_match_the_generator() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/inputs/video");
    let all = scenes::all();
    assert!(all.len() > 40);
    let mut names = std::collections::HashSet::new();
    for s in &all {
        assert!(names.insert(s.name.clone()), "duplicate scene {}", s.name);
        assert!(s.code.len() < 0x100, "{}: code does not fit page 3", s.name);
        let on_disk = std::fs::read_to_string(dir.join(format!("{}.scene", s.name)))
            .unwrap_or_else(|e| {
                panic!(
                    "{}: {e}; run `cargo run -p apple2-video --example gen_scenes`",
                    s.name
                )
            });
        assert_eq!(on_disk, s.to_text(), "{} is stale; run gen_scenes", s.name);
    }
}
