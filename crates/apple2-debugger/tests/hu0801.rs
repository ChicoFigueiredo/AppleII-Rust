//! HU-0801: keyboard shortcuts, the commands that used to say "Not available", and the Breakpoint
//! Card hand-off. Oracle: AppleWin a8f0d1d (`DebuggerProcessKey`, Debug.cpp:9653; the commands in
//! Debug.cpp and Debugger_Win32.cpp).

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Write};
use std::rc::Rc;

use apple2_cards::BreakpointCard;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::console::line_string;
use apple2_debugger::debugger::{OutputView, Window};
use apple2_debugger::host::Host;
use apple2_debugger::{Debugger, Key, KeyMods, RunMode};
use apple2_storage::disk2::Disk2Card;

type Files = Rc<RefCell<HashMap<String, Vec<u8>>>>;

/// In-memory files that the test can still read after the debugger took the host.
struct SharedHost(Files);

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

fn nop_machine() -> Machine {
    let mut rom = vec![0xEAu8; 0x3000];
    for (vec, target) in [(0xFFFAu16, 0x0500u16), (0xFFFC, 0x0300), (0xFFFE, 0x0400)] {
        let o = vec as usize - 0xD000;
        rom[o] = target as u8;
        rom[o + 1] = (target >> 8) as u8;
    }
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.custom_rom = Some(rom);
    Machine::new(cfg)
}

fn setup() -> (Machine, Debugger, Files) {
    let mut m = nop_machine();
    let files = Files::default();
    let mut d = Debugger::new();
    d.set_host(Box::new(SharedHost(files.clone())));
    d.initialize(&mut m);
    d.enter(&mut m);
    (m, d, files)
}

fn text(d: &Debugger) -> String {
    d.console
        .lines()
        .map(line_string)
        .collect::<Vec<_>>()
        .join("\n")
}

const NONE: KeyMods = KeyMods {
    shift: false,
    ctrl: false,
    alt: false,
};
const SHIFT: KeyMods = KeyMods {
    shift: true,
    ctrl: false,
    alt: false,
};
const CTRL: KeyMods = KeyMods {
    shift: false,
    ctrl: true,
    alt: false,
};

/// Runs a bounded number of slices so a regression fails instead of hanging the suite.
fn run_until_stopped(d: &mut Debugger, m: &mut Machine) {
    for _ in 0..200 {
        if d.mode != RunMode::Stepping {
            return;
        }
        d.run(m, 1000);
    }
    panic!("o depurador não parou");
}

fn key(d: &mut Debugger, m: &mut Machine, k: Key, mods: KeyMods) {
    d.process_key(m, k, mods);
}

// ---------------------------------------------------------------- teclas

/// Debug.cpp:9848-9867. Espaço = Trace, Shift+Espaço = StepOut, Ctrl+Espaço = StepOver.
#[test]
fn space_family_traces_steps_out_and_steps_over() {
    let (mut m, mut d, _) = setup();
    m.poke(0x0300, 0x20); // JSR $0310
    m.poke(0x0301, 0x10);
    m.poke(0x0302, 0x03);
    m.poke(0x0310, 0xEA);
    m.poke(0x0311, 0x60); // RTS
    d.goto(&m, 0x0300);
    m.cpu_mut().pc = 0x0300;
    // Ctrl+Espaço sobre o JSR passa pela sub-rotina inteira
    key(&mut d, &mut m, Key::Char(' '), CTRL);
    assert_eq!(m.cpu().pc, 0x0303);
    // Espaço dá um passo
    m.cpu_mut().pc = 0x0300;
    key(&mut d, &mut m, Key::Char(' '), NONE);
    assert_eq!(m.cpu().pc, 0x0310);
    // Shift+Espaço roda até o endereço de retorno da pilha
    key(&mut d, &mut m, Key::Char(' '), SHIFT);
    run_until_stopped(&mut d, &mut m);
    assert_eq!(m.cpu().pc, 0x0303);
}

/// Debug.cpp:9766, :9544-9550. Com texto digitado o Espaço entra na linha; sem texto é ignorado
/// pelo console (só dá o passo).
#[test]
fn space_is_text_only_when_the_line_is_not_empty() {
    let (mut m, mut d, _) = setup();
    key(&mut d, &mut m, Key::Char('R'), NONE);
    let pc = m.cpu().pc;
    key(&mut d, &mut m, Key::Char(' '), NONE);
    assert_eq!(m.cpu().pc, pc);
    assert_eq!(d.console.input(), "R ");
    key(&mut d, &mut m, Key::Escape, NONE);
    assert_eq!(d.console.input(), "");
    key(&mut d, &mut m, Key::Char(' '), NONE);
    assert_eq!(d.console.input(), "", "o passo não deixa espaço na linha");
}

