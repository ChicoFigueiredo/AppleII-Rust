# Debugger: comandos, atalhos e estado do port (wave0)

Oráculo: AppleWin `a8f0d1d`, `source/Debugger/Debugger_Commands.cpp:40-403` (`g_aCommands[]`) e `source/Debugger/Debug.cpp:9653-10122` (`DebuggerProcessKey`).
Port: `crates/apple2-debugger/src/cmdtable.rs` (tabela gerada), `commands.rs:260` (`dispatch`), `crates/apple2-rust/src/keymap.rs:148` (`debug_key`), `crates/apple2-rust/src/emu.rs:626` (`Command::Debug`).
Estados: IMPLEMENTED (código existe), PASS (teste concorda com o oráculo), FAIL (discorda ou ausente), N/A (o AppleWin também não implementa). Nenhum comando está PASS por teste diferencial contra o C++: os testes existentes (`crates/apple2-debugger/tests/*.rs`) foram escritos a partir do código.

## 1. Atalhos de teclado (`DebuggerProcessKey`)

| Tecla (AppleWin) | Ação no AppleWin (Debug.cpp) | Estado no port | Evidência no port | ID |
|---|---|---|---|---|
| Espaço (linha vazia) | Trace: `CmdTrace` (:9848-9867) | PASS | `keys.rs` `process_key`; `aud_p_035_space_traces_one_instruction_in_the_debugger`, `space_family_traces_steps_out_and_steps_over` | AUD-P-035 |
| Espaço (com texto ou montador) | Insere o espaço na linha (`DebuggerInputConsoleChar`, :9544-9550) | PASS | `space_is_text_only_when_the_line_is_not_empty` | AUD-P-035 |
| Shift+Espaço | Step out: `CmdStepOut` (:9859) | PASS | `space_family_traces_steps_out_and_steps_over` | AUD-P-035 |
| Ctrl+Espaço | Step over: `CmdStepOver` (:9862) | PASS | idem | AUD-P-035 |
| Tab (linha vazia) | `CmdCursorJumpPC(CENTER)` (:9845); com texto: completar (`ConsoleInputTabCompletion` é um stub vazio); Ctrl+Tab / Ctrl+Shift+Tab: ciclar janelas | PASS | `arrows_tab_and_alignment`, `ctrl_digits_are_bookmarks_and_ctrl_tilde_toggles_the_console` (o ciclo percorre código, dados e console: `WINDOW_INFO`/`WINDOW_NULL` não têm o que mostrar) | AUD-P-035 |
| Seta-direita | `CmdCursorFollowTarget(TOP)` (:10059); segue RTS/RTI (#1456) e salto | PASS | `Debugger::cursor_follow_target_at`; `aud_p_037_follow_target_on_rts_wraps_the_stack_page_gh1456`, `arrows_tab_and_alignment` | AUD-P-037 |
| Ctrl+Seta-direita | `CmdCursorSetPC` (:10051); #1460 | PASS | `aud_p_037_set_pc_from_cursor_updates_targets_gh1460`, `aud_p_035_navigation_keys_move_the_cursor` | AUD-P-037 |
| Shift+Seta-direita | `CmdCursorJumpPC(TOP)` (:10054) | PASS | `arrows_tab_and_alignment` | AUD-P-035 |
| Alt+Seta-direita | `CmdCursorJumpPC(CENTER)` (:10057) | PASS | `aud_p_035_navigation_keys_move_the_cursor` | AUD-P-035 |
| Seta-esquerda / Shift+Seta-esquerda | `CmdCursorJumpRetAddr` (centro / topo) (:10062-10066) | PASS | `arrows_tab_and_alignment` | AUD-P-035 |
| Seta-cima / Seta-baixo (linha vazia) | Linha acima/abaixo no disassembly (`CmdCursorLineUp/Down`); Shift = 1 byte; Ctrl+Baixo = `CmdCursorRunUntil` (:9989-10047) | PASS | `up_and_down_move_the_cursor_and_ctrl_down_runs_until_it`, `aud_p_036_existing_debug_keys` | AUD-P-036 |
| Seta-cima / Seta-baixo (janela do console, ou com texto + Shift) | Rola o console 1 linha (`ConsoleScrollUp/Dn(1)`); com texto sem Shift não faz nada (o AppleWin tem "TODO: FIXME: Scroll through console input history", então o histórico por setas deixou de existir) | PASS | `console_window_keys_only_scroll` | AUD-P-036 |
| PgUp/PgDn | Cursor por página; Shift = 256 bytes, Ctrl = 4K; com texto: rolar 3 linhas do console; Shift+texto ou janela do console: página do console (:9929-9987) | PASS | `page_keys_move_the_cursor_or_scroll_the_console` | AUD-P-036 |
| Home / End | Cursor em $0000 / $8000; na janela do console ou Shift+texto: rolar ao início/fim (:9869-9927) | PASS | `home_and_end_go_to_zero_and_8000`, `console_window_keys_only_scroll` | AUD-P-035 |
| Enter (vazio) | Alterna console em tela cheia; no montador sai dele (:9788-9799) | PASS | `aud_p_035_empty_enter_toggles_the_full_screen_console` | AUD-P-035 |
| Enter (com texto) | Executa o comando | PASS | `crates/apple2-rust/tests/debugger.rs` |  |
| Backspace / Esc | Apagar caractere / limpar linha (Esc com paginação do console: pula) | IMPLEMENTED | `keys.rs`. A paginação de saída longa (`g_nConsoleBuffer`, `ConsoleBufferTryUnpause`) não existe no port: o console é um buffer rolável e nada fica retido | |
| ~ | Insere "~" (:9822) | PASS | `ctrl_digits_are_bookmarks_and_ctrl_tilde_toggles_the_console` | AUD-P-035 |
| Ctrl+~ | Console em tela cheia (:9816) | PASS | idem | AUD-P-035 |
| Ctrl+0..9 / Ctrl+Shift+0..9 | Ir ao marcador / criar marcador (`CmdBookmarkGoto/Add`) (:10069-10090) | PASS | `ctrl_digits_are_bookmarks_and_ctrl_tilde_toggles_the_console`; `actions::action_for_mode` impede que Ctrl+0/1/3 virem atalhos de velocidade com o depurador na tela (no AppleWin só valem rodando, WinFrame.cpp:1336-1393) | AUD-P-035 |
| 0..9 (vendo a saída de vídeo) | Páginas 0-8 e 9 (modo atual); qualquer outra tecla volta ao depurador (:9658-9740) | IMPLEMENTED (estado) | `output_view_digits_pick_a_page_and_other_keys_leave`. O port muda `output_view` como os comandos `HGRn`; o frontend continua mostrando a saída corrente da máquina, sem forçar a página (limitação que já valia para os comandos `HGR0..`) | AUD-P-035 |
| Roda do mouse | `DebuggerProcessKey(VK_UP/VK_DOWN)` (WinFrame.cpp:1647) | PASS | `keymap::wheel_debug_key` + um braço `MouseWheel` em `app.rs` (`debugger_wheel`); teste `wheel_is_up_and_down_with_modifiers` | AUD-P-035 |
| F7 (entrar/sair) | `ToggleDebugger` | IMPLEMENTED | actions.rs:72 |  |
| Ctrl+F11 (passo) | Sem equivalente: WinFrame.cpp:1286 só trata F11 sem Ctrl (save-state); o port acrescentou `Action::Step` | EXTENSÃO | actions.rs:78, emu.rs:567 |  |

## 2. Comandos (`g_aCommands[]`, 251 entradas)

Contagem: 0 FAIL, 249 IMPLEMENTED, 2 N/A (total 251). Atualizado pela HU-0801: os 17 FAIL da wave0 foram resolvidos (7 eram linhas de ajuda de atalho, hoje teclas; 1 é N/A no oráculo).

| Comando | Ação (CMD_) | Entrada no port | Estado | Observação |
|---|---|---|---|---|
| `A` | CMD_ASSEMBLE | `Assemble` | IMPLEMENTED |  |
| `.` | CMD_CURSOR_JUMP_PC | `CursorJumpPc` | IMPLEMENTED |  |
| `=` | CMD_CURSOR_SET_PC | `CursorSetPc` | IMPLEMENTED |  |
| `G` | CMD_GO_NORMAL_SPEED | `GoNormalSpeed` | IMPLEMENTED |  |
| `GG` | CMD_GO_FULL_SPEED | `GoFullSpeed` | IMPLEMENTED |  |
| `IN` | CMD_IN | `In` | IMPLEMENTED |  |
| `KEY` | CMD_INPUT_KEY | `InputKey` | IMPLEMENTED |  |
| `JSR` | CMD_JSR | `Jsr` | IMPLEMENTED |  |
| `NOP` | CMD_NOP | `Nop` | IMPLEMENTED |  |
| `OUT` | CMD_OUT | `Out` | IMPLEMENTED |  |
| `LBR` | CMD_LBR | `Lbr` | IMPLEMENTED | depende de gravar o registro de desvios no núcleo; sem teste diferencial |
| `PROFILE` | CMD_PROFILE | `Profile` | IMPLEMENTED | sem teste diferencial |
| `R` | CMD_REGISTER_SET | `RegisterSet` | IMPLEMENTED |  |
| `POP` | CMD_STACK_POP | `StackPop` | IMPLEMENTED |  |
| `PPOP` | CMD_STACK_POP_PSEUDO | `StackPopPseudo` | IMPLEMENTED |  |
| `PUSH` | CMD_STACK_PUSH | `StackPush` | IMPLEMENTED |  |
| `P` | CMD_STEP_OVER | `StepOver` | IMPLEMENTED |  |
| `RTS` | CMD_STEP_OUT | `StepOut` | IMPLEMENTED |  |
| `T` | CMD_TRACE | `Trace` | IMPLEMENTED |  |
| `TF` | CMD_TRACE_FILE | `TraceFile` | IMPLEMENTED |  |
| `TL` | CMD_TRACE_LINE | `TraceLine` | IMPLEMENTED |  |
| `U` | CMD_UNASSEMBLE | `Unassemble` | IMPLEMENTED |  |
| `BM` | CMD_BOOKMARK | `Bookmark` | IMPLEMENTED |  |
| `BMA` | CMD_BOOKMARK_ADD | `BookmarkAdd` | IMPLEMENTED |  |
| `BMC` | CMD_BOOKMARK_CLEAR | `BookmarkClear` | IMPLEMENTED |  |
| `BML` | CMD_BOOKMARK_LIST | `BookmarkList` | IMPLEMENTED |  |
| `BMG` | CMD_BOOKMARK_GOTO | `BookmarkGoto` | IMPLEMENTED |  |
| `BMSAVE` | CMD_BOOKMARK_SAVE | `BookmarkSave` | IMPLEMENTED |  |
| `BRK` | CMD_BREAK_INVALID | `BreakInvalid` | IMPLEMENTED |  |
| `BRKOP` | CMD_BREAK_OPCODE | `BreakOpcode` | IMPLEMENTED |  |
| `BRKINT` | CMD_BREAK_ON_INTERRUPT | `BreakOnInterrupt` | IMPLEMENTED |  |
| `BP` | CMD_BREAKPOINT | `Breakpoint` | IMPLEMENTED |  |
| `BPA` | CMD_BREAKPOINT_ADD_SMART | `BreakpointAddSmart` | IMPLEMENTED |  |
| `BPR` | CMD_BREAKPOINT_ADD_REG | `BreakpointAddReg` | IMPLEMENTED |  |
| `BPX` | CMD_BREAKPOINT_ADD_PC | `BreakpointAddPc` | IMPLEMENTED |  |
| `BPIO` | CMD_BREAKPOINT_ADD_IO | `BreakpointAddIo` | IMPLEMENTED |  |
| `BPM` | CMD_BREAKPOINT_ADD_MEM | `BreakpointAddMem` | IMPLEMENTED |  |
| `BPMR` | CMD_BREAKPOINT_ADD_MEMR | `BreakpointAddMemr` | IMPLEMENTED |  |
| `BPMW` | CMD_BREAKPOINT_ADD_MEMW | `BreakpointAddMemw` | IMPLEMENTED |  |
| `BPV` | CMD_BREAKPOINT_ADD_VIDEO | `BreakpointAddVideo` | IMPLEMENTED |  |
| `BPC` | CMD_BREAKPOINT_CLEAR | `BreakpointClear` | IMPLEMENTED |  |
| `BPD` | CMD_BREAKPOINT_DISABLE | `BreakpointDisable` | IMPLEMENTED |  |
| `BPEDIT` | CMD_BREAKPOINT_EDIT | `BreakpointEdit` | IMPLEMENTED | idêntico ao C++: `CmdBreakpointEdit` só devolve `UPDATE_DISASM \| UPDATE_BREAKPOINTS` (Debug.cpp:1986). A observação "é editor" da wave0 estava errada; teste `sync_and_bpedit_do_what_applewin_does` |
| `BPE` | CMD_BREAKPOINT_ENABLE | `BreakpointEnable` | IMPLEMENTED |  |
| `BPL` | CMD_BREAKPOINT_LIST | `BreakpointList` | IMPLEMENTED |  |
| `BPSAVE` | CMD_BREAKPOINT_SAVE | `BreakpointSave` | IMPLEMENTED |  |
| `BPCHANGE` | CMD_BREAKPOINT_CHANGE | `BreakpointChange` | IMPLEMENTED |  |
| `BENCHMARK` | CMD_BENCHMARK | `Benchmark` | IMPLEMENTED | `cmd_host.rs`: `CpuSetupBenchmark` (programa de opcodes em $0300, A=X=Y=0, SP=$FF, PC=$0300, cursor no PC) e, em seguida, uma execução cronometrada de 1.000.000 de ciclos que imprime ciclos, segundos, MHz e o múltiplo do clock real. Diferença conhecida: no AppleWin o primeiro `BENCHMARK` cai em `CmdBenchmarkStop` (sai do depurador e mede com `GetTickCount` + o sinal de alto-falante do disco de benchmark, Speaker.cpp:112); isso depende de Win32 e do disco, então o port mede por conta própria. Teste `benchmark_loads_the_program_and_measures_mhz` |
| `BW` | CMD_CONFIG_BW | `ConfigBw` | IMPLEMENTED |  |
| `COLOR` | CMD_CONFIG_COLOR | `ConfigColor` | IMPLEMENTED |  |
| `DISASM` | CMD_CONFIG_DISASM | `ConfigDisasm` | IMPLEMENTED |  |
| `FONT` | CMD_CONFIG_FONT | `ConfigFont` | IMPLEMENTED | Igual ao AppleWin com `USE_APPLE_FONT` (a via GDI `OLD_FONT` está compilada fora, então `CmdConfigSetFont` só redesenha): `FONT` lista as 4 fontes (nome vazio, A:7 M:7), `FONT *` mostra linhas/px, `FONT MODE n` valida 0-2, `FONT LOAD/SAVE` não fazem nada. A fonte GDI por nome/tamanho não existe aqui (justificativa: o depurador desenha com a fonte de vídeo do Apple //e). Teste `font_prints_the_apple_font_cell_and_validates_mode` |
| `HCOLOR` | CMD_CONFIG_HCOLOR | `ConfigHcolor` | IMPLEMENTED | Idêntico ao C++, que só valida os argumentos (1 ou 4, cor < `NUM_DEBUG_COLORS`) e não altera nada (corpo comentado com "TODO/FIXME", Debug.cpp:2950). Teste `hcolor_validates_like_applewin` |
| `LOAD` | CMD_CONFIG_LOAD | `ConfigLoad` | IMPLEMENTED |  |
| `MONO` | CMD_CONFIG_MONOCHROME | `ConfigMonochrome` | IMPLEMENTED |  |
| `SAVE` | CMD_CONFIG_SAVE | `ConfigSave` | IMPLEMENTED |  |
| `PWD` | CMD_CONFIG_GET_DEBUG_DIR | `ConfigGetDebugDir` | IMPLEMENTED |  |
| `CD` | CMD_CONFIG_SET_DEBUG_DIR | `ConfigSetDebugDir` | IMPLEMENTED |  |
| `RET` | CMD_CURSOR_JUMP_RET_ADDR | `CursorJumpRetAddr` | IMPLEMENTED |  |
| `^` | CMD_CURSOR_LINE_UP | `-` | IMPLEMENTED (tecla) | linha de ajuda sem função no AppleWin (`pFunction` NULL); no port a tecla Seta-cima funciona (`keys.rs`), ver §1 |
| `Shift ^` | CMD_CURSOR_LINE_UP_1 | `-` | IMPLEMENTED (tecla) | linha de ajuda sem função no AppleWin (`pFunction` NULL); no port a tecla Shift+Seta-cima funciona (`keys.rs`), ver §1 |
| `v` | CMD_CURSOR_LINE_DOWN | `-` | IMPLEMENTED (tecla) | linha de ajuda sem função no AppleWin (`pFunction` NULL); no port a tecla Seta-baixo funciona (`keys.rs`), ver §1 |
| `Shift v` | CMD_CURSOR_LINE_DOWN_1 | `-` | IMPLEMENTED (tecla) | linha de ajuda sem função no AppleWin (`pFunction` NULL); no port a tecla Shift+Seta-baixo funciona (`keys.rs`), ver §1 |
| `PAGEUP` | CMD_CURSOR_PAGE_UP | `CursorPageUp` | IMPLEMENTED |  |
| `PAGEUP256` | CMD_CURSOR_PAGE_UP_256 | `CursorPageUp256` | IMPLEMENTED |  |
| `PAGEUP4K` | CMD_CURSOR_PAGE_UP_4K | `CursorPageUp4k` | IMPLEMENTED |  |
| `PAGEDN` | CMD_CURSOR_PAGE_DOWN | `CursorPageDown` | IMPLEMENTED |  |
| `PAGEDOWN256` | CMD_CURSOR_PAGE_DOWN_256 | `CursorPageDown256` | IMPLEMENTED |  |
| `PAGEDOWN4K` | CMD_CURSOR_PAGE_DOWN_4K | `CursorPageDown4k` | IMPLEMENTED |  |
| `CYCLES` | CMD_CYCLES_INFO | `CyclesInfo` | IMPLEMENTED |  |
| `RCC` | CMD_CYCLES_RESET | `CyclesReset` | IMPLEMENTED |  |
| `Z` | CMD_DISASM_DATA | `DisasmData` | IMPLEMENTED |  |
| `X` | CMD_DISASM_CODE | `DisasmCode` | IMPLEMENTED |  |
| `B` | CMD_DISASM_LIST | `DisasmList` | IMPLEMENTED |  |
| `DB` | CMD_DEFINE_DATA_BYTE1 | `DefineDataByte1` | IMPLEMENTED |  |
| `DB2` | CMD_DEFINE_DATA_BYTE2 | `DefineDataByte2` | IMPLEMENTED |  |
| `DB4` | CMD_DEFINE_DATA_BYTE4 | `DefineDataByte4` | IMPLEMENTED |  |
| `DB8` | CMD_DEFINE_DATA_BYTE8 | `DefineDataByte8` | IMPLEMENTED |  |
| `DW` | CMD_DEFINE_DATA_WORD1 | `DefineDataWord1` | IMPLEMENTED |  |
| `DW2` | CMD_DEFINE_DATA_WORD2 | `DefineDataWord2` | IMPLEMENTED |  |
| `DW4` | CMD_DEFINE_DATA_WORD4 | `DefineDataWord4` | IMPLEMENTED |  |
| `ASC` | CMD_DEFINE_DATA_STR | `DefineDataStr` | IMPLEMENTED |  |
| `DF` | CMD_DEFINE_DATA_FLOAT | `DefineDataFloat` | IMPLEMENTED |  |
| `DA` | CMD_DEFINE_ADDR_WORD | `DefineAddrWord` | IMPLEMENTED |  |
| `DISK` | CMD_DISK | `Disk` | IMPLEMENTED | `cmd_host.rs`: `DISK INFO`, `DISK SLOT [n]`, `DISK n EJECT`, `DISK n PROTECT [0\|1]` e `DISK n <READ\|INFO\|SLOT> "arquivo"` (monta protegido contra escrita) sobre o `Disk2Card` do slot. Mantida a esquisitice do AppleWin: o nome da imagem vai no 3º argumento e o 2º precisa ser uma das palavras (a ajuda diz `DISK # "file"`, o código não aceita). `READ`/`WRITE` de setor são TODO no AppleWin. Acessores novos em `Disk2Card`: `current_bit_offset`, `current_extra_cycles`, `current_shift_reg`, `current_phase_string`. Teste `disk_shows_and_controls_the_drives` |
| `CL` | CMD_FLAG_CLEAR | `FlagClear` | IMPLEMENTED |  |
| `CLC` | CMD_FLAG_CLR_C | `FlagClrC` | IMPLEMENTED |  |
| `CLZ` | CMD_FLAG_CLR_Z | `FlagClrZ` | IMPLEMENTED |  |
| `CLI` | CMD_FLAG_CLR_I | `FlagClrI` | IMPLEMENTED |  |
| `CLD` | CMD_FLAG_CLR_D | `FlagClrD` | IMPLEMENTED |  |
| `CLB` | CMD_FLAG_CLR_B | `FlagClrB` | IMPLEMENTED |  |
| `CLR` | CMD_FLAG_CLR_R | `FlagClrR` | IMPLEMENTED |  |
| `CLV` | CMD_FLAG_CLR_V | `FlagClrV` | IMPLEMENTED |  |
| `CLN` | CMD_FLAG_CLR_N | `FlagClrN` | IMPLEMENTED |  |
| `SE` | CMD_FLAG_SET | `FlagSet` | IMPLEMENTED |  |
| `SEC` | CMD_FLAG_SET_C | `FlagSetC` | IMPLEMENTED |  |
| `SEZ` | CMD_FLAG_SET_Z | `FlagSetZ` | IMPLEMENTED |  |
| `SEI` | CMD_FLAG_SET_I | `FlagSetI` | IMPLEMENTED |  |
| `SED` | CMD_FLAG_SET_D | `FlagSetD` | IMPLEMENTED |  |
| `SEB` | CMD_FLAG_SET_B | `FlagSetB` | IMPLEMENTED |  |
| `SER` | CMD_FLAG_SET_R | `FlagSetR` | IMPLEMENTED |  |
| `SEV` | CMD_FLAG_SET_V | `FlagSetV` | IMPLEMENTED |  |
| `SEN` | CMD_FLAG_SET_N | `FlagSetN` | IMPLEMENTED |  |
| `?` | CMD_HELP_LIST | `HelpList` | IMPLEMENTED |  |
| `HELP` | CMD_HELP_SPECIFIC | `HelpSpecific` | IMPLEMENTED |  |
| `VERSION` | CMD_VERSION | `Version` | IMPLEMENTED |  |
| `MOTD` | CMD_MOTD | `Motd` | IMPLEMENTED |  |
| `MC` | CMD_MEMORY_COMPARE | `MemoryCompare` | IMPLEMENTED | no AppleWin `CmdMemoryCompare` é esqueleto; o port lista as diferenças (extensão) |
| `MD1` | CMD_MEM_MINI_DUMP_HEX_1 | `MemMiniDumpHex1` | IMPLEMENTED |  |
| `MD2` | CMD_MEM_MINI_DUMP_HEX_2 | `MemMiniDumpHex2` | IMPLEMENTED |  |
| `MA1` | CMD_MEM_MINI_DUMP_ASCII_1 | `MemMiniDumpAscii1` | IMPLEMENTED |  |
| `MA2` | CMD_MEM_MINI_DUMP_ASCII_2 | `MemMiniDumpAscii2` | IMPLEMENTED |  |
| `MT1` | CMD_MEM_MINI_DUMP_APPLE_1 | `MemMiniDumpApple1` | IMPLEMENTED |  |
| `MT2` | CMD_MEM_MINI_DUMP_APPLE_2 | `MemMiniDumpApple2` | IMPLEMENTED |  |
| `ME` | CMD_MEMORY_EDIT | `MemoryEdit` | N/A | AppleWin também não implementa ("Memory Editor - Not Implemented!") |
| `MEB` | CMD_MEMORY_ENTER_BYTE | `MemoryEnterByte` | IMPLEMENTED |  |
| `MEW` | CMD_MEMORY_ENTER_WORD | `MemoryEnterWord` | IMPLEMENTED |  |
| `BLOAD` | CMD_MEMORY_LOAD | `MemoryLoad` | IMPLEMENTED |  |
| `M` | CMD_MEMORY_MOVE | `MemoryMove` | IMPLEMENTED |  |
| `BSAVE` | CMD_MEMORY_SAVE | `MemorySave` | IMPLEMENTED |  |
| `S` | CMD_MEMORY_SEARCH | `MemorySearch` | IMPLEMENTED |  |
| `@` | CMD_MEMORY_FIND_RESULTS | `MemoryFindResults` | IMPLEMENTED |  |
| `SH` | CMD_MEMORY_SEARCH_HEX | `MemorySearchHex` | IMPLEMENTED |  |
| `F` | CMD_MEMORY_FILL | `MemoryFill` | IMPLEMENTED |  |
| `NTSC` | CMD_NTSC | `Ntsc` | IMPLEMENTED (parcial) | `NTSC SAVE [arquivo]` grava a tabela de matiz (4 fases x 4096 x 32 bits) em `.data` (RGBA, padrão `AppleWinNTSC4096x4@32.data`) ou `.bmp` 64x256x32, no mesmo layout do C++; `NTSC RESET` imprime o aviso (as tabelas nunca mudam). `NTSC LOAD` faz as mesmas validações (tamanho, 32 bpp, 64x256/64x1/16x1) mas **não aplica** a paleta: `apple2_video::chroma` constrói a tabela uma vez (`OnceLock`) e o renderer não tem gancho para trocá-la; mudar isso é trabalho no crate de vídeo. Teste `ntsc_save_writes_raw_and_bmp_palettes` |
| `TSAVE` | CMD_TEXT_SAVE | `TextSave` | IMPLEMENTED |  |
| `CALC` | CMD_OUTPUT_CALC | `OutputCalc` | IMPLEMENTED |  |
| `ECHO` | CMD_OUTPUT_ECHO | `OutputEcho` | IMPLEMENTED |  |
| `LOG` | CMD_OUTPUT_LOG | `OutputLog` | IMPLEMENTED |  |
| `PRINT` | CMD_OUTPUT_PRINT | `OutputPrint` | IMPLEMENTED |  |
| `PRINTF` | CMD_OUTPUT_PRINTF | `OutputPrintf` | IMPLEMENTED |  |
| `RUN` | CMD_OUTPUT_RUN | `OutputRun` | IMPLEMENTED |  |
| `SOURCE` | CMD_SOURCE | `Source` | IMPLEMENTED | `SOURCE [MEM] [SYM] arquivo...` lê a listagem do montador, grava os bytes na memória (`MEM`), cria símbolos na tabela Src2 (`SYM`, linhas `EQU`/`DFB`) e guarda o mapa endereço->linha; `SOURCE` sem argumentos desliga. O C++ perde o último caractere do rótulo (`sName[nLen - 1] = 0`) e o port reproduz isso. Não há janela de código-fonte desenhada (`DrawSubWindow_Source` é vazia no AppleWin; `SOURCE2` não é desenhada aqui). Teste `source_reads_listing_symbols_and_bytes` |
| `SYNC` | CMD_SYNC | `Sync` | IMPLEMENTED | Idêntico ao C++: `CmdSync` é um `// TODO` que só devolve `UPDATE_CONSOLE_DISPLAY` (Debug.cpp:7079) |
| `SYM` | CMD_SYMBOLS_LOOKUP | `SymbolsLookup` | IMPLEMENTED |  |
| `SYMMAIN` | CMD_SYMBOLS_ROM | `SymbolsRom` | IMPLEMENTED |  |
| `SYMBASIC` | CMD_SYMBOLS_APPLESOFT | `SymbolsApplesoft` | IMPLEMENTED |  |
| `SYMASM` | CMD_SYMBOLS_ASSEMBLY | `SymbolsAssembly` | IMPLEMENTED |  |
| `SYMUSER` | CMD_SYMBOLS_USER_1 | `SymbolsUser1` | IMPLEMENTED |  |
| `SYMUSER2` | CMD_SYMBOLS_USER_2 | `SymbolsUser2` | IMPLEMENTED |  |
| `SYMSRC` | CMD_SYMBOLS_SRC_1 | `SymbolsSrc1` | IMPLEMENTED |  |
| `SYMSRC2` | CMD_SYMBOLS_SRC_2 | `SymbolsSrc2` | IMPLEMENTED |  |
| `SYMDOS33` | CMD_SYMBOLS_DOS33 | `SymbolsDos33` | IMPLEMENTED |  |
| `SYMPRODOS` | CMD_SYMBOLS_PRODOS | `SymbolsProdos` | IMPLEMENTED |  |
| `SYMINFO` | CMD_SYMBOLS_INFO | `SymbolsInfo` | IMPLEMENTED |  |
| `SYMLIST` | CMD_SYMBOLS_LIST | `SymbolsList` | IMPLEMENTED |  |
| `VIDEOINFO` | CMD_VIDEO_SCANNER_INFO | `VideoScannerInfo` | IMPLEMENTED |  |
| `TEXT` | CMD_VIEW_TEXT4X | `ViewText4x` | IMPLEMENTED |  |
| `TEXT1` | CMD_VIEW_TEXT41 | `ViewText41` | IMPLEMENTED |  |
| `TEXT2` | CMD_VIEW_TEXT42 | `ViewText42` | IMPLEMENTED |  |
| `TEXT80` | CMD_VIEW_TEXT8X | `ViewText8x` | IMPLEMENTED |  |
| `TEXT81` | CMD_VIEW_TEXT81 | `ViewText81` | IMPLEMENTED |  |
| `TEXT82` | CMD_VIEW_TEXT82 | `ViewText82` | IMPLEMENTED |  |
| `GR` | CMD_VIEW_GRX | `ViewGrx` | IMPLEMENTED |  |
| `GR1` | CMD_VIEW_GR1 | `ViewGr1` | IMPLEMENTED |  |
| `GR2` | CMD_VIEW_GR2 | `ViewGr2` | IMPLEMENTED |  |
| `DGR` | CMD_VIEW_DGRX | `ViewDgrx` | IMPLEMENTED |  |
| `DGR1` | CMD_VIEW_DGR1 | `ViewDgr1` | IMPLEMENTED |  |
| `DGR2` | CMD_VIEW_DGR2 | `ViewDgr2` | IMPLEMENTED |  |
| `HGR` | CMD_VIEW_HGRX | `ViewHgrx` | IMPLEMENTED |  |
| `HGR0` | CMD_VIEW_HGR0 | `ViewHgr0` | IMPLEMENTED |  |
| `HGR1` | CMD_VIEW_HGR1 | `ViewHgr1` | IMPLEMENTED |  |
| `HGR2` | CMD_VIEW_HGR2 | `ViewHgr2` | IMPLEMENTED |  |
| `HGR3` | CMD_VIEW_HGR3 | `ViewHgr3` | IMPLEMENTED |  |
| `HGR4` | CMD_VIEW_HGR4 | `ViewHgr4` | IMPLEMENTED |  |
| `HGR5` | CMD_VIEW_HGR5 | `ViewHgr5` | IMPLEMENTED |  |
| `HGR6` | CMD_VIEW_HGR6 | `ViewHgr6` | IMPLEMENTED |  |
| `HGR7` | CMD_VIEW_HGR7 | `ViewHgr7` | IMPLEMENTED |  |
| `HGR8` | CMD_VIEW_HGR8 | `ViewHgr8` | IMPLEMENTED |  |
| `DHGR` | CMD_VIEW_DHGRX | `ViewDhgrx` | IMPLEMENTED |  |
| `DHGR1` | CMD_VIEW_DHGR1 | `ViewDhgr1` | IMPLEMENTED |  |
| `DHGR2` | CMD_VIEW_DHGR2 | `ViewDhgr2` | IMPLEMENTED |  |
| `SHR` | CMD_VIEW_SHR | `ViewShr` | IMPLEMENTED |  |
| `W` | CMD_WATCH | `Watch` | IMPLEMENTED |  |
| `WA` | CMD_WATCH_ADD | `WatchAdd` | IMPLEMENTED |  |
| `WC` | CMD_WATCH_CLEAR | `WatchClear` | IMPLEMENTED |  |
| `WD` | CMD_WATCH_DISABLE | `WatchDisable` | IMPLEMENTED |  |
| `WE` | CMD_WATCH_ENABLE | `WatchEnable` | IMPLEMENTED |  |
| `WL` | CMD_WATCH_LIST | `WatchList` | IMPLEMENTED |  |
| `WSAVE` | CMD_WATCH_SAVE | `WatchSave` | IMPLEMENTED |  |
| `WIN` | CMD_WINDOW | `Window` | IMPLEMENTED |  |
| `CODE` | CMD_WINDOW_CODE | `WindowCode` | IMPLEMENTED |  |
| `CODE1` | CMD_WINDOW_CODE_1 | `WindowCode1` | IMPLEMENTED |  |
| `CODE2` | CMD_WINDOW_CODE_2 | `WindowCode2` | IMPLEMENTED |  |
| `CONSOLE` | CMD_WINDOW_CONSOLE | `WindowConsole` | IMPLEMENTED |  |
| `DATA` | CMD_WINDOW_DATA | `WindowData` | IMPLEMENTED |  |
| `DATA1` | CMD_WINDOW_DATA_1 | `WindowData1` | IMPLEMENTED |  |
| `DATA2` | CMD_WINDOW_DATA_2 | `WindowData2` | IMPLEMENTED |  |
| `SOURCE1` | CMD_WINDOW_SOURCE_1 | `WindowSource1` | IMPLEMENTED |  |
| `SOURCE2` | CMD_WINDOW_SOURCE_2 | `WindowSource2` | IMPLEMENTED |  |
| `\\` | CMD_WINDOW_OUTPUT | `WindowOutput` | IMPLEMENTED |  |
| `ZP` | CMD_ZEROPAGE_POINTER | `ZeropagePointer` | IMPLEMENTED |  |
| `ZP0` | CMD_ZEROPAGE_POINTER_0 | `ZeropagePointer0` | IMPLEMENTED |  |
| `ZP1` | CMD_ZEROPAGE_POINTER_1 | `ZeropagePointer1` | IMPLEMENTED |  |
| `ZP2` | CMD_ZEROPAGE_POINTER_2 | `ZeropagePointer2` | IMPLEMENTED |  |
| `ZP3` | CMD_ZEROPAGE_POINTER_3 | `ZeropagePointer3` | IMPLEMENTED |  |
| `ZP4` | CMD_ZEROPAGE_POINTER_4 | `ZeropagePointer4` | IMPLEMENTED |  |
| `ZP5` | CMD_ZEROPAGE_POINTER_5 | `ZeropagePointer5` | IMPLEMENTED |  |
| `ZP6` | CMD_ZEROPAGE_POINTER_6 | `ZeropagePointer6` | IMPLEMENTED |  |
| `ZP7` | CMD_ZEROPAGE_POINTER_7 | `ZeropagePointer7` | IMPLEMENTED |  |
| `ZPA` | CMD_ZEROPAGE_POINTER_ADD | `ZeropagePointerAdd` | IMPLEMENTED |  |
| `ZPC` | CMD_ZEROPAGE_POINTER_CLEAR | `ZeropagePointerClear` | IMPLEMENTED |  |
| `ZPD` | CMD_ZEROPAGE_POINTER_DISABLE | `ZeropagePointerDisable` | IMPLEMENTED |  |
| `ZPE` | CMD_ZEROPAGE_POINTER_ENABLE | `ZeropagePointerEnable` | IMPLEMENTED |  |
| `ZPL` | CMD_ZEROPAGE_POINTER_LIST | `ZeropagePointerList` | IMPLEMENTED |  |
| `ZPSAVE` | CMD_ZEROPAGE_POINTER_SAVE | `ZeropagePointerSave` | IMPLEMENTED |  |
| `STARTUP` | CMD_STARTUP | `Startup` | IMPLEMENTED |  |
| `->` | CMD_CURSOR_JUMP_PC | `-` | IMPLEMENTED (tecla) | linha de ajuda sem função no AppleWin (`pFunction` NULL); no port a tecla Seta-direita funciona (`keys.rs`), ver §1 |
| `Ctrl ->` | CMD_CURSOR_SET_PC | `-` | IMPLEMENTED (tecla) | linha de ajuda sem função no AppleWin (`pFunction` NULL); no port a tecla Ctrl+Seta-direita funciona (`keys.rs`), ver §1 |
| `Shift ->` | CMD_CURSOR_JUMP_PC | `-` | IMPLEMENTED (tecla) | linha de ajuda sem função no AppleWin (`pFunction` NULL); no port a tecla Shift+Seta-direita funciona (`keys.rs`), ver §1 |
| `INPUT` | CMD_IN | `In` | IMPLEMENTED |  |
| `RC` | CMD_FLAG_CLR_C | `FlagClrC` | IMPLEMENTED |  |
| `RZ` | CMD_FLAG_CLR_Z | `FlagClrZ` | IMPLEMENTED |  |
| `RI` | CMD_FLAG_CLR_I | `FlagClrI` | IMPLEMENTED |  |
| `RD` | CMD_FLAG_CLR_D | `FlagClrD` | IMPLEMENTED |  |
| `RB` | CMD_FLAG_CLR_B | `FlagClrB` | IMPLEMENTED |  |
| `RR` | CMD_FLAG_CLR_R | `FlagClrR` | IMPLEMENTED |  |
| `RV` | CMD_FLAG_CLR_V | `FlagClrV` | IMPLEMENTED |  |
| `RN` | CMD_FLAG_CLR_N | `FlagClrN` | IMPLEMENTED |  |
| `SC` | CMD_FLAG_SET_C | `FlagSetC` | IMPLEMENTED |  |
| `SZ` | CMD_FLAG_SET_Z | `FlagSetZ` | IMPLEMENTED |  |
| `SI` | CMD_FLAG_SET_I | `FlagSetI` | IMPLEMENTED |  |
| `SD` | CMD_FLAG_SET_D | `FlagSetD` | IMPLEMENTED |  |
| `SB` | CMD_FLAG_SET_B | `FlagSetB` | IMPLEMENTED |  |
| `SR` | CMD_FLAG_SET_R | `FlagSetR` | IMPLEMENTED |  |
| `SV` | CMD_FLAG_SET_V | `FlagSetV` | IMPLEMENTED |  |
| `SN` | CMD_FLAG_SET_N | `FlagSetN` | IMPLEMENTED |  |
| `D` | CMD_MEM_MINI_DUMP_HEX_1 | `MemMiniDumpHex1` | IMPLEMENTED |  |
| `M1` | CMD_MEM_MINI_DUMP_HEX_1 | `MemMiniDumpHex1` | IMPLEMENTED |  |
| `M2` | CMD_MEM_MINI_DUMP_HEX_2 | `MemMiniDumpHex2` | IMPLEMENTED |  |
| `ME8` | CMD_MEMORY_ENTER_BYTE | `MemoryEnterByte` | IMPLEMENTED |  |
| `ME16` | CMD_MEMORY_ENTER_WORD | `MemoryEnterWord` | IMPLEMENTED |  |
| `MM` | CMD_MEMORY_MOVE | `MemoryMove` | IMPLEMENTED |  |
| `MS` | CMD_MEMORY_SEARCH | `MemorySearch` | IMPLEMENTED |  |
| `P0` | CMD_ZEROPAGE_POINTER_0 | `ZeropagePointer0` | IMPLEMENTED |  |
| `P1` | CMD_ZEROPAGE_POINTER_1 | `ZeropagePointer1` | IMPLEMENTED |  |
| `P2` | CMD_ZEROPAGE_POINTER_2 | `ZeropagePointer2` | IMPLEMENTED |  |
| `P3` | CMD_ZEROPAGE_POINTER_3 | `ZeropagePointer3` | IMPLEMENTED |  |
| `P4` | CMD_ZEROPAGE_POINTER_4 | `ZeropagePointer4` | IMPLEMENTED |  |
| `REGISTER` | CMD_REGISTER_SET | `RegisterSet` | IMPLEMENTED |  |
| `TRACE` | CMD_TRACE | `Trace` | IMPLEMENTED |  |
| `SYMDOS` | CMD_SYMBOLS_DOS33 | `SymbolsDos33` | IMPLEMENTED |  |
| `SYMPRO` | CMD_SYMBOLS_PRODOS | `SymbolsProdos` | IMPLEMENTED |  |
| `TEXT40` | CMD_VIEW_TEXT4X | `ViewText4x` | IMPLEMENTED |  |
| `TEXT41` | CMD_VIEW_TEXT41 | `ViewText41` | IMPLEMENTED |  |
| `TEXT42` | CMD_VIEW_TEXT42 | `ViewText42` | IMPLEMENTED |  |
| `WINDOW` | CMD_WINDOW | `Window` | IMPLEMENTED |  |
| `ZAP` | CMD_NOP | `Nop` | IMPLEMENTED |  |
| `BENCH` | CMD_BENCHMARK | `Benchmark` | IMPLEMENTED | alias de BENCHMARK (no AppleWin chama `CmdBenchmarkStart`) |
| `EXITBENCH` | CMD_BENCHMARK | `-` | N/A | A entrada tem `pFunction` NULL (Debugger_Commands.cpp:400, comentário "CmdBenchmarkStop") e `FindCommand` ignora entradas sem função (Debug.cpp:8152): no AppleWin o comando também é ilegal. Teste `exitbench_is_not_a_command_like_in_applewin` |
| `MDB` | CMD_MEM_MINI_DUMP_HEX_1 | `MemMiniDumpHex1` | IMPLEMENTED |  |

## 3. Issues citadas na HU-0801

| Issue | Situação no AppleWin a8f0d1d | Situação no port | Estado |
|---|---|---|---|
| #1460 Ctrl-Right não atualiza alvos | corrigida (fechada em 2026-01-01) | os alvos são calculados no `render` a partir do PC; o comando `=` funciona | PASS (função e tecla) |
| #1456 Right-arrow deve seguir RTS | corrigida (2026-01-01) | `target_address` trata RTS/RTI com a volta da pilha (disasm.rs:363-375) | PASS (função e tecla) |
| #1451 Right-arrow não localiza a instrução atual (doc diz Alt+Right/Tab) | corrigida (2026-05-23); mapeamento atual: Tab = centro no PC, Alt+Right = centro, Shift+Right = topo, Right = seguir alvo | Tab, Alt+Direita, Shift+Direita e Direita implementadas em `keys.rs` | PASS |

## 4. Reprodução

```
cargo test -p apple2-rust --test audit_wave0_peripherals aud_p_035 aud_p_036
cargo test -p apple2-debugger --test audit_wave0_debugger --test hu0801
```
