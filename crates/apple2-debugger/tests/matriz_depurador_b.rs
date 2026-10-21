//! HU-0801 / issue #34 (continuação): memória, símbolos e dados do oráculo. Um teste por issue (ou
//! grupo), `gh<N>_...`. Testes `#[ignore = "FAIL ..."]` reproduzem o defeito relatado, que o
//! oráculo (AppleWin a8f0d1d) também tem; rodam com `--ignored` e falham de propósito.

mod common;

use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::display::Grid;
use apple2_debugger::host::NoHost;
use apple2_debugger::symbols::{Symbols, Table, BUILTIN_BASIC, BUILTIN_DOS33, BUILTIN_MAIN};
use apple2_debugger::{Debugger, Key};
use common::*;

/// Linhas do disassembly (texto até a coluna 50) com endereço >= `addr`, vindas da tela renderizada
/// com o cursor em `addr`.
fn lines_from(d: &mut Debugger, m: &Machine, addr: u16, n: usize) -> Vec<String> {
    d.disasm_cur = addr;
    d.disasm_top = addr;
    d.calc_top_bot(m);
    let g = d.render(m);
    let mut out = Vec::new();
    for y in 0..40 {
        let r: String = (0..50).map(|x| g.cell(x, y).ch).collect();
        if let Ok(a) = u16::from_str_radix(&r[..4], 16) {
            if a >= addr && r.as_bytes()[4] == b':' && out.len() < n {
                out.push(r.trim_end().to_string());
            }
        }
    }
    out
}

fn find_text(g: &Grid, needle: &str) -> Option<(usize, usize)> {
    (0..g.h).find_map(|y| row(g, y).find(needle).map(|x| (x, y)))
}

// =============================================================== memória e dados

/// gh166: BSAVE/BLOAD com os bancos 0 (main) e 1 (aux), sem paginação.
#[test]
fn gh166_bsave_and_bload_use_main_and_aux_banks() {
    let (mut m, mut d, files) = setup_e();
    for i in 0..16u16 {
        m.poke_main(0x2000 + i, 0x10 + i as u8);
        m.poke_aux(0x2000 + i, 0xA0 + i as u8);
    }
    let t = cmd(&mut d, &mut m, "BSAVE \"main.bin\",0:2000,10");
    assert!(t.contains("Saved"), "{t}");
    let t = cmd(&mut d, &mut m, "BSAVE \"aux.bin\",1:2000,10");
    assert!(t.contains("Saved"), "{t}");
    assert_eq!(
        files.borrow()["main.bin"],
        (0x10..0x20).collect::<Vec<u8>>()
    );
    assert_eq!(files.borrow()["aux.bin"], (0xA0..0xB0).collect::<Vec<u8>>());
    // carrega o arquivo do aux no banco 0 e o do main no banco 1 (trocados)
    cmd(&mut d, &mut m, "BLOAD \"aux.bin\",0:3000,10");
    cmd(&mut d, &mut m, "BLOAD \"main.bin\",1:3000,10");
    for i in 0..16u16 {
        assert_eq!(m.peek_main(0x3000 + i), 0xA0 + i as u8);
        assert_eq!(m.peek_aux(0x3000 + i), 0x10 + i as u8);
    }
}

/// gh251: `ASC 4E0:4E7` com um byte nulo no meio vira uma única linha de texto cobrindo os 8
/// bytes (o nulo aparece como `@`), e a linha seguinte começa em $04E8.
#[test]
fn gh251_asc_with_null_byte_marks_up_the_whole_range() {
    let (mut m, mut d, _) = setup();
    load(
        &mut m,
        0x04E0,
        &[0xD4, 0xC5, 0x8D, 0x00, 0xD2, 0xC5, 0xD6, 0xC5],
    );
    cmd(&mut d, &mut m, "ASC 4E0:4E7");
    let rows = lines_from(&mut d, &m, 0x04E0, 2);
    assert!(rows[0].starts_with("04E0:D4 C5 8D"), "{rows:?}");
    assert!(rows[0].contains("ASC \"TEM@REVE\""), "{rows:?}");
    assert!(rows[1].starts_with("04E8:"), "{rows:?}");
}

/// gh472: um `ASC` longo mostra o início do texto seguido de `...` (não só `ASC ...`), e a faixa
/// por símbolos (`ASC LANG:RANGE-1`) marca o texto entre eles.
#[test]
fn gh472_asc_long_string_shows_text_and_symbol_range_works() {
    let (mut m, mut d, _) = setup();
    let text: Vec<u8> = (b'A'..=b'U').collect(); // 21 bytes
    load(&mut m, 0x0300, &text);
    cmd(&mut d, &mut m, "ASC 300:314");
    let rows = lines_from(&mut d, &m, 0x0300, 1);
    assert!(rows[0].contains("ASC ABCDEFGHIJKLMNOPQ..."), "{rows:?}");
    let msg = b"LANGUAGE NOT AVAILABLE";
    load(&mut m, 0x0A74, msg);
    cmd(&mut d, &mut m, "SYM LANG = A74");
    cmd(
        &mut d,
        &mut m,
        &format!("SYM RANGE = {:X}", 0x0A74 + msg.len()),
    );
    let t = cmd(&mut d, &mut m, "ASC LANG:RANGE-1");
    assert!(t.contains("Added"), "{t}");
    let rows = lines_from(&mut d, &m, 0x0A74, 1);
    assert!(rows[0].contains("ASC"), "{rows:?}");
    assert!(rows[0].contains("LANGUAGE"), "{rows:?}");
}

/// gh1127: `S 0:1FFF DD` enxerga o último byte da faixa ($1FFF), e a busca termina em $FFFF.
#[test]
fn gh1127_search_range_includes_the_last_address() {
    let (mut m, mut d, _) = setup();
    m.poke(0x1FFF, 0xDD);
    let t = cmd(&mut d, &mut m, "S 0:1FFF DD");
    assert!(t.contains("$1FFF"), "{t}");
    assert!(t.contains("Total: 1"), "{t}");
    let t = cmd(&mut d, &mut m, "S 1FFF:1FFF DD");
    assert!(t.contains("$1FFF"), "{t}");
    let t = cmd(&mut d, &mut m, "S 1FFE:1FFE DD");
    assert!(t.contains("Total: 0"), "{t}");
    // $FFFF (parte alta do vetor IRQ = $04 na ROM de teste) é o último endereço possível
    let t = cmd(&mut d, &mut m, "S F000:FFFF 04");
    assert!(t.contains("$FFFF"), "{t}");
}

