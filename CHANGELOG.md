# Changelog

## v0.1.0 — 2026-10-20

Primeira versão: reescrita em Rust do [AppleWin](https://github.com/AppleWin/AppleWin), com paridade de funcionalidades em Windows, Linux e macOS. As diferenças conhecidas estão em [docs/manual/12-diferencas-em-relacao-ao-applewin.md](docs/manual/12-diferencas-em-relacao-ao-applewin.md).

### Emulação
- CPU 6502 (incluindo os opcodes não documentados do AppleWin) e 65C02, com tempo por ciclo; passa nos testes de Klaus Dormann e nos testes do próprio AppleWin.
- Todos os modelos do AppleWin: II, II+, J-Plus, IIe, IIe Enhanced, Pravets 82/8M/8A, TK3000 //e e Base64A (IIc apenas como esqueleto, igual ao AppleWin).
- Memória //e completa: Language Card, Saturn 128K, 80 colunas, Extended 80 colunas e RamWorks III; floating bus e contador de vídeo NTSC/PAL.
- Vídeo: todos os modos (texto 40/80, LORES, DLORES, HIRES, DHIRES, misto), os 9 tipos de vídeo do AppleWin, cartas RGB e Super Hi-Res da VidHD.
- Disco: Disk II (DSK, DO, PO, NIB, WOZ1/2, 2MG, zip/gz) com gravação, disco rígido e SmartPort.
- Som: alto-falante, Mockingboard C, Phasor, MegaAudio, SD Music, SSI263, SC-01 e SAM.
- Cartas: mouse, Z80 SoftCard, relógios (No-Slot Clock, ThunderClock Plus), Super Serial Card, impressora, Uthernet I e II, 4Play, SNES MAX, VidHD, Breakpoint card e dongles de proteção.

### Aplicativo
- Janela com barra de ferramentas e atalhos do AppleWin (F1–F12), tela de configuração completa, menus de drive e criação de discos.
- Ícones SVG redesenhados a partir dos bitmaps do AppleWin.
- Depurador do AppleWin (F7): disassembler, assembler, símbolos, breakpoints e comandos.
- Save-states no formato `.aws.yaml` do AppleWin (F11/F12).
- Linha de comando compatível com a do AppleWin; configuração em TOML, com importação do registro/INI do AppleWin.
- Joysticks em todos os modos do AppleWin, teclado por layout do sistema, colagem de texto.
- Manual em português no próprio programa (F1).

### Distribuição
- Pacotes para Windows x64, Linux x64 e macOS universal, com os discos do DOS 3.3 e do ProDOS.
