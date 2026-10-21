//! Ligação com JavaScript (`wasm-bindgen`). Só compila em `wasm32`.

use wasm_bindgen::prelude::*;

use crate::Emulator;

/// Máquina Apple II para o navegador.
#[wasm_bindgen]
pub struct Apple2 {
    inner: Emulator,
}

#[wasm_bindgen]
impl Apple2 {
    /// Cria uma Máquina com as ROMs embutidas. `modelo`: `apple2`, `apple2plus`, `apple2e` ou
    /// `apple2e-enhanced`.
    #[wasm_bindgen(constructor)]
    pub fn new(modelo: &str) -> Result<Apple2, JsError> {
        Ok(Apple2 {
            inner: Emulator::new(modelo).map_err(|e| JsError::new(&e))?,
        })
    }

    /// Cria uma Máquina com a ROM de sistema fornecida pelo usuário.
    pub fn with_rom(modelo: &str, rom: Vec<u8>) -> Result<Apple2, JsError> {
        Ok(Apple2 {
            inner: Emulator::with_rom(modelo, rom).map_err(|e| JsError::new(&e))?,
        })
    }

    /// Roda um Quadro. Devolve os Ciclos executados.
    pub fn run_frame(&mut self) -> f64 {
        self.inner.run_frame() as f64
    }

    pub fn width(&self) -> u32 {
        self.inner.size().0 as u32
    }

    pub fn height(&self) -> u32 {
        self.inner.size().1 as u32
    }

    /// Framebuffer RGBA8 (`width * height * 4` bytes), pronto para `ImageData`.
    pub fn framebuffer(&self) -> Vec<u8> {
        self.inner.framebuffer_rgba()
    }

    /// Envia uma tecla (código ASCII). `false` se o modelo não tem a tecla.
    pub fn key(&mut self, ascii: u8) -> bool {
        self.inner.key(ascii)
    }

    /// Insere uma imagem de disco a partir dos bytes. `nome` define o formato pela extensão.
    pub fn insert_disk(&mut self, drive: u32, nome: &str, bytes: Vec<u8>) -> Result<(), JsError> {
        self.inner
            .insert_disk(drive as usize, nome, bytes)
            .map_err(|e| JsError::new(&e))
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn frames(&self) -> f64 {
        self.inner.frames() as f64
    }

    /// Hash FNV-1a do framebuffer, em hexa de 16 dígitos.
    pub fn framebuffer_hash(&self) -> String {
        format!("{:016x}", self.inner.framebuffer_hash())
    }
}