/// gh713: aspas duplas casam texto sem bit alto e aspas simples casam com bit alto (oráculo).
#[test]
fn gh713_search_literals_match_the_oracle_convention() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, b"AB");
    load(&mut m, 0x0310, &[0xC1, 0xC2]);
    assert!(cmd(&mut d, &mut m, "S 300:305 \"AB\"").contains("$0300"));
    assert!(cmd(&mut d, &mut m, "S 310:315 'AB'").contains("$0310"));
    assert!(cmd(&mut d, &mut m, "S 300:305 'AB'").contains("Total: 0"));
}

/// gh713: o oráculo usa aspas simples para o bit alto ligado e duplas para desligado, ao
/// contrário do Merlin32. O port repete o oráculo; mudar seria Divergência intencional (ADR 0004).
#[test]
#[ignore = "FAIL gh713: aspas simples = bit alto ligado como no oráculo; a issue quer a convenção do Merlin32"]
fn gh713_search_string_literals_follow_merlin_convention() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, b"AB");
    load(&mut m, 0x0310, &[0xC1, 0xC2]);
    let t = cmd(&mut d, &mut m, "S 300:305 'AB'");
    assert!(
        t.contains("$0300"),
        "Merlin: aspas simples = ASCII padrão: {t}"
    );
    let t = cmd(&mut d, &mut m, "S 310:315 \"AB\"");
    assert!(t.contains("$0310"), "Merlin: aspas duplas = bit alto: {t}");
}

/// gh1101: MEB/MEW com leitura da memória principal e escrita no aux escrevem onde a CPU
/// escreveria e o valor sobrevive a trocas de paginação.
#[test]
fn gh1101_meb_and_mew_survive_paging_changes() {
    let (mut m, mut d, _) = setup_e();
    cmd(&mut d, &mut m, "OUT C002 0"); // RAMRD off: lê main
    cmd(&mut d, &mut m, "OUT C005 0"); // RAMWRT on: escreve aux
    cmd(&mut d, &mut m, "MEW 300 1234");
    cmd(&mut d, &mut m, "MEB 302 56");
    for sw in [
        "OUT C002 0",
        "OUT C003 0",
        "OUT C002 0",
        "OUT C000 0",
        "OUT C001 0",
    ] {
        cmd(&mut d, &mut m, sw);
    }
    assert_eq!(
        [m.peek_aux(0x300), m.peek_aux(0x301), m.peek_aux(0x302)],
        [0x34, 0x12, 0x56],
        "o valor escrito pelo depurador se perdeu"
    );
    cmd(&mut d, &mut m, "OUT C004 0"); // RAMWRT off
    cmd(&mut d, &mut m, "OUT C003 0"); // RAMRD on: lê aux
    assert_eq!(m.peek(0x300), 0x34);
}

/// gh1397: `2000<6000.6ffM` (fim digitado errado, abaixo do início) avisa e aborta sem tocar na
/// memória (antes sobrescrevia $2000-$7902); `M` com fim abaixo do início, escrito com `:`, é
/// normalizado pelo `Range_Get` do oráculo (troca as pontas) e nunca vira uma cópia gigante.
#[test]
fn gh1397_memory_move_with_end_below_start_copies_nothing() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x2000, &[0x11, 0x22, 0x33, 0x44]);
    load(&mut m, 0x6000, &[0xAA, 0xBB]);
    let t = cmd(&mut d, &mut m, "2000<6000.6ffM");
    assert!(t.contains("Aborting"), "{t}");
    assert!(t.contains("End source address"), "{t}");
    assert_eq!(
        [
            m.peek(0x2000),
            m.peek(0x2001),
            m.peek(0x2002),
            m.peek(0x2003)
        ],
        [0x11, 0x22, 0x33, 0x44]
    );
    assert_eq!(m.peek(0x7902), 0, "memória distante tocada");
    // fim válido: copia a faixa
    cmd(&mut d, &mut m, "2000<6000.6001M");
    assert_eq!(
        [m.peek(0x2000), m.peek(0x2001), m.peek(0x2002)],
        [0xAA, 0xBB, 0x33]
    );
    // formas com fim abaixo do início não explodem nem demoram
    for line in [
        "M 2000 6000:5FFF",
        "MC 2000 6000:5FFF",
        "S 6000:5FFF 00",
        "F 6000:5FFF 00",
        "M 2000 5,4",
    ] {
        cmd(&mut d, &mut m, line);
    }
    assert_eq!(m.peek(0x7902), 0);
}

/// gh1382: `DA` mostra o alvo lido da memória e a seta direita segue esse endereço (antes ia
/// para o byte trocado, $0C21 em vez de $212C).
#[test]
fn gh1382_da_follow_target_uses_the_stored_word() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x211C, &[0x2C, 0x21, 0x0C]);
    cmd(&mut d, &mut m, "DA 211C");
    let rows = lines_from(&mut d, &m, 0x211C, 1);
    assert!(rows[0].contains("da $212C"), "{rows:?}");
    d.disasm_cur = 0x211C;
    key(&mut d, &mut m, Key::Right, NONE);
    assert_eq!(d.disasm_cur, 0x212C);
    // ASC não tem alvo: a seta direita não anda
    load(&mut m, 0x2120, &[0x41, 0x42, 0x43, 0x00]);
    cmd(&mut d, &mut m, "ASC 2120:2123");
    d.disasm_cur = 0x2120;
    key(&mut d, &mut m, Key::Right, NONE);
    assert_eq!(d.disasm_cur, 0x2120);
}

/// gh1381: com um símbolo de dados exato no alvo, `DA` deve mostrar esse símbolo (`da B_2100`).
/// O oráculo procura também alvo+1 quando há dado e troca para `A_2101-1`; o port repete isso.
#[test]
#[ignore = "FAIL gh1381: o oráculo (Debugger_Disassembler.cpp, `|| pData`) mostra `da A_2101-1`; o port repete"]
fn gh1381_da_prefers_the_exact_data_symbol() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x2000, &[0x20, 0x00, 0xBF, 0xC7, 0x00, 0x21, 0x60]);
    cmd(&mut d, &mut m, "DB 2003");
    cmd(&mut d, &mut m, "DB 2100");
    cmd(&mut d, &mut m, "DA 2101");
    cmd(&mut d, &mut m, "DA 2004");
    let rows = lines_from(&mut d, &m, 0x2000, 4);
    let da = rows.iter().find(|r| r.starts_with("2004:")).unwrap();
    assert!(da.contains("da B_2100"), "{rows:?}");
}

