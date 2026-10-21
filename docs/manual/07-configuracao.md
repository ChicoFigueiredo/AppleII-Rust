# Configuração, vídeo, som e velocidade

A janela de configuração abre com **F8** (botão Config). A janela tem as abas Configuration, Input, Sound, Slots e Advanced, e os botões **OK**, **Cancel** e **Apply**, como no AppleWin. Mudanças que não mexem no hardware (vídeo, joystick, velocidade, volume, nomes de arquivo) valem na hora. Mudar Modelo, CPU, 50/60 Hz, cartas, ROMs, Freeze's F8 ROM ou No-Slot clock abre a pergunta "The emulator needs to restart", que lista o que mudou, e reinicia a Máquina (power-cycle) se você confirmar. Os botões "..." da aba Slots abrem as opções de cada carta (SSC serial ou TCP, impressora, Disk II, discos rígidos, Mockingboard/Phasor, Uthernet, mouse, Saturn, RamWorks). No modo TCP da SSC a janela avisa que a porta escuta em todas as interfaces (porta 1977 por padrão). As configurações ficam num arquivo TOML no diretório de configuração do sistema:

| Sistema | Local |
|---|---|
| Linux | `~/.config/applewin-rs/settings.toml` |
| macOS | `~/Library/Application Support/org.AppleII-Rust.applewin-rs/settings.toml` |
| Windows | `%APPDATA%\AppleII-Rust\applewin-rs\config\settings.toml` |

Chaves desconhecidas são ignoradas e chaves ausentes assumem os valores padrão do AppleWin. O arquivo pode ser editado à mão. As opções de linha de comando prevalecem sobre o arquivo, mas só valem para aquela execução se você não salvar a configuração.

O módulo `apple2-config` também sabe importar uma configuração existente do AppleWin, seja um arquivo `.reg` exportado do Registro do Windows, seja o arquivo INI do porte Linux (`-conf`). Veja "Diferenças em relação ao AppleWin" para o que a interface expõe hoje.

As seções a seguir seguem as páginas da configuração do AppleWin (Configuration, Input, Sound, Disk, Advanced e Slots) e dizem a que chaves de configuração e opções de linha de comando cada uma corresponde. A janela cobre quase todas essas opções; as restantes valem pelo arquivo TOML e pela linha de comando.

## Configuração geral (Configuration)

**Modelo.** O Modelo emulado:

- Apple ][: 48K sem autostart, Integer BASIC, sem minúsculas nem 80 colunas, CPU 6502.
- Apple ][+: 64K com autostart, Applesoft.
- Apple ][ J-Plus: versão japonesa do ][+.
- Apple //e (original): 128K, minúsculas e 80 colunas, 6502.
- Apple //e (Enhanced): 128K, 65C02.
- Clones: Pravets 82, Pravets 8M, Pravets 8A, Base64A e TK3000 //e. No Pravets 8A, F10 funciona como Caps Lock (e o Caps Lock do PC alterna cirílico e latino); no TK3000 o Scroll Lock é a tecla "mode" (acentos); no Base64A a tecla Delete é o "F2" (pressione e solte F2 e depois uma tecla para digitar uma palavra-chave do BASIC).

No ][ e no ][+ a modificação da tecla Shift fica ligada por padrão (só com o joystick 2 desativado).

**Confirmar reinício.** Mostra uma pergunta ao pressionar F2 (opção `confirm_reboot`).

**Volume mestre.** Volume do alto-falante e das cartas de som (0 é o mais alto, 59 é mudo, a mesma escala do AppleWin). A janela mostra o controle de forma que arrastar para a direita aumenta o volume.

**Padrão de vídeo (60 Hz / 50 Hz).** NTSC de 60 Hz (padrão) ou PAL de 50 Hz (`-50hz`, `-60hz`).

**Velocidade.** Controle de 1 a 40 em décimos: 10 é 1,0x (a velocidade autêntica), valores menores desaceleram, maiores aceleram, e 40 é "max" (sem limite). A opção `-clock-multiplier N` define um multiplicador exato (por exemplo 4 para 4,08 MHz).

**Janela.** Escala da janela (1 a 4) e escala inteira.

**Desabilitar som.** Desliga todo o áudio (`-m`).

## Vídeo

O tipo de monitor emulado (também percorrido por F9):

| Modo | O que simula |
|---|---|
| Monochrome (custom) | Monitor monocromático na cor escolhida |
| Color (Composite Idealized) | Monitor composto idealizado: texto nítido, 16 cores sem borrão. Único modo com mistura vertical |
| Color (RGB Card/Monitor) | Placa RGB padrão: texto nítido, 16 cores, double hi-res em pixels de 140x192 |
| Color (Composite Monitor) | Monitor NTSC (ou PAL): texto nítido com artefatos de cor |
| Color TV | TV NTSC: texto e gráficos borrados com "ringing" (padrão) |
| B&W TV | TV NTSC preto e branco |
| Monochrome Amber / Green / White | Monocromático nas cores clássicas |

Opções:

- **Linhas de varredura a 50%** (Ctrl+Shift+F9): desenha só as linhas alternadas, para um visual mais próximo do monitor.
- **Mistura vertical** (`-video-style=vertical-blend`): no modo Composite Idealized mistura verticalmente a tela hi-res.
- **Placa RGB**: `-rgb-card-type apple|sl7|eve|feline` escolhe a placa (Apple Extended 80-Column/AppleColor, Video-7 SL7, Le Chat Mauve Eve ou Feline). `-rgb-card-foreground` e `-rgb-card-background` fixam as cores do modo de texto de primeiro e segundo plano (0 a 15). `-rgb-card-invert-bit7` inverte o bit 7 do double hi-res (necessário em Dragon Wars). `-mac-lc-card-dlgr` ativa o double lo-res da placa Mac LC.
- **Tela cheia**: `-f` ou `-full-screen`, `-no-full-screen`, `-fs-width=N`, `-fs-height=N` ou `-fs-height=best`.
- **Mostrar LEDs dos discos** e, em tela cheia, o estado dos drives e do Caps Lock.
- **Alt+Enter**: `-alt-enter=toggle-full-screen` (padrão) ou `-alt-enter=open-apple-enter`.

Veja "Solução de problemas" para problemas de compatibilidade com a placa RGB.

## Som

O programa emula o alto-falante do Apple II, a Mockingboard, a Phasor e a SAM. A saída usa a biblioteca `cpal` (WASAPI, ALSA/PulseAudio/PipeWire e CoreAudio). Se nenhum dispositivo de áudio puder ser aberto, o programa continua sem som e escreve um aviso no terminal.

- **Volume do alto-falante**: o volume mestre. `-spkr-inc N` e `-spkr-max N` ajustam o filtro do alto-falante (`speaker_error_inc` e `speaker_error_max`).
- **Cartas de som**: escolhidas na aba de slots. `-no-mb` desativa a Mockingboard e a família. `-m` desliga todo o som.
- **Fala**: `-speech` habilita a fala (o SSI263 reproduz fonemas em taxa fixa).
- **Gravação em WAV**: `-wav-speaker arquivo` e `-wav-mockingboard arquivo` gravam o som do alto-falante e das cartas.
- Em velocidade máxima o áudio fica mudo.

## Entrada (Input)

- **Joystick 0 e 1**: veja "Joystick e mouse".
- **Trim X e Y**: soma um valor à posição central (127) do joystick de teclado.
- **Cursor lido pelo teclado**: com as setas como joystick, define se elas também chegam ao Apple (o cursor para cima é Ctrl+K e para baixo é Ctrl+J). Se chegam, alguns jogos (como Lode Runner) não funcionam; se não chegam, a edição de linha do Applesoft com as setas não funciona.
- **Trocar botões 0/1** (`-swap-buttons`).
- **Auto-fire**: inverte o estado do botão a cada leitura.
- **Centralização automática** do joystick de teclado.
- **Modificação da tecla Shift**: PB2 lê a tecla Shift (só ][ e ][+ com o joystick 2 desativado). Dongles que usam PB2 têm prioridade.
- **Alt como botões**: `-left-alt-control-buttons`, `-right-alt-control-buttons` e `-no-hook-alt`.
- **Colar da área de transferência**: Shift+Insert ou Ctrl+Shift+V.
- **Scroll Lock** como alternância de velocidade máxima ou só enquanto pressionado (`scroll_lock_toggle`).
- **Caps Lock**: `-capslock=off` inicia com o Caps Lock do Apple desligado.

## Discos (Disk)

- **Enhanced Speed** (padrão) ou Authentic Speed: com a velocidade aprimorada, a emulação corre sem limite enquanto o motor do Disk II está ligado. Alguns programas só funcionam na velocidade autêntica.
- Pastas de imagens de disco e de disco rígido (`disk_dir`, `hdv_dir`).
- **CiderPress**: caminho do programa para o menu do drive do AppleWin (não usado aqui).
- Opções de novos discos ProDOS: quais arquivos são copiados (`BITSY.BOOT`, `BITSY.BYE`, `BASIC.SYSTEM`, `PRODOS`).
- `-no-disk2-stepper-defer`: executa de imediato o movimento do braço do drive, em vez de adiá-lo como o AppleWin.
- `-s7-empty-on-exit`: esvazia o slot 7 ao sair.

## Avançado (Advanced)

- **Nome do arquivo de save-state** e **salvar o estado ao sair**. Veja "Save-states".
- **Clone**: o tipo de clone: Pravets 82, 8M, 8A, TK3000 //e, Base64A. Para o Pravets 8A e o TK3000 há também o layout do teclado (`Auto`, `Latin`, `Cyrillic`).
- **ROM F8 do Freeze** (somente ][ e ][+): substitui a ROM de 2K em $F800 por uma versão hacker, sem autostart.
- **ROMs alternativas**: `-rom arquivo` (ROM completa), `-f8rom arquivo` (apenas $F800) e `-videorom arquivo` (ROM de vídeo). Estas ROMs ainda não entram no save-state (como no AppleWin). A seção "Usar ROMs próprias" explica o conjunto completo, inclusive a tabela `[roms]` do `settings.toml`.
- **Padrão de memória ao ligar**: `-memclear 0` a `7`.
- **Setor de boot**: `-bootsector arquivo`.
- **Game I/O Connector**: o dongle de proteção (nenhum, SpeedStar, CodeWriter, Robocom 500/1000/1500 ou Hayden).
- **No-Slot Clock**: ligado por padrão (`-no-nsc` remove).
- **Benchmark**: o AppleWin tem um teste de desempenho na interface; aqui o comando `BENCHMARK` do depurador cumpre o papel.
- **Reiniciar tudo**: apague o arquivo `settings.toml` para voltar aos padrões.

## Usar ROMs próprias

O programa traz ROMs embutidas e funciona sem nenhum arquivo extra. Se você tem a sua própria ROM (uma revisão diferente, uma ROM modificada ou um clone), pode usá-la no lugar da embutida. O programa não distribui nem baixa ROMs: você fornece o arquivo.

Há três maneiras, da mais forte para a mais fraca. Quando mais de uma vale para a mesma ROM, vence a primeira:

1. A linha de comando: `-rom`, `-f8rom` e `-videorom` (veja "Linha de comando").
2. As chaves `custom_rom`, `custom_f8_rom` e `video_rom` da seção `[machine]` do `settings.toml`, que são o que a aba Advanced e a linha de comando preenchem.
3. A seção opcional `[roms]` do `settings.toml`, com uma ROM de sistema por Modelo.

Exemplo de `settings.toml`:

```
[roms]
apple2e_enhanced = "/home/eu/roms/minha-iie.rom"
apple2_plus = "/home/eu/roms/minha-iiplus.rom"
f8 = "/home/eu/roms/f8.rom"
video = "/home/eu/roms/video.rom"
```

Se a seção `[roms]` não existe, tudo continua como antes.

### Chaves de `[roms]` e tamanhos aceitos

| Chave | ROM | Tamanho exato |
|---|---|---|
| `apple2` | Sistema do Apple ][ | 12288 bytes (12K) |
| `apple2_plus` | Sistema do Apple ][+ | 12288 bytes (12K) |
| `apple2_jplus` | Sistema do ][ J-Plus | 12288 bytes (12K) |
| `apple2e` | Sistema do Apple //e | 16384 bytes (16K) |
| `apple2e_enhanced` | Sistema do //e Enhanced | 16384 bytes (16K) |
| `pravets82` | Sistema do Pravets 82 | 12288 bytes (12K) |
| `pravets8m` | Sistema do Pravets 8M | 12288 bytes (12K) |
| `pravets8a` | Sistema do Pravets 8A | 16384 bytes (16K) |
| `tk3000e` | Sistema do TK3000 //e | 16384 bytes (16K) |
| `base64a` | Sistema do Base64A | 49152 bytes (48K) |
| `f8` | 2K em $F800 a $FFFF (como `-f8rom`) | 2048 bytes (2K) |
| `video` | Gerador de caracteres (como `-videorom`) | 2K, 4K, 8K ou 16K |

Só a ROM do Modelo escolhido é lida; as demais chaves ficam guardadas. Uma ROM de 16K para o //e contém os 4K internos de $C000 seguidos dos 12K de $D000; uma de 12K cobre apenas $D000 a $FFFF. O `-rom` da linha de comando, como no AppleWin, aceita 12K ou 16K para qualquer Modelo. O AppleWin não permite trocar as ROMs das cartas de slot (Disk II, SSC, impressora, disco rígido, mouse), então este programa também não.

### Validação

Ao iniciar (e ao clicar em OK ou Apply), o programa confere cada ROM do usuário. Se o arquivo não existe, não é um arquivo comum, não pode ser lido ou tem um tamanho que a regra acima não aceita, a Máquina não inicia e aparece uma mensagem descritiva, por exemplo:

```
custom system ROM /home/eu/roms/minha-iie.rom: 100 bytes, expected 16384 (16K)
```

Na janela de configuração a mensagem aparece como problema e bloqueia OK/Apply; na linha de comando o programa imprime o erro e termina. Diferente do AppleWin, que avisa e cai de volta para a ROM embutida, aqui um arquivo errado nunca é substituído em silêncio.

### Escolher o arquivo na janela de configuração

Na aba Advanced, em "Custom ROMs", cada ROM tem um campo de texto, o botão Browse... para escolher o arquivo e o botão Clear para voltar à ROM embutida. A linha "System ROM for this model" edita a chave `[roms]` do Modelo selecionado; "System ROM" edita o `-rom` geral. Trocar uma ROM reinicia a Máquina.

### Saber quais ROMs estão em uso

Na inicialização o programa escreve no log (saída de erro) uma linha por ROM, com a origem (embutida ou arquivo), o tamanho e o SHA-256:

```
rom: system ROM: file /home/eu/roms/minha-iie.rom, 16384 bytes, sha256=...
rom: video ROM: embedded Apple2e_Enhanced_Video.rom, 4096 bytes, sha256=...
```

O mesmo relatório (ROM de sistema, F8, vídeo e firmware das cartas de slot) está disponível para o manifesto de conformidade. Sem nenhum override, os SHA-256 são os das ROMs embutidas.

## Velocidade máxima

Veja "Teclado e atalhos". Em resumo: Scroll Lock alterna a velocidade máxima; o acesso a disco acelera sozinho no modo Enhanced Speed; o comando `GG` do depurador executa em velocidade máxima. Nesse modo o som é silenciado, o vídeo é atualizado uma vez por Quadro do relógio real e um núcleo do processador fica em 100%.
