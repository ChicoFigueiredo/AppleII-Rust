//! Trace didático: executa N instruções numa Máquina e registra o estado antes e depois de cada
//! uma, destacando o que mudou. Exporta CSV e Markdown.

use apple2_core::Machine;
use apple2_debugger::mem::{regs_of, Regs};

use crate::explain::{explicar, Escrita};
use crate::flags_text;

/// Uma linha do trace.
#[derive(Clone, Debug)]
pub struct LinhaTrace {
    pub passo: usize,
    pub endereco: u16,
    pub bytes: Vec<u8>,
    pub instrucao: String,
    pub ciclos: u32,
    pub ciclos_total: u64,
    pub antes: Regs,
    pub depois: Regs,
    pub escritas: Vec<Escrita>,
}

impl LinhaTrace {
    /// Nomes dos registradores que mudaram (`A`, `X`, `Y`, `SP`, `P`, `PC`) mais `MEM`.
    pub fn mudou(&self) -> Vec<&'static str> {
        let (a, d) = (&self.antes, &self.depois);
        let mut v = Vec::new();
        if a.a != d.a {
            v.push("A");
        }
        if a.x != d.x {
            v.push("X");
        }
        if a.y != d.y {
            v.push("Y");
        }
        if a.sp != d.sp {
            v.push("SP");
        }
        if a.p != d.p {
            v.push("P");
        }
        if !self.escritas.is_empty() {
            v.push("MEM");
        }
        v
    }

    fn bytes_hex(&self) -> String {
        self.bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn escritas_texto(&self) -> String {
        self.escritas
            .iter()
            .map(|(a, ant, dep)| format!("${a:04X}: ${ant:02X}->${dep:02X}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Executa até `n` instruções (para antes se a próxima for BRK) e devolve as linhas.
pub fn rastrear(m: &mut Machine, n: usize) -> Vec<LinhaTrace> {
    let cmos = m.config().is_cmos();
    let mut out = Vec::new();
    for passo in 1..=n {
        let antes = regs_of(m);
        if m.peek(antes.pc) == 0x00 {
            break;
        }
        let e = explicar(&*m, antes, cmos);
        let ciclos = m.step();
        let depois = regs_of(m);
        out.push(LinhaTrace {
            passo,
            endereco: antes.pc,
            bytes: e.bytes,
            instrucao: e.instrucao,
            ciclos,
            ciclos_total: m.cycles(),
            antes,
            depois,
            escritas: e.escritas,
        });
        if depois.pc == antes.pc {
            break; // laço de uma instrução: nada mais a mostrar
        }
    }
    out
}

fn csv_campo(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV com cabeçalho. Colunas `*_antes` e `*_depois` em hexa; `mudou` lista os registradores
/// alterados.
pub fn para_csv(linhas: &[LinhaTrace]) -> String {
    let mut s = String::from(
        "passo,endereco,bytes,instrucao,ciclos,ciclos_total,\
pc_antes,a_antes,x_antes,y_antes,sp_antes,p_antes,\
pc_depois,a_depois,x_depois,y_depois,sp_depois,p_depois,mudou,escritas\n",
    );
    for l in linhas {
        let (a, d) = (&l.antes, &l.depois);
        let campos = [
            l.passo.to_string(),
            format!("{:04X}", l.endereco),
            l.bytes_hex(),
            l.instrucao.clone(),
            l.ciclos.to_string(),
            l.ciclos_total.to_string(),
            format!("{:04X}", a.pc),
            format!("{:02X}", a.a),
            format!("{:02X}", a.x),
            format!("{:02X}", a.y),
            format!("{:02X}", a.sp),
            format!("{:02X}", a.p),
            format!("{:04X}", d.pc),
            format!("{:02X}", d.a),
            format!("{:02X}", d.x),
            format!("{:02X}", d.y),
            format!("{:02X}", d.sp),
            format!("{:02X}", d.p),
            l.mudou().join(" "),
            l.escritas_texto(),
        ];
        s.push_str(
            &campos
                .iter()
                .map(|c| csv_campo(c))
                .collect::<Vec<_>>()
                .join(","),
        );
        s.push('\n');
    }
    s
}

fn celula(antes: String, depois: String) -> String {
    if antes == depois {
        depois
    } else {
        format!("**{antes} -> {depois}**")
    }
}

/// Tabela Markdown. Células que mudaram aparecem em negrito como `antes -> depois`.
pub fn para_markdown(linhas: &[LinhaTrace]) -> String {
    let mut s = String::from(
        "| # | Endereço | Bytes | Instrução | Ciclos | A | X | Y | SP | P (NV-BDIZC) | Memória |\n\
|--:|---|---|---|--:|---|---|---|---|---|---|\n",
    );
    for l in linhas {
        let (a, d) = (&l.antes, &l.depois);
        s.push_str(&format!(
            "| {} | ${:04X} | {} | `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
            l.passo,
            l.endereco,
            l.bytes_hex(),
            l.instrucao,
            l.ciclos,
            celula(format!("{:02X}", a.a), format!("{:02X}", d.a)),
            celula(format!("{:02X}", a.x), format!("{:02X}", d.x)),
            celula(format!("{:02X}", a.y), format!("{:02X}", d.y)),
            celula(format!("{:02X}", a.sp), format!("{:02X}", d.sp)),
            celula(flags_text(a.p), flags_text(d.p)),
            l.escritas_texto(),
        ));
    }
    let total: u64 = linhas.iter().map(|l| l.ciclos as u64).sum();
    s.push_str(&format!(
        "\n{} instrução(ões), {} ciclos. Em negrito: o que mudou (`antes -> depois`).\n",
        linhas.len(),
        total
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bare::{bare_machine, load, BareConfig};

    #[test]
    fn trace_de_tres_instrucoes() {
        let mut m = bare_machine(BareConfig::new(false, 0x800));
        // LDA #5 ; STA $0300 ; BRK
        load(&mut m, 0x800, &[0xA9, 0x05, 0x8D, 0x00, 0x03, 0x00]).unwrap();
        let t = rastrear(&mut m, 10);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].mudou(), vec!["A"]);
        assert_eq!(t[1].escritas, vec![(0x300, 0, 5)]);
        assert_eq!(t[1].ciclos, 4);
        let csv = para_csv(&t);
        assert!(csv.lines().count() == 3);
        let md = para_markdown(&t);
        assert!(md.contains("-> 05**"), "{md}");
    }
}