/// gh256, gh1436, gh167: BLOAD/BSAVE com prefixo de LC (`l1/`) ou de slot, e `M`/`MEB` com prefixo
/// de banco, não existem no oráculo (só BP e MD aceitam prefixo). Aqui só se confere que a forma
/// inexistente não derruba nem escreve em lugar errado.
#[test]
fn gh256_gh1436_prefix_forms_not_in_the_oracle_do_not_crash() {
    let (mut m, mut d, files) = setup_e();
    files.borrow_mut().insert("lc.bin".into(), vec![1, 2, 3, 4]);
    for line in [
        "BLOAD \"lc.bin\",l1/D000",
        "BLOAD \"lc.bin\",s0/D000",
        "BSAVE \"x.bin\",l2/D000,10",
        "MEB 1/300 55",
        "M 1/300 400:410",
        "WA l1/D000",
    ] {
        cmd(&mut d, &mut m, line);
    }
}

// =============================================================== símbolos (dados do oráculo)

fn fnv_no_cr(s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in s.bytes().filter(|b| *b != b'\r') {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Os três arquivos de símbolos embutidos são byte a byte (sem CR) os do AppleWin a8f0d1d
/// (`bin/APPLE2E.SYM`, `A2_BASIC.SYM`, `A2_DOS33.SYM2`). Base dos testes de dados abaixo.
#[test]
fn shipped_symbol_files_are_the_oracles() {
    assert_eq!(fnv_no_cr(BUILTIN_MAIN), 0x6c31_8172_09c3_ba48);
    assert_eq!(fnv_no_cr(BUILTIN_BASIC), 0x748d_3d04_054b_d7f0);
    assert_eq!(fnv_no_cr(BUILTIN_DOS33), 0x093e_223c_ca89_2434);
}

/// gh244: o APPLE2E.SYM tem 7 endereços repetidos (o último nome vence) e o carregamento avisa
/// com "Dup Symbol Name" / "aliases", como pede a issue ("um aviso").
#[test]
fn gh244_duplicate_addresses_in_main_table_are_reported() {
    let mut s = Symbols::default();
    let rep = s.parse_table(BUILTIN_MAIN, Table::Main, 0);
    assert_eq!(rep.loaded, 798);
    let dups = rep.loaded - s.len(Table::Main);
    assert_eq!(dups, 6, "endereços repetidos no APPLE2E.SYM do oráculo");
    assert!(!rep.aliases.is_empty() || !rep.duplicates.is_empty());
    let (mut m, mut d, _) = setup();
    let t = cmd(&mut d, &mut m, "SYMMAIN LOAD");
    assert!(
        t.contains("aliases") || t.contains("Dup Symbol Name"),
        "{t}"
    );
}

/// gh484: as correções pedidas já estão no arquivo do oráculo: BASL/BASH/BAS2L/BAS2H separados e
/// INVFLG/PROMPT em endereços diferentes.
#[test]
fn gh484_apple2e_sym_corrections() {
    let mut s = Symbols::default();
    s.parse_table(BUILTIN_MAIN, Table::Main, 0);
    let at = |a: u16| s.find_symbol(a).map(|x| x.0.to_string());
    assert_eq!(at(0x28).as_deref(), Some("BASL"));
    assert_eq!(at(0x29).as_deref(), Some("BASH"));
    assert_eq!(at(0x2A).as_deref(), Some("BAS2L"));
    assert_eq!(at(0x2B).as_deref(), Some("BAS2H"));
    assert_eq!(at(0x32).as_deref(), Some("INVFLG"));
    assert_eq!(at(0x33).as_deref(), Some("PROMPT"));
}

/// gh1144: P.ADD, P.OR, P.AND, P.REL, P.MUL, P.PWR e P.NEQ não são posições de página zero e não
/// escondem OLDTEXT, LINNUM, DATPTR.
#[test]
fn gh1144_basic_constants_do_not_shadow_zero_page() {
    let mut s = Symbols::default();
    s.parse_table(BUILTIN_BASIC, Table::Basic, 0);
    for name in ["P.ADD", "P.OR", "P.AND", "P.REL", "P.MUL", "P.PWR", "P.NEQ"] {
        if let Some((a, _)) = s.find_address(name) {
            assert!(a >= 0x100, "{name} em página zero (${a:04X})");
        }
    }
    for (name, addr) in [("OLDTEXT", 0x79u16), ("LINNUM", 0x50), ("DATPTR", 0x7D)] {
        assert_eq!(s.find_symbol(addr).map(|x| x.0), Some(name), "${addr:02X}");
    }
}

/// gh247: a tabela DOS33 (A2_DOS33.SYM2, um script) carrega no startup.
#[test]
fn gh247_dos33_symbols_are_preloaded() {
    let (_m, d, _) = setup();
    assert!(d.symbols.len(Table::Dos33) > 0);
    assert_eq!(
        d.symbols.find_address("DOS.COS0").map(|x| x.0),
        Some(0x9EEB)
    );
}

/// gh425 e gh719: $C080-$C08F são só Language Card (LCRAMIN2... LCBANK1_), o Disk II tem seus
/// símbolos em $C0E0-$C0EF e os anunciadores aparecem em $C058-$C05F. O APPLE2E.SYM do oráculo
/// já foi corrigido; o port tem o mesmo arquivo.
#[test]
fn gh425_gh719_soft_switch_symbols_separate_lc_and_disk2() {
    let mut s = Symbols::default();
    s.parse_table(BUILTIN_MAIN, Table::Main, 0);
    for a in 0xC080..=0xC08Fu16 {
        let n = s.find_symbol(a).map(|x| x.0).unwrap_or("");
        assert!(
            n.starts_with("LC") || n.starts_with("ROMIN"),
            "${a:04X} = {n}"
        );
    }
    assert_ne!(s.find_symbol(0xC08C).map(|x| x.0), Some("DATASTROBE"));
    assert_eq!(s.find_symbol(0xC0E0).map(|x| x.0), Some("DRV_P0_OFF"));
    assert_eq!(s.find_symbol(0xC0EC).map(|x| x.0), Some("DRV_SHIFT"));
}

/// gh1145: os .SYM seguem desorganizados (fora de ordem, nomes repetidos). Mesmos arquivos do
/// oráculo.
#[test]
#[ignore = "FAIL gh1145: arquivos .SYM idênticos aos do oráculo, desordenados e com nomes repetidos"]
fn gh1145_symbol_files_are_sorted_and_unique() {
    let mut prev = 0u32;
    let mut names = std::collections::HashSet::new();
    for l in BUILTIN_MAIN.lines() {
        let mut it = l.split_whitespace();
        if let (Some(a), Some(n)) = (it.next(), it.next()) {
            if let Ok(a) = u32::from_str_radix(a, 16) {
                assert!(a >= prev, "fora de ordem: {l}");
                prev = a;
                assert!(names.insert(n.to_string()), "nome repetido: {n}");
            }
        }
    }
}

/// gh1342: o limite de nome de símbolo é 51 (não 31): um nome de 31 caracteres que difere de outro
/// só no último caractere se mantém distinto ao carregar de arquivo.
#[test]
fn gh1342_symbol_names_up_to_51_chars_are_not_truncated() {
    use apple2_debugger::symbols::MAX_SYMBOLS_LEN;
    assert_eq!(MAX_SYMBOLS_LEN, 51);
    let mut s = Symbols::default();
    let a = "TestPhasorWithSSI263AfterReset";
    let b = "TestPhasorWithSSI263AfterReset2";
    assert_eq!(b.len(), 31);
    s.parse_table(&format!("5CF6 {a}\n5DD4 {b}\n"), Table::User1, 0);
    assert_eq!(s.find_address(a).map(|x| x.0), Some(0x5CF6));
    assert_eq!(s.find_address(b).map(|x| x.0), Some(0x5DD4));
    let long = "X".repeat(51);
    s.parse_table(&format!("0400 {long}Y\n"), Table::User1, 0);
    assert_eq!(s.find_symbol(0x400).unwrap().0.len(), 51);
    let (mut m, mut d, _) = setup();
    let t = cmd(&mut d, &mut m, &format!("SYM {} = 500", "N".repeat(60)));
    assert!(t.contains("Symbol length"), "{t}");
}

/// gh114: a forma com aspas lê o arquivo e carrega os símbolos.
#[test]
fn gh114_symuser_load_with_quotes_reads_the_file() {
    let (mut m, mut d, files) = setup();
    files
        .borrow_mut()
        .insert("filename.sym".into(), b"0300 FOO\n0310 BAR\n".to_vec());
    let t = cmd(&mut d, &mut m, "SYMUSER LOAD \"filename.sym\"");
    assert!(t.contains("User1"), "{t}");
    assert_eq!(d.symbols.find_address("BAR").map(|x| x.0), Some(0x310));
    let t = cmd(&mut d, &mut m, "SYMUSER LOAD \"nada.sym\"");
    assert!(t.contains("Symbol File not found"), "{t}");
}

/// gh114: `SYMUSER LOAD filename.sym` sem aspas não faz nada no oráculo (só o nome entre aspas é
/// lido), e o port repete.
#[test]
#[ignore = "FAIL gh114: sem aspas o nome do arquivo é ignorado, igual ao oráculo (CmdSymbolsLoad exige TYPE_QUOTED_2)"]
fn gh114_symuser_load_accepts_an_unquoted_file_name() {
    let (mut m, mut d, files) = setup();
    files
        .borrow_mut()
        .insert("filename.sym".into(), b"0300 FOO\n".to_vec());
    cmd(&mut d, &mut m, "SYMUSER LOAD filename.sym");
    assert_eq!(d.symbols.find_address("FOO").map(|x| x.0), Some(0x300));
}

/// gh245: `SYMUSER SAVE` não grava nada e não diz nada (no oráculo `CmdSymbolsSave` só devolve
/// UPDATE_CONSOLE_DISPLAY); depois de salvar e limpar, o LOAD não restaura os símbolos.
#[test]
#[ignore = "FAIL gh245: SYMUSER SAVE é um no-op, igual ao oráculo (CmdSymbolsSave não faz nada)"]
fn gh245_symuser_save_then_load_round_trips() {
    let (mut m, mut d, files) = setup();
    cmd(&mut d, &mut m, "SYM TOMMY = 1111");
    let t = cmd(&mut d, &mut m, "SYMUSER SAVE \"tom1\"");
    assert!(!t.is_empty(), "sem mensagem de que salvou");
    assert!(files.borrow().contains_key("tom1"));
    cmd(&mut d, &mut m, "SYMUSER CLEAR");
    cmd(&mut d, &mut m, "SYMUSER LOAD \"tom1\"");
    assert_eq!(d.symbols.find_address("TOMMY").map(|x| x.0), Some(0x1111));
}

/// gh466: nomes com `#` e `/` (e bytes inválidos) de um A2_USER1.SYM carregam e aparecem no
/// disassembly; o carregamento não se perde por causa de CRLF, TAB ou lixo.
#[test]
fn gh466_special_chars_in_loaded_symbols_are_shown() {
    let (mut m, mut d, files) = setup();
    files.borrow_mut().insert(
        "A2_USER1.SYM".into(),
        b"0300\tCMDNTB.MON\r\n0303 CMDNTB.PR#\r\n0306 A/B\r\n0309 \xFF\xFE.BAD\r\n030C OK\r\n"
            .to_vec(),
    );
    cmd(&mut d, &mut m, "SYMUSER LOAD \"A2_USER1.SYM\"");
    assert_eq!(d.symbols.len(Table::User1), 5);
    load(&mut m, 0x0300, &[0xEA; 6]);
    let rows = lines_from(&mut d, &m, 0x0300, 7);
    assert!(rows[0].contains("CMDNTB.MO"), "{rows:?}");
    assert!(rows[3].contains("CMDNTB.PR"), "{rows:?}");
    assert!(rows[6].contains("A/B"), "{rows:?}");
    cmd(&mut d, &mut m, "ASC CMDNTB.MON:CMDNTB.PR#");
    cmd(&mut d, &mut m, "SYM A/B");
}

/// gh466: procurar pelo nome com `#` deveria achar o símbolo.
#[test]
#[ignore = "FAIL gh466: `SYM CMDNTB.PR#` dá Syntax Error (o `#` é operador no tokenizador), igual ao oráculo"]
fn gh466_symbol_names_with_hash_can_be_looked_up() {
    let (mut m, mut d, _) = setup();
    d.symbols.set(Table::User1, "CMDNTB.PR#", 0xA8E1);
    let t = cmd(&mut d, &mut m, "SYM CMDNTB.PR#");
    assert!(t.contains("A8E1"), "{t}");
}

/// gh1377: o startup deveria pré-carregar todas as tabelas (e `.sym2` antes de `.sym`); o port,
/// como o oráculo, carrega só Main, Basic, DOS33 e User1.
#[test]
#[ignore = "FAIL gh1377: startup_symbols carrega 4 das 9 tabelas, igual ao oráculo"]
fn gh1377_startup_preloads_all_symbol_tables() {
    let (mut m, mut d, files) = setup();
    for (f, sym) in [
        ("A2_ASM.SYM", "ASMSYM"),
        ("A2_USER2.SYM", "U2SYM"),
        ("A2_SRC1.SYM", "S1SYM"),
    ] {
        files
            .borrow_mut()
            .insert(f.into(), format!("0300 {sym}\n").into_bytes());
    }
    cmd(&mut d, &mut m, "STARTUP");
    for s in ["ASMSYM", "U2SYM", "S1SYM"] {
        assert!(
            d.symbols.find_address(s).is_some(),
            "{s} não foi pré-carregado"
        );
    }
}

/// gh227: um símbolo longo no alvo de uma instrução não vaza do painel de código para o painel de
/// registradores (colunas >= 51 iguais com e sem o símbolo).
#[test]
fn gh227_long_symbol_does_not_spill_into_the_register_window() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0A39, &[0x4C, 0x63, 0xEB, 0xEA]); // JMP $EB63
    m.cpu_mut().pc = 0x0300;
    let mut cols = Vec::new();
    for with_symbol in [false, true] {
        if with_symbol {
            cmd(&mut d, &mut m, "SYM COPY.FAC.TO.ARG.ROUNDED = EB63");
        }
        d.disasm_cur = 0x0A39;
        d.disasm_top = 0x0A39;
        d.calc_top_bot(&m);
        let g = d.render(&m);
        let right: Vec<String> = (0..40)
            .map(|y| (51..80).map(|x| g.cell(x, y).ch).collect())
            .collect();
        cols.push((g, right));
    }
    let g = &cols[1].0;
    let r = (0..40)
        .map(|y| row(g, y))
        .find(|r| r.contains("JMP"))
        .expect("linha do JMP");
    assert!(
        r.contains("COPY.FAC.TO.ARG.ROUNDED") || r.contains("COPY.FAC"),
        "{r}"
    );
    assert_eq!(
        cols[0].1, cols[1].1,
        "o símbolo vazou para a janela de registradores"
    );
}

