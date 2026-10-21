//! Auditoria wave0 (HU-0801): comportamento do depurador sem teclado, contra o AppleWin a8f0d1d.
//!
//! Os atalhos de teclado (`DebuggerProcessKey`, Debug.cpp:9653) vivem no frontend e são testados
//! em `crates/apple2-rust/tests/audit_wave0_peripherals.rs`; aqui ficam as funções da biblioteca
//! que os atalhos chamariam, e a garantia de que desenhar a tela não mexe no emulador.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::Debugger;

fn bare() -> Machine {
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

fn debugger(m: &mut Machine) -> Debugger {
    let mut d = Debugger::new();
    d.initialize(m);
    d.enter(m);
    d
}

/// #1456: Seta-direita sobre um RTS segue o endereço de retorno da pilha, inclusive com a volta
/// da página da pilha ("3323 sem overflow / 4434 / 5545 com overflow" da issue).
#[test]
fn aud_p_037_follow_target_on_rts_wraps_the_stack_page_gh1456() {
    for (sp, expected) in [(0xFDu8, 0x3323u16), (0xFE, 0x4434), (0xFF, 0x5545)] {
        let mut m = bare();
        m.poke(0x100, 0x44);
        m.poke(0x101, 0x55);
        for (i, b) in [0x11u8, 0x22, 0x33].iter().enumerate() {
            m.poke(0x1FD + i as u16, *b);
        }
        m.poke(0x0300, 0x60); // RTS
        m.cpu_mut().pc = 0x0300;
        m.cpu_mut().sp = sp;
        let mut d = debugger(&mut m);
        d.goto(&m, 0x0300);
        d.cursor_follow_target(&m);
        assert_eq!(d.disasm_cur, expected, "S={sp:02X}");
    }
}

/// HU-0801: "não permitir que o debugger altere o estado do emulador apenas por renderizar".
/// `render` recebe `&Machine` (o compilador impede escrita); confere também que ciclos, registros
/// e a RAM não mudam, e que os switches (que uma leitura de $C0xx alteraria) ficam intactos.
#[test]
fn aud_p_039_rendering_the_debugger_screen_does_not_touch_the_machine() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    for _ in 0..120 {
        m.run_frame();
    }
    let mut d = debugger(&mut m);
    let snapshot = |m: &Machine| {
        (
            m.cycles(),
            m.cpu().pc,
            m.cpu().sp,
            m.memmode(),
            m.board().video,
            (0u16..=0xFFFF).map(|a| m.peek(a)).collect::<Vec<u8>>(),
        )
    };
    let before = snapshot(&m);
    for _ in 0..3 {
        let _ = d.render(&m);
    }
    assert!(before == snapshot(&m));
}

/// #1460: após `=` (Ctrl-Right: PC := cursor) a janela de alvos usa o PC novo, porque o Rust
/// calcula alvos a cada render a partir de `m.cpu().pc`.
#[test]
fn aud_p_037_set_pc_from_cursor_updates_targets_gh1460() {
    let mut m = bare();
    m.poke(0x0300, 0xEA);
    m.poke(0x0400, 0x4C); // JMP $1234
    m.poke(0x0401, 0x34);
    m.poke(0x0402, 0x12);
    let mut d = debugger(&mut m);
    d.goto(&m, 0x0400);
    d.execute(&mut m, "=");
    assert_eq!(m.cpu().pc, 0x0400);
    let grid = d.render(&m);
    let text: String = (0..grid.h)
        .map(|y| (0..grid.w).map(|x| grid.cell(x, y).ch).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("1234"),
        "alvo do JMP ausente da tela:\n{text}"
    );
}
