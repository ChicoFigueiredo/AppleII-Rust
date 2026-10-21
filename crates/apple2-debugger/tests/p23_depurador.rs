//! P23 (gate G0): issues upstream de depurador que estavam UNVERIFIED. Um teste por issue, `gh<N>_...`;
//! o resultado fica em `docs/conformance/results/p23-depurador.csv`. Testes `#[ignore = "FAIL ..."]`
//! reproduzem o pedido da issue, que o oráculo (AppleWin a8f0d1d) também não atende; rodam com
//! `--ignored` e falham de propósito.

mod common;

use apple2_core::Machine;
use apple2_debugger::debugger::OutputView;
use apple2_debugger::{Debugger, RunMode};
use common::*;

fn flags(m: &Machine) -> u8 {
    m.cpu().p
}

// ------------------------------------------------------------------ #81 reboot

/// gh81: depois de um reboot (reset) os breakpoints seguem valendo e o depurador volta a parar
/// neles (WinFrame.cpp:2067-2069 entra no depurador quando há breakpoints).
#[test]
fn gh81_breakpoints_survive_a_reset_and_stop_the_machine() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0xEA; 8]);
    cmd(&mut d, &mut m, "bpx 302");
    m.reset();
    assert_eq!(m.cpu().pc, 0x0300);
    cmd(&mut d, &mut m, "g");
    run_until_stopped(&mut d, &mut m);
    assert_eq!(m.cpu().pc, 0x0302);
    assert_eq!(d.mode, RunMode::Debug);
    let t = cmd(&mut d, &mut m, "bpl");
    assert!(t.contains("0302"), "{t}");
}

// ------------------------------------------------------------------ #83 MAX_SYMBOLS_LEN

/// gh83: o limite do nome de símbolo é `MAX_SYMBOLS_LEN` = 51 (Debugger_Types.h:135; o pedido era
/// 30), aceito até 50 caracteres (`nSymLen < MAX_SYMBOLS_LEN`, Debugger_Symbols.cpp:898).
#[test]
fn gh83_symbol_names_up_to_50_chars_are_accepted() {
    let (mut m, mut d, _) = setup();
    let n30 = "A".repeat(30);
    let n50 = "B".repeat(50);
    let n51 = "C".repeat(51);
    cmd(&mut d, &mut m, &format!("sym {n30} = 300"));
    cmd(&mut d, &mut m, &format!("sym {n50} = 310"));
    let t = cmd(&mut d, &mut m, &format!("sym {n51} = 320"));
    assert_eq!(d.symbols.find_address(&n30).map(|x| x.0), Some(0x300));
    assert_eq!(d.symbols.find_address(&n50).map(|x| x.0), Some(0x310));
    assert!(d.symbols.find_address(&n51).is_none());
    assert!(t.contains("Symbol length"), "{t}");
}

// ------------------------------------------------------------------ #91 relocatable symbols

/// gh91: `SYMSRC LOAD "file",offset` soma o deslocamento a cada endereço (Debugger_Symbols.cpp:840-868,
/// 'nAddress += nSymbolOffset' em :621).
#[test]
fn gh91_symsrc_load_applies_the_symbol_offset() {
    let (mut m, mut d, files) = setup();
    files
        .borrow_mut()
        .insert("rel.sym".into(), b"0010 FOO\n0020 BAR\n".to_vec());
    cmd(&mut d, &mut m, "symsrc load \"rel.sym\",300");
    assert_eq!(d.symbols.find_address("FOO").map(|x| x.0), Some(0x310));
    assert_eq!(d.symbols.find_address("BAR").map(|x| x.0), Some(0x320));
    // sem deslocamento, os endereços ficam como no arquivo
    cmd(&mut d, &mut m, "symsrc clear");
    cmd(&mut d, &mut m, "symsrc load \"rel.sym\"");
    assert_eq!(d.symbols.find_address("FOO").map(|x| x.0), Some(0x10));
}

// ------------------------------------------------------------------ #239 / #458 Ctrl+PrintScreen

/// gh239: o texto da tela do depurador (Ctrl+PrintScreen, `Util_GetDebuggerText`) sai com 43
/// linhas de 80 colunas terminadas em LF, com o disassembly e os registradores.
#[test]
fn gh239_debugger_screen_text_has_the_disassembly_and_registers() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0xA9, 0x42, 0xEA]);
    d.goto(&m, 0x0300);
    let t = d.screen_text(&m);
    let lines: Vec<&str> = t.split_inclusive('\n').collect();
    assert_eq!(lines.len(), 43);
    assert!(lines.iter().all(|l| l.len() == 81 && l.ends_with('\n')));
    assert!(t.contains("0300:A9 42"), "{t}");
    assert!(t.contains("LDA #$42"), "{t}");
    assert!(t.contains("PC    0300"), "{t}");
    assert!(t.is_ascii());
}