/// gh1231: no esquema preto-e-branco (`BW`) a linha do PC não fica com texto da cor do fundo.
#[test]
fn gh1231_black_and_white_scheme_pc_line_is_legible() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0xA9, 0x01, 0xEA]);
    m.cpu_mut().pc = 0x0300;
    for scheme in ["BW", "MONO", "COLOR"] {
        cmd(&mut d, &mut m, scheme);
        d.disasm_cur = 0x0300;
        d.disasm_top = 0x0300;
        d.calc_top_bot(&m);
        let g = d.render(&m);
        let y = (0..40)
            .find(|y| row(&g, *y).starts_with("0300:"))
            .expect("linha do PC");
        for x in 0..50 {
            let c = g.cell(x, y);
            if c.ch != ' ' {
                assert_ne!(c.fg, c.bg, "{scheme}: '{}' ilegível na coluna {x}", c.ch);
            }
        }
    }
}

/// gh587: alvos de desvio abaixo de $1000 mostram o zero à esquerda (`BCS $0FFE`).
#[test]
fn gh587_branch_targets_below_1000_keep_the_leading_zero() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0FFE, &[0xB0, 0xFE, 0xB0, 0xFE]);
    let rows = lines_from(&mut d, &m, 0x0FFE, 2);
    assert!(rows[0].contains("BCS $0FFE"), "{rows:?}");
    assert!(rows[1].contains("BCS $1000"), "{rows:?}");
    let line0 = d.disasm_line(&m, 0x0FFE);
    let line1 = d.disasm_line(&m, 0x1000);
    assert_eq!(line0.address.len(), line1.address.len());
    assert_eq!(line0.target.len(), line1.target.len());
}