/// Debug.cpp:9989-10047. Cima/Baixo movem o cursor do disassembly; Shift = 1 byte; Ctrl+Baixo =
/// `CmdCursorRunUntil`.
#[test]
fn up_and_down_move_the_cursor_and_ctrl_down_runs_until_it() {
    let (mut m, mut d, _) = setup();
    for a in 0x0302..0x0310 {
        m.poke(a, 0xEA);
    }
    m.poke(0x0300, 0xA9); // LDA #$01 (2 bytes), depois NOPs
    m.poke(0x0301, 0x01);
    d.goto(&m, 0x0300);
    key(&mut d, &mut m, Key::Down, NONE);
    assert_eq!(d.disasm_cur, 0x0302);
    key(&mut d, &mut m, Key::Up, NONE);
    assert_eq!(d.disasm_cur, 0x0300);
    let top = d.disasm_top;
    key(&mut d, &mut m, Key::Down, SHIFT);
    assert_eq!(d.disasm_cur, 0x0301, "Shift+Baixo anda um byte");
    let _ = top;
    let top = d.disasm_top;
    key(&mut d, &mut m, Key::Up, SHIFT);
    assert_eq!(
        d.disasm_top,
        top.wrapping_sub(1),
        "Shift+Cima recua o topo um byte"
    );
    d.goto(&m, 0x0305);
    m.cpu_mut().pc = 0x0300;
    key(&mut d, &mut m, Key::Down, CTRL);
    run_until_stopped(&mut d, &mut m);
    assert_eq!(m.cpu().pc, 0x0305);
}

/// Debug.cpp:9929-9987. PgUp/PgDn movem o cursor (Shift: 256, Ctrl: 4K); com texto rolam o console.
#[test]
fn page_keys_move_the_cursor_or_scroll_the_console() {
    let (mut m, mut d, _) = setup();
    d.goto(&m, 0x1234);
    key(&mut d, &mut m, Key::PageDown, SHIFT);
    assert_eq!(d.disasm_top & 0xFF, 0, "Shift+PgDn alinha o topo em 256");
    key(&mut d, &mut m, Key::PageDown, CTRL);
    assert_eq!(d.disasm_top & 0xFFF, 0, "Ctrl+PgDn alinha o topo em 4K");
    for i in 0..30 {
        d.console.print(&format!("linha {i}"));
    }
    let cur = d.disasm_cur;
    key(&mut d, &mut m, Key::Char('X'), NONE);
    key(&mut d, &mut m, Key::PageUp, NONE);
    assert_eq!(d.console.scroll(), 3, "com texto o PgUp rola 3 linhas");
    assert_eq!(d.disasm_cur, cur);
    key(&mut d, &mut m, Key::PageDown, NONE);
    assert_eq!(d.console.scroll(), 0);
}

/// Debug.cpp:9869-9927.
#[test]
fn home_and_end_go_to_zero_and_8000() {
    let (mut m, mut d, _) = setup();
    key(&mut d, &mut m, Key::End, NONE);
    assert_eq!(d.disasm_cur, 0x8000);
    key(&mut d, &mut m, Key::Home, NONE);
    assert_eq!(d.disasm_cur, 0x0000);
}

/// Debug.cpp:10049-10066 e :9830-9847.
#[test]
fn arrows_tab_and_alignment() {
    let (mut m, mut d, _) = setup();
    m.poke(0x0300, 0x4C); // JMP $1234
    m.poke(0x0301, 0x34);
    m.poke(0x0302, 0x12);
    m.cpu_mut().pc = 0x0300;
    d.goto(&m, 0x0300);
    // Direita segue o alvo e põe o cursor no topo
    key(&mut d, &mut m, Key::Right, NONE);
    assert_eq!((d.disasm_cur, d.disasm_cur_line), (0x1234, 0));
    // Tab: cursor no PC, centralizado
    key(&mut d, &mut m, Key::Tab, NONE);
    assert_eq!(d.disasm_cur, 0x0300);
    assert!(d.disasm_cur_line > 0);
    // Shift+Direita: PC no topo
    key(&mut d, &mut m, Key::Right, SHIFT);
    assert_eq!((d.disasm_cur, d.disasm_cur_line), (0x0300, 0));
    // Ctrl+Direita: PC := cursor
    d.goto(&m, 0x0400);
    key(&mut d, &mut m, Key::Right, CTRL);
    assert_eq!(m.cpu().pc, 0x0400);
    // Esquerda: endereço de retorno da pilha
    m.cpu_mut().sp = 0xFD;
    m.poke(0x1FE, 0x33);
    m.poke(0x1FF, 0x22);
    key(&mut d, &mut m, Key::Left, NONE);
    assert_eq!(d.disasm_cur, 0x2234);
    key(&mut d, &mut m, Key::Left, SHIFT);
    assert_eq!(d.disasm_cur_line, 0);
}

