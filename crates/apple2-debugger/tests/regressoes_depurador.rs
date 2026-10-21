//! HU-0801 / issue #34: regressões das issues do depurador do AppleWin (wave 1, seção
//! "Depurador"). Os crashes do AppleWin viram "não dá panic e devolve erro descritivo".
//!
//! Cada teste leva o número da issue upstream (`gh<N>`); o resultado por issue fica em
//! `docs/conformance/results/depurador.csv`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Write};
use std::rc::Rc;

use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::console::line_string;
use apple2_debugger::host::{Host, NoHost};
use apple2_debugger::{Debugger, RunMode};

pub type Files = Rc<RefCell<HashMap<String, Vec<u8>>>>;

/// Arquivos em memória que o teste ainda lê depois que o depurador pegou o host.
pub struct SharedHost(pub Files);

struct Sink(Files, String);

impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0
            .borrow_mut()
            .entry(self.1.clone())
            .or_default()
            .extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Host for SharedHost {
    fn read(&mut self, name: &str) -> io::Result<Vec<u8>> {
        self.0
            .borrow()
            .get(name)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, name.to_string()))
    }
    fn write(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        self.0.borrow_mut().insert(name.into(), data.to_vec());
        Ok(())
    }
    fn create(&mut self, name: &str) -> io::Result<Box<dyn Write>> {
        self.0.borrow_mut().insert(name.into(), Vec::new());
        Ok(Box::new(Sink(self.0.clone(), name.into())))
    }
    fn dir(&self) -> String {
        "/d".into()
    }
    fn set_dir(&mut self, _: &str) -> bool {
        true
    }
}

fn machine() -> Machine {
    Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced))
}

fn setup() -> (Machine, Debugger, Files) {
    let mut m = machine();
    let files = Files::default();
    let mut d = Debugger::new();
    d.set_host(Box::new(SharedHost(files.clone())));
    d.initialize(&mut m);
    d.enter(&mut m);
    d.console.clear();
    (m, d, files)
}

fn run(d: &mut Debugger, m: &mut Machine, line: &str) -> String {
    d.console.clear();
    d.execute(m, line);
    text(d)
}

fn text(d: &Debugger) -> String {
    d.console
        .lines()
        .map(line_string)
        .collect::<Vec<_>>()
        .join("\n")
}

// ------------------------------------------------------------------ #64 SOURCE

/// gh64: `source asdf.txt` com o arquivo inexistente derrubava o AppleWin. Aqui: erro descritivo.
#[test]
fn gh64_source_of_a_missing_file_reports_an_error() {
    let (mut m, mut d, _) = setup();
    let t = run(&mut d, &mut m, "source asdf.txt");
    assert!(t.contains("Error reading"), "{t}");
    assert!(t.contains("asdf.txt"), "{t}");
    // sem host nenhum também
    let mut d2 = Debugger::new();
    d2.set_host(Box::new(NoHost));
    d2.enter(&mut m);
    let t = run(&mut d2, &mut m, "source asdf.txt");
    assert!(t.contains("Error reading"), "{t}");
    // nome gigantesco e Unicode
    let long = format!("source {}", "ñ".repeat(500));
    run(&mut d, &mut m, &long);
    run(&mut d, &mut m, "source 日本語.txt SYM MEM");
}

/// gh64: com o arquivo existente o comando lê o listing sem crash.
#[test]
fn gh64_source_of_an_existing_listing_reads_lines() {
    let (mut m, mut d, files) = setup();
    files.borrow_mut().insert(
        "asdf.txt".into(),
        b"0300:A9 01  LDA #$01\n0302:60     RTS\nfoo = $1234\n".to_vec(),
    );
    let t = run(&mut d, &mut m, "source asdf.txt");
    assert!(t.contains("Read:"), "{t}");
    assert!(!t.contains("Error"), "{t}");
}

// ------------------------------------------------------------------ #109 / #111 RUN

/// gh109: `RUN` sem argumento ou com arquivo inexistente derrubava o AppleWin.
#[test]
fn gh109_run_without_argument_or_missing_script_reports_an_error() {
    let (mut m, mut d, _) = setup();
    let t = run(&mut d, &mut m, "run");
    assert!(t.contains("RUN, Run script file"), "{t}");
    let t = run(&mut d, &mut m, "run missing.txt");
    assert!(t.contains("Couldn't load script"), "{t}");
    assert!(t.contains("missing.txt"), "{t}");
    let t = run(&mut d, &mut m, "run a b c");
    assert!(t.contains("RUN, Run script file"), "{t}");
    run(&mut d, &mut m, &format!("run {}", "x".repeat(10_000)));
}