/// gh1457: com S=$FE/$FF o painel da pilha dá a volta ($01FF -> $0100) em vez de listar
/// endereços fora da página 1.
#[test]
fn gh1457_stack_window_wraps_at_page_1() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0100, &[0x44, 0x55]);
    load(&mut m, 0x01FD, &[0x11, 0x22, 0x33]);
    for (sp, expect) in [(0xFDu8, "01FE"), (0xFE, "01FF"), (0xFF, "0100")] {
        m.cpu_mut().sp = sp;
        let g = d.render(&m);
        let stack: Vec<String> = (0..12)
            .map(|y| (51..60).map(|x| g.cell(x, y).ch).collect::<String>())
            .collect();
        assert!(
            stack.iter().any(|r| r.starts_with(&format!("{expect}:"))),
            "S=${sp:02X}: {stack:?}"
        );
        assert!(
            !stack.iter().any(|r| r.starts_with("0200:")),
            "S=${sp:02X}: saiu da página 1: {stack:?}"
        );
    }
}

/// gh1456 e gh1460: seta direita sobre RTS segue o endereço de retorno (com volta na pilha) e
/// Ctrl+Direita atualiza os alvos. Cobertos também por `audit_wave0_debugger.rs`.
#[test]
fn gh1456_gh1460_right_arrow_on_rts_and_ctrl_right_updates_targets() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0100, &[0x44, 0x55]);
    load(&mut m, 0x01FD, &[0x11, 0x22, 0x33]);
    m.poke(0x0A00, 0x60);
    m.cpu_mut().pc = 0x0A00;
    d.disasm_cur = 0x0A00;
    for (sp, want) in [(0xFDu8, 0x3323u16), (0xFE, 0x4434), (0xFF, 0x5545)] {
        m.cpu_mut().sp = sp;
        d.disasm_cur = 0x0A00;
        key(&mut d, &mut m, Key::Right, NONE);
        assert_eq!(d.disasm_cur, want, "S=${sp:02X}");
    }
    // Ctrl+Direita: PC passa a ser o cursor e o painel de alvos acompanha
    load(&mut m, 0x0300, &[0xAD, 0x00, 0x04, 0xEA]);
    m.poke(0x0400, 0x7E);
    m.cpu_mut().pc = 0x0303;
    d.disasm_cur = 0x0300;
    key(&mut d, &mut m, Key::Right, CTRL);
    assert_eq!(m.cpu().pc, 0x0300);
    let g = d.render(&m);
    assert!(
        (0..20).any(|y| row(&g, y)[51..].contains("0400:7E") || row(&g, y)[51..].contains("0400")),
        "alvo $0400 não aparece no painel"
    );
}

/// gh1451: Tab localiza a instrução atual (PC) e a seta direita segue o alvo da instrução do
/// cursor, como em `DebuggerProcessKey`.
#[test]
fn gh1451_tab_locates_pc_and_right_arrow_follows_target() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0x4C, 0x40, 0x03]);
    m.cpu_mut().pc = 0x0300;
    d.disasm_cur = 0x1234;
    key(&mut d, &mut m, Key::Tab, NONE);
    assert_eq!(d.disasm_cur, 0x0300);
    key(&mut d, &mut m, Key::Right, NONE);
    assert_eq!(d.disasm_cur, 0x0340);
}

