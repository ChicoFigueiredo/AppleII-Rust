//! Explica uma instrução em português.
//!
//! Mnemônico, modo e tamanho vêm das tabelas do depurador (`apple2_debugger::opcodes`); o texto
//! da instrução vem do disassembler dele; os ciclos (com o ciclo extra de page crossing) e o efeito
//! nos registradores vêm de executar a instrução no `apple2-cpu` sobre uma cópia da memória, sem
//! tocar na original.

use std::collections::BTreeMap;

use apple2_cpu::{Bus, Cpu, Variant};
use apple2_debugger::disasm::{
    disassemble, line_text, render_line, DataBlocks, DisasmConfig, DisasmCtx, Tabs,
};
use apple2_debugger::mem::{DebugMem, Regs};
use apple2_debugger::opcodes::{self, Mode};
use apple2_debugger::symbols::Symbols;

use crate::flags_text;

/// Uma escrita na memória: endereço, valor antes, valor depois.
pub type Escrita = (u16, u8, u8);

/// Resultado de executar uma instrução sobre uma cópia da memória.
#[derive(Clone, Debug)]
pub struct Simulacao {
    pub ciclos: u32,
    pub depois: Regs,
    pub escritas: Vec<Escrita>,
}

struct SimBus<'a> {
    mem: &'a dyn DebugMem,
    sobreposto: BTreeMap<u16, u8>,
    escritas: Vec<Escrita>,
}

impl Bus for SimBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        self.sobreposto
            .get(&addr)
            .copied()
            .unwrap_or_else(|| self.mem.read(addr))
    }
    fn write(&mut self, addr: u16, value: u8) {
        let antes = self.read(addr);
        self.escritas.push((addr, antes, value));
        self.sobreposto.insert(addr, value);
    }
}

/// Executa a instrução em `regs.pc` sobre uma cópia virtual de `mem`.
pub fn simular(mem: &dyn DebugMem, regs: Regs, cmos: bool) -> Simulacao {
    let mut cpu = Cpu::new(if cmos {
        Variant::Cmos65C02
    } else {
        Variant::Nmos6502
    });
    cpu.a = regs.a;
    cpu.x = regs.x;
    cpu.y = regs.y;
    cpu.sp = regs.sp;
    cpu.pc = regs.pc;
    cpu.p = regs.p | 0x20;
    let mut bus = SimBus {
        mem,
        sobreposto: BTreeMap::new(),
        escritas: Vec::new(),
    };
    let ciclos = cpu.step(&mut bus);
    Simulacao {
        ciclos,
        depois: Regs {
            a: cpu.a,
            x: cpu.x,
            y: cpu.y,
            sp: cpu.sp,
            pc: cpu.pc,
            p: cpu.p,
        },
        escritas: bus.escritas,
    }
}

/// Explicação completa de uma instrução.
#[derive(Clone, Debug)]
pub struct Explicacao {
    pub endereco: u16,
    pub opcode: u8,
    pub bytes: Vec<u8>,
    /// Mnemônico em maiúsculas (`LDA`).
    pub mnemonico: String,
    /// Instrução como o depurador a escreve (`LDA #$05`).
    pub instrucao: String,
    pub modo: Mode,
    /// Ciclos reais com os registradores dados (inclui page crossing e desvio tomado).
    pub ciclos: u32,
    /// Ciclos sem nenhum extra.
    pub ciclos_base: u32,
    /// Motivo do ciclo extra, se houver.
    pub extra: Option<&'static str>,
    /// Flags que a instrução pode alterar, ex.: `NZC`.
    pub flags_afetadas: &'static str,
    /// O que a instrução faz, em português.
    pub descricao: String,
    pub antes: Regs,
    pub depois: Regs,
    pub escritas: Vec<Escrita>,
    /// Frases curtas: o que mudou nos registradores, flags e memória.
    pub efeitos: Vec<String>,
    /// Instrução não documentada (6502) ou exclusiva do 65C02 vista num 6502.
    pub extensao: bool,
}