/// gh109: o script no diretório corrente executa de verdade.
#[test]
fn gh109_run_executes_the_script_in_the_current_directory() {
    let (mut m, mut d, files) = setup();
    files
        .borrow_mut()
        .insert("name.txt".into(), b"MEB 300 AA BB\nR A 12\n".to_vec());
    let t = run(&mut d, &mut m, "run name.txt");
    assert!(t.contains("Run script"), "{t}");
    assert_eq!(m.peek(0x300), 0xAA);
    assert_eq!(m.peek(0x301), 0xBB);
    assert_eq!(m.cpu().a, 0x12);
}

/// gh111: o script com várias linhas, linhas em branco, CRLF e uma linha ruim no meio continua
/// até o fim; e um script que chama a si mesmo termina em vez de recursar para sempre.
#[test]
fn gh111_run_script_with_many_lines_crlf_bad_line_and_self_recursion() {
    let (mut m, mut d, files) = setup();
    files.borrow_mut().insert(
        "s.txt".into(),
        b"MEB 400 01\r\n\r\nNOSUCHCOMMAND 1 2\r\nMEB 401 02\r\nMEB 402 03".to_vec(),
    );
    run(&mut d, &mut m, "run s.txt");
    assert_eq!(
        [m.peek(0x400), m.peek(0x401), m.peek(0x402)],
        [1, 2, 3],
        "a linha ruim não interrompe o script"
    );
    files
        .borrow_mut()
        .insert("loop.txt".into(), b"MEB 410 07\nrun loop.txt\n".to_vec());
    let t = run(&mut d, &mut m, "run loop.txt");
    assert!(t.contains("nesting too deep"), "{t}");
    assert_eq!(m.peek(0x410), 7);
    // linha só com espaços e arquivo vazio
    files.borrow_mut().insert("e.txt".into(), Vec::new());
    run(&mut d, &mut m, "run e.txt");
    // bytes que não são UTF-8
    files
        .borrow_mut()
        .insert("b.txt".into(), vec![0xFF, 0xFE, b'\n', 0x80, 0x81]);
    run(&mut d, &mut m, "run b.txt");
}

// ------------------------------------------------------------------ #226 SAVE

/// gh226: argumentos ruins no SAVE estouravam o buffer em ConfigSave_PrepareHeader(). Aqui o
/// nome é só uma string: tudo vira "Saved:" ou erro descritivo, sem panic.
#[test]
fn gh226_save_with_bad_arguments_does_not_crash() {
    let (mut m, mut d, files) = setup();
    for line in [
        "SAVE".to_string(),
        "SAVE D000:FFFF \"OVERFLOW\"".to_string(),
        "SAVE \"\"".to_string(),
        "SAVE ((( ,,, ###".to_string(),
        "SAVE 1 2 3 4 5 6 7 8 9".to_string(),
        format!("SAVE {}", "A".repeat(100_000)),
        format!("SAVE \"{}\"", "é".repeat(5_000)),
        "SAVE \u{0} \u{202e} 😀".to_string(),
    ] {
        let t = run(&mut d, &mut m, &line);
        assert!(t.contains("Saved:") || t.contains("Error"), "{line:.40}: {t}");
    }
    assert!(files.borrow().contains_key("Debugger.cfg"));
    // host que recusa gravação: erro descritivo, não panic
    let mut d2 = Debugger::new();
    d2.set_host(Box::new(NoHost));
    d2.enter(&mut m);
    let t = run(&mut d2, &mut m, "SAVE x");
    assert!(t.contains("Error saving config"), "{t}");
    // LOAD simétrico
    let t = run(&mut d, &mut m, "LOAD nada.cfg");
    assert!(t.contains("Couldn't load config"), "{t}");
}

// ------------------------------------------------------------------ #349 NTSC