/// gh458: como no oráculo (DEBUG_VIRTUAL_TEXT_HEIGHT = 43, Debugger_Display.h:64), as 5 últimas
/// linhas da tela (o console) não entram no texto copiado.
#[test]
fn gh458_ctrl_prtscn_text_drops_the_last_console_rows() {
    let (mut m, mut d, _) = setup();
    for i in 0..8 {
        cmd(&mut d, &mut m, &format!("echo LINE{i}"));
    }
    let grid = d.render(&m);
    let console_rows: String = (43..48)
        .map(|y| grid.row_text(y))
        .collect::<Vec<_>>()
        .join("|");
    assert!(console_rows.contains("LINE7"), "{console_rows}");
    let t = d.screen_text(&m);
    assert!(!t.contains("LINE7"), "o oráculo corta o console: {t}");
}

/// gh458: o pedido é capturar a tela inteira (48 linhas); o oráculo perde as 5 últimas.
#[test]
#[ignore = "FAIL gh458: Ctrl+PrintScreen copia só 43 das 48 linhas, igual ao oráculo (DEBUG_VIRTUAL_TEXT_HEIGHT)"]
fn gh458_ctrl_prtscn_text_has_the_whole_screen() {
    let (m, mut d, _) = setup();
    let t = d.screen_text(&m);
    assert_eq!(t.lines().count(), 48);
}

// ------------------------------------------------------------------ #246 E <symbol> <byte>

/// gh246: `e LIFE 64` do exemplo do CHM não é "Ambiguous 2 Commands": o único comando que começa
/// com E é ECHO (EXITBENCH tem função NULL, Debugger_Commands.cpp:400, e não entra em
/// FindCommand, Debug.cpp:8128-8160); o texto é ecoado e nada é gravado.
#[test]
fn gh246_e_symbol_value_is_not_ambiguous() {
    let (mut m, mut d, _) = setup();
    cmd(&mut d, &mut m, "sym LIFE = 300");
    m.poke(0x300, 0);
    let t = cmd(&mut d, &mut m, "e LIFE 64");
    assert!(!t.contains("Ambiguous"), "{t}");
    assert!(t.contains("LIFE 64"), "{t}");
    assert_eq!(m.peek(0x300), 0, "E é ECHO: não grava");
    let t = cmd(&mut d, &mut m, "meb LIFE 64");
    assert!(!t.contains("Ambiguous"), "{t}");
    assert_eq!(m.peek(0x300), 0x64);
}

// ------------------------------------------------------------------ #327 HGR0..HGR8

/// gh327: HGR0 a HGR8 existem (Debugger_Commands.cpp:267-276): 1 e 2 são as páginas reais, as outras
/// são páginas pseudo (VIEW_PAGE_0/3-8, Debug.cpp:7273-7310).
#[test]
fn gh327_hgr_pages_0_to_8_are_view_commands() {
    let (mut m, mut d, _) = setup();
    for n in 0..=8u8 {
        cmd(&mut d, &mut m, &format!("hgr{n}"));
        let want = if n == 1 || n == 2 {
            OutputView::Hgr(Some(n))
        } else {
            OutputView::HgrPseudo(n)
        };
        assert_eq!(d.output_view, Some(want), "HGR{n}");
        d.output_view = None;
    }
    cmd(&mut d, &mut m, "hgr");
    assert_eq!(d.output_view, Some(OutputView::Hgr(None)));
}

// ------------------------------------------------------------------ #449 DB com ou sem '='

/// gh449: `DB nome = faixa` e `DB nome faixa` definem o mesmo dado (Debugger_DisassemblerData.cpp:
/// 94-103, "2.9.1.1 Add: Support for equal sign, also make it optional for command DB"); nenhum
/// mostra o help.
#[test]
fn gh449_db_equal_sign_is_optional() {
    let (mut m, mut d, _) = setup();
    for line in ["db TABLE = b64d:b65c", "db TABLE b64d:b65c"] {
        let t = cmd(&mut d, &mut m, line);
        assert!(!t.contains("Usage"), "{line}: {t}");
        assert_eq!(
            d.symbols.find_address("TABLE").map(|x| x.0),
            Some(0xB64D),
            "{line}"
        );
    }
    let t = cmd(&mut d, &mut m, "db HGR = 2000:3fff");
    assert!(!t.contains("Usage"), "{t}");
    assert_eq!(d.symbols.find_address("HGR").map(|x| x.0), Some(0x2000));
    // DW: com '=' e um endereço só vale; com '=' e faixa o oráculo mostra o help (`nArgs > 4`,
    // Debugger_DisassemblerData.cpp:~455), e o port também.
    let t = cmd(&mut d, &mut m, "dw FOO = 300");
    assert!(!t.contains("Usage"), "{t}");
    assert_eq!(d.symbols.find_address("FOO").map(|x| x.0), Some(0x300));
    let t = cmd(&mut d, &mut m, "dw BAR = 310:311");
    assert!(t.contains("Usage"), "{t}");
}