/// Nome do modo em português.
pub fn modo_pt(m: Mode) -> &'static str {
    match m {
        Mode::Implied => "Implícito (sem operando)",
        Mode::Imm => "Imediato",
        Mode::Abs => "Absoluto",
        Mode::Zp => "Página zero",
        Mode::AbsX => "Absoluto indexado por X",
        Mode::AbsY => "Absoluto indexado por Y",
        Mode::ZpX => "Página zero indexado por X",
        Mode::ZpY => "Página zero indexado por Y",
        Mode::Rel => "Relativo (desvio)",
        Mode::IndZpX => "Indireto indexado por X, (página zero,X)",
        Mode::IndAbsX => "Indireto absoluto indexado por X",
        Mode::IndZpY => "Indireto indexado por Y, (página zero),Y",
        Mode::IndZp => "Indireto de página zero",
        Mode::IndAbs => "Indireto absoluto",
    }
}

/// Flags afetadas e frase explicativa por mnemônico. `{o}` vira a descrição do operando.
fn info_mnemonico(m: &str) -> Option<(&'static str, &'static str)> {
    Some(match m {
        "ADC" => ("NVZC", "Soma {o} e o carry ao acumulador (A). Em modo decimal (flag D) soma em BCD."),
        "AND" => ("NZ", "Faz E lógico bit a bit entre A e {o}; o resultado fica em A."),
        "ASL" => ("NZC", "Desloca {o} um bit para a esquerda; o bit 7 vai para o carry e entra 0 no bit 0."),
        "BCC" => ("", "Desvia se o carry estiver desligado (C=0)."),
        "BCS" => ("", "Desvia se o carry estiver ligado (C=1)."),
        "BEQ" => ("", "Desvia se o resultado anterior foi zero (Z=1)."),
        "BIT" => ("NVZ", "Testa bits: Z recebe o resultado de A E {o}; N e V copiam os bits 7 e 6 de {o}. A não muda."),
        "BMI" => ("", "Desvia se o resultado anterior foi negativo (N=1)."),
        "BNE" => ("", "Desvia se o resultado anterior não foi zero (Z=0)."),
        "BPL" => ("", "Desvia se o resultado anterior foi positivo ou zero (N=0)."),
        "BRA" => ("", "Desvia sempre (65C02)."),
        "BRK" => ("I", "Interrupção por software: empilha PC+2 e P, liga I e salta pelo vetor IRQ/BRK ($FFFE)."),
        "BVC" => ("", "Desvia se não houve overflow (V=0)."),
        "BVS" => ("", "Desvia se houve overflow (V=1)."),
        "CLC" => ("C", "Desliga o carry (C=0)."),
        "CLD" => ("D", "Desliga o modo decimal (D=0)."),
        "CLI" => ("I", "Habilita interrupções IRQ (I=0)."),
        "CLV" => ("V", "Desliga o overflow (V=0)."),
        "CMP" => ("NZC", "Compara A com {o} (faz A - operando sem guardar). C=1 se A >= operando; Z=1 se forem iguais."),
        "CPX" => ("NZC", "Compara X com {o}. C=1 se X >= operando; Z=1 se forem iguais."),
        "CPY" => ("NZC", "Compara Y com {o}. C=1 se Y >= operando; Z=1 se forem iguais."),
        "DEC" => ("NZ", "Subtrai 1 de {o} (A, no 65C02 sem operando)."),
        "DEX" => ("NZ", "Subtrai 1 de X."),
        "DEY" => ("NZ", "Subtrai 1 de Y."),
        "EOR" => ("NZ", "Faz OU exclusivo bit a bit entre A e {o}; o resultado fica em A."),
        "INC" => ("NZ", "Soma 1 a {o} (A, no 65C02 sem operando)."),
        "INX" => ("NZ", "Soma 1 a X."),
        "INY" => ("NZ", "Soma 1 a Y."),
        "JMP" => ("", "Salta para o endereço indicado (o PC passa a valer esse endereço)."),
        "JSR" => ("", "Chama uma sub-rotina: empilha o endereço da última instrução (PC+2) e salta. Volta com RTS."),
        "LDA" => ("NZ", "Carrega {o} no acumulador (A)."),
        "LDX" => ("NZ", "Carrega {o} no registrador X."),
        "LDY" => ("NZ", "Carrega {o} no registrador Y."),
        "LSR" => ("NZC", "Desloca {o} um bit para a direita; o bit 0 vai para o carry e entra 0 no bit 7."),
        "NOP" => ("", "Não faz nada, só gasta ciclos."),
        "ORA" => ("NZ", "Faz OU lógico bit a bit entre A e {o}; o resultado fica em A."),
        "PHA" => ("", "Empilha A e decrementa SP."),
        "PHP" => ("", "Empilha o registrador de flags P (com B e bit 5 ligados) e decrementa SP."),
        "PHX" => ("", "Empilha X (65C02)."),
        "PHY" => ("", "Empilha Y (65C02)."),
        "PLA" => ("NZ", "Incrementa SP e desempilha o valor para A."),
        "PLP" => ("NVDIZC", "Incrementa SP e desempilha o valor para as flags (P)."),
        "PLX" => ("NZ", "Desempilha para X (65C02)."),
        "PLY" => ("NZ", "Desempilha para Y (65C02)."),
        "ROL" => ("NZC", "Gira {o} um bit para a esquerda: o carry entra no bit 0 e o bit 7 vira o novo carry."),
        "ROR" => ("NZC", "Gira {o} um bit para a direita: o carry entra no bit 7 e o bit 0 vira o novo carry."),
        "RTI" => ("NVDIZC", "Volta de uma interrupção: desempilha P e depois o PC."),
        "RTS" => ("", "Volta de uma sub-rotina: desempilha o endereço e continua na instrução seguinte ao JSR."),
        "SBC" => ("NVZC", "Subtrai {o} e o empréstimo (carry invertido) de A. Antes de subtrair, ligue o carry com SEC. Em modo decimal subtrai em BCD."),
        "SEC" => ("C", "Liga o carry (C=1)."),
        "SED" => ("D", "Liga o modo decimal (D=1): ADC e SBC passam a operar em BCD."),
        "SEI" => ("I", "Desabilita interrupções IRQ (I=1)."),
        "STA" => ("", "Grava o acumulador (A) em {o}."),
        "STX" => ("", "Grava X em {o}."),
        "STY" => ("", "Grava Y em {o}."),
        "STZ" => ("", "Grava zero em {o} (65C02)."),
        "TAX" => ("NZ", "Copia A para X."),
        "TAY" => ("NZ", "Copia A para Y."),
        "TRB" => ("Z", "Desliga em {o} os bits ligados em A; Z mostra se A E {o} era zero (65C02)."),
        "TSB" => ("Z", "Liga em {o} os bits ligados em A; Z mostra se A E {o} era zero (65C02)."),
        "TSX" => ("NZ", "Copia SP para X."),
        "TXA" => ("NZ", "Copia X para A."),
        "TXS" => ("", "Copia X para SP (não altera flags)."),
        "TYA" => ("NZ", "Copia Y para A."),
        _ => return None,
    })
}

