//! HU-0801 / issue #34: as outras issues `debugger` da matriz de conformidade (P2/P3 e UNVERIFIED).
//! Um teste por issue (ou por grupo), nomeado `gh<N>_...`. Resultado por issue em
//! `docs/conformance/results/depurador.csv`. Oráculo: AppleWin a8f0d1d (Debugger/*.cpp).

mod common;

use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::{Key, RunMode};
use common::*;

// =============================================================== execução, passos, traço

/// gh20: o registrador de VBL ($C019) tem que mudar no modo "Stepping" (o depurador chamava
/// CpuExecute direto e o VBL nunca era atualizado).
#[test]
fn gh20_vbl_register_changes_while_stepping() {
    let (mut m, mut d, _) = setup_e();
    // LDA $C019 / AND #$80 / TAX / LDA #1 / STA $0500,X / JMP $0300
    load(
        &mut m,
        0x0300,
        &[
            0xAD, 0x19, 0xC0, 0x29, 0x80, 0xAA, 0xA9, 0x01, 0x9D, 0x00, 0x05, 0x4C, 0x00, 0x03,
        ],
    );
    m.poke(0x0500, 0);
    m.poke(0x0580, 0);
    m.cpu_mut().pc = 0x0300;
    d.goto(&m, 0x0300);
    d.trace_n(&mut m, 30_000); // > 2 quadros de vídeo
    run_until_stopped(&mut d, &mut m);
    assert!(m.cycles() > 40_000, "{}", m.cycles());
    assert_eq!(m.peek(0x0500), 1, "bit 7 de $C019 = 0 nunca visto");
    assert_eq!(m.peek(0x0580), 1, "bit 7 de $C019 = 1 nunca visto");
}

/// gh38: `TF` escreve de verdade o cabeçalho e, por instrução, ciclos, registradores, flags e a
/// linha desmontada (o bug era texto de lixo no lugar do disassembly).
#[test]
fn gh38_trace_file_writes_disassembly_per_instruction() {
    let (mut m, mut d, files) = setup();
    load(&mut m, 0x0300, &[0xA9, 0x80, 0xAA, 0xEA]); // LDA #$80 / TAX / NOP
    m.cpu_mut().pc = 0x0300;
    let t = cmd(&mut d, &mut m, "TF \"trace.txt\"");
    assert!(t.contains("Trace started"), "{t}");
    d.trace_n(&mut m, 3);
    run_until_stopped(&mut d, &mut m);
    let t = cmd(&mut d, &mut m, "TF");
    assert!(t.contains("Trace stopped"), "{t}");
    let f = String::from_utf8(files.borrow()["trace.txt"].clone()).unwrap();
    let lines: Vec<&str> = f.lines().collect();
    assert_eq!(
        lines[0],
        "Cycles   A: X: Y: SP:  Flags     Addr:Opcode    Mnemonic"
    );
    assert!(
        lines[1].contains("0300:A9 80") && lines[1].contains("LDA #$80"),
        "{f}"
    );
    assert!(
        lines[2].contains("0302:AA") && lines[2].contains("TAX"),
        "{f}"
    );
    assert!(
        lines[3].contains("0303:EA") && lines[3].contains("NOP"),
        "{f}"
    );
    // depois do LDA #$80 o A é $80 e a flag N está ligada
    assert!(lines[2].contains(" 80 "), "{f}");
    assert!(lines[2].contains('N'), "{f}");
    // o traço não desenha o disassembly na tela do Apple II: nenhuma escrita em $0400
    assert_eq!(m.peek(0x0400), 0);
}

