//! As soluções de referência dos exercícios passam; soluções erradas reprovam.

use std::fs;
use std::path::PathBuf;

use apple2_lab::bare::Parada;
use apple2_lab::exercise::Exercicio;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("exercicios")
}

fn carrega(nome: &str) -> (Exercicio, String) {
    let toml = fs::read_to_string(dir().join(format!("{nome}.toml"))).unwrap();
    let asm = fs::read_to_string(dir().join(format!("{nome}.asm"))).unwrap();
    (Exercicio::de_toml(&toml).unwrap(), asm)
}

#[test]
fn todas_as_solucoes_de_referencia_passam() {
    let mut n = 0;
    for e in fs::read_dir(dir()).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "toml") {
            let nome = p.file_stem().unwrap().to_str().unwrap().to_string();
            let (ex, asm) = carrega(&nome);
            let v = ex.verificar(&asm).unwrap_or_else(|e| panic!("{nome}: {e}"));
            assert!(v.aprovado, "{nome}:\n{}", v.texto());
            n += 1;
        }
    }
    assert!(n >= 5, "esperava pelo menos 5 exercícios, achei {n}");
}

#[test]
fn solucao_errada_reprova_com_mensagem() {
    let (ex, _) = carrega("01-somar");
    // Esqueceu o CLC e usa SBC em vez de ADC.
    let v = ex
        .verificar("ORG $0800\nLDA $0300\nSEC\nSBC $0301\nSTA $0302\nBRK\n")
        .unwrap();
    assert!(!v.aprovado);
    assert!(
        v.casos[0].falhas[0].contains("$0302"),
        "{:?}",
        v.casos[0].falhas
    );
}

#[test]
fn laco_infinito_estoura_o_limite() {
    let (ex, _) = carrega("02-laco-contador");
    let v = ex.verificar("ORG $0800\nINX\nJMP $0800\n").unwrap();
    assert!(!v.aprovado);
    assert_eq!(v.casos[0].parada, Parada::Limite);
}

#[test]
fn limite_de_ciclos_e_cobrado() {
    let (mut ex, asm) = carrega("06-maior-de-dois");
    ex.max_ciclos = Some(5);
    let v = ex.verificar(&asm).unwrap();
    assert!(!v.aprovado);
    assert!(v.casos[0].falhas[0].contains("ciclos"));
}

#[test]
fn subrotina_desequilibrada_e_detectada_pelo_sp() {
    let (ex, _) = carrega("04-subrotina");
    // Um PHA a mais deixa SP diferente de $FD.
    let v = ex
        .verificar("ORG $0800\nLDA $0300\nPHA\nJSR $0900\nJSR $0900\nJSR $0900\nSTA $0301\nBRK\n")
        .unwrap();
    assert!(!v.aprovado);
}

#[test]
fn erros_de_montagem_trazem_numero_da_linha() {
    let (ex, _) = carrega("01-somar");
    let e = ex.verificar("ORG $0800\nLDA #1\nXYZ #2\n").unwrap_err();
    assert!(e.contains("linha 3"), "{e}");
}

#[test]
fn toml_invalido_e_rejeitado() {
    assert!(Exercicio::de_toml("titulo = \"x\"\nenunciado = \"y\"\n").is_err()); // sem casos
    assert!(Exercicio::de_toml("campo_novo = 1\n").is_err());
}

#[test]
fn nao_usa_rom_da_apple() {
    // Sem ROM: o vetor RESET vem do exercício e a área da ROM só tem os vetores.
    let m = apple2_lab::bare::bare_machine(apple2_lab::bare::BareConfig::new(false, 0x0800));
    assert_eq!(m.peek(0xFFFC), 0x00);
    assert_eq!(m.peek(0xFFFD), 0x08);
    assert_eq!(m.peek(0xD000), 0x00);
    assert_eq!(m.peek(0xFA00), 0x00);
}