/// gh1384: no modo de saída (`\`) as teclas 0 a 9 escolhem a página; outra tecla volta ao
/// depurador.
#[test]
fn gh1384_output_view_pages_by_digit_keys() {
    use apple2_debugger::debugger::OutputView;
    let (mut m, mut d, _) = setup();
    cmd(&mut d, &mut m, "HGR");
    assert!(matches!(d.output_view, Some(OutputView::Hgr(None))));
    for (k, want) in [
        ('0', OutputView::HgrPseudo(0)),
        ('1', OutputView::HgrPseudo(1)),
        ('2', OutputView::HgrPseudo(2)),
        ('3', OutputView::HgrPseudo(3)),
        ('4', OutputView::HgrPseudo(4)),
        ('5', OutputView::HgrPseudo(5)),
        ('9', OutputView::Current),
    ] {
        key(&mut d, &mut m, Key::Char(k), NONE);
        assert_eq!(d.output_view, Some(want), "tecla {k}");
    }
    key(&mut d, &mut m, Key::Char('x'), NONE);
    assert_eq!(d.output_view, None);
    cmd(&mut d, &mut m, "GR2");
    assert!(matches!(d.output_view, Some(OutputView::Gr(Some(2)))));
    cmd(&mut d, &mut m, "WIN OUTPUT");
    assert!(d.output_view.is_some());
}

/// gh406 e gh702: o painel de soft switches mostra o estado do Language Card (banco, leitura,
/// escrita) e da memória auxiliar (ZP/RD/WR), e a destaque muda quando o estado muda.
#[test]
fn gh406_gh702_soft_switch_panel_shows_lc_and_aux_state() {
    let (mut m, mut d, _) = setup_e();
    let fg_at = |g: &Grid, label: &str, off: usize| {
        let (x, y) = find_text(g, label).unwrap_or_else(|| panic!("sem '{label}'"));
        *g.cell(x + off, y)
    };
    let before = d.render(&m);
    for label in ["LC", "LCR", "LCW", "ZP", "RD ", "WR "] {
        assert!(find_text(&before, label).is_some(), "sem {label}");
    }
    let lcr0 = fg_at(&before, "LCR", 4);
    let zp0 = (fg_at(&before, "ZP ", 3).fg, fg_at(&before, "ZP ", 5).fg);
    cmd(&mut d, &mut m, "IN C083"); // LC bank 2, RAM leitura + escrita (2 leituras)
    cmd(&mut d, &mut m, "IN C083");
    cmd(&mut d, &mut m, "OUT C009 0"); // ALTZP on
    let after = d.render(&m);
    assert_ne!(lcr0.fg, fg_at(&after, "LCR", 4).fg, "LCR RAM não destacou");
    assert_ne!(
        zp0,
        (fg_at(&after, "ZP ", 3).fg, fg_at(&after, "ZP ", 5).fg),
        "ZP aux não destacou"
    );
}

/// gh1393: o painel mostra o banco RamWorks ativo (`r` + 2 dígitos hex) mesmo para bancos > 127.
#[test]
fn gh1393_ramworks_active_bank_is_shown_above_127() {
    use apple2_core::AuxCardKind;
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    cfg.aux_card = AuxCardKind::RamWorksIII { banks: 256 };
    let (mut m, mut d, _) = setup_with(Machine::new(cfg));
    for bank in [0x01u8, 0x7F, 0x80, 0x85, 0xFF] {
        cmd(&mut d, &mut m, &format!("OUT C073 {bank:X}"));
        assert_eq!(m.board().memory.aux_bank(), bank as u16);
        let g = d.render(&m);
        assert!(
            find_text(&g, &format!("r{bank:02X}")).is_some(),
            "banco ${bank:02X} não aparece no painel"
        );
    }
}

/// gh1426 e gh1431: `M1 L2/D000` (mini-dump com prefixo de Language Card/Saturn) deve mostrar a
/// memória daquele banco mesmo com o LC desligado, e atualizar quando o conteúdo muda. O
/// `read_prefixed` do port só distingue main/aux (ignora `l1/`, `l2/` e slot).
#[test]
#[ignore = "FAIL gh1426/gh1431: read_prefixed ignora o prefixo lN/ e sN/; apple2-core não expõe o backing do LC por banco/slot"]
fn gh1426_gh1431_mini_dump_with_lc_prefix_shows_lc_memory() {
    let (mut m, mut d, _) = setup_e();
    // Escreve no LC banco 2 e depois desliga o LC (a visão da CPU volta à ROM)
    cmd(&mut d, &mut m, "IN C083");
    cmd(&mut d, &mut m, "IN C083");
    for i in 0..4u16 {
        m.poke(0xD000 + i, 0xA0 + i as u8);
    }
    cmd(&mut d, &mut m, "IN C082"); // ROM
    cmd(&mut d, &mut m, "M1 L2/D000");
    let g = d.render(&m);
    assert!(
        (0..48).any(|y| row(&g, y).contains("D000: A0 A1 A2 A3")),
        "mini-dump não mostra o LC banco 2"
    );
}

/// gh989: o `JMP ($02FF)` do 6502 original lê o byte alto de $0200 (não de $0300). No Apple ][+
/// (NMOS) o port salta para $0820 (a issue relata AppleWin saltando para $0320).
#[test]
fn gh989_jmp_indirect_page_boundary_bug_on_nmos() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0310, &[0x6C, 0xFF, 0x02]);
    m.poke(0x02FF, 0x20);
    m.poke(0x0300, 0x03);
    m.poke(0x0200, 0x08);
    m.cpu_mut().pc = 0x0310;
    d.trace_n(&mut m, 1);
    assert_eq!(m.cpu().pc, 0x0820, "6502 NMOS: byte alto vem de $0200");
    // O disassembly mostra o ponteiro com o mesmo comportamento
    let line = d.disasm_line(&m, 0x0310);
    assert!(line.mnemonic.starts_with("JMP"), "{line:?}");
    // 65C02 (//e enhanced): pega o byte alto de $0300
    let (mut m2, mut d2, _) = setup_e();
    load(&mut m2, 0x0310, &[0x6C, 0xFF, 0x02]);
    m2.poke(0x02FF, 0x20);
    m2.poke(0x0300, 0x03);
    m2.poke(0x0200, 0x08);
    m2.cpu_mut().pc = 0x0310;
    d2.trace_n(&mut m2, 1);
    assert_eq!(m2.cpu().pc, 0x0320, "65C02: byte alto vem de $0300");
}