/// Endereço efetivo do operando (ou destino do desvio) com os registradores dados.
fn endereco_efetivo(mem: &dyn DebugMem, modo: Mode, bytes: &[u8], r: Regs, addr: u16) -> u16 {
    let b1 = bytes.get(1).copied().unwrap_or(0);
    let w = b1 as u16 | (bytes.get(2).copied().unwrap_or(0) as u16) << 8;
    let zp_word =
        |p: u8| mem.read(p as u16) as u16 | (mem.read(p.wrapping_add(1) as u16) as u16) << 8;
    match modo {
        Mode::Implied | Mode::Imm => 0,
        Mode::Zp => b1 as u16,
        Mode::Abs => w,
        Mode::ZpX => b1.wrapping_add(r.x) as u16,
        Mode::ZpY => b1.wrapping_add(r.y) as u16,
        Mode::AbsX => w.wrapping_add(r.x as u16),
        Mode::AbsY => w.wrapping_add(r.y as u16),
        Mode::IndZpX => zp_word(b1.wrapping_add(r.x)),
        Mode::IndZpY => zp_word(b1).wrapping_add(r.y as u16),
        Mode::IndZp => zp_word(b1),
        Mode::IndAbs => mem.read_word(w),
        Mode::IndAbsX => mem.read_word(w.wrapping_add(r.x as u16)),
        Mode::Rel => addr.wrapping_add(2).wrapping_add(b1 as i8 as u16),
    }
}

