# Diferenças em relação ao AppleWin

O objetivo do projeto é a paridade com o AppleWin atual nas três plataformas. Esta página é a lista honesta do que ainda falta ou é diferente. Ela descreve o estado deste código no momento em que o manual foi escrito; os itens "pendentes" devem desaparecer à medida que forem integrados.

## O que já é igual

- Todos os Modelos do AppleWin (exceto o IIc, que nem lá tem ROM), com os mesmos valores, ROMs e peculiaridades de memória, Language Card, Saturn, cartas AUX e Floating bus.
- Temporização por Ciclo, incluindo o escalonador de eventos e a regra de IRQ no último ciclo de um opcode.
- Renderização NTSC, monitores RGB e todos os tipos de vídeo.
- Disk II (DSK, DO, PO, NIB, WOZ, 2MG, compactados), disco rígido (ProDOS e SmartPort).
- Mockingboard C, Phasor, SSI263, SC-01, SAM, MegaAudio e SD Music.
- A linha de comando, com as grafias do AppleWin.
- O depurador, com a mesma tabela de comandos e o mesmo texto de ajuda.
- O formato YAML dos save-states do AppleWin (módulo `apple2-savestate`).
- Os atalhos F1 a F12, Pause, Scroll Lock, Print Screen e a colagem.

## Fábrica de cartas

O frontend cria, a partir da configuração, todas as cartas que o AppleWin aceita: Disk II (inclusive o firmware de 13 setores), controlador de disco rígido (imagens da configuração), Super Serial Card (TCP, porta COM com o recurso `serialport`, ou laço local), impressora paralela (arquivo `Printer.txt` ou o escolhido), Uthernet I e II (rede real exige o recurso `pcap`; sem ele a carta fica sem rede, mas o DNS virtual do Uthernet II funciona), Mockingboard C, Phasor, MegaAudio, SD Music, SAM, mouse, Z80 SoftCard, 4Play, SNES MAX, VidHD (a tela passa a 640x400 e o Super Hi-Res é desenhado), Breakpoint card, No-Slot Clock, dongles da porta de jogos, Language Card, Saturn, RamWorks e a carta AUX. Generic Clock e Echo são cartas "vazias" também no AppleWin.

Limitações: a Breakpoint card existe, mas o depurador ainda não lhe entrega os breakpoints; o SNES MAX nasce sem controles ligados e os arquivos `-snes-max-user-joy` ainda não são lidos; o Mockingboard usa um só tipo de AY por carta (os soquetes 2 e 3 do Phasor e as chaves `6522a-bad` não são modelados); o ThunderClock Plus existe na biblioteca, mas não há como escolhê-lo na configuração.

## Interface

- **Janela de configuração.** É uma versão reduzida: Modelo, padrão de vídeo, velocidade, volume, escala da janela, escala inteira e som. Cartas, discos, joysticks e as demais opções precisam ser editados no arquivo `settings.toml` ou passados na linha de comando. O AppleWin tem uma caixa de diálogo com várias abas.
- **Janela única.** A tela, a barra e as janelas de ajuda e configuração são desenhadas dentro da mesma janela, com a biblioteca egui, e não por caixas de diálogo do sistema.
- **Idioma.** Os textos da interface são em inglês; este manual está em português do Brasil.
- **Ajuda.** F1 abre este manual dentro do emulador, no lugar do arquivo CHM do AppleWin.
- **Discos.** O seletor de arquivos só abre imagens existentes; não há "New ProDOS" e "New DOS 3.3" no menu do drive, nem a caixa "Open as Read Only", nem soltar uma imagem diretamente sobre o botão de um drive (um arquivo solto sempre vai ao drive 1 e reinicia). Não há o menu "Send to CiderPress".
- **Registro de tipos de arquivo.** O AppleWin registra as extensões de disco no Windows; este programa não.
- **Tela cheia.** Alt+Enter alterna a tela cheia (o `-alt-enter=open-apple-enter` é aceito mas não muda o comportamento nesta versão).
- **Status de trilha e setor** dos drives (2x) é mostrado na linha de status, não numa dica sobre o LED.

## Teclado

