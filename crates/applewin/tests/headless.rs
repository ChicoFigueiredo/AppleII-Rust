use applewin_shell::backend::DefaultBackend;
use applewin_shell::headless::{run, HeadlessOptions};

#[test]
fn boots_to_basic_and_dumps_a_png() {
    let png = std::env::temp_dir().join(format!("applewin-headless-{}.png", std::process::id()));
    let opts = HeadlessOptions {
        frames: 300,
        dump_screen: Some(png.clone()),
        type_text: Some("PRINT 6*7\r".into()),
        type_at_frame: 180,
        ..Default::default()
    };
    let text = run(apple2_config::Settings::default(), Box::new(DefaultBackend), &opts).unwrap();
    assert!(text.iter().any(|l| l.starts_with("]PRINT 6*7")), "{text:#?}");
    assert!(text.iter().any(|l| l.trim() == "42"), "{text:#?}");
    let img = image::open(&png).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (560, 384));
    assert!(img.pixels().any(|p| p.0[0] > 100), "screen is blank");
    std::fs::remove_file(png).unwrap();
}

#[test]
fn headless_run_is_deterministic() {
    let opts = HeadlessOptions { frames: 200, ..Default::default() };
    let a = run(apple2_config::Settings::default(), Box::new(DefaultBackend), &opts).unwrap();
    let b = run(apple2_config::Settings::default(), Box::new(DefaultBackend), &opts).unwrap();
    assert_eq!(a, b);
}
