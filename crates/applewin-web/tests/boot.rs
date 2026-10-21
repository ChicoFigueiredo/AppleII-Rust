//! Testes headless do protótipo web, rodando no desktop (nativo).
//!
//! O mesmo hash dourado é conferido em WASM por `tests/wasm.rs` (wasm-bindgen-test no Node).

use apple2_core::{Machine, MachineConfig, Model};
use applewin_web::{fnv1a, hash_do_boot, Emulator};

/// Hash do framebuffer após 60 Quadros de boot a frio do //e Enhanced. Calculado pelo caminho do
/// desktop (Machine + Renderer sem a camada web); o WASM tem de dar o mesmo valor.
pub const HASH_BOOT_60: u64 = 0x4f1dc409f736b225;

fn hash_caminho_desktop(model: Model, quadros: u32) -> u64 {
    let mut m = Machine::new(MachineConfig::for_model(model));
    // Como o desktop: Disk II no slot 6.
    assert!(m.insert_card(6, Box::new(apple2_storage::disk2::Disk2Card::new())));
    let mut v = apple2_video::Renderer::new(apple2_video::RenderConfig::default());
    for _ in 0..quadros {
        v.run_frame(&mut m);
    }
    fnv1a(&v.rgba8())
}

#[test]
fn hash_web_igual_ao_do_desktop() {
    let web = hash_do_boot("apple2e-enhanced", 60).unwrap();
    let desktop = hash_caminho_desktop(Model::Apple2eEnhanced, 60);
    assert_eq!(web, desktop);
}

#[test]
fn hash_dourado_do_boot() {
    let h = hash_do_boot("apple2e-enhanced", 60).unwrap();
    assert_eq!(h, HASH_BOOT_60, "novo hash: {h:#018x}");
}

#[test]
fn framebuffer_tem_o_tamanho_esperado() {
    let mut e = Emulator::new("apple2e-enhanced").unwrap();
    e.run_frame();
    let (w, h) = e.size();
    assert_eq!((w, h), (560, 384));
    assert_eq!(e.framebuffer_rgba().len(), w * h * 4);
    assert_eq!(e.frames(), 1);
}

#[test]
fn quadro_executa_cerca_de_17030_ciclos() {
    let mut e = Emulator::new("apple2e-enhanced").unwrap();
    let c = e.run_frame();
    assert!((17_030..17_060).contains(&c), "{c}");
}

#[test]
fn tecla_e_aceita_pelo_modelo() {
    let mut a = Emulator::new("apple2e-enhanced").unwrap();
    let mut b = Emulator::new("apple2e-enhanced").unwrap();
    for _ in 0..90 {
        a.run_frame();
        b.run_frame();
    }
    assert_eq!(a.framebuffer_hash(), b.framebuffer_hash());
    // O //e Enhanced aceita ASCII; um byte acima de $7F (acentuado) não existe no teclado dele.
    assert!(a.key(b'A'));
    assert!(!a.key(0xE9));
    // Sem tecla pendente o resto do estado continua idêntico ao do outro.
    assert_eq!(b.frames(), a.frames());
}

#[test]
fn modelo_invalido_e_erro() {
    assert!(Emulator::new("zx81").is_err());
}

#[test]
fn rom_do_usuario_substitui_a_embutida() {
    // 12K de zeros: a CPU cai em BRK a partir do vetor RESET (0000); o vídeo continua rodando.
    let mut e = Emulator::with_rom("apple2plus", vec![0; 0x3000]).unwrap();
    for _ in 0..3 {
        e.run_frame();
    }
    assert_eq!(e.frames(), 3);
    assert!(Emulator::with_rom("apple2plus", vec![0; 100]).is_err());
}

#[test]
fn disco_a_partir_de_bytes() {
    let mut e = Emulator::new("apple2e-enhanced").unwrap();
    // Imagem .dsk de 143360 bytes (35 trilhas x 16 setores x 256).
    e.insert_disk(0, "teste.dsk", vec![0; 143_360]).unwrap();
    for _ in 0..30 {
        e.run_frame();
    }
    assert!(e.insert_disk(0, "lixo.dsk", vec![1, 2, 3]).is_err());
    assert!(e.insert_disk(2, "teste.dsk", vec![0; 143_360]).is_err());
}
