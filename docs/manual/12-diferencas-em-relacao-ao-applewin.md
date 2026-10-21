# Diferenças em relação ao AppleWin

O objetivo do projeto é a paridade com o AppleWin atual nas três plataformas. Esta página é a lista honesta do que ainda falta ou é diferente. Ela descreve o estado deste código na versão 0.2.0-rc.1 (pré-lançamento para Linux). Os números e as provas estão na pasta `docs/conformance` do repositório (veja `relatorio-final.md` e `gaps-and-known-limitations.md`); aqui fica o que importa para quem usa o programa.

## O que foi medido contra o AppleWin

"Medido" quer dizer que um teste automático rodou o AppleWin (um Oráculo executável, baseado no fork Linux do audetto) e o AppleII-Rust com a mesma configuração e comparou o resultado. Windows nativo e macOS ainda **não** foram testados com um programa instalado (o teste nativo no Windows está em andamento).

- **CPU e temporização.** Trace instrução a instrução idêntico (ciclo, PC, registradores e flags) em 2 a 5 milhões de ciclos nos Modelos ][, ][+, //e e //e Enhanced, inclusive o boot do DOS 3.3; mais de 10 milhões de vetores de hardware do 6502 e do 65C02.
- **Vídeo.** 1.573 telas completas comparadas pixel a pixel (todos os modos, os 9 tipos de vídeo, NTSC e PAL, cartas RGB e VidHD). Isso inclui a cena de floating bus do Ansi Story no Color Monitor PAL (AppleWin #241), que antes diferia em 1 pixel.
- **Disco.** 56 casos de Disk II e disco rígido, mais de 103 milhões de instruções comparadas, com a imagem final conferida. Inclui WOZ reais do corpus do AppleWin.
- **Save-states.** 88 de 115 arquivos `.aws.yaml` reais (do AppleWin 1.32.1.0 e do corpus AppleWin-Test) importam e rodam 100.000 ciclos com o mesmo trace do AppleWin; nenhum diverge. Os outros 27 não puderam ser comparados (imagem de disco ausente do corpus, rede ou serial reais, hora real, Shift Key Mod, Z80).
- **Áudio.** As escritas nos chips, os timers e as interrupções do Mockingboard, do Phasor e do SSI263 têm eventos idênticos aos do AppleWin, ciclo a ciclo. A forma de onda (PCM) não é idêntica (veja "Limitações aceitas"). O IRQ do 6522 no último ciclo do opcode (AppleWin #1176) é tratado como no AppleWin.
- **Z80.** O núcleo passa nos 1.604.000 vetores do SingleStepTests e nos testes ZEXDOC e ZEXALL. Não foi comparado com o Z80 do AppleWin.
- **Depurador.** Das 91 issues do AppleWin sobre o depurador que se aplicam, 79 têm teste que passa (inclusive as seis Divergências aprovadas), 7 dependem de janela e de entrada e não têm Oráculo, e 5 ficam como no AppleWin (veja abaixo).

O que continua sem comparação: linha de comando, janela de configuração, atalhos F1 a F12 e menus (só testes do próprio projeto); vídeo dos clones (Pravets, TK3000, Base64A, J-Plus); jogos de terceiros; hardware físico (gamepad, serial, rede real, som que se escuta).

## Divergências aprovadas pelo dono

Em alguns pontos o AppleII-Rust faz de propósito algo diferente do AppleWin (commit `a8f0d1d`), porque o AppleWin tem ali um defeito que só atrapalha o usuário. Cada ponto foi aprovado pelo dono do projeto em 21/10/2026 ([ADR 0006](../adr/0006-divergencias-aprovadas-pelo-dono.md)). Nenhum deles toca a emulação do hardware (CPU, memória, vídeo, disco, áudio, cartas). A tabela técnica, com o código do AppleWin e o teste de cada um, está em `docs/conformance/gaps-and-known-limitations.md`.

- **Impressora Pravets (#916).** A conversão de latim para cirílico sai correta (CP-1251). No AppleWin as tabelas foram gravadas com o encoding errado e o texto sai quebrado.
- **`DebuggerAutoRun.txt` (#855).** O script roda com a máquina já pronta: `bsave` e `bpx` do script funcionam. No AppleWin o script roda antes da memória existir e esses comandos não têm efeito.
- **Alt+F2 (#1396).** Alt+F2 mantém o Open Apple apertado depois do reinício, o que permite, por exemplo, pular o boot do disco rígido. No AppleWin o F2 solta o Open Apple.
- **Tela cheia (#280, #358, #1374, #117, #932, #1448).** A tela ocupa toda a altura do monitor, mantém a proporção 4:3 (nunca estica em tela larga) e fica centralizada; a escala usa o monitor onde a janela está, e a velocidade "Fastest" funciona em todos os Modelos. A opção "Integer scaling" devolve a escala inteira do AppleWin.
- **Depurador (#114, #245, #458, #466, #468, #1381).**
  - `SYMUSER LOAD arquivo.sym` carrega o arquivo mesmo sem aspas, e `SYMUSER SAVE` grava o arquivo e mostra mensagem.
  - `SYM` aceita nomes com `#` (como `CMDNTB.PR#`), e `CD ..` volta ao diretório pai de verdade.
  - Ctrl+Print Screen copia as 48 linhas do console, não só 43.
  - `DA` com um símbolo exato não mostra mais `da A_2101-1` quando existe `B_2100`.

## Limitações aceitas

Estes pontos continuam diferentes do AppleWin e o dono decidiu que eles não impedem a versão (ADR 0006). Eles seguem como FAIL nos relatórios; nunca são contados como PASS.

- **PCM do Mockingboard/Phasor e do alto-falante, e o primeiro fonema do SSI263 (5 casos).** Os eventos dos chips são idênticos aos do AppleWin, ciclo a ciclo, mas as amostras diferem, porque o AppleWin gera o som em fatias de tempo do computador onde roda (blocos de amostras cerca de 0,4 % mais curtos, alto-falante amostrado a cada 23 ciclos, fim do fonema visto só a cada ~1000 ciclos). O AppleII-Rust gera cada amostra no ciclo exato. Na prática, o som é igual ao ouvido, mas não é bit a bit. Os casos: `audio-ay-tone-pcm`, `audio-ay-envelope-pcm`, `audio-ay-noise-pcm`, `audio-speaker-clicks-pcm` e `audio-ssi263-phonemes-p333`.

## Diferenças que ainda existem

- **Depurador, igual ao AppleWin de propósito.** O dono manteve a paridade em: aspas simples (bit alto ligado) e duplas (bit alto desligado) na busca (#713), a ordem e os nomes repetidos de `APPLE2E.SYM` (#1145) e só 4 das 9 tabelas de símbolos carregadas na partida (#1377).
- **Depurador, adiado.** O mini dump `M` do Language Card por banco ou slot (#1426 e #1431) não mostra o banco 2 com o LC desligado, como no AppleWin; a correção exige expor o LC por banco no núcleo e ficou para depois.
- **Cursor de espera (AppleWin #781).** O AppleWin mostra o cursor de espera ao carregar um save-state grande; o AppleII-Rust não muda o cursor.

## Onde o AppleWin difere do hardware e o AppleII-Rust copia o AppleWin

O AppleII-Rust segue o AppleWin onde ele difere do hardware real (ADR 0004). Nenhum desvio do hardware foi aprovado como Divergência intencional (as Divergências aprovadas acima não são sobre o hardware).

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
