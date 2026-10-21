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
#   --state x.aws.yaml  carrega save-state antes de rodar (o modelo vem do arquivo; "cycle" continua do valor salvo)
#   --memclear 0..7     padrão de preenchimento da RAM (padrão do AppleWin: 2)
#   --cards disk2|none
```

Cache (fora do Git, compartilhado entre worktrees): `target/conformance-cache/oracle/audetto-AppleWin` (clone) e `target/conformance-cache/oracle/build` (binário `audetto-oracle`); `ORACLE_CACHE=...` altera a raiz.

## Formato (v1)

Linhas `#chave=valor` (UTF-8, antes do cabeçalho): `trace_format`, `oracle`, `model`, `cpu`, `frames` (se pedido), `cycles`, `start_cycle`, `memclear`, `cards`, `input`, `disk`, `state`, `rom_sha256`, `applewin_sha`, `port_base_applewin_sha`, `fork_sha`, `fork_url` (e `fixture_sha256` com disco/save-state). Depois `cycle,pc,op,a,x,y,sp,p` e uma linha por instrução: `cycle` decimal, `pc` com 4 dígitos hexa maiúsculos, os demais com 2. Finais de linha `\n`.

## Traces versionados

`tests/conformance/reference-traces/applewin-<modelo>-boot-500k.csv`: 500.000 ciclos do boot sem disco, para `apple2ee` e `apple2p` (~5,8 MB cada). Os 2.000.000 de ciclos pedidos geram ~23 MB por modelo (776 mil instruções), acima do limite de ~10 MB; gere-os localmente com `--cycles 2000000`. O comando é determinístico (duas execuções produzem arquivos idênticos).

## Limitações

1. O oráculo é o upstream de 2026-07-26 + 22 commits a menos que o port (ver abaixo); divergência em área tocada por esses commits não prova bug do port.
2. P tem os bits 4 e 5 forçados em 1 (comportamento do AppleWin); o RESET deixa A/X/Y em `FF`.
3. Interrupções e Z80 não geram linhas de trace; só o 6502/65C02 principal.
4. Só o fluxo de CPU é comparável: memória, framebuffer e áudio não são exportados ainda.
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
