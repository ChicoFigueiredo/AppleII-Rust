# Oraculo executavel: AppleWin 1.32.1.0 (Windows) sob Wine

Gera save-states `.aws.yaml` reais (gravados pelo AppleWin de verdade), capturas
de tela de referencia e checagens pontuais, sem interacao humana. Alimenta o
corpus real da HU-0401 (plano, secao 17, "Corpus real").

Versao do oraculo: release oficial `v1.32.1.0` (`AppleWin1.32.1.0.zip`,
SHA-256 `94783120695ff1749823c933c02798a7df6155d16cc9d75363f73b0dccc88ff0`).
Codigo de referencia consultado: commit `a8f0d1d` (5 commits depois da tag), so
para ler `source/CmdLine.cpp` e afins.

## Arquivos

| Arquivo | Funcao |
|---|---|
| `fetch-applewin-windows.sh` | `gh release download` + confere SHA-256 + descompacta em `target/conformance-cache/oracle/applewin-1.32.1.0/` |
| `run-applewin-wine.sh` | roda o AppleWin no Wine, salva estado, captura tela, escreve a proveniencia |
| `xsendkey.py` | injeta teclas X11 via XTest (ctypes, so libX11/libXtst); substitui o `xdotool`, que nao esta instalado |

Tudo que e grande fica em `/mnt/d/Chico/AppleRust/target/conformance-cache/oracle/`
(fora do Git, compartilhado entre worktrees): zip, `app/`, `wineprefix/`,
`work/<nome>/`, `out/<nome>/`.

## Uso

```
tools/conformance/oracle/fetch-applewin-windows.sh
tools/conformance/oracle/run-applewin-wine.sh --name apple2ee-applesoft-prompt --model apple2ee --seconds 15 \
    --out tests/conformance/inputs/applewin-real
tools/conformance/oracle/run-applewin-wine.sh --name apple2ee-dos33-master-boot --model apple2ee \
    --disk "assets/disks/DOS 3.3 System Master - 680-0210-A.dsk" --seconds 25 --out tests/conformance/inputs/applewin-real
```

Requer: Wine 10 (`wine`, `wineboot`, `wine taskkill`), `DISPLAY` (a janela do
AppleWin abre de fato; nao ha modo headless), `gh`, `unzip`, `python3`
(Pillow opcional, so para o PNG). Prefixo isolado em
`target/conformance-cache/oracle/wineprefix` (`WINEPREFIX` + `WINEDEBUG=-all`);
`~/.wine` nao e tocado. Cada execucao leva ~30 s.

## Como funciona (e por que)

1. **Configuracao sem registro.** `-noreg` (CmdLine.cpp:125) evita registrar tipos
   de arquivo; `-conf aw.ini` (CmdLine.cpp:129) troca o Registro do Windows por
   um `.ini` (Registry.cpp usa `GetPrivateProfileString` quando `g_sConfigFile`
   nao e vazio). Nao e preciso `wine reg add`. O `.ini` precisa de
   `Version=1.32.1.0`, senao abre o dialogo de licenca/"About" e trava a automacao.
   Slots e cartas: o AppleWin grava as secoes `[Configuration\Slot N]` no proprio
   `.ini`; as opcoes `-s0..-s7`, `-aux`, `-d1/-d2`, `-h1/-h2` (CmdLine.cpp:140-380)
   sobrepoem.
2. **Salvar estado = "Save State On Exit".** Nao existe opcao de linha de comando
   para salvar estado, nem comando do depurador (sem "Snapshot" em `Debugger/`).
   F11 existe (WinFrame.cpp:1286) mas abre um dialogo Win32 de arquivo. A saida
   limpa e `Save State On Exit=1` no `.ini` (Utilities.cpp:191; grava em
   `Snapshot_Shutdown`, chamado do `WM_DESTROY`, WinFrame.cpp:1088) com
   `Save State Filename=` apontando o destino. O encerramento e pedido por
   `wine taskkill /im AppleWin.exe` (envia `WM_CLOSE`, nao mata: o estado e gravado).
   `wineserver -k` mata sem salvar e so e usado como rede de seguranca.
3. **Passada semente.** Com `Save State On Exit=1` o AppleWin tenta *carregar* o
   arquivo no start (`Snapshot_Startup`, SaveState.cpp:~582) e, se nao existe,
   abre a caixa modal "Load State". A primeira passada espera essa caixa
   (`xwininfo`), aperta Enter (`xsendkey.py`) e encerra: grava um estado
   pre-boot do modelo escolhido. A segunda passada carrega essa semente, faz o
   `-power-on` (reboot completo, `WM_USER_BOOT`), roda N segundos e salva.
   O estado semente e descartado (e sobrescrito).