/// Descreve o operando conforme o modo; `alvo` é o endereço efetivo calculado pelo disassembler.
fn operando_pt(modo: Mode, bytes: &[u8], alvo: u16, mem: &dyn DebugMem, mn: &str) -> String {
    let b1 = bytes.get(1).copied().unwrap_or(0);
    let w = || {
        bytes.get(1).copied().unwrap_or(0) as u16 | (bytes.get(2).copied().unwrap_or(0) as u16) << 8
    };
    // Instruções que só gravam: "o byte em" soaria como leitura.
    let _ = mn;
    match modo {
        Mode::Implied => "o acumulador ou o registrador implícito".into(),
        Mode::Imm => format!("o valor imediato ${b1:02X}"),
        Mode::Zp => format!("o byte da página zero ${b1:02X}"),
        Mode::Abs => format!("o byte no endereço ${:04X}", w()),
        Mode::ZpX => format!("o byte em ${b1:02X}+X, ou seja, ${alvo:04X}"),
        Mode::ZpY => format!("o byte em ${b1:02X}+Y, ou seja, ${alvo:04X}"),
        Mode::AbsX => format!("o byte em ${:04X}+X, ou seja, ${alvo:04X}", w()),
        Mode::AbsY => format!("o byte em ${:04X}+Y, ou seja, ${alvo:04X}", w()),
        Mode::IndZpX => {
            format!("o byte apontado pelo ponteiro em ${b1:02X}+X (endereço ${alvo:04X})")
        }
        Mode::IndZpY => {
            format!("o byte apontado pelo ponteiro em ${b1:02X}, mais Y (endereço ${alvo:04X})")
        }
        Mode::IndZp => format!("o byte apontado pelo ponteiro em ${b1:02X} (endereço ${alvo:04X})"),
        Mode::IndAbs => format!("o endereço guardado em ${:04X} (${alvo:04X})", w()),
        Mode::IndAbsX => format!("o endereço guardado em ${:04X}+X (${alvo:04X})", w()),
        Mode::Rel => {
            let _ = mem;
            format!("o destino ${alvo:04X}")
        }
    }
}

fn instrucao_texto(ctx: &DisasmCtx, line: &apple2_debugger::disasm::DisasmLine) -> String {
    let spans = render_line(line, None, ctx.cfg);
    let tabs = Tabs::new(ctx.cfg);
    let text = line_text(&spans);
    let tail: String = text.chars().skip(tabs.instruction).collect();
    let n = tabs.immediate.saturating_sub(tabs.instruction);
    let s: String = tail.chars().take(n).collect();
    s.trim().to_string()
}