/// gh349: `NTSC LOAD` / `NTSC SAVE` sem nome de arquivo travavam. Usam o nome padrão e
/// reportam o resultado; sem arquivo, erro descritivo.
#[test]
fn gh349_ntsc_load_and_save_without_filename_are_handled() {
    let (mut m, mut d, files) = setup();
    let t = run(&mut d, &mut m, "NTSC LOAD");
    assert!(t.contains("couldn't open file for reading"), "{t}");
    let t = run(&mut d, &mut m, "NTSC SAVE");
    assert!(t.contains("Saved:"), "{t}");
    assert!(files.borrow().contains_key("AppleWinNTSC4096x4@32.data"));
    // agora o arquivo existe e o LOAD sem nome o valida
    let t = run(&mut d, &mut m, "NTSC LOAD");
    assert!(!t.contains("couldn't open"), "{t}");
    for line in ["NTSC", "NTSC FOO", "NTSC RESET", "NTSC SAVE .bmp", "NTSC LOAD 😀", "NTSC SAVE 😀.bmp"] {
        run(&mut d, &mut m, line);
    }
    // sem host: erro de escrita descritivo
    let mut d2 = Debugger::new();
    d2.set_host(Box::new(NoHost));
    d2.enter(&mut m);
    let t = run(&mut d2, &mut m, "NTSC SAVE");
    assert!(t.contains("couldn't open file for writing"), "{t}");
}

// ------------------------------------------------------------------ #622 logo screen

/// gh622: entrar no depurador na tela do logo (PC em $FFFF/$FFFE, operando de 16 bits cruzando
/// $FFFF/$0000) crashava o desmontador. Aqui o desmontador e a tela do depurador lidam com isso.
#[test]
fn gh622_disassembly_with_operand_across_ffff_does_not_panic() {
    let mut m = machine();
    let mut d = Debugger::new();
    d.initialize(&mut m);
    for (pc, op) in [
        (0xFFFFu16, 0xAD), // LDA abs: operando em $0000/$0001
        (0xFFFE, 0xAD),    // operando em $FFFF/$0000
        (0xFFFD, 0x6C),    // JMP (abs)
        (0xFFFF, 0x7C),    // JMP (abs,X)
        (0xFFFF, 0x20),    // JSR
        (0xFFFF, 0x10),    // BPL relativo
        (0xFFFF, 0xB1),    // LDA (zp),Y
    ] {
        m.cpu_mut().pc = pc;
        m.poke(pc, op);
        d.enter(&mut m);
        let line = d.disasm_line(&m, pc);
        assert!(!line.mnemonic.is_empty());
        let _ = d.render(&m);
        d.goto(&m, pc);
        let _ = d.render(&m);
        d.execute(&mut m, &format!("U {pc:X}"));
        d.execute(&mut m, "T");
        d.mode = RunMode::Debug;
        d.exit(&mut m);
    }
}

/// gh622: máquina recém-criada (nenhum ciclo rodou, tela do logo), F7 e navegação.
#[test]
fn gh622_entering_the_debugger_on_a_fresh_machine_renders() {
    for model in [Model::Apple2, Model::Apple2Plus, Model::Apple2e, Model::Apple2eEnhanced] {
        let mut m = Machine::new(MachineConfig::for_model(model));
        let mut d = Debugger::new();
        d.initialize(&mut m);
        d.enter(&mut m);
        let g = d.render(&m);
        let _ = g;
        for k in ["U", "U FFFF", "U FFFE", "M FFFF", "R", "ZP", "T 3", "P"] {
            d.execute(&mut m, k);
            let _ = d.render(&m);
        }
        d.exit(&mut m);
        m.run_cycles(20_000);
        d.enter(&mut m);
        let _ = d.render(&m);
    }
}

// ------------------------------------------------------------------ #1204 linha longa

/// gh1204: comando com 79+ colunas e backspace corrompia o console. A linha é limitada à
/// largura do console, o backspace remove um caractere por vez e Enter executa sem panic.
#[test]
fn gh1204_long_command_line_edit_is_bounded() {
    let (mut m, mut d, _) = setup();
    let width = apple2_debugger::console::CONSOLE_WIDTH;
    for i in 0..(width * 3) {
        d.console.input_char(if i % 2 == 0 { 'A' } else { 'é' });
    }
    assert!(d.console.input().chars().count() <= width - 2);
    let before = d.console.input().chars().count();
    assert!(d.console.backspace());
    assert_eq!(d.console.input().chars().count(), before - 1);
    d.console.input_char('Z');
    assert_eq!(d.console.input().chars().count(), before);
    let g = d.render(&m);
    let _ = g;
    d.input_enter(&mut m);
    assert_eq!(d.console.input(), "");
    // backspace com linha vazia não faz nada
    for _ in 0..10 {
        d.console.backspace();
    }
    // linha gigante direto no execute e via set_input
    d.execute(&mut m, &"1".repeat(1_000_000));
    d.console.set_input(&"X".repeat(5_000));
    assert!(d.console.input().chars().count() <= width - 2);
    d.input_enter(&mut m);
    let _ = d.render(&m);
}