4. **Captura de tela.** `-screenshot-and-exit <arq.bmp>` (CmdLine.cpp:645) so
   funciona junto com `-load-state` (CmdLine.cpp:395); o script recarrega o estado
   salvo e obtem um BMP 560x384. A tecla Print Screen (hotkey) **nao** foi
   confiavel sob Wine/XTest; nao usada. A captura reflete o estado salvo, nao a
   execucao anterior.
5. **Drive Q:.** O script aponta `dosdevices/q:` para o diretorio de trabalho para
   que o caminho do disco gravado no YAML seja `Q:\\disk1.dsk` e nao um caminho
   da maquina.
6. **Slot 6 vazio para o prompt.** Sem disco, `-s6 empty` evita o loop de "boot
   do disco" e a maquina para no prompt `]` do Applesoft.

## Opcoes de linha de comando confirmadas (CmdLine.cpp, commit a8f0d1d)

`-l/-log` :121 · `-noreg` :125 · `-conf <ini>` :129 · `-d1`/`-d2` :140/:146 ·
`-h1`/`-h2` :160/:166 · `-s0`..`-s7 <cartao>` :172 (empty, diskii, diskii13, hdc,
saturn, megaaudio, sdmusic, parallel, ssc, vidhd, ...) · `-aux <tipo>` :353 ·
`-load-state <arq>` :395 · `-load-state-ignore-hdc-fw` :401 · `-f/-full-screen`
:405 · `-no-full-screen` :409 · `-memclear` :456 · `-r` :468 · `-f8rom` :480 ·
`-rom` :492 · `-videorom` :504 · `-no-printscreen-key` :524 ·
`-no-printscreen-dlg` :528 · `-no-hook-system-key` :532 ·
`-alt-enter=...` :609/:613 · `-video-mode=...` :617-625 ·
`-screenshot-and-exit <bmp>` :645 · `-clock-multiplier` :650 ·
`-model apple2|apple2p|apple2jp|apple2e|apple2ee` :656-669 · `-50hz/-60hz`
:674/:678 · `-power-on` :713 · `-current-dir` :717 · `-bootsector` :777 ·
`-debugger-auto-run <script>` :817 · `-alt-cpu-emu` :813.

**Nao existem:** `-snapshot`, `-save-state*`, `-wait-before-exit`, `-dbg`,
`-save-state-and-exit`. Nao ha opcao de sair sozinho alem de
`-screenshot-and-exit`.

## Limitacoes

- **Nao deterministico.** O AppleWin roda em tempo real sob Wine: o numero de
  ciclos no momento do `WM_CLOSE` varia entre execucoes (`Cumulative Cycles`).
  Cada `.aws.yaml` e um *instantaneo real de formato e estado* (um corpus), nao um
  trace ciclo a ciclo. Para comparar com o Rust use o estado carregado como ponto
  de partida, nao a contagem de ciclos de chegada. A RAM inicial e o padrao
  de power-on do AppleWin (ver `-memclear`).
- Sem entrada de teclado simulada alem de Enter para a caixa semente; estados que
  exigem digitar comandos nao sao gerados por este fluxo (daria para estender
  `xsendkey.py`, mas o resultado depende de temporizacao).
- A janela do AppleWin aparece de fato na sessao grafica (XWayland); nao ha
  headless. Nao rode duas instancias em paralelo no mesmo prefixo.
- O YAML nao contem a ROM da Apple (so RAM principal/auxiliar, LC e estado de
  cartas); contem `Absolute Path` do disco (`Q:\\disk1.dsk`) e o cabecalho
  com data (`# Date-stamp`) e versao. Fins de linha: CRLF.
- Audio: PulseAudio via Wine; sem verificacao de audio aqui.
- Wine 10.0 (Ubuntu); outras versoes podem mudar a mecanica do foco/taskkill.

## Corpus gerado (`tests/conformance/inputs/applewin-real/`)

Cada `<nome>.aws.yaml` acompanha `<nome>.provenance.toml` (versao do AppleWin,
SHA-256 do zip, versao do Wine, comando, data UTC, SHA-256 do YAML e do disco)
e `<nome>.png` (captura do estado). Todos abaixo de 2 MB, por isso versionados.