/// gh783: o traço leva o contador de ciclos (8 dígitos hex) na primeira coluna e ele cresce.
#[test]
fn gh783_trace_file_has_cycle_count_column() {
    let (mut m, mut d, files) = setup();
    load(&mut m, 0x0300, &[0xEA, 0xEA, 0xEA]);
    m.cpu_mut().pc = 0x0300;
    cmd(&mut d, &mut m, "TF c.txt");
    d.trace_n(&mut m, 3);
    run_until_stopped(&mut d, &mut m);
    cmd(&mut d, &mut m, "TF");
    let f = String::from_utf8(files.borrow()["c.txt"].clone()).unwrap();
    let cyc: Vec<u32> = f
        .lines()
        .skip(1)
        .map(|l| u32::from_str_radix(l.split_whitespace().next().unwrap(), 16).unwrap())
        .collect();
    assert_eq!(cyc.len(), 3);
    assert_eq!(cyc[1] - cyc[0], 2, "NOP gasta 2 ciclos: {cyc:?}");
    assert_eq!(cyc[2] - cyc[1], 2);
}

/// gh106: `BPC <id>`, `BPC <id>,<id>` e `BPC *` removem breakpoints sem "Illegal operation"; um id
/// inexistente é ignorado.
#[test]
fn gh106_breakpoint_clear_by_id_list_and_star() {
    let (mut m, mut d, _) = setup();
    for a in ["300", "310", "320", "330"] {
        cmd(&mut d, &mut m, &format!("BPX {a}"));
    }
    let t = cmd(&mut d, &mut m, "BPC 1");
    assert!(!t.contains("Illegal"), "{t}");
    let t = cmd(&mut d, &mut m, "BPL");
    assert!(
        t.contains("#0 ") && t.contains("#2 ") && t.contains("#3 "),
        "{t}"
    );
    assert!(!t.contains("#1 "), "{t}");
    cmd(&mut d, &mut m, "BPC 99");
    cmd(&mut d, &mut m, "BPC 0,2");
    let t = cmd(&mut d, &mut m, "BPL");
    assert!(
        t.contains("#3 ") && !t.contains("#0 ") && !t.contains("#2 "),
        "{t}"
    );
    cmd(&mut d, &mut m, "BPX 340");
    cmd(&mut d, &mut m, "BPC *");
    let t = cmd(&mut d, &mut m, "BPL");
    assert!(t.contains("no current breakpoints"), "{t}");
}

/// gh112 e gh316: `G <addr>` para em <addr> sem deixar um breakpoint na lista, e os breakpoints
/// existentes não somem nem mudam de id (o `bpc 1` do meio não faz o b2 sumir).
#[test]
fn gh112_gh316_go_until_leaves_the_breakpoint_list_untouched() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0800, &[0xEA; 16]);
    for a in ["123", "456", "789"] {
        cmd(&mut d, &mut m, &format!("BPX {a}"));
    }
    cmd(&mut d, &mut m, "BPC 1");
    let before = cmd(&mut d, &mut m, "BPL");
    m.cpu_mut().pc = 0x0800;
    cmd(&mut d, &mut m, "G 805");
    run_until_stopped(&mut d, &mut m);
    assert_eq!(m.cpu().pc, 0x0805);
    let after = cmd(&mut d, &mut m, "BPL");
    assert_eq!(before, after, "G alterou a lista de breakpoints");
    assert!(after.contains("0123") && after.contains("0789"), "{after}");
    assert!(!after.contains("0805"), "{after}");
    // e uma segunda execução passa por $0805 sem parar lá
    m.cpu_mut().pc = 0x0800;
    cmd(&mut d, &mut m, "G 80A");
    run_until_stopped(&mut d, &mut m);
    assert_eq!(m.cpu().pc, 0x080A);
}

/// gh217: `G` roda em velocidade normal e só `GG` pede velocidade máxima.
#[test]
fn gh217_go_is_normal_speed_and_gg_is_full_speed() {
    let (mut m, mut d, _) = setup();
    cmd(&mut d, &mut m, "G 400");
    assert_eq!(d.mode, RunMode::Stepping);
    assert!(!d.full_speed);
    d.mode = RunMode::Debug;
    d.steps = 0;
    cmd(&mut d, &mut m, "GG 400");
    assert_eq!(d.mode, RunMode::Stepping);
    assert!(d.full_speed);
}

