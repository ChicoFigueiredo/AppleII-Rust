//! A Máquina "nua": só RAM e os vetores que o exercício define. Nenhuma ROM da Apple entra:
//! a "ROM" é um bloco de 12K zerado, exceto pelos seis bytes dos vetores NMI/RESET/IRQ.
//!
//! Usa o modelo ][+ sem Language Card, então a RAM vai de $0000 a $BFFF. Programas devem evitar
//! $C000-$CFFF (soft switches e I/O).

use apple2_core::{Machine, MachineConfig, Model};

/// Última página de RAM utilizável pelos exercícios.
pub const RAM_FIM: u16 = 0xBFFF;

/// Parâmetros da Máquina nua.
#[derive(Clone, Copy, Debug)]
pub struct BareConfig {
    /// `true` para 65C02, `false` para o 6502 NMOS.
    pub cmos: bool,
    pub reset: u16,
    pub irq: u16,
    pub nmi: u16,
}

impl BareConfig {
    pub fn new(cmos: bool, reset: u16) -> Self {
        BareConfig {
            cmos,
            reset,
            irq: reset,
            nmi: reset,
        }
    }
}

/// Constrói a Máquina nua. O PC já sai do reset apontando para `cfg.reset`.
pub fn bare_machine(cfg: BareConfig) -> Machine {
    let mut rom = vec![0u8; 0x3000]; // $D000-$FFFF
    for (vec, val) in [(0xFFFAu16, cfg.nmi), (0xFFFC, cfg.reset), (0xFFFE, cfg.irq)] {
        let i = (vec - 0xD000) as usize;
        rom[i] = val as u8;
        rom[i + 1] = (val >> 8) as u8;
    }
    let mut c = MachineConfig::for_model(Model::Apple2Plus);
    c.slots = [None; 8];
    c.custom_rom = Some(rom);
    c.cpu_cmos = Some(cfg.cmos);
    let mut m = Machine::new(c);
    m.cpu_mut().pc = cfg.reset;
    m
}

/// Grava bytes na RAM da Máquina (sem efeitos colaterais). Erro se sair da RAM.
pub fn load(m: &mut Machine, addr: u16, bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() {
        return Ok(());
    }
    let end = addr as usize + bytes.len() - 1;
    if end > RAM_FIM as usize {
        return Err(format!(
            "gravar {} byte(s) em ${addr:04X} passa do fim da RAM (${RAM_FIM:04X})",
            bytes.len()
        ));
    }
    for (i, b) in bytes.iter().enumerate() {
        m.poke(addr + i as u16, *b);
    }
    Ok(())
}

/// Por que a execução parou.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parada {
    /// O PC chegou numa instrução BRK (ela não é executada).
    Brk,
    /// A instrução saltou para si mesma (`JMP *`, `BEQ *`...).
    LacoInfinito,
    /// O PC chegou ao endereço de parada pedido.
    Endereco,
    /// Estourou o limite de instruções.
    Limite,
}

impl Parada {
    pub fn texto(self) -> &'static str {
        match self {
            Parada::Brk => "parou em BRK",
            Parada::LacoInfinito => "parou num laço de uma instrução (JMP *)",
            Parada::Endereco => "chegou ao endereço de parada",
            Parada::Limite => "estourou o limite de instruções",
        }
    }
}

/// Resultado de [`run`].
#[derive(Clone, Copy, Debug)]
pub struct Execucao {
    pub instrucoes: u64,
    pub ciclos: u64,
    pub parada: Parada,
}

/// Executa até BRK, laço de uma instrução, `fim` ou `max` instruções.
pub fn run(m: &mut Machine, max: u64, fim: Option<u16>) -> Execucao {
    let mut instrucoes = 0u64;
    let mut ciclos = 0u64;
    loop {
        let pc = m.cpu().pc;
        if Some(pc) == fim {
            return Execucao {
                instrucoes,
                ciclos,
                parada: Parada::Endereco,
            };
        }
        if m.peek(pc) == 0x00 {
            return Execucao {
                instrucoes,
                ciclos,
                parada: Parada::Brk,
            };
        }
        if instrucoes >= max {
            return Execucao {
                instrucoes,
                ciclos,
                parada: Parada::Limite,
            };
        }
        ciclos += m.step() as u64;
        instrucoes += 1;
        if m.cpu().pc == pc {
            return Execucao {
                instrucoes,
                ciclos,
                parada: Parada::LacoInfinito,
            };
        }
    }
}
