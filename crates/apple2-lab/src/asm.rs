//! Montador de várias linhas, em cima do montador de uma instrução do depurador.
//!
//! Sintaxe aceita (uma instrução por linha, `;` começa um comentário):
//!
//! ```text
//!         ORG $0800        ; endereço de montagem (também ".ORG" ou "*=")
//! TOTAL   = $0300          ; constante (também ".EQU" / "EQU")
//! INICIO: LDA #5           ; rótulo com ":" ou sem, na coluna 0
//!         .BYTE $01,$02    ; bytes (números, constantes ou rótulos já definidos)
//!         .WORD INICIO     ; palavras de 16 bits, little-endian
//! ```
//!
//! Números sem prefixo são decimais e `$` é hexa, como no depurador. Rótulos podem ser usados
//! antes de definidos em instruções (o depurador completa depois).

use apple2_debugger::assembler::Assembler;
use apple2_debugger::mem::{DebugMem, Ram};
use apple2_debugger::opcodes;
use apple2_debugger::symbols::{Symbols, Table};

/// Resultado da montagem.
#[derive(Clone, Debug, Default)]
pub struct Montado {
    /// Blocos contíguos `(endereço, bytes)`, na ordem de montagem.
    pub segmentos: Vec<(u16, Vec<u8>)>,
    /// Rótulos e constantes, ordenados por nome.
    pub simbolos: Vec<(String, u16)>,
}

impl Montado {
    /// Endereço de um rótulo/constante (sem diferenciar maiúsculas).
    pub fn simbolo(&self, nome: &str) -> Option<u16> {
        self.simbolos
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nome))
            .map(|(_, a)| *a)
    }

    /// Total de bytes gerados.
    pub fn tamanho(&self) -> usize {
        self.segmentos.iter().map(|(_, b)| b.len()).sum()
    }
}

fn eh_ident(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(f) if f.is_ascii_alphabetic() || f == '_')
        && c.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn eh_mnemonico(s: &str, cmos: bool) -> bool {
    opcodes::table(cmos)
        .iter()
        .any(|o| o.mnemonic.eq_ignore_ascii_case(s))
}

fn valor(expr: &str, syms: &Symbols) -> Result<u32, String> {
    let e = expr.trim();
    let (modo, resto) = match e.chars().next() {
        Some('<') => ('<', &e[1..]),
        Some('>') => ('>', &e[1..]),
        _ => (' ', e),
    };
    let (base, off) = match resto
        .char_indices()
        .skip(1)
        .find(|(_, c)| *c == '+' || *c == '-')
    {
        Some((i, c)) => {
            let o = valor(&resto[i + 1..], syms)?;
            (&resto[..i], if c == '+' { o as i64 } else { -(o as i64) })
        }
        None => (resto, 0),
    };
    let b = base.trim();
    let v = if let Some((a, _)) = syms.find_address(b) {
        a as u32
    } else if b.starts_with('$') || b.starts_with("0x") || b.chars().all(|c| c.is_ascii_digit()) {
        crate::parse_num(b)?
    } else {
        return Err(format!("símbolo indefinido: '{b}'"));
    };
    let v = (v as i64 + off) as u32 & 0xFFFF;
    Ok(match modo {
        '<' => v & 0xFF,
        '>' => v >> 8,
        _ => v,
    })
}