/// gh249: Ctrl+Espaço num JSR com breakpoint sempre cai na instrução seguinte, mesmo repetindo
/// (o relato era "às vezes" parar em outra instrução).
#[test]
fn gh249_step_over_jsr_at_a_breakpoint_is_repeatable() {
    let (mut m, mut d, _) = setup();
    // 0300: JSR $0320 / 0303: JMP $0300 ; 0320: JSR $0330 / 0323: PLA/PHA / RTS ; 0330: RTS
    load(&mut m, 0x0300, &[0x20, 0x20, 0x03, 0x4C, 0x00, 0x03]);
    load(&mut m, 0x0320, &[0x20, 0x30, 0x03, 0x68, 0x48, 0x60]);
    load(&mut m, 0x0330, &[0x60]);
    m.cpu_mut().pc = 0x0300;
    m.cpu_mut().sp = 0xFF;
    cmd(&mut d, &mut m, "BPX 300");
    for round in 0..5 {
        cmd(&mut d, &mut m, "G");
        run_until_stopped(&mut d, &mut m);
        assert_eq!(m.cpu().pc, 0x0300, "rodada {round}");
        key(&mut d, &mut m, Key::Char(' '), CTRL);
        assert_eq!(m.cpu().pc, 0x0303, "rodada {round}");
        assert_eq!(m.cpu().sp, 0xFF, "rodada {round}: pilha desbalanceada");
    }
}

/// gh1194: step-over de um `JSR WAIT` com A grande (muitos ciclos) volta ao fim do JSR, com a
/// pilha limpa, em vez de parar dentro do WAIT.
#[test]
fn gh1194_step_over_wait_with_large_delay_completes() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    // JSR $FCA8 (WAIT) em $0300
    load(&mut m, 0x0300, &[0x20, 0xA8, 0xFC, 0xEA]);
    let (mut m2, mut d, _) = {
        m.cpu_mut().pc = 0x0300;
        setup_with(m)
    };
    for a in [0xFFu8, 0xFE, 0x1A, 0x78] {
        m2.cpu_mut().pc = 0x0300;
        m2.cpu_mut().sp = 0xF0;
        m2.cpu_mut().a = a;
        d.step_over_n(&mut m2, 1);
        assert_eq!(m2.cpu().pc, 0x0303, "A=${a:02X}");
        assert_eq!(m2.cpu().sp, 0xF0, "A=${a:02X}: pilha suja");
        d.mode = RunMode::Debug;
        d.steps = 0;
    }
}

/// gh450: `R PC <reset-vector>` seguido de `G` reinicia a execução no vetor (PC alterado e a
/// execução continua dali).
#[test]
fn gh450_set_pc_then_go_restarts_execution_at_the_new_pc() {
    let (mut m, mut d, _) = setup();
    load(
        &mut m,
        0x0300,
        &[0xA9, 0x5A, 0x8D, 0x00, 0x04, 0x4C, 0x05, 0x03],
    );
    m.cpu_mut().pc = 0x0900;
    cmd(&mut d, &mut m, "R PC 300");
    assert_eq!(m.cpu().pc, 0x0300);
    assert_eq!(
        d.disasm_cur, 0x0300,
        "o cursor do disassembly acompanha o PC"
    );
    cmd(&mut d, &mut m, "G 305");
    run_until_stopped(&mut d, &mut m);
    assert_eq!(m.cpu().pc, 0x0305);
    assert_eq!(m.peek(0x0400), 0x5A);
}