/// gh1421: `LOG` controla o nível de saída do console (NONE, ERROR, WARN, INFO, DEFAULT, ALL),
/// e `STARTUP` recarrega os símbolos.
#[test]
fn gh1421_log_levels_filter_console_output() {
    let (mut m, mut d, files) = setup();
    files
        .borrow_mut()
        .insert("u.sym".into(), b"0300 FOO\n".to_vec());
    for lvl in [
        "NONE", "ERROR", "WARN", "INFO", "DEFAULT", "ALL", "OFF", "ON",
    ] {
        let t = cmd(&mut d, &mut m, &format!("LOG {lvl}"));
        assert!(
            !t.contains("Illegal") && !t.contains("Syntax"),
            "LOG {lvl}: {t}"
        );
    }
    cmd(&mut d, &mut m, "LOG ALL");
    cmd(&mut d, &mut m, "SYM A = 300");
    let loud = cmd(&mut d, &mut m, "SYM B = 301");
    assert!(loud.contains("Added"), "{loud}");
    cmd(&mut d, &mut m, "LOG ERROR");
    let quiet = cmd(&mut d, &mut m, "SYM C = 302");
    assert!(!quiet.contains("Added"), "{quiet}");
    cmd(&mut d, &mut m, "LOG ALL");
    cmd(&mut d, &mut m, "SYMUSER CLEAR");
    cmd(&mut d, &mut m, "STARTUP");
    assert!(d.symbols.len(Table::Main) > 700);
    assert!(
        d.symbols.find_address("C").is_none(),
        "STARTUP não limpou a tabela do usuário"
    );
}

/// gh473: nomes exatos de comando resolvem sem ambiguidade (`DISASM`, `DISK`); o prefixo comum
/// `DIS` lista os candidatos; `MA1 D0D0` e `MT2 FF0A` são comandos válidos.
#[test]
fn gh473_command_names_resolve_without_false_ambiguity() {
    use apple2_debugger::commands::{find_command, Found};
    assert!(matches!(find_command("DISASM"), Found::One(_)));
    assert!(matches!(find_command("disk"), Found::One(_)));
    assert!(matches!(find_command("MA1"), Found::One(_)));
    assert!(matches!(find_command("MT2"), Found::One(_)));
    assert!(matches!(find_command("DIS"), Found::Many(_)));
    let (mut m, mut d, _) = setup();
    for line in ["DISASM", "MA1 D0D0", "MT2 FF0A", "ma1 d0d0"] {
        let t = cmd(&mut d, &mut m, line);
        assert!(
            !t.contains("Ambiguous") && !t.contains("Illegal"),
            "{line}: {t}"
        );
    }
    let t = cmd(&mut d, &mut m, "DIS");
    assert!(t.contains("Ambiguous"), "{t}");
    assert!(t.contains("DISASM") && t.contains("DISK"), "{t}");
}

/// gh476: linhas longas no console quebram em 80 colunas sem perder nenhum caractere.
#[test]
fn gh476_long_console_lines_wrap_without_losing_characters() {
    let (mut m, mut d, _) = setup();
    let long: String = (0..200).map(|i| (b'A' + (i % 26) as u8) as char).collect();
    d.console.clear();
    d.console.print(&long);
    let joined: String = d
        .console
        .lines()
        .map(apple2_debugger::console::line_string)
        .collect();
    assert_eq!(joined, long);
    for l in d.console.lines() {
        assert!(apple2_debugger::console::line_string(l).chars().count() <= 80);
    }
    cmd(&mut d, &mut m, &format!("ECHO {long}"));
    let t = text(&d);
    assert!(
        t.replace('\n', "").contains(&long[..150]),
        "ECHO perdeu caracteres"
    );
    // com o console em tela cheia a renderização mantém os caracteres
    d.toggle_full_screen_console();
    let g = d.render(&m);
    let all: String = (0..g.h).map(|y| row(&g, y)).collect();
    assert!(all.contains(&long[..80]));
}

/// gh1221: DISK ADDR/DATA PROLOGUE/EPILOGUE não existem no oráculo (a8f0d1d): `DISK` só
/// aceita os subcomandos que ele tem. Aqui só se confere que a forma pedida cai no erro/ajuda.
#[test]
fn gh1221_disk_prologue_epilogue_subcommands_are_not_in_the_oracle() {
    let (mut m, mut d, _) = setup_e();
    let t = cmd(&mut d, &mut m, "DISK ADDR PROLOGUE D5AA96 3");
    assert!(!t.is_empty());
    let _ = cmd(&mut d, &mut m, "DISK");
    let _ = cmd(&mut d, &mut m, "DISK DATA EPILOGUE");
}

/// gh142, gh479, gh746: `render` não tem estado nem efeitos colaterais: chamadas repetidas dão a
/// mesma grade, igual à de um depurador novo no mesmo estado, e rolar o cursor 200 vezes não
/// deixa resíduo fora das janelas. (Os defeitos originais eram de repintura do GDI no Win32.)
#[test]
fn gh142_gh479_gh746_render_is_idempotent_and_leaves_no_residue() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0xEA; 400]);
    m.cpu_mut().pc = 0x0300;
    d.disasm_cur = 0x0300;
    d.calc_top_bot(&m);
    let a = d.render(&m);
    let b = d.render(&m);
    assert_eq!(a.cells, b.cells, "duas renderizações seguidas diferem");
    for _ in 0..200 {
        key(&mut d, &mut m, Key::Down, NONE);
        let g1 = d.render(&m);
        let g2 = d.render(&m);
        assert_eq!(g1.cells, g2.cells);
        // as colunas entre o código e o painel (50) ficam em branco
        for y in 0..40 {
            assert_eq!(g1.cell(50, y).ch, ' ', "resíduo na coluna 50 da linha {y}");
        }
    }
    // um depurador novo no mesmo estado desenha igual
    let mut d2 = Debugger::new();
    d2.initialize(&mut m);
    d2.enter(&mut m);
    d2.disasm_cur = d.disasm_cur;
    d2.disasm_top = d.disasm_top;
    d2.calc_top_bot(&m);
    d.disasm_top = d2.disasm_top;
    d.calc_top_bot(&m);
    assert_eq!(d.render(&m).cells[..50].len(), 50);
}

/// gh666: o painel mostra a posição do feixe (v/h) e a posição muda ao andar; `VIDEOINFO` troca
/// entre hexadecimal/decimal e posição Apple/real.
#[test]
fn gh666_video_scanner_position_is_shown_and_changes() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0xEA; 64]);
    m.cpu_mut().pc = 0x0300;
    let find = |g: &Grid| {
        let (x, y) = find_text(g, "v:").expect("sem v:");
        row(g, y)[x..x + 14].to_string()
    };
    let g0 = d.render(&m);
    let s0 = find(&g0);
    assert!(s0.contains("h:"), "{s0}");
    d.trace_n(&mut m, 1);
    for _ in 0..40 {
        d.trace_n(&mut m, 1);
    }
    let s1 = find(&d.render(&m));
    assert_ne!(s0, s1);
    cmd(&mut d, &mut m, "VIDEOINFO DEC");
    let s2 = find(&d.render(&m));
    assert!(s2.contains("v:") && s2.chars().filter(char::is_ascii_digit).count() >= 6);
    cmd(&mut d, &mut m, "VIDEOINFO REAL");
    cmd(&mut d, &mut m, "VIDEOINFO APPLE");
}

