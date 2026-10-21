//! `apple2-lab`: explica instruções, gera traces didáticos e verifica exercícios.

use std::process::ExitCode;

use apple2_debugger::mem::{Ram, Regs};
use apple2_lab::bare::{bare_machine, BareConfig};
use apple2_lab::exercise::Exercicio;
use apple2_lab::explain::explicar_sequencia;
use apple2_lab::trace::{para_csv, para_markdown, rastrear};
use apple2_lab::{asm, parse_hex_bytes, parse_num};

const AJUDA: &str = "\
apple2-lab: laboratório educacional de 6502/65C02

Uso:
  apple2-lab explain [opções] [BYTES_HEXA]
  apple2-lab trace   [opções] [PROGRAMA.asm]
  apple2-lab check   EXERCICIO.toml SOLUCAO.asm

explain: explica instruções (modo, bytes, ciclos, flags, efeito).
  BYTES_HEXA        bytes em hexa, ex.: \"A9 05\"
  --asm TEXTO       em vez dos bytes, monta este assembly (use \\n para várias linhas)
  --arquivo BIN     lê os bytes de um arquivo binário (com --org)
  --org END         endereço da primeira instrução (padrão $0800)
  -n N              quantas instruções explicar (padrão: todas as dos bytes dados, ou 1)
  --cpu 6502|65c02  variante da CPU (padrão 6502)
  --a V --x V --y V --sp V --p V   registradores iniciais (padrão 0, SP=$FF)
  --mem END=HEX     coloca bytes na memória virtual, ex.: --mem 0300=05,07

