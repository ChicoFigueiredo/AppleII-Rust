# Plano até o produto final

Combinado com o usuário em 2026-10-19: Opus supervisiona e revisa, agentes Sonnet implementam, com até 6 em paralelo. A meta continua sendo paridade com o que o AppleWin faz hoje, nas três plataformas.

## Arquitetura de crates

| Crate | Conteúdo | Fonte AppleWin |
|---|---|---|
| `apple2-cpu` | 6502/65C02 | CPU/ |
| `apple2-core` | Máquina headless: memória, LC, slots, vídeo (estado), teclado, alto-falante (toques), porta de jogos, save-state serde | Memory, LanguageCard, CardManager, Keyboard, Joystick, NTSC (scanner) |
| `apple2-roms` | ROMs e assets embutidos | resource/, bin/ |
| `apple2-video` | Renderizador: todos os modos, NTSC/RGB/mono, charsets, 80col, DHGR, cartas RGB | NTSC.cpp, NTSC_CharSet, RGBMonitor, Video.cpp |
| `apple2-storage` | Formatos de imagem, Disk II, HDD/SmartPort, fita | Disk*, DiskImage*, Harddisk, ProDOS_*, Tape, minizip/zlib |
| `apple2-audio` | Síntese do alto-falante, Mockingboard/Phasor (6522, AY8910, SSI263, SC01), SAM, MegaAudio, SDMusic | Speaker, Mockingboard*, 6522, AY8910, SSI263, SAM, SoundCore |
| `apple2-cards` | Mouse, Z80 SoftCard, relógios, No-Slot-Clock, 4Play, SNES MAX, VidHD, Breakpoint card, dongles | MouseInterface, 6821, z80emu/Z80VICE, Peripheral_Clock_*, NoSlotClock, FourPlay, SNESMAX, VidHD, CopyProtectionDongles |
| `apple2-hostio` | Cartas ligadas ao hospedeiro: SSC (serial/TCP), impressora paralela, Uthernet I (CS8900A) e II (W5100) | SerialComms, ParallelPrinter, Uthernet1/2, Tfe/ |
| `apple2-debugger` | Debugger completo sem UI: disassembler, assembler, símbolos, breakpoints, console de comandos | Debugger/ |
| `applewin` (bin) | Frontend winit + wgpu + egui + cpal + gilrs: janela, toolbar, configuração, CLI, save-state YAML do AppleWin, UI do debugger | Windows/, Configuration/, CmdLine, SaveState, Registry, Frame* |

## Regras para os agentes

- Cada agente é dono de um crate novo, trabalha numa worktree própria e faz commit no seu branch. O supervisor faz o merge.
- `apple2-core` só recebe ganchos mínimos e genéricos, listados no relatório. Não mexer em `CardKind` (config.rs): a fábrica de cartas fica no frontend.
- O núcleo continua sem I/O do hospedeiro (ADR 0001). Arquivos, rede, serial e relógio do host entram por traits, com as implementações nos crates de fora.
- TDD, clippy sem avisos, `cargo test --workspace --release` verde.

## Ondas

1. **Onda 1** (6 em paralelo): vídeo, storage, áudio, cartas, cartas de host, debugger.
2. **Onda 2**: frontend (shell, render, áudio, entrada), configuração e CLI, save-state YAML, UI do debugger, CI e empacotamento para Windows, Linux e macOS, testes de integração com imagens reais (DOS 3.3 e ProDOS dando boot).
3. **Revisão final** e publicação.

## Auditoria de paridade e conformidade

Depois das ondas acima, a equipe de auditoria escreveu um plano para provar a paridade com o AppleWin e evoluir o produto: [docs/prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md](prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). Ele será executado integralmente. As decisões sobre o Oráculo estão na [ADR 0004](adr/0004-oraculo-de-conformidade.md), e os resultados ficam em [docs/conformance](conformance/README.md).
