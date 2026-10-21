# Debugger: comandos, atalhos e estado do port (wave0)

Oráculo: AppleWin `a8f0d1d`, `source/Debugger/Debugger_Commands.cpp:40-403` (`g_aCommands[]`) e `source/Debugger/Debug.cpp:9653-10122` (`DebuggerProcessKey`).
Port: `crates/apple2-debugger/src/cmdtable.rs` (tabela gerada), `commands.rs:260` (`dispatch`), `crates/applewin/src/keymap.rs:148` (`debug_key`), `crates/applewin/src/emu.rs:626` (`Command::Debug`).
Estados: IMPLEMENTED (código existe), PASS (teste concorda com o oráculo), FAIL (discorda ou ausente), N/A (o AppleWin também não implementa). Nenhum comando está PASS por teste diferencial contra o C++: os testes existentes (`crates/apple2-debugger/tests/*.rs`) foram escritos a partir do código.

## 1. Atalhos de teclado (`DebuggerProcessKey`)

| Tecla (AppleWin) | Ação no AppleWin (Debug.cpp) | Estado no port | Evidência no port | ID |
|---|---|---|---|---|
| Espaço (console vazio) | Trace: `CmdTrace` (:9770-9784) | FAIL | `keymap.rs:153` entrega `Char(' ')` ao console; `aud_p_035_space_traces_one_instruction_in_the_debugger` (ignorado) reproduz | AUD-P-035 |
| Shift+Espaço | Step out: `CmdStepOut` | FAIL | sem tradução; o comando `RTS` equivale | AUD-P-035 |
| Ctrl+Espaço | Step over: `CmdStepOver` | FAIL | `debug_key` devolve `None` com Ctrl (keymap.rs:149); o comando `P` equivale | AUD-P-035 |
| Tab (console vazio) | `CmdCursorJumpPC(CENTER)`; com texto: completar comando; Ctrl+Tab/Ctrl+Shift+Tab: ciclar janelas | FAIL | `HostKey::Tab` cai em `_ => return None` (keymap.rs:161) | AUD-P-035 |
| Seta-direita | `CmdCursorFollowTarget(TOP)`; segue RTS/RTI (#1456) e salto | FAIL (tecla) / IMPLEMENTED (função) | `Debugger::cursor_follow_target` (cmd_exec.rs:58) existe e passa o repro de #1456 (`aud_p_037_follow_target_on_rts_wraps_the_stack_page_gh1456`); nenhuma tecla a chama | AUD-P-037 |
| Ctrl+Seta-direita | `CmdCursorSetPC` (PC := cursor); #1460 | IMPLEMENTED (comando `=`) / FAIL (tecla) | comando `=` OK e atualiza alvos (`aud_p_037_set_pc_from_cursor_updates_targets_gh1460`) | AUD-P-037 |
| Shift+Seta-direita | `CmdCursorJumpPC(TOP)` | FAIL (tecla) | o comando `.` equivale (centrado) | AUD-P-035 |
| Alt+Seta-direita | `CmdCursorJumpPC(CENTER)` | FAIL (tecla) |  | AUD-P-035 |
| Seta-esquerda / Shift+Seta-esquerda | `CmdCursorJumpRetAddr` (centro / topo) | FAIL (tecla) / IMPLEMENTED (comando `RET`) | cmd_exec.rs:51 | AUD-P-035 |
| Seta-cima / Seta-baixo (console vazio) | Linha acima/abaixo no disassembly (`CmdCursorLineUp/Down`); Shift = 1 byte; Ctrl+Baixo = `CmdCursorRunUntil` | FAIL (diverge) | `keymap.rs:156-157`: Up/Down viram histórico de comandos. As funções `cursor_line_up/down` existem (cmd_exec.rs:67,85) sem tecla | AUD-P-036 |
| PgUp/PgDn | Cursor por página; Shift = 256 bytes, Ctrl = 4K; com texto: rolar histórico do console | FAIL (diverge) | `keymap.rs:159-160` rola o console 4 linhas. `cursor_page` existe (cmd_exec.rs:131) sem tecla | AUD-P-036 |
| Home / End | Cursor em $0000 / $8000 (console: rolar ao início/fim) | FAIL | sem tradução | AUD-P-035 |
| Enter (vazio) | Alterna console em tela cheia; no montador sai dele | FAIL | `Enter` sempre executa a linha (`input_enter`) | AUD-P-035 |
| Enter (com texto) | Executa o comando | IMPLEMENTED | emu.rs:634; `crates/applewin/tests/debugger.rs` |  |
| Backspace / Esc | Apagar caractere / limpar linha (Esc com paginação: pula) | IMPLEMENTED | keymap.rs:154-158 |  |
| ~ (Ctrl+~) | Insere "~" (Ctrl: console em tela cheia) | FAIL (Ctrl+~) | `debug_key` devolve `None` com Ctrl | AUD-P-035 |
| Ctrl+0..9 / Ctrl+Shift+0..9 | Ir ao marcador / criar marcador (`CmdBookmarkGoto/Add`) | FAIL (teclas) / IMPLEMENTED (BMG/BMA) | comandos existem; teclas não | AUD-P-035 |
| 0..9 (vendo a saída de vídeo) | Mostra páginas 0-5/9 do vídeo (`_ViewOutput`) | FAIL | comandos `HGR0..`/`TEXT..` existem (`cmd_view`); a tecla não | AUD-P-035 |
| F7 (entrar/sair) | `ToggleDebugger` | IMPLEMENTED | actions.rs:72 |  |
| Ctrl+F11 (passo) | Sem equivalente: WinFrame.cpp:1286 só trata F11 sem Ctrl (save-state); o port acrescentou `Action::Step` | EXTENSÃO | actions.rs:78, emu.rs:567 |  |

## 2. Comandos (`g_aCommands[]`, 251 entradas)

Contagem: 17 FAIL, 233 IMPLEMENTED, 1 N/A (total 251)

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
| `BPEDIT` | CMD_BREAKPOINT_EDIT | `BreakpointEdit` | FAIL | só pede redesenho (commands.rs:306); no AppleWin é editor de breakpoints |
| `BPE` | CMD_BREAKPOINT_ENABLE | `BreakpointEnable` | IMPLEMENTED |  |
| `BPL` | CMD_BREAKPOINT_LIST | `BreakpointList` | IMPLEMENTED |  |
| `BPSAVE` | CMD_BREAKPOINT_SAVE | `BreakpointSave` | IMPLEMENTED |  |
| `BPCHANGE` | CMD_BREAKPOINT_CHANGE | `BreakpointChange` | IMPLEMENTED |  |
| `BENCHMARK` | CMD_BENCHMARK | `Benchmark` | FAIL | `dispatch` imprime "Not available in the debugger core: BENCHMARK (needs the frontend)" (commands.rs:311); o botão da UI também está desativado (pages.rs:540) |
| `BW` | CMD_CONFIG_BW | `ConfigBw` | IMPLEMENTED |  |
| `COLOR` | CMD_CONFIG_COLOR | `ConfigColor` | IMPLEMENTED |  |
| `DISASM` | CMD_CONFIG_DISASM | `ConfigDisasm` | IMPLEMENTED |  |
| `FONT` | CMD_CONFIG_FONT | `ConfigFont` | FAIL | imprime "Not available" (commands.rs:314) |
| `HCOLOR` | CMD_CONFIG_HCOLOR | `ConfigHcolor` | FAIL | imprime "Not available" (commands.rs:315) |
| `LOAD` | CMD_CONFIG_LOAD | `ConfigLoad` | IMPLEMENTED |  |
| `MONO` | CMD_CONFIG_MONOCHROME | `ConfigMonochrome` | IMPLEMENTED |  |
| `SAVE` | CMD_CONFIG_SAVE | `ConfigSave` | IMPLEMENTED |  |
| `PWD` | CMD_CONFIG_GET_DEBUG_DIR | `ConfigGetDebugDir` | IMPLEMENTED |  |
| `CD` | CMD_CONFIG_SET_DEBUG_DIR | `ConfigSetDebugDir` | IMPLEMENTED |  |
| `RET` | CMD_CURSOR_JUMP_RET_ADDR | `CursorJumpRetAddr` | IMPLEMENTED |  |
| `^` | CMD_CURSOR_LINE_UP | `-` | FAIL | só existe como linha de ajuda de atalho no AppleWin; o port não tem a linha nem a tecla |
| `Shift ^` | CMD_CURSOR_LINE_UP_1 | `-` | FAIL | só existe como linha de ajuda de atalho no AppleWin; o port não tem a linha nem a tecla |
| `v` | CMD_CURSOR_LINE_DOWN | `-` | FAIL | só existe como linha de ajuda de atalho no AppleWin; o port não tem a linha nem a tecla |
| `Shift v` | CMD_CURSOR_LINE_DOWN_1 | `-` | FAIL | só existe como linha de ajuda de atalho no AppleWin; o port não tem a linha nem a tecla |
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
| `DISK` | CMD_DISK | `Disk` | FAIL | imprime "Not available" (commands.rs:344) |
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
| `NTSC` | CMD_NTSC | `Ntsc` | FAIL | imprime "Not available" (commands.rs:373) |
| `TSAVE` | CMD_TEXT_SAVE | `TextSave` | IMPLEMENTED |  |
| `CALC` | CMD_OUTPUT_CALC | `OutputCalc` | IMPLEMENTED |  |
| `ECHO` | CMD_OUTPUT_ECHO | `OutputEcho` | IMPLEMENTED |  |
| `LOG` | CMD_OUTPUT_LOG | `OutputLog` | IMPLEMENTED |  |
| `PRINT` | CMD_OUTPUT_PRINT | `OutputPrint` | IMPLEMENTED |  |
| `PRINTF` | CMD_OUTPUT_PRINTF | `OutputPrintf` | IMPLEMENTED |  |
| `RUN` | CMD_OUTPUT_RUN | `OutputRun` | IMPLEMENTED |  |
| `SOURCE` | CMD_SOURCE | `Source` | FAIL | imprime "Not available" (commands.rs:382) |
| `SYNC` | CMD_SYNC | `Sync` | FAIL | imprime "Not available" (commands.rs:382) |
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
| `->` | CMD_CURSOR_JUMP_PC | `-` | FAIL | só existe como linha de ajuda de atalho no AppleWin; o port não tem a linha nem a tecla |
| `Ctrl ->` | CMD_CURSOR_SET_PC | `-` | FAIL | só existe como linha de ajuda de atalho no AppleWin; o port não tem a linha nem a tecla |
| `Shift ->` | CMD_CURSOR_JUMP_PC | `-` | FAIL | só existe como linha de ajuda de atalho no AppleWin; o port não tem a linha nem a tecla |
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
| `BENCH` | CMD_BENCHMARK | `Benchmark` | FAIL | alias de BENCHMARK |
| `EXITBENCH` | CMD_BENCHMARK | `-` | FAIL | ausente da tabela do port |
| `MDB` | CMD_MEM_MINI_DUMP_HEX_1 | `MemMiniDumpHex1` | IMPLEMENTED |  |

## 3. Issues citadas na HU-0801

| Issue | Situação no AppleWin a8f0d1d | Situação no port | Estado |
|---|---|---|---|
| #1460 Ctrl-Right não atualiza alvos | corrigida (fechada em 2026-01-01) | os alvos são calculados no `render` a partir do PC; o comando `=` funciona | PASS (função), FAIL (sem tecla) |
| #1456 Right-arrow deve seguir RTS | corrigida (2026-01-01) | `target_address` trata RTS/RTI com a volta da pilha (disasm.rs:363-375) | PASS (função), FAIL (sem tecla) |
| #1451 Right-arrow não localiza a instrução atual (doc diz Alt+Right/Tab) | corrigida (2026-05-23); mapeamento atual: Tab = centro no PC, Alt+Right = centro, Shift+Right = topo, Right = seguir alvo | nenhuma dessas teclas existe | FAIL |

## 4. Reprodução

```
cargo test -p applewin --test audit_wave0_peripherals -- --include-ignored aud_p_035
cargo test -p apple2-debugger --test audit_wave0_debugger
```