/// Debug.cpp:10069-10090, :9811-9825.
#[test]
fn ctrl_digits_are_bookmarks_and_ctrl_tilde_toggles_the_console() {
    let (mut m, mut d, _) = setup();
    d.goto(&m, 0x4321);
    key(
        &mut d,
        &mut m,
        Key::Char('7'),
        KeyMods {
            shift: true,
            ctrl: true,
            alt: false,
        },
    );
    assert_eq!(d.bookmarks[7], Some(0x4321));
    d.goto(&m, 0);
    key(&mut d, &mut m, Key::Char('7'), CTRL);
    assert_eq!(d.disasm_cur, 0x4321);
    // Ctrl+~ e Enter vazio alternam o console em tela cheia
    key(&mut d, &mut m, Key::Char('~'), CTRL);
    assert_eq!(d.window, Window::Console);
    key(&mut d, &mut m, Key::Enter, NONE);
    assert_eq!(d.window, Window::Code);
    // Tab com Ctrl percorre as janelas
    key(&mut d, &mut m, Key::Tab, CTRL);
    assert_eq!(d.window, Window::Data);
    key(
        &mut d,
        &mut m,
        Key::Tab,
        KeyMods {
            shift: true,
            ..CTRL
        },
    );
    assert_eq!(d.window, Window::Code);
    // um "~" sem Ctrl é um caractere comum
    key(&mut d, &mut m, Key::Char('~'), NONE);
    assert_eq!(d.console.input(), "~");
}

/// Na janela do console, as setas e PgUp/PgDn/Home/End só rolam o texto.
#[test]
fn console_window_keys_only_scroll() {
    let (mut m, mut d, _) = setup();
    for i in 0..100 {
        d.console.print(&format!("linha {i}"));
    }
    key(&mut d, &mut m, Key::Enter, NONE);
    assert_eq!(d.window, Window::Console);
    let cur = d.disasm_cur;
    key(&mut d, &mut m, Key::Up, NONE);
    assert_eq!(d.console.scroll(), 1);
    key(&mut d, &mut m, Key::PageUp, NONE);
    assert!(d.console.scroll() > 10);
    key(&mut d, &mut m, Key::Home, NONE);
    let top = d.console.scroll();
    assert!(top > 50);
    key(&mut d, &mut m, Key::End, NONE);
    assert_eq!(d.console.scroll(), 0);
    assert_eq!(d.disasm_cur, cur);
}

/// Debug.cpp:9658-9740. No modo "ver a saída" os dígitos escolhem a página, outra tecla sai.
#[test]
fn output_view_digits_pick_a_page_and_other_keys_leave() {
    let (mut m, mut d, _) = setup();
    d.execute(&mut m, "HGR");
    assert!(d.output_view.is_some());
    key(&mut d, &mut m, Key::Char('2'), NONE);
    assert_eq!(d.output_view, Some(OutputView::HgrPseudo(2)));
    key(&mut d, &mut m, Key::Char('9'), NONE);
    assert_eq!(d.output_view, Some(OutputView::Current));
    key(&mut d, &mut m, Key::Char('A'), NONE);
    assert_eq!(d.output_view, None);
    assert_eq!(d.console.input(), "", "a tecla que sai não é digitada");
}

// ---------------------------------------------------------------- comandos

