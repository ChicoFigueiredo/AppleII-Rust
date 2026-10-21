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

Limitações: a Breakpoint card existe, mas o depurador ainda não lhe entrega os breakpoints; no SNES MAX os controles do computador estão ligados, mas as opções `-snes-max-alt-joy*` e os arquivos `-snes-max-user-joy` ainda não valem; o Mockingboard usa um só tipo de AY por carta (os soquetes 2 e 3 do Phasor e as chaves `6522a-bad` não são modelados); o ThunderClock Plus existe na biblioteca, mas não há como escolhê-lo na configuração.

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

- Só o console foi ligado à interface. Os atalhos de teclado do depurador do AppleWin (rolar o código com as setas, Tab, Espaço para passo, Shift+Espaço para sair de sub-rotina, Ctrl+setas) não existem. Os mesmos efeitos estão nos comandos.
- Os comandos funcionam como no AppleWin. `ME` (editor de memória) também não é implementado no AppleWin.

## Arquivos e configuração

- **Formato de configuração.** O AppleWin guarda a configuração no Registro do Windows (ou num INI no Linux). Este programa usa um arquivo TOML. O módulo `apple2-config` sabe importar um `.reg` e um INI do AppleWin; a interface ainda não oferece um comando de importação.
- **`-conf arquivo`**: um `.ini` ou `.reg` do AppleWin é importado na partida; qualquer outro arquivo é lido e gravado como `settings.toml` alternativo. `-current-dir`, `-load-state`, `-screenshot-and-exit` (grava um PNG e sai) e `-debugger-auto-run` funcionam. `-power-on` é aceito (o programa já inicia em execução, sem tela de logo).
- **`--help`, `-h` e `--version`** imprimem o texto e saem.
- **Save-state.** F11 e F12 abrem o seletor de arquivos, como no AppleWin, e usam o formato `.aws.yaml` do AppleWin (padrão `SaveState.aws.yaml` no diretório atual, ou o nome do disco inserido). "Save State On Exit" é respeitado. Soltar um `.aws.yaml` na janela o carrega. O nome padrão não muda sozinho ao trocar de disco durante a sessão.
- **`DELREG.INF`.** Não existe; para restaurar os padrões, apague `settings.toml`.

## Emulação

- **CPU.** Os opcodes não documentados do NMOS 6502 que o AppleWin implementa (ASO/SLO, RLA, LSE/SRE, RRA, AXS/SAX, LAX, DCM/DCP, INS/ISC, ANC, ALR, ARR, XAA, OAL, SAX, LAS, AXA, SAY, TAS, XAS, os NOP com tamanho e ciclos corretos e o HLT que trava o PC) foram portados com as mesmas fórmulas, inclusive as peculiaridades (por exemplo, DCP de $00 limpa o carry). O 65C02 não tem as instruções Rockwell/WDC (BBR, BBS, RMB, SMB, WAI, STP): $CB e $DB são NOP de 1 ciclo, igual ao AppleWin. O teste estendido de Klaus Dormann roda com essas instruções emuladas pelo próprio teste (a versão pura está `#[ignore]`). O reset segue o `CpuReset` do AppleWin (S desce 3, flags preservadas); a emulação não faz leituras falsas (a única é a do 6522 do Mockingboard, na carta). O NMI do AppleWin não é tratado pela CPU deles; aqui ele existe.
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
