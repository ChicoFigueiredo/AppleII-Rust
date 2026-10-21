# Linha de comando

```
applewin [opções] [arquivo...]
```

A linha de comando segue a do AppleWin (`ProcessCmdLine`), com as grafias de um só hífen. As opções sobrepõem a configuração salva apenas naquela execução. Os cartões inseridos com `-sN` valem antes das opções de cada cartão, então `-s6 diskii -s6d1 x.dsk` funciona em qualquer ordem.

Argumentos que não começam com `-` são tratados como arquivos a abrir: um arquivo que termina em `.aws.json` ou `.aws.yaml` é carregado como save-state, qualquer outro é inserido no drive 1 (e a Máquina reinicia).

Um argumento desconhecido é guardado na lista de "comandos não suportados" e não aborta o programa; uma opção que exige valor sem o valor faz o programa terminar com a mensagem `option X requires a value`.

## Modelo e máquina

| Opção | Efeito |
|---|---|
| `-model apple2\|apple2p\|apple2jp\|apple2e\|apple2ee` | Escolhe o Modelo. Clones são escolhidos na configuração |
| `-50hz`, `-60hz` | Vídeo PAL de 50 Hz ou NTSC de 60 Hz |
| `-clock-multiplier N` | Multiplicador de velocidade (decimal) |
| `-memclear N` | Padrão de memória ao ligar, 0 a 7 |
| `-rom arquivo` | ROM de sistema própria: arquivo de exatamente 16K (em $C000-$FFFF) ou 12K (em $D000-$FFFF) |
| `-f8rom arquivo` | ROM própria de exatamente 2K em $F800-$FFFF |
| `-videorom arquivo` | ROM de vídeo própria de exatamente 2K, 4K, 8K ou 16K |

Estas três opções são conferidas antes de abrir a janela, com as mesmas regras e mensagens do AppleWin:

- `-f8rom` com arquivo ausente ou que não tem exatamente 2K: `Failed to load custom F8 rom (not found or not exactly 2KiB)`.
- `-rom` com arquivo ausente ou que não tem exatamente 12K nem 16K: `Failed to load custom rom (not found or not exactly 12KiB or 16KiB)`.
- `-rom` e `-f8rom` juntos: `Unsupported -rom and -f8rom being used at the same time`.

Nesses três casos o programa imprime a mensagem e termina com código 1, sem abrir a janela. Com `-videorom` inexistente ou de tamanho diferente de 2K, 4K, 8K e 16K o AppleWin só avisa (`Failed to load video rom (not found or not exactly 2/4/8/16KiB)`) e segue com a ROM de vídeo embutida; aqui também. Os arquivos de ROM do `settings.toml` (`[machine]` e `[roms]`) têm regra mais tolerante: se não puderem ser usados, a Máquina inicia com a ROM embutida e mostra um aviso. Veja "Usar ROMs próprias", no capítulo de configuração.
| `-bootsector arquivo` | Setor de boot customizado |
| `-no-nsc` | Remove o No-Slot Clock |
| `-alt-cpu-emu` | CPU alternativa (testes) |

## Slots e cartas

| Opção | Efeito |
|---|---|
| `-s0 empty\|lc\|languagecard\|saturn\|saturn128\|saturn64` | Carta do slot 0 |
| `-sN carta` | Carta do slot N (1 a 7), veja abaixo |
| `-aux empty\|std80\|ext80\|rw3` | Carta do slot AUX do //e |
| `-r N` | RamWorks III com N bancos de 64K (1 a 256) |
| `-s7-empty-on-exit` | Esvazia o slot 7 ao sair |
| `-harddisknumblocks N` | Número de blocos de discos rígidos novos |
| `-hdc-firmware-v1`, `-hdc-firmware-v2` | Versão do firmware do HDC |
| `-no-disk2-stepper-defer` | Não adia o passo do braço do Disk II |
| `-ssc-tcp-port N` | Porta TCP da SSC (padrão 1977) |
| `-dcd`, `-modem` | A SSC usa o DCD real da porta serial |
| `-use-real-printer` | Permite imprimir numa impressora real |
| `-no-mb` | Desativa a Mockingboard e a família |
| `-speech` | Habilita a fala |
| `-mb-audit` | Expõe os tipos extras de cartas Mockingboard |

Nomes de carta aceitos por `-sN`:

| Nome | Carta |
|---|---|
| `empty` | Slot vazio |
| `diskii`, `diskii13` | Disk II (firmware de 16 ou 13 setores) |
| `hdc`, `hdc-sp`, `hdc-bm2`, `hdc-bm4` | HDC (auto, SmartPort, modo bloco de 2 ou 4 unidades) |
| `saturn`, `saturn128` | Saturn 128K |
| `megaaudio`, `sdmusic` | MegaAudio, SD Music |
| `breakpointcard` | Breakpoint card |
| `parallel` | Impressora paralela (uma só) |
| `ssc` | Super Serial Card (uma só) |
| `vidhd` | VidHD (uma só) |
| `sc01`, `no-sc01` | Liga ou remove o Votrax SC-01 da Carta de som |
| `ay-socket0=chip` a `ay-socket3=chip` | Chip AY no soquete: `empty`, `ay8910`, `ay8912`, `ay8913`, `ym2149` |
| `socket0=chip`, `socket1=chip` | Chip de fala: `empty`, `ssi263p`, `ssi263ap` |
| `6522a-bad`, `6522b-bad` | 6522 defeituoso |

As cartas sem nome em `-sN` (mouse, SAM, Z80, Uthernet, 4Play, SNES MAX, Mockingboard, Phasor) são escolhidas na configuração; nomes desconhecidos viram um aviso, não um erro.