trace: executa N instruções numa Máquina nua e mostra o estado antes e depois.
  PROGRAMA.asm      programa em assembly (ou --hex \"A9 05 ...\")
  -n N              máximo de instruções (padrão 50)
  --formato F       csv, md ou ambos (padrão md)
  --saida PREFIXO   grava PREFIXO.csv / PREFIXO.md em vez de escrever na tela
  --org END --cpu C --a V --x V --y V   como em explain

check: roda a solução contra os casos do exercício; sai com código 0 se aprovada.
";

struct Args(Vec<String>);

impl Args {
    fn flag(&mut self, nome: &str) -> Result<Option<String>, String> {
        if let Some(i) = self.0.iter().position(|a| a == nome) {
            if i + 1 >= self.0.len() {
                return Err(format!("{nome} precisa de um valor"));
            }
            let v = self.0.remove(i + 1);
            self.0.remove(i);
            return Ok(Some(v));
        }
        Ok(None)
    }
    fn num(&mut self, nome: &str) -> Result<Option<u32>, String> {
        self.flag(nome)?.map(|v| parse_num(&v)).transpose()
    }
}

fn cmos(a: &mut Args) -> Result<bool, String> {
    match a.flag("--cpu")?.as_deref() {
        None | Some("6502") => Ok(false),
        Some("65c02") | Some("65C02") => Ok(true),
        Some(o) => Err(format!("cpu desconhecida: '{o}'")),
    }
}

fn regs_iniciais(a: &mut Args, pc: u16) -> Result<Regs, String> {
    Ok(Regs {
        a: a.num("--a")?.unwrap_or(0) as u8,
        x: a.num("--x")?.unwrap_or(0) as u8,
        y: a.num("--y")?.unwrap_or(0) as u8,
        sp: a.num("--sp")?.unwrap_or(0xFF) as u8,
        p: a.num("--p")?.unwrap_or(0x20) as u8,
        pc,
    })
}

fn cmd_explain(mut a: Args) -> Result<ExitCode, String> {
    let c = cmos(&mut a)?;
    let org = a.num("--org")?.unwrap_or(0x0800) as u16;
    let n = a.num("-n")?;
    let asm_txt = a.flag("--asm")?;
    let arquivo = a.flag("--arquivo")?;
    let mem_arg = a.flag("--mem")?;
    let regs = regs_iniciais(&mut a, org)?;

    let mut ram = Ram::default();
    let mut total_bytes = 0usize;
    let carrega = |ram: &mut Ram, end: u16, b: &[u8]| -> Result<(), String> {
        if end as usize + b.len() > 0x10000 {
            return Err("bytes passam do fim da memória de 64K".into());
        }
        ram.load(end, b);
        Ok(())
    };
    if let Some(txt) = asm_txt {
        let m = asm::montar(&txt.replace("\\n", "\n"), org, c)?;
        for (e, b) in &m.segmentos {
            carrega(&mut ram, *e, b)?;
            total_bytes += b.len();
        }
    } else if let Some(f) = arquivo {
        let b = std::fs::read(&f).map_err(|e| format!("{f}: {e}"))?;
        carrega(&mut ram, org, &b)?;
        total_bytes = b.len();
    } else if !a.0.is_empty() {
        let b = parse_hex_bytes(&a.0.join(" "))?;
        carrega(&mut ram, org, &b)?;
        total_bytes = b.len();
    } else {
        return Err("informe BYTES_HEXA, --asm ou --arquivo (veja --ajuda)".into());
    }
    if let Some(m) = mem_arg {
        let (e, h) = m.split_once('=').ok_or("--mem espera END=HEX")?;
        let e = e.trim().trim_start_matches('$').trim_start_matches("0x");
        let end =
            u16::from_str_radix(e, 16).map_err(|_| format!("endereço hexa inválido: '{e}'"))?;
        carrega(&mut ram, end, &parse_hex_bytes(h)?)?;
    }

    let explicacoes = match n {
        Some(n) => explicar_sequencia(&mut ram, regs, c, n as usize),
        None => {
            // Até cobrir todos os bytes dados.
            let mut out = Vec::new();
            let mut regs = regs;
            let fim = org as usize + total_bytes;
            while (regs.pc as usize) < fim && out.len() < 10_000 {
                let mut e = explicar_sequencia(&mut ram, regs, c, 1);
                let x = e.remove(0);
                regs = x.depois;
                let seguiu = x.endereco as usize + x.bytes.len();
                out.push(x);
                if regs.pc as usize != seguiu {
                    break; // desvio: o fluxo sai da sequência linear
                }
            }
            out
        }
    };
    for e in &explicacoes {
        println!("{}", e.texto());
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_trace(mut a: Args) -> Result<ExitCode, String> {
    let c = cmos(&mut a)?;
    let org = a.num("--org")?.unwrap_or(0x0800) as u16;
    let n = a.num("-n")?.unwrap_or(50) as usize;
    let formato = a.flag("--formato")?.unwrap_or_else(|| "md".into());
    let saida = a.flag("--saida")?;
    let hex = a.flag("--hex")?;
    let regs = regs_iniciais(&mut a, org)?;

    let mut m = bare_machine(BareConfig::new(c, org));
    if let Some(h) = hex {
        apple2_lab::bare::load(&mut m, org, &parse_hex_bytes(&h)?)?;
    } else if let Some(f) = a.0.first() {
        let src = std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"))?;
        let montado = asm::montar(&src, org, c)?;
        asm::carregar(&mut m, &montado)?;
    } else {
        return Err("informe PROGRAMA.asm ou --hex".into());
    }
    {
        let cpu = m.cpu_mut();
        cpu.a = regs.a;
        cpu.x = regs.x;
        cpu.y = regs.y;
        cpu.sp = regs.sp;
        cpu.p = regs.p;
    }
    let linhas = rastrear(&mut m, n);
    let (quer_csv, quer_md) = match formato.as_str() {
        "csv" => (true, false),
        "md" => (false, true),
        "ambos" => (true, true),
        o => return Err(format!("formato desconhecido: '{o}' (csv, md ou ambos)")),
    };
    match saida {
        Some(p) => {
            if quer_csv {
                std::fs::write(format!("{p}.csv"), para_csv(&linhas)).map_err(|e| e.to_string())?;
            }
            if quer_md {
                std::fs::write(format!("{p}.md"), para_markdown(&linhas))
                    .map_err(|e| e.to_string())?;
            }
        }
        None => {
            if quer_csv {
                print!("{}", para_csv(&linhas));
            }
            if quer_md {
                print!("{}", para_markdown(&linhas));
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_check(a: Args) -> Result<ExitCode, String> {
    if a.0.len() != 2 {
        return Err("uso: apple2-lab check EXERCICIO.toml SOLUCAO.asm".into());
    }
    let toml = std::fs::read_to_string(&a.0[0]).map_err(|e| format!("{}: {e}", a.0[0]))?;
    let src = std::fs::read_to_string(&a.0[1]).map_err(|e| format!("{}: {e}", a.0[1]))?;
    let ex = Exercicio::de_toml(&toml)?;
    let v = ex.verificar(&src)?;
    print!("{}", v.texto());
    Ok(if v.aprovado {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty()
        || args
            .iter()
            .any(|a| a == "--ajuda" || a == "-h" || a == "--help")
    {
        print!("{AJUDA}");
        return ExitCode::SUCCESS;
    }
    let cmd = args.remove(0);
    let r = match cmd.as_str() {
        "explain" => cmd_explain(Args(args)),
        "trace" => cmd_trace(Args(args)),
        "check" => cmd_check(Args(args)),
        o => Err(format!("comando desconhecido: '{o}' (veja --ajuda)")),
    };
    match r {
        Ok(c) => c,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::from(2)
        }
    }
}
