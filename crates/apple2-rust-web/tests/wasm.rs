//! Os mesmos testes headless, executados em WASM (Node) com `wasm-bindgen-test`.
//!
//! Rodar: `cargo test -p apple2-rust-web --target wasm32-unknown-unknown --release` com
//! `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner` (veja
//! docs/conformance/wasm-estudo.md). No desktop este arquivo compila vazio.

#![cfg(target_arch = "wasm32")]

use apple2_rust_web::wasm::Apple2;
use wasm_bindgen_test::*;

/// Mesmo valor de `HASH_BOOT_60` em `tests/boot.rs` (calculado no desktop).
const HASH_BOOT_60: u64 = 0x4f1dc409f736b225;

#[wasm_bindgen_test]
fn hash_do_boot_igual_ao_do_desktop() {
    let h = apple2_rust_web::hash_do_boot("apple2e-enhanced", 60).unwrap();
    assert_eq!(h, HASH_BOOT_60, "WASM divergiu do desktop: {h:#018x}");
}

#[wasm_bindgen_test]
fn api_javascript_roda_quadros_e_devolve_rgba() {
    let mut m = Apple2::new("apple2e-enhanced").unwrap_or_else(|_| panic!("new falhou"));
    for _ in 0..60 {
        m.run_frame();
    }
    assert_eq!((m.width(), m.height()), (560, 384));
    assert_eq!(m.framebuffer().len(), 560 * 384 * 4);
    assert_eq!(m.framebuffer_hash(), format!("{HASH_BOOT_60:016x}"));
    assert!(m.key(b'A'));
}

#[wasm_bindgen_test]
fn disco_a_partir_de_bytes_em_wasm() {
    let mut m = Apple2::new("apple2e-enhanced").unwrap_or_else(|_| panic!("new falhou"));
    assert!(m.insert_disk(0, "t.dsk", vec![0; 143_360]).is_ok());
    for _ in 0..10 {
        m.run_frame();
    }
}