#[test]
fn benchmark_loads_the_program_and_measures_mhz() {
    let (mut m, mut d, _) = setup();
    d.execute(&mut m, "BENCHMARK");
    let t = text(&d);
    assert!(t.contains("MHz"), "{t}");
    assert!(d.benchmarking);
    assert_eq!(m.cpu().pc, 0x0300);
    assert_eq!(
        (m.cpu().a, m.cpu().x, m.cpu().y, m.cpu().sp),
        (0, 0, 0, 0xFF)
    );
    // CpuSetupBenchmark: ASL $06 ; ASL $06 ... primeira linha 06 06 16 16 24 24 ...
    let first: Vec<u8> = (0x300..0x30C).map(|a| m.peek(a)).collect();
    assert_eq!(
        first,
        [6, 6, 0x16, 0x16, 0x24, 0x24, 0x45, 0x45, 0x48, 0x48, 0x65, 0x65]
    );
    // a linha termina com JMP $0310
    assert_eq!(
        (m.peek(0x30C), m.peek(0x30D), m.peek(0x30E)),
        (0x4C, 0x10, 0x03)
    );
    // o último bloco volta a $0300
    let last_jmp = (0x300u16..0x400)
        .rev()
        .find(|a| m.peek(*a) == 0x4C && m.peek(a + 2) == 0x03)
        .unwrap();
    assert_eq!(m.peek(last_jmp + 1), 0x00);
    d.execute(&mut m, "BENCH");
    assert!(text(&d).matches("MHz").count() >= 2);
}

/// `EXITBENCH` tem função NULL na tabela do AppleWin (Debugger_Commands.cpp:400), então
/// `FindCommand` não a encontra (Debug.cpp:8152): comando ilegal nos dois.
#[test]
fn exitbench_is_not_a_command_like_in_applewin() {
    let (mut m, mut d, _) = setup();
    d.execute(&mut m, "EXITBENCH");
    assert!(
        !text(&d).contains("MHz"),
        "EXITBENCH não deve rodar nada: {}",
        text(&d)
    );
    assert!(text(&d).to_lowercase().contains("illegal") || text(&d).contains("EXITBENCH"));
}

#[test]
fn font_prints_the_apple_font_cell_and_validates_mode() {
    let (mut m, mut d, _) = setup();
    d.execute(&mut m, "FONT");
    let t = text(&d);
    assert_eq!(t.matches("Font:").count(), 4, "{t}");
    assert!(t.contains("A: 7  M: 7"), "{t}");
    d.console.clear();
    d.execute(&mut m, "FONT *");
    assert!(text(&d).contains("Font Px: 8"), "{}", text(&d));
    d.execute(&mut m, "FONT MODE 2");
    assert_eq!(d.font_spacing, 2);
    d.execute(&mut m, "FONT MODE 9");
    assert_eq!(d.font_spacing, 2, "modo inválido mostra a ajuda");
    assert!(text(&d).contains("FONT"), "{}", text(&d));
}

#[test]
fn hcolor_validates_like_applewin() {
    let (mut m, mut d, _) = setup();
    d.execute(&mut m, "HCOLOR 1");
    assert!(!text(&d).contains("Not available"));
    d.execute(&mut m, "HCOLOR 1 2");
    let t = text(&d);
    assert!(t.contains("HCOLOR, Sets/Shows colors"), "ajuda: {t}");
}

#[test]
fn sync_and_bpedit_do_what_applewin_does() {
    let (mut m, mut d, _) = setup();
    let before = text(&d);
    d.execute(&mut m, "SYNC");
    d.execute(&mut m, "BPEDIT");
    assert_eq!(text(&d), before, "SYNC e BPEDIT só pedem redesenho");
    assert!(!text(&d).contains("Not available"));
}