- Os atalhos de velocidade **Ctrl+0**, **Ctrl+1** e **Ctrl+3** do AppleWin não existem. Use o controle de velocidade da configuração e o Scroll Lock.
- **Shift+Print Screen** (imagem 280x192) e **Ctrl+Print Screen** (copiar o texto da tela para a área de transferência) não existem. Print Screen salva sempre a imagem de 560x384, no diretório atual, como `AppleWin_NNNN.png`.
- As opções de captura de teclas do sistema (`-hook-alt-tab`, `-no-hook-system-key` e outras) são aceitas, mas só fazem sentido no AppleWin para Windows e não têm efeito aqui.

## Depurador

- Só o console foi ligado à interface. Os atalhos de teclado do depurador do AppleWin (rolar o código com as setas, Tab, Espaço para passo, Shift+Espaço para sair de sub-rotina, Ctrl+setas) não existem. Os mesmos efeitos estão nos comandos.
- Os comandos funcionam como no AppleWin. `ME` (editor de memória) também não é implementado no AppleWin.

## Arquivos e configuração

- **Formato de configuração.** O AppleWin guarda a configuração no Registro do Windows (ou num INI no Linux). Este programa usa um arquivo TOML. O módulo `apple2-config` sabe importar um `.reg` e um INI do AppleWin; a interface ainda não oferece um comando de importação.
- **`-conf arquivo`**: um `.ini` ou `.reg` do AppleWin é importado na partida; qualquer outro arquivo é lido e gravado como `settings.toml` alternativo. `-current-dir`, `-load-state`, `-screenshot-and-exit` (grava um PNG e sai) e `-debugger-auto-run` funcionam. `-power-on` é aceito (o programa já inicia em execução, sem tela de logo).
- **`--help`, `-h` e `--version`** imprimem o texto e saem.
- **Save-state.** F11 e F12 abrem o seletor de arquivos, como no AppleWin, e usam o formato `.aws.yaml` do AppleWin (padrão `SaveState.aws.yaml` no diretório atual, ou o nome do disco inserido). "Save State On Exit" é respeitado. Soltar um `.aws.yaml` na janela o carrega. O nome padrão não muda sozinho ao trocar de disco durante a sessão.
- **`DELREG.INF`.** Não existe; para restaurar os padrões, apague `settings.toml`.

## Emulação

- **CPU.** Os opcodes não documentados do NMOS 6502 são tratados como NOP do tamanho correto (o AppleWin implementa alguns). O 65C02 não tem as instruções de bit Rockwell/WDC (BBR, BBS, RMB, SMB), igual ao AppleWin; o teste estendido de Klaus Dormann está marcado como `#[ignore]` por isso.
- **Apple IIc.** Só o esqueleto, igual ao AppleWin.
- **VidHD.** Os modos de texto 80x45 e a cor da borda não são modelados.
- **Placas RGB.** O estado da placa AppleColor/RGB não é guardado no save-state.
- **Uthernet II.** PPPoE, interrupções e SPI não estão implementados, o lado servidor tem pouco teste e os sockets TCP e UDP são fechados ao carregar um save-state. Há extensões que o AppleWin não tem: `LISTEN` para servidores TCP e vinculação de sockets UDP à porta pedida.
- **Fita cassete.** Não é emulada, igual ao AppleWin (apenas `$C020` e `$C060` leem o Floating bus).
- **SSC.** As limitações do 6551 são as mesmas do AppleWin (veja "Cartas por slot").
- **Fala.** O SSI263 reproduz os fonemas em taxa fixa, sem inflexão nem filtros.
- **No-Slot Clock no ][ e ][+.** O soquete "F8" ainda não funciona com o ProDOS-8 2.5.0a8, igual ao AppleWin.

## Extensões que o AppleWin não tem

- Camada de execução sem janela (`--headless-frames`, `--dump-screen`, `--dump-text`, `--type`) para testes e integração contínua.
- Bloco de extensão no YAML do save-state, que permite retomar de forma exata.
- Relógio ThunderClock Plus emulado de verdade (no AppleWin é um esqueleto).
- Sockets UDP e servidor TCP no Uthernet II.
- Um núcleo determinístico sem dependência de janela, áudio ou relógio do sistema.
