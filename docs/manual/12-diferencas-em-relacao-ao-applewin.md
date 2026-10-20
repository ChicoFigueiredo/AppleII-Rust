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

## Frontend: o que falta ligar

Os crates das Cartas existem e têm testes, mas o frontend ainda precisa criá-las a partir da configuração. No momento da escrita, a "fábrica de cartas" do frontend cria Disk II, mouse, a família Mockingboard e SAM. As seguintes cartas ainda precisam ser ligadas ao frontend (a biblioteca está pronta):

| Carta | Biblioteca |
|---|---|
| Controlador de disco rígido | `apple2-storage` |
| Super Serial Card (COM e TCP) | `apple2-hostio` (COM exige o recurso `serialport`) |
| Impressora paralela | `apple2-hostio` |
| Uthernet I (exige `pcap`) e Uthernet II | `apple2-hostio` |
| Z80 SoftCard, relógios, 4Play, SNES MAX, VidHD, Breakpoint card, dongles | `apple2-cards` |

Mesmo sem a ligação, essas opções são aceitas na linha de comando e na configuração, e gravadas no arquivo.

## Interface

- **Janela de configuração.** Reproduz as páginas do AppleWin (Configuration, Input, Slots, Advanced e as opções de cada carta), mais uma aba Sound para os interruptores de som que o AppleWin só tem na linha de comando. Faltam o botão "Benchmark Emulator" (desativado), o volume separado do Mockingboard e a lista de interfaces do Npcap/libpcap (usa os nomes do sistema ou um nome digitado). A lista de portas seriais vem de /dev (ou COM1 a COM9).
- **Janela única.** A tela, a barra e as janelas de ajuda e configuração são desenhadas dentro da mesma janela, com a biblioteca egui, e não por caixas de diálogo do sistema.
- **Idioma.** Os textos da interface são em inglês; este manual está em português do Brasil.
- **Ajuda.** F1 abre este manual dentro do emulador, no lugar do arquivo CHM do AppleWin.
- **Discos.** O menu do drive (Ctrl+F3/F4 ou botão direito) tem inserir, ejetar, proteção contra escrita, imagens recentes, trocar drives e "Create new disk" (em branco, DOS 3.3 ou ProDOS). Os discos rígidos aparecem no mesmo menu, mas trocar uma imagem reinicia a Máquina. Não há a caixa "Open as Read Only", nem soltar uma imagem sobre o botão de um drive (um arquivo solto sempre vai ao drive 1 e reinicia), nem o menu "Send to CiderPress".
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
- **`-conf arquivo`** é reconhecido pelo analisador, mas o frontend ainda usa o arquivo de configuração padrão.
- **`--help` e `--version`** são reconhecidos mas ainda não imprimem texto.
- **Save-state.** F11 e F12 gravam e leem, por enquanto, o formato JSON interno (`AppleWin.aws.json`). O formato YAML já está no módulo `apple2-savestate`; falta ligar o frontend a ele. O AppleWin guarda `SaveState.yaml`.
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
