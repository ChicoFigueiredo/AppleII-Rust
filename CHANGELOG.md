# Changelog

## Não lançado

### Adicionado
- **Impressora virtual Epson LX-310** (ADR 0008; guia em [docs/printer/README.md](docs/printer/README.md)): a Generic Printer do slot 1 entrega os bytes a uma impressora de 9 agulhas emulada (ESC/P, Draft e NLQ, gráficos de bit-image, tabelas PC437/PC850/PC860/BRASCII/ABICOMP). A **Central de Impressoras** abre sozinha no primeiro byte, mostra as Páginas ao vivo em papel contínuo zebrado (ou A4/Carta), com Fita nova ou usada, e **Salvar PDF** grava pontos vetoriais com camada de texto invisível e metadados. Cada Trabalho de impressão fica no **Arquivo de impressões** permanente (`apple2-rust/impressoes`), com os bytes brutos e reimpressão em versões; nada é apagado sozinho.
- **Transporte Automático** (padrão) na impressora virtual: o que passa pelo firmware da Carta (`PR#1`, `PRINT`, `LIST`) perde o bit 7 e o que um programa escreve direto na porta chega com 8 bits (necessário para gráficos ESC K/L), com opções fixas de 8 e de 7 bits; o `Printer.txt` continua como no AppleWin (7 bits, filtrado). Destino configurável: virtual, arquivo ou ambos (padrão).
- Testes de aceite sem interface gráfica (`crates/apple2-hostio/tests/printer_aceite_*.rs`) e roteiro com AppleWorks, Print Shop e dBase II ([docs/printer/aceite.md](docs/printer/aceite.md)). Capítulo "Impressora virtual" no manual (F1).

### Corrigido
- Com a janela aberta, o renderizador de vídeo contornava `Machine::run_frame` e nenhuma Carta recebia `update`: a Super Serial Card, as Uthernet e o limite de inatividade da impressora (que fecha o Trabalho) ficavam parados. A atualização das Cartas passa a ocorrer uma vez por quadro, também no passo a passo do depurador (commit c205910).

### Alterado
- O programa passa a se chamar **apple2-rust** (ADR 0007): executável `apple2-rust`, crates `apple2-rust`, `apple2-rust-dap` e `apple2-rust-web`, pacotes `apple2-rust-<versão>-<plataforma>.*`, SBOM `apple2-rust-<versão>.cdx.json` e diretório de configuração `apple2-rust`. Não há alias `applewin`. O AppleWin segue como origem e oráculo, e a linha de comando, a importação de Registro/INI e o formato `.aws.yaml` continuam compatíveis.
- `cargo run` na raiz do repositório abre o emulador.
- Migração automática da configuração: na primeira execução, o `settings.toml` (e `recent_disks.txt`) do diretório antigo `applewin-rs` é copiado para o novo, com aviso na linha de status. O diretório antigo não é apagado.

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
