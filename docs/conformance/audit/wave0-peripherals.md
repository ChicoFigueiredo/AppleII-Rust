# Auditoria wave0: periféricos, entrada, depurador e UI

- Escopo: HU-0201/0202 (vídeo), HU-0501/0502 (som e fala), HU-0601/0602 (Z80 e cartas), HU-0701 (Uthernet), HU-0702 (entrada), HU-0703 (serial, impressora, pcap), HU-0801 (depurador), HU-0802 (UI).
- Código auditado: AppleII-Rust em `6da6048` (worktree a partir da `main`).
- Oráculo: AppleWin `a8f0d1d` (2026-10-17), C++ em `source/`. Issues lidas com `gh api` em 2026-10-20.
- Regra: só auditoria. Nenhum código de produção foi alterado. Testes novos: `crates/*/tests/audit_wave0_*.rs`.
- Estados: IMPLEMENTED, PASS, FAIL, UNVERIFIED, NOT_APPLICABLE, BLOCKED (GLOSSARY.md, "Estado de verificação"). Aqui "PASS" significa que o teste reproduz o comportamento descrito pelo oráculo (código C++ lido ou issue do AppleWin); nenhum item tem comparação diferencial automática contra o binário do AppleWin (não existe ainda, ver §3.2).
- Prioridades: P0 bloqueia a afirmação de paridade; P1 diverge do uso comum; P2 divergência localizada; P3 detalhe ou extensão.

## 1. Resumo das hipóteses

| Hipótese do plano | Veredito |
|---|---|
| Interrupções do Z80 incompletas | Confirmada no Rust, mas o AppleWin também não entrega IRQ/NMI (`Z80VICE/z80.cpp:470`, `:5540`): paridade com o oráculo, lacuna contra a HU-0601 (AUD-P-011) |
| MEMPTR em INIR/OTIR incompleto | Parcialmente refutada: o MEMPTR de INI/OUTI/INIR/OTIR está certo e o AppleWin nem o modela. Faltam só os efeitos de F3/F5 (e PF/HF) quando o bloco repete (AUD-P-014) |
| Uthernet II parcial | Confirmada, mas SPI, PPPoE e interrupções também não existem no AppleWin; o Rust acrescenta LISTEN e UDP com porta (AUD-P-021) |
| VidHD sem 80x45 nem cor de borda | Refutada como defeito: o AppleWin também não suporta (`VidHD.cpp:27-33`) (AUD-P-004) |
| ThunderClock sem conexão com a UI | Confirmada, mas o AppleWin também não a expõe (`Card.h` sem `CT_ThunderClock`, esqueleto comentado) (AUD-P-015) |
| Atalhos do depurador ausentes | Confirmada, e é mais grave do que o manual diz: até Espaço, Tab e Home/End faltam, e Up/Down têm outro significado (AUD-P-035, 036) |
| Breakpoint Card sem ligação com o depurador | Confirmada (AUD-P-016) |
| SNES MAX: flags sem efeito | Confirmada, e pior: nenhum controle chega à carta (AUD-P-017) |

## 2. Tabela de achados