/// gh451 e gh445: BPMR só dispara em leitura, BPMW só em escrita, BPM em ambas, e o acesso à
/// pilha (PHA/PLA/JSR/RTS) dispara o BPM da página 1. Como no AppleWin, o endereço efetivo é
/// conferido antes da instrução executar (o PC fica na instrução que acessa).
#[test]
fn gh451_gh445_memory_breakpoints_read_write_and_stack() {
    // leitura: LDA $0600 ; escrita: STA $0601
    let program = [0xAD, 0x00, 0x06, 0x8D, 0x01, 0x06, 0xEA];
    for (cmdline, expect_stop_after) in [
        ("BPMR 600", Some(0x0300u16)),
        ("BPMW 600", None),
        ("BPMR 601", None),
        ("BPMW 601", Some(0x0303)),
        ("BPM 600", Some(0x0300)),
        ("BPM 601", Some(0x0303)),
    ] {
        let (mut m, mut d, _) = setup();
        load(&mut m, 0x0300, &program);
        m.cpu_mut().pc = 0x0300;
        cmd(&mut d, &mut m, cmdline);
        cmd(&mut d, &mut m, "G 307");
        run_until_stopped(&mut d, &mut m);
        match expect_stop_after {
            Some(pc) => {
                assert!(
                    d.stop_reason
                        .as_deref()
                        .is_some_and(|r| r.contains("access")),
                    "{cmdline}"
                );
                assert_eq!(m.cpu().pc, pc, "{cmdline}");
            }
            None => assert_eq!(m.cpu().pc, 0x0307, "{cmdline} não devia parar antes"),
        }
    }
    // pilha: PHA grava em $01FF
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0xA9, 0x11, 0x48, 0x68, 0xEA]);
    m.cpu_mut().pc = 0x0300;
    m.cpu_mut().sp = 0xFF;
    cmd(&mut d, &mut m, "BPMW 1FF");
    cmd(&mut d, &mut m, "G 304");
    run_until_stopped(&mut d, &mut m);
    assert_eq!(m.cpu().pc, 0x0302, "BPMW na pilha para no PHA");
    cmd(&mut d, &mut m, "BPC *");
    cmd(&mut d, &mut m, "BPMR 1FF");
    cmd(&mut d, &mut m, "G 304");
    run_until_stopped(&mut d, &mut m);
    assert_eq!(m.cpu().pc, 0x0303, "BPMR na pilha para no PLA");
}

/// gh1103: o BPMW para na primeira escrita no endereço, mesmo que a instrução seguinte
/// sobrescreva o valor logo depois (o relato era o valor já trocado quando o depurador parava) e
/// também em `GG` (velocidade máxima).
#[test]
fn gh1103_bpmw_stops_at_the_first_write_even_at_full_speed() {
    for go in ["G", "GG"] {
        let (mut m, mut d, _) = setup();
        // LDA #$9E / STA $7FB5 / LDA #$85 / STA $7FB5 / JMP $0300
        load(
            &mut m,
            0x0300,
            &[
                0xA9, 0x9E, 0x8D, 0xB5, 0x7F, 0xA9, 0x85, 0x8D, 0xB5, 0x7F, 0x4C, 0x00, 0x03,
            ],
        );
        m.cpu_mut().pc = 0x0300;
        m.poke(0x7FB5, 0x00);
        cmd(&mut d, &mut m, "BPMW 7FB5");
        cmd(&mut d, &mut m, go);
        run_until_stopped(&mut d, &mut m);
        assert_eq!(m.cpu().pc, 0x0302, "{go}: para no primeiro STA");
        assert_eq!(m.peek(0x7FB5), 0x00, "{go}: antes da escrita");
        assert!(
            d.stop_reason
                .as_deref()
                .is_some_and(|r| r.contains("Write access")),
            "{go}: {:?}",
            d.stop_reason
        );
    }
}

/// gh737 e gh787: o contador de ciclos do último passo (`CYCLES REL`) aparece na tela e `CYCLES
/// PART`/`RCC` contam a partir de uma base.
#[test]
fn gh737_gh787_cycle_counter_modes() {
    let (mut m, mut d, _) = setup();
    load(
        &mut m,
        0x0300,
        &[0xAD, 0x00, 0x06, 0xEA, 0xEA, 0xEA, 0xEA, 0xEA],
    );
    m.cpu_mut().pc = 0x0300;
    d.goto(&m, 0x0300);
    let t = cmd(&mut d, &mut m, "CYCLES REL");
    assert!(
        t.contains("Cycles display updated: REL") || t.contains("updated"),
        "{t}"
    );
    d.trace_n(&mut m, 1); // LDA abs = 4 ciclos
    assert!(screen(&d.render(&m)).contains("cycles:00000004"));
    d.trace_n(&mut m, 1); // NOP = 2
    assert!(screen(&d.render(&m)).contains("cycles:00000002"));
    // PART: a base é o ciclo atual
    cmd(&mut d, &mut m, "CYCLES PART");
    assert!(screen(&d.render(&m)).contains("cycles:00000000"));
    d.trace_n(&mut m, 2);
    run_until_stopped(&mut d, &mut m);
    assert!(screen(&d.render(&m)).contains("cycles:00000004"));
    // RCC zera de novo
    cmd(&mut d, &mut m, "RCC");
    assert!(screen(&d.render(&m)).contains("cycles:00000000"));
    d.trace_n(&mut m, 1);
    assert!(screen(&d.render(&m)).contains("cycles:00000002"));
    // ABS mostra o total
    cmd(&mut d, &mut m, "CYCLES ABS");
    let abs = format!("cycles:{:08X}", m.cycles() as u32);
    assert!(screen(&d.render(&m)).contains(&abs), "{abs}");
    // argumento inválido ou ausente -> ajuda
    let t = cmd(&mut d, &mut m, "CYCLES FOO");
    assert!(t.contains("CYCLES, Cycles display"), "{t}");
    let t = cmd(&mut d, &mut m, "CYCLES");
    assert!(t.contains("CYCLES, Cycles display"), "{t}");
}

