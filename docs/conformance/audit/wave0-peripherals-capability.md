# Matriz de capacidades: periféricos (wave0)

> **Superada.** Esta tabela é o retrato da wave 0 (SHA `6da6048`, 20/10/2026) e muitos estados mudaram depois (por exemplo, Saturn, save-state, depurador, vídeo, áudio). O estado vigente está em [capability-matrix.md](../capability-matrix.md).

Código `6da6048`; oráculo AppleWin `a8f0d1d`. Regra do plano: IMPLEMENTED não é PASS. PASS só aparece onde há teste que reproduz o comportamento do oráculo (código C++ lido ou repro da issue); nenhuma linha tem comparação diferencial automática com o binário do AppleWin (ver `wave0-peripherals.md`, §3.2). IDs `AUD-P-NNN` apontam para a tabela de achados.

| Componente | Modelo | Estado | Evidência | Divergência |
|---|---|---|---|---|
| Modo de vídeo: texto 40 colunas, flash, inverso | ][, ][+, //e, //e Enhanced | IMPLEMENTED | `renderer.rs:820-826`; `tests/modes.rs:20,59` | Sem oráculo (AUD-P-002/003) |
| Modo de vídeo: texto 80 colunas | //e, //e Enhanced | IMPLEMENTED | `tests/modes.rs:76`, `tests/models.rs:116` | Sem oráculo |
| Modo de vídeo: lores, dlores | todos com 80COL/AUX conforme o caso | IMPLEMENTED | `tests/modes.rs:92`, `composite.rs:96` | Sem oráculo |
| Modo de vídeo: hires, dhires | todos / //e | IMPLEMENTED | `tests/modes.rs:109,183` | Sem oráculo |
| Modo de vídeo: misto e troca de página | //e Enhanced | IMPLEMENTED | `tests/modes.rs:151,165`; troca no meio da linha `tests/machine.rs:109,147` | Sem oráculo; #1443 e #1131 sem demo (AUD-P-001) |
| Modo de vídeo: bordas (20x18 px) | todos | IMPLEMENTED | `renderer.rs:27-28` | Sem oráculo |
| Modo de vídeo: SHR (via VidHD) | ][, ][+, //e | IMPLEMENTED | `tests/vidhd.rs:48` | Sem imagem de referência |
| Tipo de vídeo: Mono (custom, TV, âmbar, verde, branco) | todos | IMPLEMENTED | `tests/composite.rs:142` | Sem oráculo |
| Tipo de vídeo: Color Idealized | todos | IMPLEMENTED | `tests/composite.rs:96,123` | Sem oráculo |
| Tipo de vídeo: Color Monitor NTSC / PAL | todos | IMPLEMENTED | `tests/composite.rs:24,56` | Sem oráculo (AUD-P-002) |
| Tipo de vídeo: Color TV (NTSC/PAL, scanlines) | todos | IMPLEMENTED | `tests/composite.rs:67,175,197`; `renderer.rs:603-609` | #1443 ported mas sem demo (AUD-P-001); desempenho 15x o tempo real (`pendencias.md`) |
| Cartas RGB (Video-7 SL7, Le Chat Mauve Eve/Feline, AppleColor) | //e, //e Enhanced | IMPLEMENTED | `tests/rgb_card.rs` | Estado da placa não vai ao save-state: FAIL (AUD-P-005) |
| VidHD (SHR, modos herdados centralizados) | ][, ][+, //e | IMPLEMENTED | `vidhd.rs`, `tests/vidhd.rs` | 80x45 e cor de borda: o AppleWin também não (AUD-P-004, NOT_APPLICABLE) |
| VidHD: save/load dos registradores | ][, ][+, //e | IMPLEMENTED | `cards_peripherals.rs:423-470` | Sem teste visual (AUD-P-050) |
| Alto-falante | todos | IMPLEMENTED | `apple2-audio/tests/machine.rs:72,108` | Sem PCM de referência (AUD-P-010) |
| Mockingboard C (6522, AY-3-8910) | ][+, //e | IMPLEMENTED | `via6522.rs:250-312` vs `6522.cpp:338-400`; `tests/machine.rs:151-340` | `ay-socketN` e `6522a/b-bad` ignorados: FAIL (AUD-P-006/007); PCM sem oráculo (AUD-P-010) |
| Mockingboard: IRQs do 6522 | ][+, //e | PASS (comportamental, GH#701/GH#985) | `tests/machine.rs:207-270,533-640` | Só registradores e temporizadores; sem trace do C++ |
| Phasor (modo nativo, 4 AY) | ][+, //e | IMPLEMENTED | `tests/machine.rs:337` | Soquetes 2 e 3 (3º e 4º AY) sem tipo próprio: FAIL (AUD-P-006) |
| MegaAudio, SD Music | //e | IMPLEMENTED | `tests/machine.rs:366` | Sem oráculo |
| SSI263 (power-down, #1412) | ][+, //e | PASS | `audit_wave0_ssi263.rs` | PCM da fala em taxa fixa, sem inflexão (manual); sem oráculo de áudio |
| SSI263 (save-state de fonema contínuo, #1372) | ][+, //e | PASS (chip); IMPLEMENTED (cartão) | `audit_wave0_ssi263.rs`; `tests/machine.rs:495` | Posição dentro do fonema não é guardada (igual ao AppleWin) |
| SC-01 (Votrax) | ][+, //e | IMPLEMENTED | `tests/machine.rs:456` | Sem oráculo |
| SAM | ][+, //e | IMPLEMENTED | `tests/machine.rs:132` | Sem oráculo |
| Z80 SoftCard: instruções, flags, ciclos | ][+, //e | IMPLEMENTED | `z80.rs` (testes internos), `tests/softcard.rs` | Sem trace de referência; CP/M não executado (UNVERIFIED) |
| Z80: IRQ, NMI, IM 0/1/2 | ][+, //e | FAIL (contra a HU-0601); paridade com o AppleWin | AUD-P-011 | O AppleWin também não entrega interrupções |
| Z80: MEMPTR (BIT, INI/OUTI/INIR/OTIR, saltos) | ][+, //e | PASS (contra a especificação) | `audit_wave0_z80.rs`; `z80.rs:1557-1700` | F3/F5 de LDIR/INIR/OTIR repetindo: FAIL (AUD-P-014) |
| Z80: opcode indefinido | ][+, //e | FAIL (contra a HU-0601) | AUD-P-013 | NOP silencioso |
| Mouse (Mouse Interface, 6821) | ][+, //e | IMPLEMENTED | `apple2-cards/tests/mouse.rs` | Captura de mouse por SO sem teste real (AUD-P-029, BLOCKED) |
| Relógio: No-Slot Clock | ][, ][+, //e | IMPLEMENTED | `tests/clocks.rs` | Soquete F8 no ][/][+ não funciona com ProDOS 2.5.0a8, igual ao AppleWin |
| Relógio: Generic Clock | ][+, //e | IMPLEMENTED | fábrica `factory.rs` (carta vazia) | Vazia também no AppleWin |
| Relógio: ThunderClock Plus | ][+, //e | IMPLEMENTED (só biblioteca) | `thunderclock.rs`; `tests/clocks.rs` | Não selecionável; o AppleWin também não (AUD-P-015) |
| SSC (6551, TCP, laço, porta COM) | ][+, //e | IMPLEMENTED | `tests/ssc.rs` | Erros de abertura engolidos e detecção de portas heurística: FAIL (AUD-P-030/031) |
| Impressora paralela / Pravets | ][+, //e | IMPLEMENTED | `tests/printer.rs` | Erros de arquivo invisíveis (AUD-P-033) |
| Uthernet I | ][+, //e | IMPLEMENTED | `tests/uthernet1.rs` | Rede real só com `pcap`; falha de abertura invisível: FAIL (AUD-P-023) |
| Uthernet II: registradores, TCP, UDP, RAW, DNS virtual | ][+, //e | IMPLEMENTED | `tests/uthernet2.rs` | Sem teste de sessões múltiplas e pacotes malformados (AUD-P-024) |
| Uthernet II: SPI, PPPoE, interrupções | ][+, //e | NOT_APPLICABLE vs oráculo; FAIL vs HU | AUD-P-021 | Ausentes também no AppleWin |
| Uthernet II: sockets no save-state | ][+, //e | PASS (paridade) | `uthernet2.rs:308-368` vs `Uthernet2.cpp:337-362` | Nenhuma |
| 4Play | ][+, //e | FAIL (não conectada) | AUD-P-018 | A carta nunca recebe direções |
| SNES MAX | ][+, //e | FAIL (não conectada) | AUD-P-017 | Controles e flags `-snes-max-*` sem efeito |
| Dongles da porta de jogos | ][, ][+, //e | IMPLEMENTED | `dongles.rs`; `apple2-savestate/tests/units_misc_gameio.rs` | Sem teste do firmware do HDD com PB2 |
| Botões e paddles (#1522, #1234) | ][+, //e | PASS | `audit_wave0_peripherals.rs` (`aud_p_025`, `aud_p_026`) | Paddles 2/3 com o SHIFT key mod: FAIL (AUD-P-027) |
| Breakpoint Card | ][+, //e | IMPLEMENTED (isolada) | `breakpoint.rs` | Não recebe breakpoints do depurador: FAIL (AUD-P-016) |
| Configuração de slots (conflitos, slot 0) | ][+, //e | FAIL | `audit_wave0_peripherals.rs` (`aud_p_019`, `aud_p_020`, ignorados) | Validação só no diálogo |
| Depurador: comandos | ][+, //e | IMPLEMENTED (233 de 251 com efeito) | `debugger-commands.md` | 7 indisponíveis, BPEDIT sem efeito (AUD-P-038) |
| Depurador: atalhos de teclado | ][+, //e | FAIL | `debugger-commands.md` §1 | AUD-P-035/036 |
| Depurador: #1456 / #1460 (função) | ][+, //e Enhanced | PASS | `audit_wave0_debugger.rs` | Sem tecla (AUD-P-037) |
| Depurador: renderizar não altera o emulador | ][+, //e Enhanced | PASS | `audit_wave0_debugger.rs` | Nenhuma |
| UI: tecla presa ao trocar foco, F7 e egui | todos | FAIL | AUD-P-028 | Sem reprodutor automatizado |
| UI: Ctrl+Print Screen | todos | FAIL | `aud_p_040` (ignorado) | Só 40 colunas, página 1 |
| UI: atalhos F1-F12, Print Screen, Pause | todos | PASS | `aud_p_049` | Nenhuma |
| UI: Alt+Enter e `-alt-enter` | todos | FAIL | `aud_p_046` (ignorado) | Preferência sem efeito |
| UI: soltar disco, somente leitura, monitor de setores, benchmark, volumes por carta, importar configuração, listas de rede e serial | todos | FAIL (difere ou não existe) | `wave0-peripherals.md` §6 | AUD-P-031/032/041-048 |