| ID | Área | HU | Hipótese/achado | Estado | Evidência | Prioridade | Correção proposta |
|---|---|---|---|---|---|---|---|
| AUD-P-001 | Vídeo | 0201 | #1443 (PAL Color TV deve matar o color-burst por mais tempo) está no port | IMPLEMENTED (sem diferencial) | C++ `NTSC.cpp:2072-2078`; Rust `renderer.rs:603-609` (`pal_mid_line_fixups`: `!= ColorTv` antes de zerar `color_burst`), `:765-775` | P2 | Teste diferencial com a demo Part-II (HGR/TEXT por linha) em PAL Color TV |
| AUD-P-002 | Vídeo | 0201 | Verificação NTSC sem oráculo pixel a pixel (`docs/pendencias.md`, "Verificação"); os testes `crates/apple2-video/tests/composite.rs` e `modes.rs` foram escritos a partir do código | UNVERIFIED | `crates/apple2-core/tests/scanner_ref/mod.rs:1` copia só as tabelas do C++; não há frames de referência | P0 | Construir o oráculo do §3.2 e comparar |
| AUD-P-003 | Vídeo | 0201 | Flash (16 quadros), mixed mode, page flipping e bordas existem e têm testes de auto-consistência | IMPLEMENTED | Flash: C++ `NTSC.cpp:437-446`, Rust `renderer.rs:820-826`; `tests/modes.rs:59,165,151`; bordas 20x18: `renderer.rs:27-28` | P1 | Idem AUD-P-002; incluir flash com mudança de modo no meio do quadro |
| AUD-P-004 | Vídeo | 0202 | VidHD sem texto 80x45 e sem cor de borda | NOT_APPLICABLE (paridade) | C++ `VidHD.cpp:27-33` lista os dois como "Not supported yet"; Rust `vidhd.rs:12-13` | P3 | Registrar como limitação comum; sem teste `FAIL` porque o oráculo também não os desenha |
| AUD-P-005 | Vídeo | 0201/0202 | O estado da placa RGB (AppleColor, Video-7, Feline) não entra no save-state: o Rust grava zeros em `AppleColor RGB Adaptor` e perde `RGB mode`, `Previous AN3`, `Invert bit7` ao carregar | FAIL | C++ `RGBMonitor.cpp:1311-1340` (`RGB_SaveSnapshot`); Rust `apple2-savestate/src/apple2.rs:231-238` ("not modelled") | P2 | Guardar `rgb_flags`, `rgb_mode`, `prev_an3` e `invert_bit7` do `Renderer` no save-state e aplicá-los no load |
| AUD-P-006 | Mockingboard | 0501 | Um tipo de AY por carta: os soquetes 1, 2 e 3 do AY (Phasor tem 4) e `-sN ay-socketN` são descartados | FAIL | C++ `Mockingboard.cpp:136-151` (`SetSocketAY891x`), `CmdLine.cpp:277-296`; Rust `factory.rs:70-77` (`sockets()` usa só `ay[0]`), `mockingboard.rs:27-35` (`Sockets.ay` único). Teste `aud_p_006_*` (ignorado) | P2 | `Sockets.ay: [AyType; 4]`, uma instância de `Ay` por soquete; propagar `-sN ay-socketN` |
| AUD-P-007 | Mockingboard | 0501 | `6522a-bad`/`6522b-bad` são lidos da linha de comando e descartados; o DDRB não perde o bit 0 | FAIL | C++ `6522.cpp:355` (`DDRB &= ~1`), `CmdLine.cpp:254-260`; Rust `cmdline.rs:532-533`, `card.rs:204`, `factory.rs:70-77`, `via6522.rs:262` (DDRB devolve `self.ddrb`). Teste `aud_p_007_*` (ignorado) | P3 | `Via::bad` e `Sockets.bad_6522` |
| AUD-P-008 | SSI263 | 0502 | #1412 (power-down não silenciava fonema contínuo) | PASS | C++ `SSI263.cpp:213-221`; Rust `ssi263.rs:236-241`, `:409-414`; teste `aud_p_ssi263_power_down_silences_continuous_phoneme_gh1412` | P2 | Nenhuma |
| AUD-P-009 | SSI263/SC-01 | 0502 | #1372 (fonema contínuo não volta após carregar o save-state) | PASS (nível do chip) | C++ `SSI263.cpp:1085-1100` (`RepeatPhoneme`); Rust `ssi263.rs:518-565` (`needs_restart`/`restart`); teste `aud_p_ssi263_snapshot_restarts_continuous_phoneme_gh1372`. O encadeamento no cartão (`rearm_events`, `mockingboard.rs:660`) tem teste em `apple2-audio/tests/machine.rs:495` | P2 | Nenhuma; cobrir também o caso com SC-01 |
| AUD-P-010 | Som | 0501/0502 | Registradores, IRQ do 6522 e PCM sem comparação com o C++: o `Via` é um port fiel (`via6522.rs:250-312` equivale a `6522.cpp:338-400`), mas os testes são do próprio código; ritmo/prosódia da fala (taxa fixa, sem inflexão) | UNVERIFIED | `tests/machine.rs` (apple2-audio) | P1 | Gravar PCM do AppleWin (`-wav-mockingboard`) para um programa de teste e comparar eventos/frequências com tolerância de resampling |
| AUD-P-011 | Z80 | 0601 | IM 0/1/2, NMI, IRQ e HALT: `Z80` não tem entrada de interrupção; EI/DI/IM só mudam flags; HALT executa NOPs | FAIL (contra a HU) / paridade com o oráculo | C++ `Z80VICE/z80.cpp:470-519` (`DO_INTERRUPT` desativado) e `:5540`; Rust `z80.rs:1-3`, `:458-461`, `:814-820`, `:993`; `softcard.rs:14` | P2 | Se a HU exigir IRQ, acrescentar `Z80::irq/nmi` e a linha IRQ da carta como divergência intencional documentada |
| AUD-P-012 | Z80 | 0601 | MEMPTR em INI/OUTI/INIR/OTIR (WZ = BC+1, antes ou depois do decremento de B) e em repetição (WZ = PC+1) | PASS (contra a especificação, não contra o AppleWin) | Rust `z80.rs:1085-1112`; testes `aud_p_z80_ini_*`, `aud_p_z80_outi_*`, `aud_p_z80_repeating_block_*` | P3 | Nenhuma |
| AUD-P-013 | Z80 | 0601 | Opcodes ED indefinidos são NOP de 8 T-states sem nenhum relato (a HU pede relato explícito) | FAIL (contra a HU) | `z80.rs:1026-1031` (`_ => 8`); teste `aud_p_z80_undefined_ed_opcode_is_a_silent_8_tstate_nop` documenta | P3 | Contador/log de opcodes indefinidos; o NOP é o comportamento do hardware |
| AUD-P-014 | Z80 | 0601 | LDIR/CPIR/INIR/OTIR repetindo: o hardware copia F3/F5 do byte alto do PC (e ajusta PF/HF em INIR/OTIR); o Rust mantém a fórmula da última iteração | FAIL (contra o hardware; o AppleWin também não faz) | `z80.rs:1049-1111`; testes ignorados `aud_p_z80_ldir_repeat_takes_flags_3_5_from_pch`, `aud_p_z80_inir_otir_repeat_takes_flags_3_5_from_pch` | P3 | Divergência intencional só com evidência de hardware (ZEXALL/ `z80test`); caso contrário, registrar como paridade com o AppleWin |
| AUD-P-015 | Cartas | 0602 | ThunderClock Plus não é selecionável (sem `CardType`, nem na fábrica, nem na UI) | NOT_APPLICABLE vs oráculo; FAIL vs HU ("relógio selecionável") | `apple2-cards/src/thunderclock.rs`; `factory.rs` não o cita; C++ `Card.h:15-42` sem `CT_ThunderClock`, `Peripheral_Clock_ThunderClockPlus.cpp` é esqueleto. Teste `aud_p_015_thunderclock_has_no_card_type_like_applewin` | P3 | Decidir se é extensão: novo `CardType`, nome de save-state próprio (fora do formato AppleWin) |
| AUD-P-016 | Cartas | 0602 | Breakpoint Card existe, mas o depurador não lhe entrega breakpoints | FAIL | C++ `Debug.cpp:368,9185-9186` (`InterceptBreakpoints` + `g_InterceptBreakpointsCB`); Rust `breakpoint.rs:60-71` (`intercepting`, `report`) sem chamadores fora do próprio arquivo | P2 | Ligar `Debugger` à carta (`report` ao disparar breakpoint com `intercepting`) |
| AUD-P-017 | Cartas | 0602 | SNES MAX: nenhum controle do host chega à carta (`SnesMax::set_controller` nunca é chamado) e `-snes-max-alt-joy*`/`-user-joy*` são ignorados pela fábrica | FAIL | C++ `SNESMAX.cpp:110-170`, `CmdLine.cpp:727-743`; Rust `factory.rs:293` (`SnesMax { .. }`), `snesmax.rs:58`. Teste `aud_p_017_*` (ignorado) | P2 | Alimentar a carta pelo `gilrs` do frontend e ler o YAML de mapeamento |
| AUD-P-018 | Cartas | 0602/0702 | 4Play: `FourPlay::set_stick` nunca é chamado; a carta sempre lê "sem direção" | FAIL | `fourplay.rs:42`; `factory.rs:292`. Teste `aud_p_018_*` (ignorado) | P2 | Idem AUD-P-017 |
| AUD-P-019 | Cartas | 0602 | Conflitos de slot só são validados no diálogo (`settings/model.rs:600-690`); `Emulator::new` aceita dois Mouse Interface e Uthernet + Uthernet II sem erro nem aviso | FAIL | Teste `aud_p_019_*` (ignorado) mostra `Emulator::new` com sucesso; C++ `CardManager.cpp:65-114` (`_ASSERT(m_pMouseCard == NULL)` etc.) | P2 | Chamar `validate` em `Emulator::new` e no carregamento do TOML/linha de comando |
| AUD-P-020 | Cartas | 0602 | Cartão inválido no slot 0 do ][+ (por exemplo SSC) some em silêncio | FAIL | Teste `aud_p_020_*` (ignorado); C++ `CardManager.cpp:123-131` | P3 | Avisar em `Emulator::new` |
| AUD-P-021 | Uthernet II | 0701 | Matriz abaixo (§4.5): TCP cliente/servidor, UDP, IP RAW, MAC RAW, DNS virtual implementados; SPI, PPPoE, interrupções ausentes; modos desconhecidos (inclusive PPPoE, MR=5) são ignorados sem log | NOT_APPLICABLE vs oráculo (o AppleWin também não tem); FAIL vs HU ("falhar de forma controlada e documentada") | C++ `Uthernet2.cpp:405-440` (só loga sob `U2_LOG_UNKNOWN`), `W5100.h:51`; Rust `uthernet2.rs:680-713` (`_ => {}`) | P3 | Registrar aviso em `warnings` para modos não suportados; documentar |
| AUD-P-022 | Uthernet II | 0701 | Sockets TCP/UDP fechados ao carregar save-state; RAW restaurados | PASS (paridade) | C++ `Uthernet2.cpp:337-362`; Rust `uthernet2.rs:308-368`; teste `save_state_keeps_registers_and_raw_sockets` | P3 | Documentar no manual (já consta em `12-diferencas`) |
| AUD-P-023 | Uthernet I/II | 0701/0703 | Falha ao abrir a interface pcap (permissão, nome errado) é engolida: `PcapNetwork::open` encadeia `.ok()`; a fábrica só avisa quando o recurso `pcap` está desligado | FAIL | `pcap_backend.rs:26-44`; `factory.rs:158-166`; `NetworkBackend::is_valid` existe mas ninguém o consulta no frontend | P2 | Consultar `is_valid()` em `network_backend` e emitir `warnings` com o motivo (Npcap/libpcap ausente, sem permissão) |
| AUD-P-024 | Uthernet | 0701 | Reconexão, timeouts, pacotes malformados e múltiplas sessões sem teste; servidor TCP "com pouco teste" (manual) | UNVERIFIED | `tests/uthernet2.rs` cobre eco, queda remota, falha de conexão e LISTEN simples; nada de pacote malformado ou 4 sockets | P2 | Testes de loopback com 4 sockets simultâneos, quadros truncados e cliente que cai no meio |
| AUD-P-025 | Entrada | 0702 | #1522 (PREAD dá $FF com o mouse como joystick no início) | PASS | Fix AppleWin 1.32.1.0; Rust `joy.rs:450` (`mouse_pos: (CENTRE, CENTRE)`); teste `aud_p_025_mouse_joystick_starts_centred_gh1522` | P2 | Nenhuma |
| AUD-P-026 | Entrada | 0702 | #1234 (botões sem controle flutuam em nível alto) | PASS | C++ `Joystick.cpp:622-660`; Rust `game_port.rs:46-62`; teste `aud_p_026_disconnected_buttons_float_high_gh1234` | P2 | Nenhuma; falta o teste do firmware do HDD com PB2 |
| AUD-P-027 | Entrada | 0702 | "SHIFT key mod": o Rust marca o joystick 1 como presente, então o tempo dos paddles 2/3 é o de um joystick conectado; no AppleWin continua "sempre ativo" | FAIL | C++ `Joystick.cpp:648-653`, `:695-697`; Rust `joy.rs:640-646`, `game_port.rs:73-75`. Teste `aud_p_027_*` (ignorado). Já listado em `12-diferencas` | P3 | Separar "PB2 dirigido pelo Shift" de "joystick 1 presente" |
| AUD-P-028 | Entrada | 0702 | Tecla presa: ao entrar no depurador (F7) ou quando o egui consome o teclado, o evento de soltar a tecla não chega a `KeyboardState`/`JoySystem`; `AnyKeyDown` e a direção do joystick de teclado ficam ligados até a próxima tecla. Alternar tela cheia não chama `release_inputs` | FAIL (por leitura de código; sem reprodutor automatizado) | `app.rs:410-420` (depurador ativo devolve antes do `kbd.handle`), `app.rs:749-765` (`egui_wants_keys` descarta `KeyboardInput`), `app.rs:802` (só `Focused(false)` libera); `keymap.rs:204` (`release_all`) | P1 | Chamar `release_inputs()` ao entrar no depurador, ao abrir/fechar painéis egui e ao mudar de tela cheia; reprodutor manual no §5 |
| AUD-P-029 | Entrada | 0702 | Gamepads físicos, captura de mouse e calibração por SO | BLOCKED | Sem hardware nem Xvfb com `/dev/input` neste ambiente | P2 | Matriz manual Windows/macOS/Linux com um controle real |
| AUD-P-030 | Serial | 0703 | Dispositivo inexistente não derruba o programa (PASS), mas o erro de abertura (permissão negada, porta ocupada) é engolido: `serialport::new(..).open().ok()` e o SSC só vê `false` | PASS (sem crash) / FAIL (mensagem) | `serial.rs:394-404`; teste `aud_p_030_missing_serial_device_does_not_crash` | P2 | Guardar o `serialport::Error` e mostrá-lo (mensagem de status + log) |
| AUD-P-031 | Serial | 0703 | Detecção de portas: no Linux varre `/dev` por `ttyUSB*/ttyACM*/cu.*/tty.usb*` (sem `ttyS*`/`rfcomm*`); no Windows devolve COM1 a COM9 fixos; `serialport::available_ports` (já presente em `serial.rs:339`, `list_serial_ports`) não é usado | FAIL | `settings/mod.rs:99-118` | P2 | Usar `list_serial_ports()` quando o recurso `serialport` está ligado |
| AUD-P-032 | Rede | 0703 | Lista de interfaces: lê `/sys/class/net` (só Linux; vazia em macOS/Windows), sem descrição; `pcap_backend::list_interfaces` não é usado pela UI | FAIL | `settings/mod.rs:120-132`, `pcap_backend.rs:9` | P2 | Com `pcap`: `list_interfaces()`; sem `pcap`: lista vazia com aviso |
| AUD-P-033 | Impressora | 0703 | O destino é o caminho cru do usuário (relativo ao diretório atual), sem canonização nem sanitização; falha de abertura (`FileSink::open`) e de escrita é descartada em silêncio. O AppleWin também grava onde mandam | FAIL (erros invisíveis) / paridade (caminho) | `printer.rs:42-56`, `:268-273`; `factory.rs:115-124` | P3 | Relatar a falha na barra de status; opcionalmente recusar caminhos fora do diretório de trabalho (extensão) |
| AUD-P-034 | Dependências | 0703 | Recursos externos são opt-in: `pcap` desligado por padrão; `serialport` ligado por padrão no frontend, desligado na biblioteca | PASS | `apple2-hostio/Cargo.toml:9-20`; `applewin/Cargo.toml:17-22` | P3 | Nenhuma |
| AUD-P-035 | Depurador | 0801 | Atalhos de teclado ausentes: Espaço (trace), Shift/Ctrl+Espaço (step out/over), Tab, Home/End, setas Esquerda/Direita (com Shift/Ctrl/Alt), Ctrl+0-9 (marcadores), Enter vazio (console cheio), Ctrl+~ | FAIL | C++ `Debug.cpp:9653-10122`; Rust `keymap.rs:148-163`. Testes `aud_p_035_*` (ignorados). Tabela completa em `debugger-commands.md` | P1 | Estender `DebugKey`/`debug_key` e chamar as funções que já existem em `cmd_exec.rs` |
| AUD-P-036 | Depurador | 0801 | Up/Down viram histórico de comandos e PgUp/PgDn rolam o console, no AppleWin movem o cursor do disassembly (o console só rola com texto digitado) | FAIL (diverge) | `keymap.rs:156-160`; C++ `Debug.cpp:9987-10030` | P2 | Seguir o comportamento do AppleWin |
| AUD-P-037 | Depurador | 0801 | #1460, #1456, #1451: as funções existem (`cursor_follow_target` com RTS/RTI e volta da pilha; `=` atualiza alvos), mas a tecla Seta-direita não existe | PASS (função) / FAIL (tecla) | `cmd_exec.rs:46-65`, `disasm.rs:353-377`; testes `aud_p_037_*`; C++ `Debug.cpp` (`CmdCursorFollowTarget` em `Debugger_Commands.cpp:3273`) | P1 | Ver AUD-P-035 |
| AUD-P-038 | Depurador | 0801 | Comandos "não disponíveis": BENCHMARK/BENCH, FONT, HCOLOR, DISK, NTSC, SOURCE, SYNC imprimem aviso; BPEDIT não faz nada; EXITBENCH não existe | FAIL | `commands.rs:306,311,314,315,344,373,382` | P2 | Ligar o `Host` do depurador ao frontend para DISK/NTSC/FONT/HCOLOR; o resto documentar |
| AUD-P-039 | Depurador | 0801 | Renderizar a tela do depurador não altera o emulador: `Debugger::render(&mut self, &Machine)` usa só `peek` | PASS | `display.rs:121`, `mem.rs:23-29`; teste `aud_p_039_rendering_the_debugger_screen_does_not_touch_the_machine` | P2 | Nenhuma |
| AUD-P-040 | UI | 0802 | Ctrl+Print Screen copia só 40 colunas, página 1 (`apple2_core::text_screen`); o AppleWin copia 40 ou 80 colunas (aux+principal) e a página 2. O crate de vídeo já tem `screen_text` (80 colunas, página 2) sem uso no frontend | FAIL | C++ `Debug.cpp:5271-5320`, `WinFrame.cpp:1221-1232`; Rust `emu.rs:540-545`, `text_screen.rs:37`, `apple2-video/src/screen_text.rs:10`. Teste `aud_p_040_*` (ignorado) | P2 | Usar `screen_text` (e a regra de página 2/80STORE) em `Command::CopyScreenText` |
| AUD-P-041 | UI | 0802 | Soltar arquivo na janela: sempre drive 1 e reinicia a máquina; não há escolha de drive nem "soltar sobre o botão" | FAIL (difere) | `app.rs:818-824` | P3 | Soltar sobre o botão do drive; não reiniciar |
| AUD-P-042 | UI | 0802 | Não há "Open as Read Only" no diálogo de montagem (só "Write protected" no menu do drive) | FAIL (não existe) | `disk_ui.rs:185-187` | P3 | Caixa "Somente leitura" no seletor |
| AUD-P-043 | UI | 0802 | Monitor de setores ("S??" ao lado da trilha) não existe | FAIL (não existe) | `docs/pendencias.md`; status de trilha em `overlay.rs` | P3 | Expor o setor atual do `Disk2Card` |
| AUD-P-044 | UI | 0802 | "Benchmark Emulator" existe como botão desativado | FAIL (não existe) | `settings/pages.rs:538-540` | P3 | Reaproveitar `tests/perf` ou o modo `--headless-frames` |
| AUD-P-045 | UI | 0802 | Só há volume mestre; `CardVolume` existe no áudio mas a UI não expõe volumes independentes (Mockingboard, alto-falante) | FAIL (não existe) | `settings/pages.rs:136-163`, `apple2-audio/src/bus.rs` | P3 | Sliders por carta |
| AUD-P-046 | UI | 0802 | A opção "Alt+Enter toggles full-screen" (`-alt-enter=...`) aparece na UI e na linha de comando, mas nenhum código a consulta: Alt+Enter sempre alterna a tela cheia | FAIL | `actions.rs:73`; `settings/pages.rs:224-225`; `cmdline.rs:699`. Teste `aud_p_046_*` (ignorado) | P2 | Passar a preferência a `action_for`; no modo `open-apple-enter`, Alt+Enter vira Open Apple + Enter |
| AUD-P-047 | UI | 0802 | Importar `.reg`/INI do AppleWin só por `-conf`; sem comando na UI | FAIL (não existe) | `apple2-config/src/import.rs`; `docs/pendencias.md` | P3 | Botão "Import AppleWin settings…" |
| AUD-P-048 | UI | 0802 | Trocar imagem de HDD na configuração reinicia a máquina; Disk II pela configuração só vale depois de reiniciar | FAIL (difere) | `disk_ui.rs:293-345`; `docs/pendencias.md` | P3 | Hot-plug no thread de emulação |
| AUD-P-049 | UI | 0802 | Atalhos F1-F12, Print Screen e Pause listados na documentação têm ação | PASS | `actions.rs:56-87`; teste `aud_p_049_listed_shortcuts_have_actions` | P3 | Nenhuma |
| AUD-P-050 | Vídeo | 0202 | VidHD sobrevive a save/load (registradores `Memory Mode`, `Screen Color`, `New Video`, `Border Color`, `Shadow`; memória auxiliar do II/II+) | IMPLEMENTED (sem teste de imagem) | C++ `VidHD.cpp:179-240`; Rust `vidhd.rs:63-90`, `apple2-savestate/src/cards/vidhd.rs`; `tests/cards_peripherals.rs:423-470` | P2 | Teste de ponta a ponta: SHR, salvar, carregar, comparar o quadro |

## 3. Vídeo

### 3.1 O que existe

- Modos: texto 40/80, lores/dlores, hires/dhires, misto, páginas 1/2, SHR com VidHD, placas RGB (Video-7 SL7, Le Chat Mauve Eve/Feline). Tipos de vídeo: os 9 de `VideoType` (`config.rs:8-23`).
- Testes: `crates/apple2-video/tests/{modes,composite,rgb_card,machine,models,vidhd}.rs` e `crates/apple2-core/tests/video_scanner.rs`. Todos usam tabelas copiadas do C++ ou resultados escritos à mão; nenhum compara com quadros do AppleWin.
- #1443: o AppleWin corrigiu em 2025-10-20 (Color TV PAL não zera o color-burst ao passar para texto). O port tem a mesma condição (`renderer.rs:605-609`). O que falta é o teste com a demo (Part-II, "Homer choking Homer"), que o AppleWin descreve na issue: esperado, em NTSC e PAL Color TV, o lado HGR em P&B quando o texto vem depois e colorido quando o texto vem antes (hpos 23 de cada linha).

### 3.2 Métrica e procedimento propostos para o oráculo NTSC

O pipeline NTSC do AppleWin é de aritmética inteira e determinístico (tabelas `g_aClockVertOffsets`, filtro FIR em ponto fixo). Por isso o alvo é **igualdade bit a bit** no buffer lógico; MAE e SSIM entram só como diagnóstico quando a igualdade falhar ou para os pós-processamentos que dependem do tipo de vídeo (scanlines, blend vertical).

1. **Oráculo**: compilar `NTSC.cpp`, `NTSC_CharSet.cpp`, `RGBMonitor.cpp` e `Video.cpp` do `a8f0d1d` com um stub de `Memory`/`Core` (g++ está disponível no ambiente) num executável que lê um arquivo de cena (bytes de $0400-$BFFF main/aux, flags de vídeo, tipo de vídeo, 50/60 Hz, ciclos por quadro) e escreve o framebuffer de 600x420 em BGRA (560x384 mais bordas de 20 e 18 pixels).
2. **Cenas** (cada uma com SHA-256 do arquivo): texto 40/80 com flash em quadros 0 e 16; lores e dlores; hires com bit 7 alternado (cores de artefato); dhires 140/560; misto; página 1/2; cada transição de modo no meio da linha (hpos 12-16 e 23-25, #1443 e #1131); linha 0 e linhas 160-191; SHR com VidHD (framebuffer 640x400); Video-7 e Feline (modos 160/560/mixed).
3. **Alinhamento temporal**: mesmo contador de ciclos no início do quadro (`Machine::cycles()` % ciclos por quadro) e mesma contagem de quadros (o flash depende dela).
4. **Métrica**:
   - modos sem simulação analógica (monocromo, `ColorIdealized`, RGB card, texto, SHR): diferença máxima de canal = 0;
   - `ColorMonitorNtsc` e `ColorTv`: diferença máxima de canal = 0 em todo o quadro; se houver divergência, relatar MAE por canal e por região (bordas de 20 e 18 pixels, primeira e última coluna de célula, linha 0, junção do modo misto);
   - pós-processamento vertical (scanlines, blend): SSIM >= 0,999 e MAE <= 1/255 por canal apenas se o port usar aritmética diferente (a definir quando a igualdade bit a bit falhar).
5. **Limiar de reprovação**: qualquer diferença nos modos determinísticos é FAIL; nos demais, MAE > 1/255 ou SSIM < 0,999.
6. **Saída**: `docs/conformance/evidence/video/<cena>.json` (cena, SHA do AppleWin, SHA do Rust, MAE por canal, máximo, SSIM, caminho do PNG de diferença).

### 3.3 VidHD

Modos do AppleWin: SHR 320x200 e 640x200 de 256 cores e os modos herdados centralizados (`Video.cpp:96-137`). Não suportados lá: texto 80x45/120x67/240x135, cores de primeiro plano/fundo, cor de tela e borda do IIgs, DHGR P&B e a chave liga/desliga (`VidHD.cpp:27-33`). O port tem as mesmas lacunas (AUD-P-004). Save/load: registradores e memória auxiliar são gravados e lidos (AUD-P-050).

## 4. Som, Z80 e cartas

### 4.1 Comparação de registradores do 6522

`Via::read` (`via6522.rs:250-312`) é transcrição linha a linha de `SY6522::Read` (`6522.cpp:338-400`): IRB/IRA com `DDR ^ 0xFF` (GH#1260), MegaAudio devolve 0, IER com bit 7 (GH#567), IFR com o underflow pendente. A única diferença é o `m_bad6522` (AUD-P-007).

### 4.2 Phasor e sockets

`Sockets { ssi263: [SsiType; 2], sc01: bool, ay: AyType }` (`mockingboard.rs:29-35`): um tipo de AY para as 4 posições (AUD-P-006). Os soquetes 3 e 4 do manual são os índices 2 e 3.

### 4.3 Z80

Não há entrada de interrupção. `Z80::step` trata HALT como NOP de 4 T-states (`z80.rs:458-461`), como o Z80VICE do AppleWin. Sessão CP/M com disco: UNVERIFIED (nenhuma imagem CP/M legal no repositório).

### 4.4 ThunderClock, Breakpoint Card, SNES MAX, slots

- ThunderClock: AUD-P-015. Breakpoint Card: AUD-P-016. SNES MAX: AUD-P-017. 4Play: AUD-P-018. Conflitos: AUD-P-019/020.
- A validação de slots (`validate`) recusa: slot 0 com cartão não permitido, cartas de instância única repetidas, Uthernet + Uthernet II, joystick por mouse com Mouse Interface, F8 ROM em modelo errado. Tudo isso só existe no caminho do diálogo.

### 4.5 Matriz Uthernet

| Recurso | Uthernet I (CS8900A) | Uthernet II (W5100) | AppleWin |
|---|---|---|---|
| Interface SPI | n/a | ausente | ausente |
| Interrupções | ausente | ausente | ausente |
| TCP cliente | n/a | implementado (thread auxiliar) | implementado |
| TCP servidor (LISTEN) | n/a | implementado (extensão) | ausente |
| UDP | n/a | implementado (porta pedida: extensão) | implementado (porta efêmera) |
| IP RAW / MAC RAW | via backend | implementado | implementado |
| PPPoE | n/a | ignorado sem log | ignorado (log opcional) |
| DNS virtual | n/a | implementado | implementado |
| Reconexão automática | n/a | ausente | ausente |
| Save-state: TCP/UDP | n/a | fechados ao carregar | fechados ao carregar |
| Save-state: RAW | n/a | restaurados | restaurados |
| Rede real | `pcap` (opt-in) | `pcap` (opt-in); TCP/UDP usam sockets do host | pcap/Npcap |

## 5. Entrada, serial, impressora e rede do host

- Reprodutor manual de AUD-P-028 (tecla presa): (1) iniciar o emulador; (2) segurar uma tecla de letra; (3) apertar F7 com a tecla ainda pressionada; (4) soltar a letra; (5) apertar F7 de novo. Uma leitura de `$C010` continua devolvendo o bit 7 (any-key-down) ligado, sem nenhuma tecla pressionada. Variante: com o joystick no teclado numérico, segurar uma direção, abrir F8 e soltar.
- Para ver o erro engolido de AUD-P-030: `applewin` com `-s2 ssc` e a porta `/dev/ttyS0` sem permissão; o SSC fica sem porta e nenhuma mensagem aparece.

## 6. UI

Tabela item por item (existe, não existe ou difere):

| Item | Estado | Evidência |
|---|---|---|
| Montar disco por diálogo | existe | `disk_ui.rs` (menu Ctrl+F3/F4, `rfd`) |
| Montar disco arrastando | difere (sempre drive 1 e reinicia) | `app.rs:818-824` |
| Somente leitura ao abrir | não existe (só proteção contra escrita depois) | `disk_ui.rs:185` |
| Ctrl+Print Screen 40/80 colunas | difere (só 40, página 1) | AUD-P-040 |
| Monitor de setores | não existe | AUD-P-043 |
| Benchmark | não existe (botão desativado) | AUD-P-044 |
| Volumes independentes | não existe | AUD-P-045 |
| Alt+Enter | existe, mas a preferência não vale | AUD-P-046 |
| Lista de interfaces de rede | difere (só Linux, sem descrição) | AUD-P-032 |
| Lista de portas seriais | difere (heurística de `/dev`; COM1-9 fixos) | AUD-P-031 |
| Importar configuração | não existe na UI (existe por `-conf`) | AUD-P-047 |
| Preferência do save-state (`Save State On Exit`, nome padrão) | existe; o nome não acompanha o disco durante a sessão | `12-diferencas` |

## 7. Como reproduzir

```
cargo test -p apple2-audio  --test audit_wave0_ssi263
cargo test -p apple2-cards  --test audit_wave0_z80 -- --include-ignored
cargo test -p apple2-debugger --test audit_wave0_debugger
cargo test -p applewin --test audit_wave0_peripherals -- --include-ignored
```

Os testes `#[ignore = "reproduz achado AUD-P-NNN"]` falham hoje de propósito. Quando um achado for corrigido, remover o `#[ignore]` no mesmo commit.