/// gh1164: `BPV` dispara em qualquer linha de varredura, inclusive as do blanking vertical
/// (>= $C0), também com a máquina rodando em velocidade máxima.
#[test]
fn gh1164_bpv_hits_non_visible_scan_lines() {
    for line in ["0", "C0", "C1", "100", "105"] {
        let (mut m, mut d, _) = setup();
        load(&mut m, 0x0300, &[0x4C, 0x00, 0x03]);
        m.cpu_mut().pc = 0x0300;
        cmd(&mut d, &mut m, &format!("BPV {line}"));
        cmd(&mut d, &mut m, "GG");
        run_until_stopped(&mut d, &mut m);
        let want = u32::from_str_radix(line, 16).unwrap();
        assert!(
            d.stop_reason
                .as_deref()
                .is_some_and(|r| r.contains("Video scanner position")),
            "BPV {line}: {:?}",
            d.stop_reason
        );
        assert_eq!(m.video_position().0, want, "BPV {line}");
    }
}

/// gh269: Escape no depurador só limpa a linha de entrada; não sai do depurador nem consome nada
/// quando o programa roda (com a máquina em `Running` a tecla é do Apple II).
#[test]
fn gh269_escape_only_clears_the_input_line() {
    let (mut m, mut d, _) = setup();
    key(&mut d, &mut m, Key::Char('A'), NONE);
    key(&mut d, &mut m, Key::Escape, NONE);
    assert_eq!(d.console.input(), "");
    assert_ne!(d.mode, RunMode::Running);
    d.exit(&mut m);
    assert_eq!(d.mode, RunMode::Running);
    key(&mut d, &mut m, Key::Escape, NONE);
    assert_eq!(
        d.mode,
        RunMode::Running,
        "Esc não devolve o depurador sozinho"
    );
}

/// gh570: a barra de espaço dá passos repetidos sem deixar espaços no buffer do console.
#[test]
fn gh570_space_bar_steps_repeatedly_without_polluting_the_input() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0xEA; 20]);
    m.cpu_mut().pc = 0x0300;
    for i in 1..=10u16 {
        key(&mut d, &mut m, Key::Char(' '), NONE);
        assert_eq!(m.cpu().pc, 0x0300 + i);
        assert_eq!(d.console.input(), "");
    }
}

/// gh291: o custo por instrução do laço de passos não explode quando nenhum break-opcode está
/// ativo (o IsDebugBreakOpcode ocupava 10% do emulador). Limite folgado, só em release.
#[test]
fn gh291_stepping_loop_throughput() {
    let (mut m, mut d, _) = setup();
    load(&mut m, 0x0300, &[0x4C, 0x00, 0x03]);
    m.cpu_mut().pc = 0x0300;
    let t = std::time::Instant::now();
    d.trace_n(&mut m, 1_000_000);
    run_until_stopped(&mut d, &mut m);
    let el = t.elapsed();
    if !cfg!(debug_assertions) {
        assert!(el.as_secs_f64() < 5.0, "1M instruções em {el:?}");
    }
}