#[test]
fn ntsc_save_writes_raw_and_bmp_palettes() {
    let (mut m, mut d, files) = setup();
    d.execute(&mut m, "NTSC SAVE");
    let raw = files
        .borrow()
        .get("AppleWinNTSC4096x4@32.data")
        .cloned()
        .expect("arquivo padrão");
    assert_eq!(raw.len(), 4 * 4096 * 4);
    assert!(raw.chunks(4).all(|p| p[3] == 255), "alfa opaco");
    assert!(text(&d).contains("Saved: /d/AppleWinNTSC4096x4@32.data"));
    // fase 0, sequência 0: preto; a última sequência (todos os bits ligados) é branco
    assert_eq!(&raw[0..3], &[0, 0, 0]);
    let white = (4095 * 4) as usize;
    assert_eq!(&raw[white..white + 3], &[255, 255, 255]);

    d.execute(&mut m, "NTSC SAVE pal.bmp");
    let bmp = files.borrow().get("pal.bmp").cloned().unwrap();
    assert_eq!(&bmp[0..2], b"BM");
    assert_eq!(bmp.len(), 54 + 64 * 256 * 4);
    assert_eq!(u32::from_le_bytes(bmp[0x12..0x16].try_into().unwrap()), 64);
    assert_eq!(u32::from_le_bytes(bmp[0x16..0x1A].try_into().unwrap()), 256);
    assert_eq!(u16::from_le_bytes(bmp[0x1C..0x1E].try_into().unwrap()), 32);
    // transposeTo64x256: linha 255 coluna 0 = fase 0, sequência 255*16
    let seq = 255 * 16;
    let px = |o: usize| &bmp[o..o + 4];
    assert_eq!(
        px(54 + 255 * 64 * 4),
        &raw[seq * 4..seq * 4 + 4]
            .iter()
            .enumerate()
            .map(|(i, b)| (i, *b))
            .fold([0u8; 4], |mut a, (i, b)| {
                a[[2, 1, 0, 3][i]] = b;
                a
            })
    );

    // LOAD valida o arquivo, mas a paleta do port é fixa
    d.console.clear();
    d.execute(&mut m, "NTSC LOAD");
    assert!(text(&d).contains("not applied"), "{}", text(&d));
    files.borrow_mut().insert("bad.data".into(), vec![0; 10]);
    d.console.clear();
    d.execute(&mut m, "NTSC LOAD bad.data");
    assert!(text(&d).contains("Raw size != 65536"), "{}", text(&d));
    d.console.clear();
    d.execute(&mut m, "NTSC RESET");
    assert!(text(&d).contains("Resetting NTSC palette"));
}

#[test]
fn source_reads_listing_symbols_and_bytes() {
    let (mut m, mut d, files) = setup();
    files.borrow_mut().insert(
        "prog.lst".into(),
        b"0300: A9 01     1  START LDA #$01\n0302: 8D 00 04  2        STA $0400\n   COUT   EQU $FDF0\n\nFDED: 60        3  ROUTINE DFB 1\n".to_vec(),
    );
    d.execute(&mut m, "SOURCE MEM SYM prog.lst");
    let t = text(&d);
    assert!(t.contains("Read: 4 lines, 2 symbols, 6 bytes"), "{t}");
    assert!(d.source.active);
    assert_eq!(
        (m.peek(0x300), m.peek(0x301), m.peek(0x302), m.peek(0x304)),
        (0xA9, 0x01, 0x8D, 0x04)
    );
    assert_eq!(d.source.by_addr.get(&0x0302), Some(&1));
    // AppleWin trunca o último caractere do rótulo (sName[nLen - 1] = 0, Debug.cpp:6987)
    let syms = d.symbols.table(apple2_debugger::symbols::Table::Src2);
    assert_eq!(syms.get(&0xFDF0).map(String::as_str), Some("COU"));
    assert_eq!(syms.get(&0xFDED).map(String::as_str), Some("ROUTIN"));
    d.execute(&mut m, "SOURCE nope.lst");
    assert!(
        text(&d).contains("Error reading: /d/nope.lst"),
        "{}",
        text(&d)
    );
    d.execute(&mut m, "SOURCE");
    assert!(!d.source.active);
}

fn dsk_machine() -> (Machine, Debugger, std::path::PathBuf) {
    let mut m = nop_machine();
    assert!(m.insert_card(6, Box::new(Disk2Card::new())));
    let dir = std::env::temp_dir().join(format!("hu0801-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.dsk"), vec![0u8; 143_360]).unwrap();
    let mut d = Debugger::new();
    d.set_host(Box::new(apple2_debugger::host::FsHost::new(&dir)));
    d.initialize(&mut m);
    d.enter(&mut m);
    (m, d, dir)
}

