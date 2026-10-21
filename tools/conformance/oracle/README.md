# Oráculo automatizado: AppleWin (fork audetto) headless no Linux

Compila no Linux o fork [audetto/AppleWin](https://github.com/audetto/AppleWin), que compartilha o núcleo C++ do AppleWin upstream, e o usa para gerar **Traces de referência v1** (CSV) sem janela, sem áudio e sem relógio real. É o Oráculo padrão da Conformidade (ver GLOSSARY.md).

## Versões congeladas

| Item | Valor |
|---|---|
| Fork (`audetto/AppleWin`, `master` de 2026-10-10) | `a1e20c0362f392dc25f1a3b218d2f84d4f1a25da` |
| Upstream (AppleWin/AppleWin) contido no fork (merge-base) | `3e8054b4627624398e4589f7f27b3d40a6b9718e` (2026-07-26, "Replace BOOL (almost all) with bool (PR #1509)") |
| Base do port Rust | `a8f0d1d40f551ad9e010522c29c2891e33c88275` (2026-10-17) |

O fork está 1861 commits **à frente** do merge-base e 0 atrás dele; o port parte de um upstream **22 commits mais novo** que o fork (`a8f0d1d` não é ancestral do `master` do fork). Ou seja, o oráculo é o upstream de 26/07 e não o de 17/10.

O trace grava `applewin_sha` = merge-base (o que o oráculo realmente executa), `port_base_applewin_sha` = base do port e `fork_sha`.

## Dependências

Nada de `sudo`. O fork embute `zlib`, `minizip` e `libyaml` (subprojetos CMake) e as ROMs como recurso; slirp/pcap ausentes viram backend de rede "dummy". Não precisa de SDL, Qt, ncurses, evdev nem Boost.

- Necessário: `git`, `cmake` >= 3.13, `g++` (C++17), `make` (ou `ninja`), `patch`, acesso a `github.com` para clonar (com submódulos: imgui, usados só pelo frontend SDL, mas o clone é recursivo).
- Pacotes `-dev` que **não** estão instalados e por isso os frontends reais do fork não compilam aqui (o oráculo não depende de nenhum): `libevdev-dev` (applen), `libsdl2-dev` e `libsdl2-image-dev` (sa2), `libslirp-dev`, `libpcap-dev`, `libminizip-dev` e `libboost-program-options-dev` (os dois últimos nem são usados pelo fork).

## Por que um executável próprio

Os frontends do fork exigem bibliotecas que faltam (applen: `libevdev`; sa2: SDL2; qt: Qt5 `Gamepad`; libretro compila, mas não é feito para gerar traces). O `applen --headless` ainda exigiria ncurses e evdev. Por isso o oráculo é `audetto-oracle`: um frontend de ~250 linhas (`audetto-oracle/main.cpp`) sobre as bibliotecas `appleii` + `common2` do fork. Ele usa o mesmo caminho de inicialização do applen/sa2 (`CommonInitialisation`, `LoadConfiguration`, `MemInitialize`, `CardManager`) e o mesmo loop de lotes de 1 ms de `CommonFrame::Execute`, sem renderização.

## Ponto de trace (patch mínimo)

O AppleWin não tem gancho por instrução reaproveitável: o trace do depurador (`TraceFile`) existe apenas no debugger do Windows e grava texto desassemblado, não os campos do formato v1. Por isso `audetto-trace.patch` (versionado aqui, ~100 linhas) faz:

- `CPU.h`/`CPU.cpp`: ponteiro de função global `g_pfnCpuTraceHook` (nulo = sem custo além de um teste);
- `CPU/cpu_general.inl`: macro `ORACLE_TRACE(op)`;
- `CPU/cpu6502.h` e `CPU/cpu65C02.h`: uma linha após o `Fetch` do opcode (vale para as 8 variantes: 6502/65C02, normal/debug, `altRW`);
- `Memory.cpp`/`CPU.h`/`CPU.cpp`: flag `g_bOracleDeterministic`; com ela, `getRandomTime()` (que mistura `rand()` com o relógio `timeGetTime()` e alimenta a RAM de power-on) devolve 0;
- `CMakeLists.txt`: opção `-DBUILD_ORACLE=ON` que compila `source/frontends/oracle/` (copiado de `audetto-oracle/` pelo script de build).

Detalhes que afetam a leitura do CSV:

- **Linha gravada antes de executar**: `pc` é o endereço do opcode, `op` o opcode lido, e A/X/Y/SP/P são os valores *antes* da instrução.
- **`cycle`**: `g_nCumulativeCycles` só é sincronizado de forma preguiçosa (em acessos a I/O e no fim do lote), então o hook usa `g_nCumulativeCycles - g_nCyclesExecuted + uExecutedCycles`, que é o ciclo exato em que a instrução começa. O 1o opcode sai em `cycle=0`; o reset de energia não consome ciclos.
- **`p`**: reconstruído como `EF_TO_AF` do AppleWin: **os bits 5 (reservado) e 4 (B) sempre valem 1** (o AppleWin os mantém fixos em `regs.ps`). Logo, `P` após o reset é `34`, não `24`. O lado Rust deve aplicar a mesma máscara ou o comparador deve ignorar os bits 4 e 5.
- **`sp`**: 8 bits baixos (`regs.sp` do AppleWin vale `0x100..0x1FF`).
- Interrupções (NMI/IRQ) tomadas consomem uma iteração do laço sem `Fetch` e **não geram linha**; o ciclo da primeira instrução do tratador mostra o custo.
- O Z80 (cartão CP/M) não é rastreado.
- O fluxo de ciclos passa de `--cycles` por até ~1 ms (um lote); o gravador descarta linhas com `cycle >= limite`.

## Estado inicial (power-on)

- **Reset**: a máquina é criada por `MemInitialize` + `CardManager::Reset(true)` (power cycle). O `pc` vem do vetor `$FFFC` da ROM e A/X/Y valem `FF`, SP `FC`, P `34` (valores do `CpuReset`/`regs` do AppleWin; o RESET do 6502 real deixaria A/X/Y indefinidos).
- **RAM**: padrão do AppleWin `g_nMemoryClearType`, definido em `source/Core.cpp` como `MIP_FF_FF_00_00` (valor **2**), que enche a RAM com `FF FF 00 00` repetido a cada 4 bytes por página (ver `MemReset()` em `Memory.cpp`). As exceções que o AppleWin sorteia com o relógio (bytes `xx28 xx29 xx68 xx69` a cada 512 bytes de `$0000-$BFFF`, e `$4E/$4F` = RNDL/RNDH) ficam fixas pelo patch: os quatro bytes viram `00` e `$4E`=`$4F`=`20`. Sem o patch o AppleWin **não** seria determinístico nem com `--memclear 2`. O resultado é reprodutível (duas execuções geram arquivos idênticos, conferido com 2.000.000 de ciclos) e é o **padrão usado nos traces**; vale também para a RAM auxiliar do //e. A opção `--memclear N` do oráculo (e `--memclear` dos frontends do fork) escolhe: 0 = zeros, 1 = **aleatório** (usa o relógio; evitar), 2 = `FF FF 00 00` (padrão), 3 = `FF` página inteira / `00` página inteira alternadas, 4 = `00 FF` meia página, 5 = `FF 00` meia página, 6 = byte baixo do endereço da página, 7 = byte alto. `-1` seria "aleatório por relógio".
- **Cartões**: o AppleWin instala por padrão impressora (slot 1), SSC (2), Mockingboard (4) e Disk II (6). Para o trace ser mínimo e determinístico, o oráculo usa só o **Disk II no slot 6** (`--cards disk2`, padrão) ou nenhum (`--cards none`); mais o Language Card/Slot 0 padrão do modelo. O //e sai com 128 KB (80 colunas auxiliar) como no padrão do AppleWin.
- **Registro**: o oráculo usa um registro em memória (nenhum `~/.config/applewin` é lido ou escrito), `Emulation Speed` e vídeo padrão (NTSC, 60 Hz, 1.0205 MHz).
- **Entrada**: nenhuma tecla, nenhum joystick (Paddle padrão: centrado, sem botões), nenhum áudio (`g_bDisableDirectSound*`).

## Comandos

```sh
# 1) Clona/atualiza o fork no SHA fixo, aplica o patch e compila (idempotente, ~2 min)
tools/conformance/oracle/build-audetto-oracle.sh

# 2) Gera um trace
tools/conformance/oracle/run-audetto-oracle.sh --model apple2ee --cycles 500000 \
    --out tests/conformance/reference-traces/applewin-apple2ee-boot-500k.csv

# Outras opções
#   --model   apple2 | apple2p | apple2jp | apple2e | apple2ee   (nomes do AppleWin)
#   --frames N          em vez de --cycles (1 quadro NTSC = 17030 ciclos)
#   --disk imagem.dsk   Disk II drive 1 (sha256 vai no cabeçalho)
#   --disk2 imagem.dsk  Disk II drive 2
#   --hdd imagem.hdv    controlador de disco rígido no slot 7, unidade 1 (--hdd2: unidade 2)
#   --authentic         "Enhance Disk Speed" desligado (velocidade autêntica do Disk II)
#   --state x.aws.yaml  carrega save-state antes de rodar (o modelo vem do arquivo; "cycle" continua do valor salvo)
#   --memclear 0..7     padrão de preenchimento da RAM (padrão do AppleWin: 2)
#   --cards disk2|none
```

Cache (fora do Git, compartilhado entre worktrees): `target/conformance-cache/oracle/audetto-AppleWin` (clone) e `target/conformance-cache/oracle/build` (binário `audetto-oracle`); `ORACLE_CACHE=...` altera a raiz.

## Formato (v1)

Linhas `#chave=valor` (UTF-8, antes do cabeçalho): `trace_format`, `oracle`, `model`, `cpu`, `frames` (se pedido), `cycles`, `start_cycle`, `memclear`, `cards`, `input`, `disk`, `state`, `rom_sha256`, `applewin_sha`, `port_base_applewin_sha`, `fork_sha`, `fork_url` (e `fixture_sha256` com disco/save-state; `disk2`, `hdd`, `hdd2` e `enhance_disk` com os dispositivos extras). Depois `cycle,pc,op,a,x,y,sp,p` e uma linha por instrução: `cycle` decimal, `pc` com 4 dígitos hexa maiúsculos, os demais com 2. Finais de linha `\n`.

## Framebuffer (HU-0201, HU-0202)

Modo à parte do trace: com `--fb-out PREFIX` o oráculo roda `--frames N` com a atualização de vídeo por opcode ligada (`CpuExecute(.., true)`, como os frontends reais), para no fim do quadro N (primeira instrução que cruza `N x ciclos por quadro`) e grava `PREFIX.rgba` (framebuffer inteiro, RGBA8, de cima para baixo) e imprime, em linhas `chave=valor`, o tamanho, o SHA-256 da área visível (`fb_sha256_visible`) e do buffer inteiro com as bordas (`fb_sha256_full`), os ciclos finais e o PC. Nenhum trace é escrito (use `--out /dev/null`). O código está em `audetto-oracle/framebuffer.{h,cpp}`; `main.cpp` só repassa as opções.

| Opção | Efeito |
|---|---|
| `--fb-out PREFIX` | seleciona o modo framebuffer |
| `--scene FILE` | carrega uma cena ("scene v1", abaixo) logo após o power-on; sem ela a ROM dá o boot |
| `--video-type N` | `VideoType_e`: 0 mono custom, 1 cor idealizada, 2 carta RGB, 3 monitor NTSC, 4 Color TV (padrão), 5 B&W TV, 6 âmbar, 7 verde, 8 branco |
| `--video-hz 50\|60` | PAL ou NTSC |
| `--rgb-card N` | 0 Video-7 SL7 (= Apple), 1 Le Chat Mauve EVE, 2 Feline |
| `--vidhd SLOT` | VidHD no slot: framebuffer 680x436, Super Hi-Res |
| `--state FILE.aws.yaml` | continua de um save-state (ver `aws-for-oracle.py`) |
| `FB_DEBUG=1`, `FB_WATCH=x,y` | diagnóstico: modo de vídeo, memória, posição do scanner; endereço de um pixel e `SIGTRAP` para `gdb` |

Cena "scene v1" (texto, `tests/conformance/inputs/video/*.scene`): linhas `main ADDR HEX...` e `aux ADDR HEX...` (RAM principal e auxiliar, escritas direto nos bancos), `entry ADDR` (o PC inicial, ciclo 0), `model` e `vidhd SLOT` informativos, `#` comentário. As cenas são geradas por `crates/apple2-video/examples/gen_scenes.rs`; o lado Rust as carrega com `conformance scene-fb` e com os casos `[video]` do harness. O lote completo é `video-differential.py` (ver [docs/conformance/audit/video-differential.md](../../../docs/conformance/audit/video-differential.md)).

Três cuidados que o modo framebuffer exigiu:

1. O framebuffer do AppleWin é um DIB bottom-up; as linhas são invertidas na saída.
2. O build usa `-fno-strict-aliasing`: o código de vídeo do AppleWin lê `RGBQUAD` como `UINT32`, comportamento indefinido que o GCC `-O2` explora (texto da carta RGB saía com cores de lixo).
3. `--state` com um save-state do port só carrega depois de `aws-for-oracle.py`: o fork (3e8054b) recusa a unidade "Game I/O Connector" versão 4 (Shift Key Mod, a8f0d1d), e sem a conversão o oráculo dá boot em vez de continuar o estado.

## Relógio de vídeo (alteração do frontend `main.cpp`, HU-0103)

A primeira versão do oráculo chamava `CpuExecute(slice, false)` (como `CommonFrame::Execute` quando `noVideoUpdate`). Com `bVideoUpdate = false` o `g_nVideoClockHorz/Vert` do NTSC nunca avança e toda leitura do floating bus (`$C056`, `$C050`, `$C0xx` sem dono) devolve o byte de um único endereço do scanner. O AppleWin do Windows sempre atualiza o vídeo, então isto era um artefato do frontend headless do fork, não comportamento do AppleWin. `main.cpp` agora chama `CpuExecute(slice, true)`; o patch `audetto-trace.patch` e o código do AppleWin não mudaram. Os traces de referência foram regenerados (o cabeçalho ganhou `#video_clock=`). O Rust já avançava o scanner a cada opcode.

Outros ajustes do script/frontend: `--out`, `--disk` e `--state` viram caminhos absolutos no `run-audetto-oracle.sh` (o oráculo muda de diretório ao carregar o disco; um `--out` relativo falhava); o cabeçalho grava só o nome do arquivo em `#disk=`/`#state=`. O caminho `--disk` foi exercitado: DOS 3.3 System Master, 5.000.000 de ciclos no //e Enhanced, 1.821.912 instruções.

## Traces grandes (não versionados)

Ficam em `target/conformance-cache/reference-traces/`; os casos `trace-applewin-*` guardam o SHA-256 e o comando. Todos com `--cards disk2` (padrão), `--memclear 2`, determinísticos:

| Arquivo | Comando | Instruções | SHA-256 |
|---|---|---|---|
| `applewin-apple2p-boot-2m.csv` | `--model apple2p --cycles 2000000` | 776.083 | `bedaa4d48ada5cce077e16faf0c3ee36d46285e64c66e11562069d9cd23f1a49` |
| `applewin-apple2ee-boot-2m.csv` | `--model apple2ee --cycles 2000000` | 776.044 | `a000df3a0f159f993aab254c692b2fbfd3ffc10a2942d116c5f8abe3d69f61d4` |
| `applewin-apple2-boot-2m.csv` | `--model apple2 --cycles 2000000` | 547.232 | `c77401fa9732c6b7dfa90d7bdd7a7849b09724d20f9b86f8cb93b718834b1a7d` |
| `applewin-apple2e-boot-2m.csv` | `--model apple2e --cycles 2000000` | 776.050 | `bf80669fa23f3403179db72d8fbdc6e92fd7b50809094bb5c0a8b4877b702e37` |
| `applewin-apple2ee-dos33-5m.csv` | `--model apple2ee --cycles 5000000 --disk "assets/disks/DOS 3.3 System Master - 680-0210-A.dsk"` | 1.821.912 | `13af08ef0a5749a64b8d0a9217d1f08a4a1725b356131dcfe5ff841d3670373d` |

Os mesmos hashes estão em `tests/conformance/cases/trace-applewin-*.toml`.

## Traces versionados

`tests/conformance/reference-traces/applewin-<modelo>-boot-500k.csv`: 500.000 ciclos do boot sem disco, para `apple2ee` e `apple2p` (~5,8 MB cada). Os 2.000.000 de ciclos pedidos geram ~23 MB por modelo (776 mil instruções), acima do limite de ~10 MB; gere-os localmente com `--cycles 2000000`. O comando é determinístico (duas execuções produzem arquivos idênticos).

## Limitações

1. O oráculo é o upstream de 2026-07-26 + 22 commits a menos que o port (ver abaixo); divergência em área tocada por esses commits não prova bug do port.
2. P tem os bits 4 e 5 forçados em 1 (comportamento do AppleWin); o RESET deixa A/X/Y em `FF`.
3. Interrupções e Z80 não geram linhas de trace; só o 6502/65C02 principal.
4. O trace só tem o fluxo de CPU; o framebuffer é exportado pelo modo `--fb-out` (acima); memória e áudio não são exportados ainda.
5. Teclado, joystick e rede não são simulados; `input=none`.
6. Modelos suportados: Apple ][, ][+, ][ J-Plus, //e e //e Enhanced. O //c e os clones (Pravets, Base64A, TK3000) exigiriam acrescentar o nome em `main.cpp`/`run-audetto-oracle.sh`.
7. O clone é recursivo (submódulos de imgui, ~100 MB) mesmo sem usá-los.
8. O build usa `-DENABLE_NETWORKING=OFF`: cartões Uthernet ficam sem backend (irrelevante para o trace).

## Diferenças conhecidas fork x upstream que afetam o núcleo

No merge-base `3e8054b`, o fork difere do upstream em (arquivos fora de `source/linux`, `source/frontends` e CMake):

- `source/Utilities.cpp`: volume padrão do speaker/Mockingboard vira 60 quando o registro não tem valor (afeta só áudio);
- `source/Uthernet1.*`, `Uthernet2.*`, `NetworkCard.h` (novo): refatoração do backend de rede (sem efeito em CPU/memória/disco);
- `source/StdAfx.h`, `resource/resource.h`: ajustes de portabilidade.

`CPU.cpp`, `Memory.cpp`, `Disk*.cpp`, `LanguageCard.cpp`, `NTSC.cpp`, `Video.cpp`, `Speaker.cpp` etc. são **idênticos** ao upstream no merge-base. O que muda é a camada de plataforma: `source/linux/duplicates/` reimplementa para o Linux `Joystick.cpp`, `Keyboard.cpp`, `SerialComms.cpp`, `FourPlay.cpp`, `SNESMAX.cpp` e as páginas de configuração (a lógica de I/O de joystick/teclado/serial pode divergir do Windows), e `source/frontends/common2` substitui `Frame`/registro/loop principal.

Commits upstream entre o merge-base e a base do port (`3e8054b..a8f0d1d`, 22 commits) que tocam o núcleo e **não** estão no oráculo:

- `d9b2bf67` Fix LC logic com Saturn em slot diferente de zero (#1520) — `LanguageCard.cpp`;
- `736a12b0` Game Port I/O: chaves de entrada sem dispositivo conectado (#1234) — `Joystick.cpp`;
- `c75971ab`/`c59b64ee`/`a565bb34` Shift Key Mod e firmware do HDC no II/II+ (#1234) — `Harddisk.cpp`, `CopyProtectionDongles.*`, `SaveState.cpp`;
- `54063517` mouse como joystick sem "desconectado";
- `6c579da5` `-no-joystick<1|2>`; `56c95d31` `-ssc-tcp-port`; `3057ddf8`/`daf813dc` versão no save-state; `18303d49` PNG em vez de BMP.

Nenhum desses altera o fluxo de CPU do boot sem disco, mas qualquer comparação de leituras de `$C061-$C063` ou de save-states com campos novos (`Shift Key Mod`, versão) pode divergir.

## Áudio (HU-0501, HU-0502)

`audetto-audio.patch` (aplicado depois de `audetto-trace.patch`) acrescenta ao fork pontos de observação de áudio, todos nulos por padrão: acessos à página da carta (`IOR`/`IOW`), escritas e leituras de registrador do AY, transições do IRQ (`IS_6522`, `IS_SPEECH`), toques do alto-falante, início e fim de fonema do SSI263 e o PCM do Mockingboard, do alto-falante e do SSI263 (`source/audio_hook.h`). `audetto-oracle/audio.cpp` os liga a um arquivo de eventos (`audio-events v1`, ver [test-oracles.md](../../../docs/conformance/test-oracles.md)) e a arquivos PCM brutos.

Como o AppleWin só roda o código de som quando há um dispositivo de áudio, o oráculo usa um "DirectSound virtual": um buffer circular cujo cursor de reprodução anda com os Ciclos emulados (o do Mockingboard começa com 3/8 de silêncio, o do SSI263 vazio). O alto-falante não ganha voz (uma voz traria o `g_nCpuCyclesFeedback` e mexeria no relógio da CPU); o PCM dele é retirado da fila do próprio `SpkrUpdate`.

Opções (todas `--audio-*`; qualquer uma liga o módulo):

```sh
tools/conformance/oracle/run-audetto-oracle.sh --model apple2p --cycles 250000 --out trace.csv \
    --cards none --audio-card mockingboard --audio-slot 4 \
    --audio-program tests/conformance/audio/programs/ay-tone.bin --audio-load 300 --audio-pc 300 \
    --audio-events ay-tone.events --audio-mb-pcm ay-tone.mb.pcm --audio-spkr-pcm ay-tone.spkr.pcm
#   --audio-card mockingboard|phasor|megaaudio|sdmusic     --audio-ay-socket N=empty|ay8910|ay8912|ay8913|ym2149
#   --audio-bad6522 a|b   --audio-ssi263 N=empty|ssi263p|ssi263ap   --audio-sc01 on|off
#   --audio-slice N        lote de CPU em Ciclos (padrão: clk/1000), usado na calibração
#   --audio-ssi-pcm PREFIXO   PCM mono de 22,05 kHz de cada SSI263
#   --audio-ay-script FILE --audio-ay-out FILE   autoteste do núcleo do AY (sem máquina)
```

`--state FILE.aws.yaml` funciona (a fixture define as cartas; os eventos têm Ciclo relativo ao fim do carregamento). Os casos e as referências: `tests/conformance/cases/audio-*.toml`, `tests/conformance/reference-audio/`; regenerar com `tools/conformance/audio/run-oracle-case.py CASO` e `run-ay-core.py`. Os programas 6502 (`tests/conformance/audio/programs/*.s`) são montados por `tools/conformance/audio/build-programs.sh`.

## Disco e HDD (HU-0301/0303)

`--disk`, `--disk2`, `--hdd`, `--hdd2` e `--authentic` repassam as opções de `EmulatorOptions` (`disk1/disk2/hardDisk1/hardDisk2`) e o valor de registro `Enhance Disk Speed`. O oráculo **escreve** nas imagens (AppleWin grava a trilha suja ao desligar o motor, ao ejetar e ao sair): rode sempre sobre uma cópia (o script de geração dos casos faz isso) e compare o SHA-256 da cópia depois da execução. Imagens `.gz` e `.zip` são aceitas; a regravação de `.gz` produz o mesmo conteúdo que a do Rust, mas bytes comprimidos diferentes (compare o conteúdo descomprimido).

O cache do oráculo (`target/conformance-cache/oracle`) é **compartilhado entre worktrees**: quem roda `build-audetto-oracle.sh` troca o binário de todos, com o `main.cpp` da sua própria worktree. Se um binário sem estas opções aparecer (o `usage` não lista `--disk2`), reconstrua, ou use um cache privado (`ORACLE_CACHE=/caminho tools/conformance/oracle/build-audetto-oracle.sh`, depois a mesma variável no `run-audetto-oracle.sh`; basta copiar `audetto-AppleWin` para lá antes).