/// Monta `fonte`. `org_padrao` vale até o primeiro `ORG`.
pub fn montar(fonte: &str, org_padrao: u16, cmos: bool) -> Result<Montado, String> {
    let mut ram = Ram::default();
    let mut syms = Symbols::default();
    let mut asm = Assembler::new(org_padrao);
    // (início, fim exclusivo) de cada bloco.
    let mut blocos: Vec<(u16, usize)> = vec![(org_padrao, org_padrao as usize)];
    let mut nomes: Vec<String> = Vec::new();

    for (n, bruta) in fonte.lines().enumerate() {
        let erro = |m: String| format!("linha {}: {m}", n + 1);
        let sem_comentario = bruta.split(';').next().unwrap_or("");
        let mut linha = sem_comentario.trim_end();
        if linha.trim().is_empty() {
            continue;
        }
        let coluna0 = !linha.starts_with(char::is_whitespace);
        linha = linha.trim_start();

        // Constante: NOME = valor | NOME .EQU valor | NOME EQU valor
        let partes: Vec<&str> = linha.splitn(3, char::is_whitespace).collect();
        let eq = if let Some((nome, v)) = linha.split_once('=') {
            let nome = nome.trim();
            if eh_ident(nome) && !nome.eq_ignore_ascii_case("ORG") {
                Some((nome.to_string(), v.to_string()))
            } else {
                None
            }
        } else if partes.len() == 3
            && eh_ident(partes[0])
            && (partes[1].eq_ignore_ascii_case("EQU") || partes[1].eq_ignore_ascii_case(".EQU"))
        {
            Some((partes[0].to_string(), partes[2].to_string()))
        } else {
            None
        };
        if let Some((nome, v)) = eq {
            if linha.starts_with('*') {
                // "*= $0800" é ORG, tratado abaixo.
            } else {
                let v = valor(&v, &syms).map_err(erro)? as u16;
                syms.set(Table::User1, &nome, v);
                nomes.push(nome);
                continue;
            }
        }

        // Rótulo: "NOME:" ou, na coluna 0, uma palavra que não é mnemônico nem diretiva.
        let primeiro = linha.split_whitespace().next().unwrap_or("");
        let mut resto = linha;
        let rotulo = if let Some(r) = primeiro.strip_suffix(':') {
            Some(r)
        } else if coluna0
            && eh_ident(primeiro)
            && !eh_mnemonico(primeiro, cmos)
            && !primeiro.eq_ignore_ascii_case("ORG")
        {
            Some(primeiro)
        } else {
            None
        };
        if let Some(r) = rotulo {
            if !eh_ident(r) {
                return Err(erro(format!("rótulo inválido: '{r}'")));
            }
            if syms.find_address(r).is_some() {
                return Err(erro(format!("rótulo repetido: '{r}'")));
            }
            syms.set(Table::User1, r, asm.address);
            nomes.push(r.to_string());
            resto = linha[primeiro.len()..].trim();
            if resto.is_empty() {
                continue;
            }
        }

        let (dir, args) = match resto.find(char::is_whitespace) {
            Some(i) => (&resto[..i], resto[i..].trim()),
            None => (resto, ""),
        };
        let dir_up = dir.to_ascii_uppercase();
        if dir_up == "ORG" || dir_up == ".ORG" || dir_up.starts_with("*=") {
            let a = if dir_up.starts_with("*=") {
                format!("{}{args}", &dir[2..])
            } else {
                args.to_string()
            };
            let end = valor(&a, &syms).map_err(erro)? as u16;
            asm.address = end;
            blocos.push((end, end as usize));
            continue;
        }
        if dir_up == ".BYTE" || dir_up == ".WORD" {
            let palavra = dir_up == ".WORD";
            for item in args.split(',') {
                let v = valor(item, &syms).map_err(erro)?;
                let n = if palavra { 2 } else { 1 };
                if asm.address as usize + n > 0x10000 {
                    return Err(erro("passou do fim da memória".into()));
                }
                ram.write(asm.address, v as u8);
                if palavra {
                    ram.write(asm.address + 1, (v >> 8) as u8);
                }
                asm.address = asm.address.wrapping_add(n as u16);
            }
        } else {
            if dir.starts_with('.') {
                return Err(erro(format!("diretiva desconhecida: '{dir}'")));
            }
            // O montador do depurador já avança `asm.address`.
            asm.assemble(&mut ram, &syms, cmos, resto).map_err(erro)?;
        }
        if let Some(b) = blocos.last_mut() {
            b.1 = b.1.max(asm.address as usize);
            if asm.address == 0 {
                b.1 = 0x10000;
            }
        }
    }
    asm.resolve_delayed(&mut ram, &syms);
    if asm.pending() > 0 {
        return Err("referência a símbolo não definido (rótulo usado e nunca declarado)".into());
    }

    let mut out = Montado::default();
    for (ini, fim) in blocos {
        if fim > ini as usize {
            out.segmentos.push((ini, ram.0[ini as usize..fim].to_vec()));
        }
    }
    nomes.sort();
    nomes.dedup();
    for n in nomes {
        if let Some((a, _)) = syms.find_address(&n) {
            out.simbolos.push((n, a));
        }
    }
    Ok(out)
}

/// Monta o texto e grava os segmentos na Máquina.
pub fn carregar(m: &mut apple2_core::Machine, montado: &Montado) -> Result<(), String> {
    for (a, b) in &montado.segmentos {
        crate::bare::load(m, *a, b)?;
    }
    Ok(())
}