// ------------------------------------------------------------------ #481 ECHO

/// gh481: ECHO mostra mais de 55 caracteres (a linha do console tem CONSOLE_WIDTH = 80,
/// Debugger_Console.h:14).
#[test]
fn gh481_echo_shows_more_than_55_chars() {
    let (mut m, mut d, _) = setup();
    let s = "0123456789".repeat(7);
    let t = cmd(&mut d, &mut m, &format!("echo {s}"));
    assert!(t.contains(&s), "{t}");
    let t = cmd(&mut d, &mut m, &format!("echo \"{s}\""));
    assert!(t.contains(&s), "{t}");
}

// ------------------------------------------------------------------ #844 CL / SE

/// gh844: o oráculo a8f0d1d ainda tem `CL` e `SE` (Debugger_Commands.cpp:160, 172) com várias
/// letras de flag (Debug.cpp:3840-3896), além de CLC/SEC e RC/SC; o port tem todos.
#[test]
fn gh844_cl_and_se_flag_commands_exist() {
    let (mut m, mut d, _) = setup();
    m.cpu_mut().p = 0x20;
    cmd(&mut d, &mut m, "se n v c");
    assert_eq!(flags(&m) & 0xC1, 0xC1);
    cmd(&mut d, &mut m, "cl n c");
    assert_eq!(flags(&m) & 0xC1, 0x40);
    cmd(&mut d, &mut m, "sec");
    assert_eq!(flags(&m) & 1, 1);
    cmd(&mut d, &mut m, "clc");
    assert_eq!(flags(&m) & 1, 0);
    cmd(&mut d, &mut m, "sz");
    assert_eq!(flags(&m) & 2, 2);
    cmd(&mut d, &mut m, "rz");
    assert_eq!(flags(&m) & 2, 0);
}

// ------------------------------------------------------------------ #855 ordem de inicialização

/// gh855: o DebuggerAutoRun.txt roda com a memória pronta: `bsave` grava e `bpx` vale. (O oráculo
/// chama DebugInitialize antes de MemInitialize, AppleWin.cpp:921-924, e ainda tem o defeito; o
/// port inicializa o depurador com a máquina já criada.)
#[test]
fn gh855_autorun_script_can_bsave_and_set_breakpoints() {
    let mut m = nop_machine();
    let files = Files::default();
    files.borrow_mut().insert(
        "DebuggerAutoRun.txt".into(),
        b"bsave \"debug.0000.bin\",0:FF\nbpx 2000\n".to_vec(),
    );
    m.poke(0x10, 0x5A);
    let mut d = Debugger::new();
    d.set_host(Box::new(SharedHost(files.clone())));
    d.initialize(&mut m);
    let saved = files.borrow().get("debug.0000.bin").cloned();
    let saved = saved.expect("bsave do script gravou o arquivo");
    assert_eq!(saved.len(), 0x100);
    assert_eq!(saved[0x10], 0x5A);
    d.enter(&mut m);
    let t = cmd(&mut d, &mut m, "bpl");
    assert!(t.contains("2000"), "{t}");
}

// ------------------------------------------------------------------ #475 rolagem para cima

/// gh475: a rolagem para cima do disassembly "pula" porque o cursor tenta ficar no meio da janela
/// e o fluxo de opcodes sai de sincronia (o exemplo `U F93D`, Up, Up da discussão do #475, na ROM
/// do Apple II+). É o desenho do oráculo: o cursor vai à primeira linha no segundo Up.
#[test]
fn gh475_scrolling_up_from_f93d_matches_the_documented_sequence() {
    use apple2_core::{MachineConfig, Model};
    let (mut m, mut d, _) = setup_with(Machine::new(MachineConfig::for_model(Model::Apple2Plus)));
    cmd(&mut d, &mut m, "u f93d");
    assert_eq!((d.disasm_top, d.disasm_cur), (0xF93D, 0xF961));
    key(&mut d, &mut m, apple2_debugger::Key::Up, NONE);
    assert_eq!((d.disasm_top, d.disasm_cur), (0xF93A, 0xF960));
    key(&mut d, &mut m, apple2_debugger::Key::Up, NONE);
    assert_eq!((d.disasm_top, d.disasm_cur), (0xF95F, 0xF95F));
}