#[test]
fn disk_shows_and_controls_the_drives() {
    let (mut m, mut d, dir) = dsk_machine();
    d.execute(&mut m, "DISK INFO");
    let t = text(&d);
    assert!(
        t.contains("FW16") && t.contains("D1") && t.contains("T$00.00"),
        "{t}"
    );
    assert!(t.contains("n/a"), "sem disco: {t}");
    d.console.clear();
    d.execute(&mut m, "DISK SLOT");
    assert!(text(&d).contains("Current Disk II slot = 6"));
    d.execute(&mut m, "DISK SLOT 5");
    d.execute(&mut m, "DISK INFO");
    assert!(
        text(&d).contains("No Disk II card in slot-5"),
        "{}",
        text(&d)
    );
    d.execute(&mut m, "DISK SLOT 6");

    d.execute(&mut m, "DISK 1 READ \"a.dsk\"");
    assert!(!m.card::<Disk2Card>(6).unwrap().is_drive_empty(0));
    assert!(
        m.card::<Disk2Card>(6).unwrap().is_write_protected(0),
        "IMAGE_FORCE_WRITE_PROTECTED"
    );
    d.execute(&mut m, "DISK 1 PROTECT 0");
    assert!(!m.card::<Disk2Card>(6).unwrap().is_write_protected(0));
    d.execute(&mut m, "DISK 1 PROTECT");
    assert!(m.card::<Disk2Card>(6).unwrap().is_write_protected(0));
    d.console.clear();
    d.execute(&mut m, "DISK INFO");
    assert!(!text(&d).contains("n/a"), "{}", text(&d));
    d.execute(&mut m, "DISK 1 EJECT");
    assert!(m.card::<Disk2Card>(6).unwrap().is_drive_empty(0));
    d.console.clear();
    d.execute(&mut m, "DISK 3 EJECT");
    assert!(text(&d).contains("DISK"), "ajuda: {}", text(&d));
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------- Breakpoint Card

fn card_io(m: &mut Machine, write: Option<u8>, reg: u8) {
    m.with_card(5, |c, ctx| match write {
        Some(v) => c.io_write(reg, v, ctx),
        None => {
            c.io_read(reg, ctx);
        }
    })
    .unwrap();
}

fn card_status(m: &Machine) -> u8 {
    m.card::<BreakpointCard>(5).unwrap().status()
}

fn breakpoint_card_machine() -> (Machine, Debugger) {
    let mut m = nop_machine();
    assert!(m.insert_card(5, Box::new(BreakpointCard::new())));
    for a in 0x0300..0x0310 {
        m.poke(a, 0xEA);
    }
    m.cpu_mut().pc = 0x0300;
    let mut d = Debugger::new();
    d.initialize(&mut m);
    d.enter(&mut m);
    (m, d)
}

/// AUD-P-016. `InterceptBreakpoints` / `g_InterceptBreakpointsCB` (Debug.cpp:368, :9185, :9218):
/// com a carta interceptando, o depurador entrega o breakpoint a ela e continua rodando; sem isso
/// ele para.
#[test]
fn aud_p_016_debugger_hands_breakpoints_to_the_breakpoint_card() {
    // 1) a carta intercepta ($C0n2): o PC breakpoint em $0302 vai para a FIFO e a execução segue
    let (mut m, mut d) = breakpoint_card_machine();
    for b in [1u8, 0x02, 0x03, 0x00, 0x00, 0x00] {
        card_io(&mut m, Some(b), 0); // tipo PC, $0302, acesso R
    }
    card_io(&mut m, None, 2);
    assert!(m.card::<BreakpointCard>(5).unwrap().intercepting());
    d.execute(&mut m, "BP 302");
    d.exit(&mut m);
    assert!(d.is_stepping());
    d.run(&mut m, 200);
    assert!(d.is_stepping(), "a carta recebeu o breakpoint: não para");
    assert_ne!(
        card_status(&m) & 0x80,
        0,
        "match: {:#04x}\n{}",
        card_status(&m),
        text(&d)
    );
    assert_eq!(card_status(&m) & 0x40, 0, "sem mismatch");
    assert!(text(&d).contains("Stop reason"), "{}", text(&d));

    // 2) um segundo disparo sem entrada na FIFO é mismatch
    let (mut m, mut d) = breakpoint_card_machine();
    card_io(&mut m, None, 2);
    d.execute(&mut m, "BP 302");
    d.exit(&mut m);
    d.run(&mut m, 200);
    assert!(d.is_stepping());
    assert_ne!(card_status(&m) & 0x40, 0, "FIFO vazia = mismatch");

    // 3) sem interceptação ($C0n3) o depurador para como sempre
    let (mut m, mut d) = breakpoint_card_machine();
    card_io(&mut m, None, 3);
    d.execute(&mut m, "BP 302");
    d.exit(&mut m);
    d.run(&mut m, 200);
    assert_eq!(d.mode, RunMode::Debug);
    assert_eq!(m.cpu().pc, 0x0302);
}
