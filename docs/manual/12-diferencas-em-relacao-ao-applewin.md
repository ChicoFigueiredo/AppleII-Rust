# Diferenças em relação ao AppleWin

O objetivo do projeto é a paridade com o AppleWin atual nas três plataformas. Esta página é a lista honesta do que ainda falta ou é diferente. Ela descreve o estado deste código na versão 0.2.0-alpha.1. Os números e as provas estão na pasta `docs/conformance` do repositório (veja `relatorio-final.md` e `gaps-and-known-limitations.md`); aqui fica o que importa para quem usa o programa.

## O que foi medido contra o AppleWin

"Medido" quer dizer que um teste automático rodou o AppleWin (um Oráculo executável, baseado no fork Linux do audetto) e o AppleII-Rust com a mesma configuração e comparou o resultado. Windows nativo e macOS ainda **não** foram testados com um programa instalado.

- **CPU e temporização.** Trace instrução a instrução idêntico (ciclo, PC, registradores e flags) em 2 a 5 milhões de ciclos nos Modelos ][, ][+, //e e //e Enhanced, inclusive o boot do DOS 3.3; mais de 10 milhões de vetores de hardware do 6502 e do 65C02.
- **Vídeo.** 1.573 telas completas comparadas pixel a pixel (todos os modos, os 9 tipos de vídeo, NTSC e PAL, cartas RGB e VidHD). A exceção é o Color Monitor PAL em uma cena específica de floating bus (veja abaixo).
- **Disco.** 56 casos de Disk II e disco rígido, mais de 103 milhões de instruções comparadas, com a imagem final conferida. Inclui WOZ reais do corpus do AppleWin.
- **Save-states.** 79 de 115 arquivos `.aws.yaml` reais (do AppleWin 1.32.1.0 e do corpus AppleWin-Test) importam e rodam 100.000 ciclos com o mesmo trace do AppleWin. Os que não batem estão listados abaixo.
- **Áudio.** As escritas nos chips, os timers e as interrupções do Mockingboard, do Phasor e do SSI263 têm eventos idênticos aos do AppleWin, ciclo a ciclo. A forma de onda (PCM) não é idêntica (veja abaixo).
- **Z80.** O núcleo passa nos 1.604.000 vetores do SingleStepTests e nos testes ZEXDOC e ZEXALL. Não foi comparado com o Z80 do AppleWin.
- **Depurador.** 62 das 86 issues do AppleWin sobre o depurador têm teste que passa; 10 pedem uma mudança que o AppleWin também não faz (veja abaixo).

O que continua sem comparação: linha de comando, janela de configuração, atalhos F1 a F12 e menus (só testes do próprio projeto); vídeo dos clones (Pravets, TK3000, Base64A, J-Plus); jogos de terceiros; hardware físico (gamepad, serial, rede real, som que se escuta).

## Diferenças medidas (FAIL)

- **PCM do Mockingboard/Phasor e do alto-falante.** Os eventos são idênticos, mas a forma de onda gerada pelo AppleWin tem blocos truncados (cerca de 0,4 % mais curtos) e o alto-falante é amostrado a cada 23 ciclos; o AppleII-Rust gera cada amostra no ciclo exato. Fora dos limiares de comparação definidos antes do teste (quatro casos). Na prática, o som é igual ao ouvido, mas não é bit a bit.
- **Primeiro fonema do SSI263** pode terminar 13 ciclos além da tolerância em uma das fases de teste.
- **IRQ do 6522 no último ciclo do opcode (AppleWin #1176).** O AppleWin trata a interrupção logo depois de `CLI`; o AppleII-Rust a adia por uma instrução nesse caso específico (dois 6522 com T1 contínuo).
- **Color Monitor PAL com floating bus (AppleWin #241).** Em uma cena do Ansi Story (texto e DLORES alternados por linha, página 2), 1 pixel de borda difere.
- **Nove save-states reais não batem:** `SNES_MAX-slot4`, os três `dazzledraw1` (a interrupção de VBL do mouse chega 7 ciclos antes depois de carregar), `SmartPort-firmware`, `Saturn64`, `Saturn128`, `6522-one-shot` e `SSI263-test`.
- **Depurador.** Dez comportamentos que as issues do AppleWin pedem para mudar são iguais aos do AppleWin: `SYMUSER LOAD` sem aspas, `SYMUSER SAVE` sem efeito, nomes de símbolo com `#`, `CD ..`, aspas simples e duplas na busca, ordem de `APPLE2E.SYM`, só 4 tabelas de símbolos na partida, `DA` com símbolo exato, e o mini dump `M` do Language Card por banco ou slot.

## Onde o AppleWin difere do hardware e o AppleII-Rust copia o AppleWin

O AppleII-Rust segue o AppleWin onde ele difere do hardware real (ADR 0004). Nenhum desvio foi aprovado como Divergência intencional.

- O 65C02 não tem as instruções Rockwell/WDC `BBR`, `BBS`, `RMB`, `SMB`, `WAI` e `STP`: são NOP de 1 ciclo.
- Os ciclos "fantasma" (leituras e escritas espúrias de alguns modos de endereçamento, que no hardware real podem tocar `$C0xx`) não são feitos.
- O floating bus com `RAMRD=1` lê a RAM auxiliar, como o AppleWin; no hardware real leria a principal.
- O NMI não chega à CPU, como no AppleWin. A SoftCard Z80 não liga as linhas de interrupção, como no AppleWin.
- Uthernet II sem SPI, PPPoE e interrupções; fita cassete não emulada; Apple IIc só o esqueleto; VidHD sem texto 80x45 nem cor de borda.

## Fábrica de cartas

O frontend cria, a partir da configuração, todas as cartas que o AppleWin aceita: Disk II (inclusive o firmware de 13 setores), controlador de disco rígido (imagens da configuração), Super Serial Card (TCP, porta COM com o recurso `serialport`, ou laço local), impressora paralela (arquivo `Printer.txt` ou o escolhido), Uthernet I e II (rede real exige o recurso `pcap`; sem ele a carta fica sem rede, mas o DNS virtual do Uthernet II funciona), Mockingboard C, Phasor, MegaAudio, SD Music, SAM, mouse, Z80 SoftCard, 4Play, SNES MAX, VidHD (a tela passa a 640x400 e o Super Hi-Res é desenhado), Breakpoint card, No-Slot Clock, dongles da porta de jogos, Language Card, Saturn, RamWorks e a carta AUX. Generic Clock e Echo são cartas "vazias" também no AppleWin.

Limitações: o ThunderClock Plus existe na biblioteca, mas não há como escolhê-lo na configuração (o AppleWin também não o oferece). A Breakpoint card recebe os breakpoints do depurador. O Mockingboard tem um tipo de AY por soquete (os quatro soquetes do Phasor e as chaves `6522a-bad` e `6522b-bad` existem). No SNES MAX, os controles do computador e as opções `-snes-max-*` estão ligados; o estado real `SNES_MAX-slot4` do corpus do AppleWin ainda difere no primeiro acesso ao controle.

## Interface

- **Janela de configuração.** Reproduz as páginas do AppleWin (Configuration, Input, Slots, Advanced e as opções de cada carta), mais uma aba Sound para os interruptores de som que o AppleWin só tem na linha de comando. O volume é um só, como no AppleWin atual. Sem o recurso `pcap`, a lista de interfaces de rede vem do sistema (`if-addrs`), descrita pelos endereços IP; a de portas seriais vem do `serialport` e de uma varredura por sistema.
- **Janela única.** A tela, a barra e as janelas de ajuda e configuração são desenhadas dentro da mesma janela, com a biblioteca egui, e não por caixas de diálogo do sistema.
- **Idioma.** Os textos da interface são em inglês; este manual está em português do Brasil.
- **Ajuda.** F1 abre este manual dentro do emulador, no lugar do arquivo CHM do AppleWin.
- **Discos.** O menu do drive (Ctrl+F3/F4 ou botão direito) tem inserir, ejetar, proteção contra escrita, imagens recentes, trocar drives e "Create new disk" (em branco, DOS 3.3 ou ProDOS). Os discos rígidos aparecem no mesmo menu e a troca vale na hora. "Open as Read Only" é um botão do menu do drive (o seletor de arquivos do sistema não tem a caixa). Soltar um arquivo segue o AppleWin (drive 2 se solto sobre o botão do drive 2; reinicia, menos se solto sobre o botão do drive 1); no Windows o winit não informa a posição do arquivo solto, então vale a última posição do cursor. Não há o menu "Send to CiderPress". O que vale na hora e o que pede reinício está em "Imagens de disco".
- **Registro de tipos de arquivo.** O AppleWin registra as extensões de disco no Windows; este programa não.
- **Tela cheia.** Alt+Enter alterna a tela cheia; com `-alt-enter=open-apple-enter` (ou desmarcando a opção) vira Open Apple + Return, como no AppleWin.
- **Status de trilha e setor** dos drives (2x) é mostrado na linha de status, não numa dica sobre o LED. O setor (`S??`) é o da RWTS do DOS 3.3 ou do ProDOS, como no AppleWin.
- **Volume.** Há um só volume ("Master volume"), que vale para o alto-falante e para o Mockingboard, como no AppleWin atual; o "Speaker Volume" antigo é lido como o mesmo valor.

## Teclado

- **Ctrl+V** não cola (envia ^V ao Apple, como no AppleWin); a colagem extra é Ctrl+Shift+V. O AppleWin não toca sons de drive, e este programa também não.
- A imagem de Print Screen vai para o diretório atual como `AppleWin_NNNN.png`. Ctrl+Print Screen copia o texto de 40 ou 80 colunas e a página 2, como o AppleWin.
- O "SHIFT key mod" liga o Shift ao botão 2 e, como no AppleWin, não conta como joystick 1: a temporização dos paddles 2 e 3 continua "sempre ativa".
- As opções de captura de teclas do sistema (`-hook-alt-tab`, `-no-hook-system-key` e outras) são aceitas, mas só fazem sentido no AppleWin para Windows e não têm efeito aqui.

## Depurador

- Os atalhos de teclado do depurador do AppleWin (setas, Tab, Espaço para passo, Shift+Espaço para sair de sub-rotina, Ctrl+Espaço para passar sobre, Ctrl+números como marcadores, roda do mouse, Home/End, Page Up/Down) existem. Ctrl+F11 como passo é uma extensão. A paginação de saída longa do console (`...press SPACE continue, ESC skip...`) não foi portada: o console é rolável.
- Os comandos funcionam como no AppleWin. `ME` (editor de memória) também não é implementado no AppleWin.

## Arquivos e configuração

- **Formato de configuração.** O AppleWin guarda a configuração no Registro do Windows (ou num INI no Linux). Este programa usa um arquivo TOML. O módulo `apple2-config` sabe importar um `.reg` e um INI do AppleWin; a interface ainda não oferece um comando de importação.
- **`-conf arquivo`**: um `.ini` ou `.reg` do AppleWin é importado na partida; qualquer outro arquivo é lido e gravado como `settings.toml` alternativo. `-current-dir`, `-load-state`, `-screenshot-and-exit` (grava um PNG e sai) e `-debugger-auto-run` funcionam. `-power-on` é aceito (o programa já inicia em execução, sem tela de logo).
- **`--help`, `-h` e `--version`** imprimem o texto e saem.
- **Save-state.** F11 e F12 abrem o seletor de arquivos, como no AppleWin, e usam o formato `.aws.yaml` do AppleWin (padrão `SaveState.aws.yaml` no diretório atual, ou o nome do disco inserido). "Save State On Exit" é respeitado. Soltar um `.aws.yaml` na janela o carrega. O nome padrão não muda sozinho ao trocar de disco durante a sessão.
- **`DELREG.INF`.** Não existe; para restaurar os padrões, apague `settings.toml`.

## Emulação

- **CPU.** Os opcodes não documentados do NMOS 6502 que o AppleWin implementa (ASO/SLO, RLA, LSE/SRE, RRA, AXS/SAX, LAX, DCM/DCP, INS/ISC, ANC, ALR, ARR, XAA, OAL, SAX, LAS, AXA, SAY, TAS, XAS, os NOP com tamanho e ciclos corretos e o HLT que trava o PC) foram portados com as mesmas fórmulas, inclusive as peculiaridades (por exemplo, DCP de $00 limpa o carry). O 65C02 não tem as instruções Rockwell/WDC (BBR, BBS, RMB, SMB, WAI, STP): $CB e $DB são NOP de 1 ciclo, igual ao AppleWin. O teste estendido de Klaus Dormann roda com essas instruções emuladas pelo próprio teste (a versão pura está `#[ignore]`). O reset segue o `CpuReset` do AppleWin (S desce 3, flags preservadas); a emulação não faz leituras falsas (a única é a do 6522 do Mockingboard, na carta). O NMI não chega à CPU, como no AppleWin (lá o tratamento está desativado no código).
- **Apple IIc.** Só o esqueleto, igual ao AppleWin.
- **VidHD.** Os modos de texto 80x45 e a cor da borda não são modelados, como no AppleWin. A tela a 640x400 e o Super Hi-Res foram comparados pixel a pixel.
- **Z80.** Sem entrega de interrupções pela carta, como no AppleWin. Nos blocos que se repetem (`LDIR`, `CPIR`, `INIR`, `OTIR`) os bits 3 e 5 de F seguem o hardware, o que é uma possível diferença em relação ao Z80 do AppleWin; ela ainda não foi comparada nem aprovada como Divergência intencional.
- **Placas RGB.** O estado da placa AppleColor/RGB (modo, AN3 anterior, inversão do bit 7) vai e volta pelo save-state.
- **Uthernet II.** PPPoE, interrupções e SPI não estão implementados (modos sem suporte falham fechados), como no AppleWin; o lado servidor tem testes locais em loopback e os sockets TCP e UDP são fechados ao carregar um save-state, como no AppleWin. Há extensões que o AppleWin não tem: `LISTEN` para servidores TCP e vinculação de sockets UDP à porta pedida.
- **Fita cassete.** Não é emulada, igual ao AppleWin (apenas `$C020` e `$C060` leem o Floating bus).
- **SSC.** As limitações do 6551 são as mesmas do AppleWin (veja "Cartas por slot").
- **Fala.** A duração e a ordem dos fonemas do SSI263 e do SC-01 foram comparadas com o AppleWin; o timbre (PCM) da fala fica para o roteiro de audição, que depende de um ouvinte. O SSI263 reproduz os fonemas em taxa fixa, sem inflexão nem filtros, como o AppleWin.
- **No-Slot Clock no ][ e ][+.** O soquete "F8" ainda não funciona com o ProDOS-8 2.5.0a8, igual ao AppleWin.

## Extensões que o AppleWin não tem

- Camada de execução sem janela (`--headless-frames`, `--dump-screen`, `--dump-text`, `--type`) para testes e integração contínua.
- Bloco de extensão no YAML do save-state, que permite retomar de forma exata.
- Relógio ThunderClock Plus emulado de verdade (no AppleWin é um esqueleto).
- Sockets UDP e servidor TCP no Uthernet II.
- Um núcleo determinístico sem dependência de janela, áudio ou relógio do sistema.
- Depuração temporal (voltar no tempo, capítulo 16), adaptador DAP para o VS Code (capítulo 15) e laboratório educacional de Assembly (capítulo 14).
- Protótipo opt-in de frontend web (WASM), sem peso no programa de mesa.
