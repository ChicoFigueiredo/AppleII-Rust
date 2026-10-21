//! Exercícios reproduzíveis de Assembly: formato TOML e verificador.
//!
//! ```toml
//! titulo    = "Somar dois números"
//! enunciado = "Some os bytes em $0300 e $0301 e guarde o resultado em $0302."
//! cpu       = "6502"       # ou "65c02"
//! org       = 0x0800       # onde a solução é montada; o RESET aponta para cá
//! max_instrucoes = 1000
//!
//! [[caso]]
//! nome = "5 + 7"
//! [[caso.entrada.memoria]]
//! endereco = 0x0300
//! bytes = "05 07"
//! [[caso.esperado.memoria]]
//! endereco = 0x0302
//! bytes = "0C"
//! ```
//!
//! A solução termina com `BRK` ou num laço de uma instrução (`FIM JMP FIM`).

use apple2_core::Machine;
use serde::Deserialize;

use crate::asm;
use crate::bare::{bare_machine, load, run, BareConfig, Parada};
use crate::{flags_text, parse_hex_bytes};

/// Registradores de entrada ou esperados (os que faltam não são tocados nem verificados).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registradores {
    pub a: Option<u8>,
    pub x: Option<u8>,
    pub y: Option<u8>,
    pub sp: Option<u8>,
    pub pc: Option<u16>,
}

/// Flags de entrada ou esperadas.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flags {
    pub n: Option<bool>,
    pub v: Option<bool>,
    pub d: Option<bool>,
    pub i: Option<bool>,
    pub z: Option<bool>,
    pub c: Option<bool>,
}

impl Flags {
    fn bits(&self) -> [(char, u8, Option<bool>); 6] {
        [
            ('N', 0x80, self.n),
            ('V', 0x40, self.v),
            ('D', 0x08, self.d),
            ('I', 0x04, self.i),
            ('Z', 0x02, self.z),
            ('C', 0x01, self.c),
        ]
    }
}

/// Um bloco de memória: `bytes` em hexa.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bloco {
    pub endereco: u16,
    pub bytes: String,
}

/// Estado de entrada (antes de rodar) ou esperado (depois).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Estado {
    #[serde(default)]
    pub registradores: Registradores,
    #[serde(default)]
    pub flags: Flags,
    #[serde(default)]
    pub memoria: Vec<Bloco>,
}

/// Programa já pronto, carregado antes da solução (rotinas auxiliares, dados fixos).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramaInicial {
    /// Assembly do montador do depurador (rótulos e `ORG` valem).
    pub assembly: Option<String>,
    /// Bytes em hexa, gravados em `endereco`.
    pub hex: Option<String>,
    pub endereco: Option<u16>,
}

/// Vetores do sistema; o RESET sempre aponta para `org`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vetores {
    pub irq: Option<u16>,
    pub nmi: Option<u16>,
}

/// Um caso de teste.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Caso {
    pub nome: String,
    #[serde(default)]
    pub entrada: Estado,
    #[serde(default)]
    pub esperado: Estado,
}

/// Um exercício completo.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exercicio {
    pub titulo: String,
    pub enunciado: String,
    #[serde(default)]
    pub dica: Option<String>,
    #[serde(default = "cpu_padrao")]
    pub cpu: String,
    #[serde(default = "org_padrao")]
    pub org: u16,
    #[serde(default = "max_padrao")]
    pub max_instrucoes: u64,
    /// Limite de ciclos da solução (desafio de desempenho); opcional.
    #[serde(default)]
    pub max_ciclos: Option<u64>,
    /// Para quando o PC chegar aqui (alternativa a BRK/laço).
    #[serde(default)]
    pub fim: Option<u16>,
    #[serde(default)]
    pub vetores: Vetores,
    #[serde(default)]
    pub programa_inicial: Option<ProgramaInicial>,
    #[serde(default)]
    pub caso: Vec<Caso>,
}

fn cpu_padrao() -> String {
    "6502".into()
}
fn org_padrao() -> u16 {
    0x0800
}
fn max_padrao() -> u64 {
    100_000
}

/// Resultado de um caso.
#[derive(Clone, Debug)]
pub struct ResultadoCaso {
    pub nome: String,
    pub aprovado: bool,
    pub falhas: Vec<String>,
    pub instrucoes: u64,
    pub ciclos: u64,
    pub parada: Parada,
}

/// Resultado do exercício.
#[derive(Clone, Debug)]
pub struct Veredito {
    pub titulo: String,
    pub aprovado: bool,
    pub casos: Vec<ResultadoCaso>,
    /// Bytes da solução montada.
    pub tamanho: usize,
}

impl Veredito {
    pub fn texto(&self) -> String {
        let mut s = format!("Exercício: {}\n", self.titulo);
        for c in &self.casos {
            s.push_str(&format!(
                "  [{}] {} ({} instruções, {} ciclos; {})\n",
                if c.aprovado { "OK" } else { "FALHOU" },
                c.nome,
                c.instrucoes,
                c.ciclos,
                c.parada.texto()
            ));
            for f in &c.falhas {
                s.push_str(&format!("      - {f}\n"));
            }
        }
        s.push_str(&format!(
            "Resultado: {} ({} byte(s) de código)\n",
            if self.aprovado {
                "APROVADO"
            } else {
                "REPROVADO"
            },
            self.tamanho
        ));
        s
    }
}

impl Exercicio {
    /// Lê o TOML e valida o que dá para validar sem executar.
    pub fn de_toml(texto: &str) -> Result<Exercicio, String> {
        let e: Exercicio = toml::from_str(texto).map_err(|e| format!("TOML inválido: {e}"))?;
        e.validar()?;
        Ok(e)
    }