/// Explica a instrução em `regs.pc`, lendo da `mem`.
pub fn explicar(mem: &dyn DebugMem, regs: Regs, cmos: bool) -> Explicacao {
    let addr = regs.pc;
    let info = opcodes::table(cmos)[mem.read(addr) as usize];
    let syms = Symbols::default();
    let data = DataBlocks::default();
    let cfg = DisasmConfig {
        branch_type: 1,
        ..Default::default()
    };
    let ctx = DisasmCtx {
        mem,
        regs,
        cmos,
        symbols: &syms,
        data: &data,
        cfg: &cfg,
    };
    let line = disassemble(&ctx, addr);
    let n = line.bytes.max(1);
    let bytes: Vec<u8> = (0..n)
        .map(|i| mem.read(addr.wrapping_add(i as u16)))
        .collect();
    let mn = info.mnemonic.to_ascii_uppercase();
    let instrucao = instrucao_texto(&ctx, &line);

    let sim = simular(mem, regs, cmos);
    let (ciclos_base, extra) = if info.mode == Mode::Rel {
        let tomado = sim.ciclos >= 3;
        (
            2,
            match (tomado, sim.ciclos) {
                (false, _) => None,
                (true, 4) => Some("desvio tomado atravessando uma página (+2)"),
                (true, _) => Some("desvio tomado (+1)"),
            },
        )
    } else {
        let base = simular(mem, Regs { x: 0, y: 0, ..regs }, cmos)
            .ciclos
            .min(sim.ciclos);
        (
            base,
            if sim.ciclos > base {
                Some("o índice atravessou uma página de 256 bytes (page crossing, +1)")
            } else {
                None
            },
        )
    };

    let (flags_afetadas, modelo) = info_mnemonico(&mn).unwrap_or((
        "",
        "Instrução não documentada ou exclusiva de outra variante da CPU.",
    ));
    let alvo = endereco_efetivo(mem, info.mode, &bytes, regs, addr);
    let operando = operando_pt(info.mode, &bytes, alvo, mem, &mn);
    let descricao = modelo.replace("{o}", &operando);

    let mut efeitos = Vec::new();
    let (a, d) = (regs, sim.depois);
    let mut reg = |nome: &str, antes: u8, depois: u8| {
        if antes != depois {
            efeitos.push(format!(
                "{nome}: ${antes:02X} -> ${depois:02X} ({depois} em decimal)"
            ));
        }
    };
    reg("A", a.a, d.a);
    reg("X", a.x, d.x);
    reg("Y", a.y, d.y);
    reg("SP", a.sp, d.sp);
    if a.p | 0x20 != d.p {
        efeitos.push(format!(
            "Flags (NV-BDIZC): {} -> {}",
            flags_text(a.p | 0x20),
            flags_text(d.p)
        ));
    }
    for (e, antes, depois) in &sim.escritas {
        efeitos.push(format!("Memória ${e:04X}: ${antes:02X} -> ${depois:02X}"));
    }
    let seq = regs.pc.wrapping_add(n as u16);
    if d.pc != seq {
        efeitos.push(format!("PC: ${:04X} -> ${:04X} (salto)", regs.pc, d.pc));
    } else {
        efeitos.push(format!(
            "PC: ${:04X} -> ${:04X} (próxima instrução)",
            regs.pc, d.pc
        ));
    }

    Explicacao {
        endereco: addr,
        opcode: bytes[0],
        bytes,
        mnemonico: mn,
        instrucao,
        modo: info.mode,
        ciclos: sim.ciclos,
        ciclos_base,
        extra,
        flags_afetadas,
        descricao,
        antes: regs,
        depois: sim.depois,
        escritas: sim.escritas,
        efeitos,
        extensao: info.is_extension() && !cmos,
    }
}