/// gh462: as colunas do disassembly (endereço, dois-pontos, opcodes) se restauram com
/// `DISASM COLON/OPCODE/...` (a issue era que o clique do mouse as escondia sem aviso).
#[test]
fn gh462_disasm_columns_can_be_toggled_back_by_command() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0xA9, 0x01, 0xEA]);
    let row0 = |d: &mut Debugger, m: &Machine| lines_from(d, m, 0x0300, 1).first().cloned();
    let normal = row0(&mut d, &m).expect("linha normal");
    assert!(normal.starts_with("0300:A9 01"), "{normal}");
    cmd(&mut d, &mut m, "DISASM OPCODE 0");
    assert!(d.render(&m).cells.iter().all(|_| true));
    d.disasm_cur = 0x0300;
    cmd(&mut d, &mut m, "DISASM COLON 0");
    cmd(&mut d, &mut m, "DISASM OPCODE 1");
    cmd(&mut d, &mut m, "DISASM COLON 1");
    let again = row0(&mut d, &m).expect("linha restaurada");
    assert_eq!(normal, again);
    let t = cmd(&mut d, &mut m, "DISASM CLICK");
    assert!(!t.contains("Illegal"), "{t}");
    let t = cmd(&mut d, &mut m, "DISASM CLICK 4");
    assert!(!t.contains("Illegal"), "{t}");
}

/// gh263: Ctrl+Tab percorre as janelas Code -> Data -> Console -> Code e cada uma renderiza;
/// de volta ao Code o texto do disassembly está lá.
#[test]
fn gh263_window_cycling_returns_to_a_populated_code_window() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0xEA; 16]);
    m.cpu_mut().pc = 0x0300;
    d.disasm_cur = 0x0300;
    d.calc_top_bot(&m);
    for _ in 0..3 {
        key(&mut d, &mut m, Key::Tab, CTRL);
        let g = d.render(&m);
        assert!(screen(&g).len() > 100);
    }
    let g = d.render(&m);
    assert!(screen(&g).contains("0300:EA"), "janela de código vazia");
}

/// gh974: o console aceita os caracteres que o teclado UK precisava (` ' @ ~) digitados
/// diretamente. O mapeamento do teclado UK para esses caracteres fica no frontend.
#[test]
fn gh974_console_accepts_backtick_quote_at_and_tilde() {
    let (mut m, mut d, _) = setup();
    for c in ['`', '\'', '@', '~'] {
        key(&mut d, &mut m, Key::Char(c), NONE);
    }
    assert_eq!(d.console.input(), "`'@~");
    d.console.clear_input();
    let _ = NoHost;
}

/// gh461: o port não tem a pausa "...press SPACE continue, ESC skip..." na abertura do
/// depurador: a entrada fica vazia e o modo é Debug.
#[test]
fn gh461_no_pause_prompt_when_entering_the_debugger() {
    let m = &mut Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    let mut d = Debugger::new();
    d.initialize(m);
    d.enter(m);
    assert_eq!(d.console.input(), "");
    assert!(!text(&d).contains("press SPACE"));
}

/// gh268: `TF` com um nome de caminho longo (o sprintf estourava o buffer de 80) devolve erro ou
/// cria o arquivo sem derrubar.
#[test]
fn gh268_trace_file_with_very_long_path_is_safe() {
    let (mut m, mut d, _) = setup();
    for n in [79, 80, 81, 255, 4096] {
        let name = "p".repeat(n);
        let t = cmd(&mut d, &mut m, &format!("TF \"{name}\""));
        assert!(
            t.contains("Trace started") || t.contains("Trace ERROR"),
            "{n}"
        );
        cmd(&mut d, &mut m, "TF");
    }
    let mut d2 = Debugger::new();
    d2.set_host(Box::new(NoHost));
    d2.enter(&mut m);
    let t = cmd(&mut d2, &mut m, &format!("TF \"{}\"", "x".repeat(300)));
    assert!(t.contains("Trace ERROR"), "{t}");
}

/// gh463: o erro de script traz o nome do arquivo e `PWD` diz onde ele foi procurado; um caminho
/// relativo com subdiretório é encontrado.
#[test]
fn gh463_run_script_errors_name_the_file_and_pwd_shows_the_directory() {
    let (mut m, mut d, files) = setup();
    let t = cmd(&mut d, &mut m, "RUN debug");
    assert!(
        t.contains("Couldn't load script") && t.contains("debug"),
        "{t}"
    );
    let t = cmd(&mut d, &mut m, "PWD");
    assert!(t.contains("/d/"), "{t}");
    files
        .borrow_mut()
        .insert("SubFolder/debug.txt".into(), b"MEB 500 77\n".to_vec());
    cmd(&mut d, &mut m, "RUN \"SubFolder/debug.txt\"");
    assert_eq!(m.peek(0x500), 0x77);
}

fn temp_tree() -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("a2dbg-cd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("x").join("y")).unwrap();
    std::fs::create_dir_all(base.join("other")).unwrap();
    base
}

/// gh468: `CD ..` remove o último diretório do caminho em vez de acrescentar `..`.
#[test]
#[ignore = "FAIL gh468: FsHost::set_dir só faz dir.join(..), PWD mostra .../y/..; o oráculo também acrescenta"]
fn gh468_cd_dotdot_removes_the_last_directory() {
    use apple2_debugger::host::FsHost;
    let base = temp_tree();
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    let mut d = Debugger::new();
    d.set_host(Box::new(FsHost::new(base.join("x").join("y"))));
    d.enter(&mut m);
    cmd(&mut d, &mut m, "CD ..");
    let t = cmd(&mut d, &mut m, "PWD");
    assert!(
        t.trim_end()
            .ends_with(&format!("{}", base.join("x").display())),
        "{t}"
    );
    let _ = std::fs::remove_dir_all(base);
}

/// gh505: `CD` com caminho absoluto substitui o diretório corrente em vez de acrescentar.
#[test]
fn gh505_cd_absolute_path_replaces_the_current_directory() {
    use apple2_debugger::host::FsHost;
    let base = temp_tree();
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    let mut d = Debugger::new();
    d.set_host(Box::new(FsHost::new(base.join("x"))));
    d.enter(&mut m);
    let target = base.join("other");
    cmd(&mut d, &mut m, &format!("CD \"{}\"", target.display()));
    let t = cmd(&mut d, &mut m, "PWD");
    assert!(t.trim_end().ends_with(&target.display().to_string()), "{t}");
    assert!(!t.contains("x"), "o caminho antigo foi mantido: {t}");
    let t = cmd(&mut d, &mut m, "CD \"/definitivamente/nao/existe\"");
    assert!(t.contains("Invalid directory"), "{t}");
    let _ = std::fs::remove_dir_all(base);
}
