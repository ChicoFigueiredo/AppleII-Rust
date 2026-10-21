//! Laboratório educacional do AppleII-Rust.
//!
//! - [`explain`]: explica uma instrução 6502/65C02 em português (modo, bytes, ciclos, flags, efeito).
//! - [`trace`]: executa N instruções e exporta CSV/Markdown com o estado antes e depois.
//! - [`exercise`]: exercícios reproduzíveis em TOML e o verificador.
//! - [`asm`]: montador de várias linhas (rótulos, `ORG`, `.BYTE`) sobre o montador do depurador.
//! - [`bare`]: a Máquina "nua", só RAM e vetores, sem ROM da Apple.
//!
//! As tabelas de opcodes e o disassembler vêm do `apple2-debugger`; os ciclos vêm de executar a
//! instrução no `apple2-cpu`. Nada disso é duplicado aqui.

#![forbid(unsafe_code)]

pub mod asm;
pub mod bare;
pub mod exercise;
pub mod explain;
pub mod trace;

/// Interpreta `$1F`, `0x1F` ou `31`.
pub fn parse_num(s: &str) -> Result<u32, String> {
    let t = s.trim();
    let r = if let Some(h) = t.strip_prefix('$') {
        u32::from_str_radix(h, 16)
    } else if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(h, 16)
    } else {
        t.parse::<u32>()
    };
    r.map_err(|_| format!("número inválido: '{s}'"))
}

/// Interpreta uma lista de bytes em hexa: `A9 05`, `A9,05`, `$A9 $05`, `0xA9 0x05`.
pub fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for tok in s.split(|c: char| c.is_whitespace() || c == ',') {
        if tok.is_empty() {
            continue;
        }
        let t = tok
            .trim_start_matches('$')
            .trim_start_matches("0x")
            .trim_start_matches("0X");
        // "A905" sem separador também vale.
        if t.len() % 2 != 0 {
            return Err(format!("byte hexa inválido: '{tok}'"));
        }
        for i in (0..t.len()).step_by(2) {
            let b = u8::from_str_radix(&t[i..i + 2], 16)
                .map_err(|_| format!("byte hexa inválido: '{tok}'"))?;
            out.push(b);
        }
    }
    Ok(out)
}

/// Flags como `NV-BDIZC`: letra se ligada, `.` se desligada.
pub fn flags_text(p: u8) -> String {
    let names = ['N', 'V', '-', 'B', 'D', 'I', 'Z', 'C'];
    (0..8)
        .map(|i| {
            let bit = 0x80u8 >> i;
            if names[i] == '-' {
                '-'
            } else if p & bit != 0 {
                names[i]
            } else {
                '.'
            }
        })
        .collect()
}