impl Explicacao {
    /// Texto para o terminal.
    pub fn texto(&self) -> String {
        let bytes: Vec<String> = self.bytes.iter().map(|b| format!("{b:02X}")).collect();
        let mut s = String::new();
        s.push_str(&format!(
            "${:04X}: {:<9} {}\n",
            self.endereco,
            bytes.join(" "),
            self.instrucao
        ));
        s.push_str(&format!("  Mnemônico: {}\n", self.mnemonico));
        s.push_str(&format!(
            "  Modo de endereçamento: {}\n",
            modo_pt(self.modo)
        ));
        s.push_str(&format!(
            "  Tamanho: {} byte(s); opcode ${:02X}\n",
            self.bytes.len(),
            self.opcode
        ));
        match self.extra {
            Some(m) => s.push_str(&format!(
                "  Ciclos: {} (base {}, mais um extra: {m})\n",
                self.ciclos, self.ciclos_base
            )),
            None => s.push_str(&format!("  Ciclos: {}\n", self.ciclos)),
        }
        s.push_str(&format!(
            "  Flags afetadas: {}\n",
            if self.flags_afetadas.is_empty() {
                "nenhuma".to_string()
            } else {
                self.flags_afetadas
                    .chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
        s.push_str(&format!("  O que faz: {}\n", self.descricao));
        if self.extensao {
            s.push_str("  Atenção: opcode não documentado no 6502 NMOS.\n");
        }
        s.push_str("  Efeito com os registradores dados:\n");
        for e in &self.efeitos {
            s.push_str(&format!("    - {e}\n"));
        }
        s
    }
}

/// Explica `n` instruções em sequência a partir de `regs.pc`, avançando os registradores e a
/// memória virtual a cada uma (os desvios seguem o fluxo real).
pub fn explicar_sequencia(
    mem: &mut dyn DebugMem,
    mut regs: Regs,
    cmos: bool,
    n: usize,
) -> Vec<Explicacao> {
    let mut out = Vec::new();
    for _ in 0..n {
        let e = explicar(mem, regs, cmos);
        for (a, _, v) in &e.escritas {
            mem.write(*a, *v);
        }
        regs = e.depois;
        out.push(e);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_debugger::mem::Ram;

    fn regs(pc: u16) -> Regs {
        Regs {
            sp: 0xFF,
            pc,
            p: 0x20,
            ..Default::default()
        }
    }

    #[test]
    fn lda_imediato() {
        let mut ram = Ram::default();
        ram.load(0x800, &[0xA9, 0x00]);
        let e = explicar(&ram, regs(0x800), false);
        assert_eq!(e.mnemonico, "LDA");
        assert_eq!(e.instrucao, "LDA #$00");
        assert_eq!(e.ciclos, 2);
        assert_eq!(e.flags_afetadas, "NZ");
        assert!(e.descricao.contains("$00"));
        assert!(e.efeitos.iter().any(|f| f.starts_with("Flags")));
    }

    #[test]
    fn page_crossing_soma_um_ciclo() {
        let mut ram = Ram::default();
        ram.load(0x800, &[0xBD, 0xF0, 0x03]); // LDA $03F0,X
        let sem = explicar(
            &ram,
            Regs {
                x: 0x0F,
                ..regs(0x800)
            },
            false,
        );
        assert_eq!((sem.ciclos, sem.extra), (4, None));
        let com = explicar(
            &ram,
            Regs {
                x: 0x10,
                ..regs(0x800)
            },
            false,
        );
        assert_eq!(com.ciclos, 5);
        assert_eq!(com.ciclos_base, 4);
        assert!(com.extra.unwrap().contains("page crossing"));
    }

    #[test]
    fn desvio_tomado_e_nao_tomado() {
        let mut ram = Ram::default();
        ram.load(0x80F0, &[0xD0, 0x20]); // BNE +32 (cruza página)
        let nao = explicar(
            &ram,
            Regs {
                p: 0x22,
                ..regs(0x80F0)
            },
            false,
        );
        assert_eq!(nao.ciclos, 2);
        let sim = explicar(&ram, regs(0x80F0), false);
        assert_eq!(sim.ciclos, 4);
        assert_eq!(sim.depois.pc, 0x8112);
    }

    #[test]
    fn escrita_na_memoria() {
        let mut ram = Ram::default();
        ram.load(0x800, &[0x8D, 0x00, 0x03]); // STA $0300
        let e = explicar(
            &ram,
            Regs {
                a: 7,
                ..regs(0x800)
            },
            false,
        );
        assert_eq!(e.escritas, vec![(0x0300, 0, 7)]);
        assert_eq!(e.ciclos, 4);
    }

    #[test]
    fn decimal_adc() {
        let mut ram = Ram::default();
        ram.load(0x800, &[0x69, 0x01]); // ADC #1
        let e = explicar(
            &ram,
            Regs {
                a: 0x09,
                p: 0x28,
                ..regs(0x800)
            },
            false,
        );
        assert_eq!(e.depois.a, 0x10);
    }
}