## Discos

| Opção | Efeito |
|---|---|
| `-d1 arquivo`, `-d2 arquivo` | Imagem nos drives 1 e 2 do slot 6 |
| `-sNd1 arquivo`, `-sNd2 arquivo` | Imagem num drive do slot N |
| `-d1-disconnected`, `-d2-disconnected` | Desconecta o drive |
| `-h1 arquivo`, `-h2 arquivo` | Imagem de disco rígido (slot 7) |
| `-sNh1 arquivo` a `-sNh8 arquivo` | Disco rígido no slot N, unidades 1 a 8 |

## Vídeo e janela

| Opção | Efeito |
|---|---|
| `-f`, `-full-screen` | Tela cheia |
| `-no-full-screen` | Janela |
| `-fs-width=N`, `-fs-height=N`, `-fs-height=best` | Resolução da tela cheia (implicam `-f`) |
| `-video-mode=idealized\|rgb-videocard\|composite-monitor` | Tipo de monitor |
| `-video-style=vertical-blend\|no-vertical-blend` | Mistura vertical |
| `-rgb-card-type apple\|sl7\|eve\|feline` | Placa RGB |
| `-rgb-card-foreground N`, `-rgb-card-background N` | Cores de texto da placa RGB (0 a 15) |
| `-rgb-card-invert-bit7` | Inverte o bit 7 do double hi-res |
| `-mac-lc-card-dlgr` | Double lo-res da placa Mac LC |
| `-alt-enter=toggle-full-screen\|open-apple-enter` | Efeito de Alt+Enter |
| `-multimon` | Múltiplos monitores |
| `-printscreen` | Mostra o nome do arquivo ao capturar a tela |
| `-no-printscreen-key`, `-no-printscreen-dlg` | Não captura a tecla Print Screen; sem diálogo |

## Entrada e som

| Opção | Efeito |
|---|---|
| `-no-di` | Desativa o DirectInput (Windows) |
| `-no-hook-system-key`, `-hook-alt-tab`, `-hook-altgr-control`, `-no-hook-alt` | Teclas do sistema e Alt (relevantes só no Windows) |
| `-altgr-sends-wmchar` | Entrega a tecla digitada com o AltGr apertado (padrão: descartar, como o AppleWin) |
| `-capslock=off` | Inicia com o Caps Lock do Apple desligado |
| `-left-alt-control-buttons`, `-right-alt-control-buttons` | Alt como botões |
| `-swap-buttons` | Troca os botões 0 e 1 |
| `-no-joystick1`, `-no-joystick2` | Ignora o controle 1 ou 2 do sistema |
| `-snes-max-alt-joy1`, `-snes-max-alt-joy2` | Mapeamento alternativo do SNES MAX |
| `-snes-max-user-joy1 arquivo`, `-snes-max-user-joy2 arquivo` | Mapeamento do usuário |
| `-m` | Desliga o som |
| `-spkr-inc N`, `-spkr-max N` | Filtro do alto-falante |
| `-wav-speaker arquivo`, `-wav-mockingboard arquivo` | Grava o som em WAV |

## Estado, configuração e depuração

| Opção | Efeito |
|---|---|
| `-load-state arquivo` | Carrega um save-state ao iniciar |
| `-load-state-ignore-hdc-fw` | Ao carregar, ignora o firmware do HDC (testes) |
| `-power-on` | Liga a Máquina imediatamente |
| `-current-dir dir` | Diretório inicial |
| `-conf arquivo` | Configuração INI alternativa (veja "Diferenças em relação ao AppleWin") |
| `-noreg` | Não registra tipos de arquivo (Windows; sem efeito aqui) |
| `-l`, `-log` | Escreve um arquivo de log |
| `-debugger-auto-run arquivo` | Executa um script do depurador ao entrar nele |
| `-screenshot-and-exit arquivo` | Com `-load-state`: salva a tela e sai (testes) |
| `--help`, `-help`, `-?` | Reconhecidos, ainda sem texto de ajuda impresso |
| `--version`, `-version` | Reconhecidos, ainda sem texto de versão impresso |

## Opções do próprio programa

Estas opções existem só no AppleII-Rust, não no AppleWin. Servem para testes e automação.

| Opção | Efeito |
|---|---|
| `--no-audio` | Não abre dispositivo de áudio |
| `--headless-frames N` | Roda N Quadros sem janela e sai |
| `--dump-screen arquivo` | Em modo sem janela, salva a tela em PNG |
| `--dump-text` | Em modo sem janela, imprime a tela de texto |
| `--type "texto"` | Digita o texto no Apple (`\r` ou `\n` viram Return) |
| `--type-at-frame N` | Quadro em que `--type` começa (padrão 180) |
| `--dump-window arquivo` | Com janela: salva o PNG após alguns Quadros e sai |
| `--window-frames N` | Quadros que `--dump-window` aguarda (padrão 120) |

No modo sem janela (`--headless-frames`) as configurações do usuário são ignoradas: o resultado é reproduzível, bom para integração contínua.

## Exemplos

```
# Boot de um DOS 3.3 no drive 1, ligando já
applewin -d1 dos33.dsk -power-on

# ][+ com Language Card e Mockingboard no slot 4 (configurada), vídeo PAL
applewin -model apple2p -s0 lc -50hz

# Disco rígido ProDOS no slot 7 com SmartPort
applewin -s7 hdc-sp -s7h1 meu.hdv

# Gravar uma captura de tela headless da tela de boot
applewin --headless-frames 300 --dump-screen boot.png
```
