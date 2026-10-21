//! Protótipo do frontend web (HU-1205): o núcleo do emulador atrás de uma API mínima.
//!
//! [`Emulator`] é Rust puro e roda no desktop (testes nativos) e no navegador/Node. O módulo
//! `wasm` (só em `wasm32`) o expõe ao JavaScript com `wasm-bindgen`.
//!
//! Escolhas do protótipo:
//! - sem áudio, sem threads, sem sistema de arquivos: tudo entra por bytes;
//! - ROMs: por padrão as do `apple2-roms` embutidas no .wasm (mesmas implicações da distribuição
//!   desktop, ver `docs/conformance/wasm-estudo.md`); [`Emulator::with_rom`] aceita as do usuário;
//! - o tempo é contado em Quadros: quem chama decide quando (`requestAnimationFrame`).

#![forbid(unsafe_code)]

use apple2_core::{Machine, MachineConfig, Model};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use apple2_video::{RenderConfig, Renderer};

#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// Slot do controlador Disk II (o padrão do Apple II).
pub const DISK_SLOT: u8 = 6;

/// Resolve o nome de um modelo (`apple2`, `apple2plus`, `apple2e`, `apple2e-enhanced`).
pub fn modelo_por_nome(nome: &str) -> Result<Model, String> {
    match nome.to_ascii_lowercase().as_str() {
        "apple2" => Ok(Model::Apple2),
        "apple2plus" | "apple2+" => Ok(Model::Apple2Plus),
        "apple2e" => Ok(Model::Apple2e),
        "" | "apple2e-enhanced" | "apple2eenhanced" => Ok(Model::Apple2eEnhanced),
        o => Err(format!(
            "modelo desconhecido: '{o}' (apple2, apple2plus, apple2e, apple2e-enhanced)"
        )),
    }
}

/// Uma Máquina com seu renderizador de vídeo.
pub struct Emulator {
    machine: Machine,
    video: Renderer,
}

impl Emulator {
    /// Máquina com as ROMs embutidas e um Disk II no slot 6.
    pub fn new(modelo: &str) -> Result<Emulator, String> {
        Self::montar(modelo, None)
    }

    /// Como [`Emulator::new`], mas com a ROM de sistema fornecida pelo usuário (16K do //e ou 12K
    /// das linhas ][).
    pub fn with_rom(modelo: &str, rom: Vec<u8>) -> Result<Emulator, String> {
        Self::montar(modelo, Some(rom))
    }

    fn montar(modelo: &str, rom: Option<Vec<u8>>) -> Result<Emulator, String> {
        let mut cfg = MachineConfig::for_model(modelo_por_nome(modelo)?);
        cfg.custom_rom = rom;
        let mut machine = Machine::try_new(cfg).map_err(|e| e.to_string())?;
        if !machine.insert_card(DISK_SLOT, Box::new(Disk2Card::new())) {
            return Err("não foi possível inserir o Disk II no slot 6".into());
        }
        let video = Renderer::new(RenderConfig::default());
        Ok(Emulator { machine, video })
    }

    /// Roda um Quadro (17.030 Ciclos em NTSC) e atualiza o vídeo. Devolve os Ciclos executados.
    pub fn run_frame(&mut self) -> u64 {
        self.video.run_frame(&mut self.machine)
    }

    /// Largura e altura do framebuffer visível, em pixels.
    pub fn size(&self) -> (usize, usize) {
        let (_, _, w, h) = self.video.visible_rect();
        (w, h)
    }

    /// Framebuffer visível como RGBA8, de cima para baixo (`width * height * 4` bytes).
    pub fn framebuffer_rgba(&self) -> Vec<u8> {
        self.video.rgba8()
    }

    /// Digita um caractere (ASCII/Latin-1). `false` se o modelo não tem a tecla.
    pub fn key(&mut self, ascii: u8) -> bool {
        self.machine.key_press(ascii)
    }

    /// Insere uma imagem de disco (`.dsk`, `.do`, `.po`, `.nib`, `.woz`, opcionalmente `.gz`/`.zip`)
    /// a partir dos bytes. `nome` serve só para reconhecer o formato e para exibir.
    pub fn insert_disk(&mut self, drive: usize, nome: &str, bytes: Vec<u8>) -> Result<(), String> {
        let img = DiskImage::from_bytes(nome, bytes).map_err(|e| e.to_string())?;
        let card = self
            .machine
            .card_mut::<Disk2Card>(DISK_SLOT)
            .ok_or("sem Disk II no slot 6")?;
        if drive > 1 {
            return Err(format!("drive {drive} inexistente (0 ou 1)"));
        }
        card.insert_disk(drive, img);
        Ok(())
    }

    /// Reset a quente (Ctrl+Reset).
    pub fn reset(&mut self) {
        self.machine.reset();
        self.video.reset();
    }

    /// Quadros executados desde o ligamento.
    pub fn frames(&self) -> u64 {
        self.machine.frames()
    }

    /// Hash FNV-1a de 64 bits do framebuffer atual (para testes de regressão).
    pub fn framebuffer_hash(&self) -> u64 {
        fnv1a(&self.framebuffer_rgba())
    }

    /// Acesso à Máquina (testes e extensões).
    pub fn machine(&self) -> &Machine {
        &self.machine
    }
}

/// FNV-1a de 64 bits.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Hash do framebuffer depois de `quadros` Quadros de um boot a frio. Mesma função no desktop e
/// no WASM.
pub fn hash_do_boot(modelo: &str, quadros: u32) -> Result<u64, String> {
    let mut e = Emulator::new(modelo)?;
    for _ in 0..quadros {
        e.run_frame();
    }
    Ok(e.framebuffer_hash())
}