    fn cmos(&self) -> Result<bool, String> {
        match self.cpu.to_ascii_lowercase().as_str() {
            "6502" => Ok(false),
            "65c02" => Ok(true),
            o => Err(format!(
                "cpu desconhecida: '{o}' (use \"6502\" ou \"65c02\")"
            )),
        }
    }

    fn validar(&self) -> Result<(), String> {
        self.cmos()?;
        if self.caso.is_empty() {
            return Err("o exercício precisa de pelo menos um [[caso]]".into());
        }
        if let Some(p) = &self.programa_inicial {
            if p.assembly.is_some() == p.hex.is_some() {
                return Err("programa_inicial: informe `assembly` ou `hex`, só um dos dois".into());
            }
        }
        for c in &self.caso {
            for b in c.entrada.memoria.iter().chain(&c.esperado.memoria) {
                parse_hex_bytes(&b.bytes).map_err(|e| format!("caso '{}': {e}", c.nome))?;
            }
        }
        Ok(())
    }

    fn montar_maquina(&self, solucao: &asm::Montado, caso: &Caso) -> Result<Machine, String> {
        let cmos = self.cmos()?;
        let mut m = bare_machine(BareConfig {
            cmos,
            reset: self.org,
            irq: self.vetores.irq.unwrap_or(self.org),
            nmi: self.vetores.nmi.unwrap_or(self.org),
        });
        if let Some(p) = &self.programa_inicial {
            if let Some(src) = &p.assembly {
                let a = asm::montar(src, p.endereco.unwrap_or(0x0900), cmos)
                    .map_err(|e| format!("programa_inicial: {e}"))?;
                asm::carregar(&mut m, &a)?;
            } else if let Some(h) = &p.hex {
                let bytes = parse_hex_bytes(h)?;
                load(&mut m, p.endereco.unwrap_or(0x0900), &bytes)?;
            }
        }
        asm::carregar(&mut m, solucao)?;
        for b in &caso.entrada.memoria {
            load(&mut m, b.endereco, &parse_hex_bytes(&b.bytes)?)?;
        }
        let r = &caso.entrada.registradores;
        let c = m.cpu_mut();
        if let Some(v) = r.a {
            c.a = v;
        }
        if let Some(v) = r.x {
            c.x = v;
        }
        if let Some(v) = r.y {
            c.y = v;
        }
        if let Some(v) = r.sp {
            c.sp = v;
        }
        if let Some(v) = r.pc {
            c.pc = v;
        }
        for (_, bit, v) in caso.entrada.flags.bits() {
            match v {
                Some(true) => c.p |= bit,
                Some(false) => c.p &= !bit,
                None => {}
            }
        }
        Ok(m)
    }

    /// Monta `fonte` (a solução do aluno) e roda todos os casos.
    pub fn verificar(&self, fonte: &str) -> Result<Veredito, String> {
        let cmos = self.cmos()?;
        let solucao = asm::montar(fonte, self.org, cmos).map_err(|e| format!("solução: {e}"))?;
        let mut casos = Vec::new();
        for caso in &self.caso {
            let mut m = self.montar_maquina(&solucao, caso)?;
            let ex = run(&mut m, self.max_instrucoes, self.fim);
            let mut falhas = Vec::new();
            if ex.parada == Parada::Limite {
                falhas.push(format!(
                    "o programa não terminou em {} instruções (laço sem fim?)",
                    self.max_instrucoes
                ));
            }
            if let Some(max) = self.max_ciclos {
                if ex.ciclos > max {
                    falhas.push(format!("gastou {} ciclos; o limite é {max}", ex.ciclos));
                }
            }
            confere(&m, &caso.esperado, &mut falhas)?;
            casos.push(ResultadoCaso {
                nome: caso.nome.clone(),
                aprovado: falhas.is_empty(),
                falhas,
                instrucoes: ex.instrucoes,
                ciclos: ex.ciclos,
                parada: ex.parada,
            });
        }
        Ok(Veredito {
            titulo: self.titulo.clone(),
            aprovado: casos.iter().all(|c| c.aprovado),
            casos,
            tamanho: solucao.tamanho(),
        })
    }
}

fn confere(m: &Machine, esp: &Estado, falhas: &mut Vec<String>) -> Result<(), String> {
    let c = m.cpu();
    let r = &esp.registradores;
    for (nome, esperado, atual) in [
        ("A", r.a, c.a),
        ("X", r.x, c.x),
        ("Y", r.y, c.y),
        ("SP", r.sp, c.sp),
    ] {
        if let Some(e) = esperado {
            if e != atual {
                falhas.push(format!("{nome}: esperado ${e:02X}, obtido ${atual:02X}"));
            }
        }
    }
    if let Some(e) = r.pc {
        if e != c.pc {
            falhas.push(format!("PC: esperado ${e:04X}, obtido ${:04X}", c.pc));
        }
    }
    for (nome, bit, v) in esp.flags.bits() {
        if let Some(e) = v {
            if (c.p & bit != 0) != e {
                falhas.push(format!(
                    "flag {nome}: esperado {}, obtido {} (P = {})",
                    u8::from(e),
                    u8::from(!e),
                    flags_text(c.p)
                ));
            }
        }
    }
    for b in &esp.memoria {
        let esperado = parse_hex_bytes(&b.bytes)?;
        for (i, e) in esperado.iter().enumerate() {
            let addr = b.endereco.wrapping_add(i as u16);
            let atual = m.peek(addr);
            if atual != *e {
                falhas.push(format!(
                    "memória ${addr:04X}: esperado ${e:02X}, obtido ${atual:02X}"
                ));
            }
        }
    }
    Ok(())
}
