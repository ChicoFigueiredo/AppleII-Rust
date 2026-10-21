# Lacunas e limitações conhecidas

Estado medido na `main` `22dbf2b7a4c57d9b6ede7be39a577e93196c1a3f` (21/10/2026). Esta página lista tudo o que hoje está em FAIL ou BLOCKED, o que continua UNVERIFIED, as diferenças entre o AppleWin e o hardware que o AppleII-Rust copia do AppleWin, e as Divergências intencionais (nenhuma). A versão anterior desta página era um inventário de afirmações do manual, sem execução; esta é a que sucede às ondas de conformidade. Os números por componente estão na [matriz de capacidades](capability-matrix.md), e o panorama no [relatório final](relatorio-final.md).

Como ler:

- **FAIL**: um teste executado discorda do Oráculo. Cada FAIL tem o caso ou o teste que o mostra.
- **BLOCKED**: o teste não pôde ser executado, com a causa. Não é PASS nem teste pulado.
- **UNVERIFIED**: ninguém comparou. `IMPLEMENTED` nunca vale PASS.
- O Oráculo é o fork Linux do audetto (`a1e20c0`) rodando headless; o AppleWin de referência é o `a8f0d1d` (ADR 0004: onde o AppleWin diverge do hardware, o AppleWin vence enquanto não houver Divergência intencional aprovada).

## 1. FAIL vigentes

### 1.1 Áudio: PCM do AY, PCM do alto-falante, SSI263 e o IRQ do 6522 (6 casos)

Rode `conformance run --case ID`. Todos têm `known_divergence` no caso e fazem o `conformance run --all` terminar com 6 FAIL.

| Caso | Issue Rust | Diferença medida | Causa raiz | Estado |
|---|---|---|---|---|
| `audio-ay-tone-pcm` (D-2) | #27 | frequência 8,77 % > 4,00 %; RMS 4,45 % > 2,00 % | O AppleWin gera o PCM em blocos de `(int)(44100 / (clk/intervalo + 0.5))` amostras (43 onde seriam 43,2: -0,4 % de tempo de AY por bloco) e o primeiro bloco sai ~1000 ciclos depois do ciclo 0. O Rust gera cada amostra no ciclo exato. O próprio Oráculo muda de fase com o fatiamento (M0 falha entre `--audio-slice 1000` e `250`) | Aberto. Núcleo do AY idêntico amostra a amostra quando a linha do tempo é a mesma (`ay_core_oracle.rs`); eventos idênticos. Recomendação: janelas maiores ou contagem absoluta de cruzamentos, para execuções futuras (não aplicada para não ajustar o limiar depois de ver o resultado) |
| `audio-ay-envelope-pcm` (D-2) | #27 | frequência 5,88 % > 1,00 % (16 contra 17 cruzamentos numa janela quase muda) | idem | idem |
| `audio-ay-noise-pcm` (D-2) | #27 | frequência 58,33 % > 40,00 % | idem; o LFSR muda a taxa de cruzamentos com a fase | idem |
| `audio-speaker-clicks-pcm` (D-3) | #7 | RMS 2,46 % > 2,00 %; 39.140 quadros no Oráculo contra 38.893 no Rust | `SetClksPerSpkrSample` (Speaker.cpp:221-229) usa 23 ciclos inteiros por amostra (clk/44100 = 23,14); o AppleWin compensa regulando a velocidade da CPU pelo buffer. O hardware não tem esse quantum | Aberto. Candidata a Divergência intencional (veja a seção 6), mas sem decisão do dono |
| `audio-ssi263-phonemes-p333` (D-4) | #28 | primeiro fonema 1.683 ciclos > limiar 1.670 (13 ciclos, 0,8 %) | O fim do fonema só é visto pelo AppleWin em `PeriodicUpdate` (~1000 ciclos) e o primeiro `Update` pré-carrega 25 % do buffer; o Rust agenda `início + comprimento` | Aberto. Na fase 0 o caso PASS; nos 12 fonemas seguintes e nas fixtures o Rust fica dentro da tolerância |
| `audio-issue-1176-last-cycle` (#1176) | #7 | primeira diferença na linha 17: IOR `0D` em 160 contra 162 | 6522 #1 em T1 contínuo com latch 0 e IRQ pendente do 6522 #0: o AppleWin toma o IRQ logo após o CLI (CLI em 932, entrada em 934); o Rust adia um opcode (o evento é marcado "último ciclo do opcode") | Aberto. Duas hipóteses sem êxito (fase dos eventos de 2 ciclos). O limite de esforço do trabalho foi atingido. Vira PASS quando a correção entrar na `main` |

### 1.2 Vídeo: #241, pixel do Color Monitor PAL (1 caso de cena)

| Item | Issue Rust | Diferença medida | Estado |
|---|---|---|---|
| #241 "Emulator's floating bus is ahead by 2 cycles" | #46 | Cena `tests/conformance/inputs/video/issue-0241-floatbus-dlores-text80.scene` (Ansi Story: `DLORES -> TEXT80 -> DLORES` por linha com o floating bus amostrado em `LDA $C0A0`). PASS em NTSC e em Color TV PAL (casos `video-issue-0241-*`). **Color Monitor a 50 Hz difere em 1 pixel de borda** (x=18, y=18; 3 pixels no quadro 1; Oráculo (225,239,255), Rust preto). Só ocorre com PAL + monitor + `TEXT/GR` por linha + conteúdo de texto na página 2 | Aberto (P1). Não é um caso do harness com `known_divergence`: está em `results/video.csv` e na matriz como FAIL |

### 1.3 Importação de save-states reais do AppleWin: 9 FAIL medidos (o relatório anterior dizia 12)

Fonte: [savestate-import.md](../../tests/conformance/reports/savestate-import.md), regenerado nesta entrega com `tools/conformance/savestate-compare.py` e `savestate-report.py` sobre os 112 estados do AppleWin-Test (SHA `4b113ce`) e os 3 estados do AppleWin 1.32.1.0 sob Wine: **79 PASS, 9 FAIL, 27 BLOCKED (115 arquivos)**. A versão anterior do relatório tinha 76 PASS, 12 FAIL, 27 BLOCKED. Os três que mudaram são os `Mockingboard/Cybernoid.aws.yaml`, `Cybernoid-TimerAtC480.aws.yaml` e `Cybernoid2-Phasor.aws.yaml`: passaram a bater no trace com a correção do 6522 (`3552588`, D-1 do diferencial de áudio) e viraram casos `savestate-corpus-*`. O número "12" do pedido refere-se à versão anterior do relatório.

| Arquivo | Ciclos até divergir | Diferença | Causa conhecida | Issue Rust |
|---|---:|---|---|---|
| `v2/SNES_MAX-slot4.aws.yaml` | 6 | bit de dados do controle lido no meio da leitura (A `A0` no Oráculo, `E0` no Rust) | Temporização da leitura serial do SNES MAX; não investigada | #30 |
| `v2/dazzledraw1-mouse-rom-page0.aws.yaml` | 12.489 | IRQ do evento de VBL do mouse 7 ciclos adiantada | Fase do evento de VBL reagendada no carregamento (#381) | #6 |
| `v2/dazzledraw1-mouse-rom-page6.aws.yaml` | 12.489 | idem | idem | #6 |
| `v2/RGB card/dazzledraw1.aws.yaml` | 7.260 | idem | idem | #6 |
| `v2/SmartPort-firmware/SmartPort-firmware.aws.yaml` | 4.373 | valor de status lido de `$C0nX` difere (`84` contra `82`) | Não investigada | #24 |
| `v2/Memory/Saturn64.aws.yaml` | 406 | `LDA` lê `$46` no Oráculo e `$00` no Rust depois de restaurar um banco do estado | Causa não investigada. A contagem de bancos (AUD-C-001) já foi corrigida; a atribuição que o relatório anterior fazia a ela está obsoleta | #18, #25 |
| `v2/Memory/Saturn128.aws.yaml` | 406 | idem (`$32` contra `$00`) | idem | #18, #25 |
| `v2/Mockingboard/6522-one-shot.aws.yaml` | 115 | A `00` no Oráculo, `0C` no Rust | Temporização do 6522 após o carregamento; mesma família de #1176 | #27 |
| `v2/Mockingboard/SSI263-test.aws.yaml` | 109 | idem | idem | #28 |

Os 27 BLOCKED do mesmo relatório (17 `DiskFormat/*` e `AZTEC` por imagem ausente; 3 SSC por TCP/serial do hospedeiro; 2 No-Slot Clock por hora real; 2 `skm-*` porque o Oráculo não tem o Shift Key Mod; `CPM2.2(56k)-3` por Z80 ativo; `contiki` por Uthernet recusado pelo Oráculo) estão listados na seção 2.

### 1.4 Depurador: 10 FAIL, todos bugs do próprio AppleWin que o Rust reproduz

Fonte: [results/depurador.csv](results/depurador.csv), testes `crates/apple2-debugger/tests/matriz_depurador_b.rs`. A matriz marca FAIL porque a issue pede um comportamento diferente do que o AppleWin `a8f0d1d` tem; o Rust faz o mesmo que o AppleWin. Corrigir qualquer um seria Divergência intencional (ADR 0004) e não foi feito.

| Issue upstream | Comportamento (igual no AppleWin e no Rust) | Issue Rust |
|---:|---|---|
| #114 | `SYMUSER LOAD filename.sym` sem aspas não faz nada (`CmdSymbolsLoad` só lê `TYPE_QUOTED_2`, Debugger_Symbols.cpp:826) | #34 |
| #245 | `SYMUSER SAVE` é um no-op sem mensagem (Debugger_Symbols.cpp:1130) | #34 |
| #466 | `SYM CMDNTB.PR#` dá Syntax Error (`#` é operador do tokenizador); os nomes carregam de `A2_USER1.SYM` | #34 |
| #468 | `CD ..` faz `dir.join("..")` e o `PWD` mostra `.../y/..` | #34 |
| #713 | Aspas simples = bit alto ligado e duplas = desligado (Debug.cpp:6199-6255); a issue quer a convenção do Merlin32 | #34 |
| #1145 | `APPLE2E.SYM` é idêntico ao do AppleWin: fora de ordem e com nomes repetidos | #34 |
| #1377 | O startup carrega só 4 das 9 tabelas de símbolos | #34 |
| #1381 | `DA 2004` com `B_2100` existente mostra `da A_2101-1`, como o AppleWin (`|| pData`, Debugger_Disassembler.cpp:318) | #34, #76 |
| #1426 | `M1 L2/D000` com o LC desligado não mostra o banco 2 do LC (`read_prefixed` só distingue main/aux); o núcleo não expõe o backing do LC por banco/slot | #34 |
| #1431 | Mesma causa de #1426, para LC/Saturn por prefixo de slot/banco | #34 |

Os 6 BLOCKED do depurador (#269 Esc, #345 GR2, #483 clique no P, #740 modo misto, #746 repintura ao arrastar a janela, #974 teclado UK) pertencem ao frontend e não têm Oráculo headless. Os 8 NOT_APPLICABLE são pedidos futuros sem comportamento de referência.

## 2. BLOCKED vigentes

### 2.1 Plataformas e hardware

| Item | Causa | Para desbloquear | Issue Rust |
|---|---|---|---|
| **Windows nativo** (build, headless, GUI, DPI por monitor, manifesto, console, WASAPI, XInput, SmartScreen, antivírus) | Sem execução em Windows de verdade. O smoke foi feito no Wine 10.0, que não é Windows ([windows-wine.md](platform/windows-wine.md)). CI é só manual e a cota está esgotada | Autorização do dono para um `workflow_dispatch`, ou a máquina/VM do dono com [manual-tests/plataformas.md](manual-tests/plataformas.md) | #36 |
| **macOS** Intel e Apple Silicon (build, smoke, Retina, Gatekeeper, `.app` ad hoc) | Sem hardware nem SDK da Apple; a licença do SDK só permite hardware Apple | Idem, em máquina macOS | #36 |
| **Hardware físico**: gamepad, captura de mouse, áudio audível, serial real, pcap/Npcap, impressora, teclado regional (ABNT2, AZERTY), minimizar, Wayland nativo | Sem dispositivos e sem ouvinte/olho humano. Os roteiros estão em [manual-tests/](manual-tests/) | O dono executa os roteiros e registra o resultado | #33, #36 |
| Pacotes assinados/notarizados | Não há certificado Authenticode nem Developer ID; os pacotes não são assinados ([assinatura.md](platform/assinatura.md)) | Decisão e custo do dono | #37 |
| `release.yml` | Os passos novos (cargo-about, cargo-cyclonedx, `licenses.sh` em Windows e macOS) nunca rodaram em CI | Um `workflow_dispatch` autorizado | #37 |

### 2.2 Falta de fixture ou de Oráculo

Os BLOCKED por issue na matriz são **87** (26 abertas, 61 fechadas). Por causa:

| Causa | Quantidade | Exemplos |
|---|---:|---|
| Software de terceiros sem fixture licenciada e com proveniência | 60 | disco 28 (#29 Ultima I, #51, #125...), áudio 11 (#19 Merlin, #72, #608, #647, #699...), vídeo 6 (#6 Karateka, #37, #371, #409, #680, #711), CPU 5 (#133 Chessmaster, #204, #206, #290, #1126), memória 6 (#222, #225, #297, #400, #422, #1126), entrada/cartas 5 (#70 UCSD Pascal, #71 Drol, #103, #127, #707) |
| Comportamento do frontend sem Oráculo headless (pacing, janela, DirectSound, cliques de UI) | 24 | áudio 7 (#95, #240, #456, #813, #1159, #1172, #1173), depurador 6, vídeo 5 (#352, #363, #397, #405, #722), disco 3 (#640, #831, #1078), CPU #4, save-state #564 e #486 |
| Outros | 3 | #100, #1104 (áudio), #372 (vídeo) |

(#1126 aparece nos resultados de CPU e de memória com o mesmo estado: 61 linhas, 60 issues; 60 + 24 + 3 = 87.) Para o G2 os BLOCKED P0 importam: **#4, #70, #400, #422, #564, #640, #1126** (7 issues P0 BLOCKED, todas fechadas upstream). Os mecanismos sintéticos de #400, #422, #564 e #640 passam em testes Rust; faltam o Oráculo ou a fixture.

Também BLOCKED, fora da contagem por issue:

- 27 estados reais do AppleWin-Test (seção 1.3), dos quais 18 por imagens ausentes do corpus (`DiskFormat/*` e `AZTEC`).
- HDC no ][+ contra o Oráculo: o fork é 22 commits mais antigo e não tem o patch de Shift do GH#1234 (disk-differential.md, L1). O modo bloco no //e está comparado.
- Z80 SoftCard (CP/M) no sistema contra o Oráculo: o trace do Oráculo cobre só a CPU 6502 e não existe fixture com SoftCard e Phasor juntos (#72).
- Uthernet I/II e SSC em rede/serial reais; estados com TCP.
- Hot swap, entrada de teclado e troca de lado de disco no Oráculo: ele não insere disco com a máquina ligada nem aceita teclas (L4 do diferencial de disco).

## 3. UNVERIFIED

- **524 issues upstream** (de 1.115) estão UNVERIFIED na matriz: **278 P3 e 246 P2**. Não há P0 nem P1 UNVERIFIED. Distribuição (CSV): interface 122, feature-futura 146, runtime 181, formato 45, documentação 30, o resto em pequenas parcelas. O plano de teste delas não foi executado; a [triagem](upstream-open-issues-triage.md) escolheu 36 das 358 abertas para histórias (#45 a #80).
- **Componentes**: vídeo dos clones (Pravets, TK3000, Base64A, J-Plus) além do trace de CPU; interface (janela de configuração página a página, menu do drive, status de trilha e setor, Ctrl+Shift+V): testes Rust, sem comparação; Disk II em outro slot que não o 6 e mais de uma carta; INIT/FORMAT e firmware de 13 setores; Mockingboard com SSI263 somado (`Mixer`), MEGA Audio e SD Music, SSI263 em modo Phasor e Echo+; VidHD em ][/][+ e SHR em PAL; Uthernet II com sessões múltiplas e servidor TCP.
- As afirmações "O que já é igual" do manual 12 estavam todas UNVERIFIED na versão anterior desta página. Hoje: CPU, memória, vídeo NTSC/PAL, Disk II/HDD, eventos do Mockingboard/Phasor/SSI263 e o formato YAML (via importação) têm comparação (seção 1 e matriz); linha de comando, janela de configuração e atalhos F1 a F12 só têm testes próprios.
- Desempenho: ~12 MHz em Color TV na máquina de desenvolvimento ([performance.md](../../tests/conformance/reports/performance.md)), sem hardware nem compilação fixos; não é comparação com o AppleWin.

## 4. Diferenças AppleWin x hardware que o AppleII-Rust copia do AppleWin

Todas seguem a ADR 0004 (o AppleWin vence). Nenhuma é Divergência intencional.

| Diferença | O que faz o hardware | O que fazem o AppleWin e o Rust | Prova |
|---|---|---|---|
| **Ciclo fantasma** (D0) | Leituras/escritas espúrias em alguns ciclos de modos de endereçamento; as de `$C0xx` têm efeito colateral | Não as fazem (AppleWin GH#967, aberta) | 5.482.901 vetores SingleStepTests/65x02 em D0; [65x02-singlestep.md](../../tests/conformance/reports/65x02-singlestep.md) |
| **RMB/SMB/BBR/BBS** (D5, D6) | 65C02 Rockwell/WDC: instruções de 2 e 3 bytes | NOP de 1 byte e 1 ciclo (`cpu65C02.h:135,151`) | vetores D5/D6 |
| **WAI/STP** (D8) | Espera e para | NOP de 1 ciclo | 40.000 vetores |
| HLT/KIL NMOS (D1) | PC no byte seguinte | PC sobre o opcode, `bJammed` | vetores D1 |
| XAA/ANE (D2), ARR decimal (D3), ISC com `$FF` (D4) | Hardware instável / subtrai `$00` | Fórmulas do AppleWin | vetores D2 a D4 |
| `$5C` (8 ciclos), `$DC/$FC` (endereço lido), `JMP (abs)` 65C02 em `$xxFF` (7 ciclos) | 4 / outro endereço / 6 | Tabela do WDC | vetores D7, D9, D10 |
| **Floating bus com RAMRD** | Lê a RAM principal | Lê a imagem de leitura da CPU (aux, com `RAMRD=1`) | AUD-C-050, `audit_floating_bus_irq.rs` |
| NMI | O 6502 trata NMI | O AppleWin não o trata (`NMI()` é código morto, `CPU.cpp:400-441`); o Rust também não entrega a linha à CPU | AUD-C-054, `machine.rs::nmi_line_never_reaches_the_cpu_as_in_applewin` |
| Z80: IRQ/NMI | A SoftCard não liga as linhas | O núcleo do Z80 do Rust implementa IRQ/NMI/IM/HALT, mas a carta não liga as linhas, como o AppleWin | `softcard_does_not_wire_the_interrupt_lines` |
| Amostragem do alto-falante | Contínua | 23 ciclos por amostra no AppleWin; clk/44100 no Rust (isto é um FAIL, seção 1.1) | D-3 |
| Reset do Saturn | Estado dependente do cartão | Banco ativo sobrevive ao power-cycle (o construtor é que o zera), como `LanguageCardUnit::Reset` | AUD-C-006 |
| Uthernet II sem SPI, PPPoE e interrupções | Tem | Modos sem suporte falham fechados | AUD-P-021 |
| VidHD sem texto 80x45 e sem cor de borda; fita cassete não emulada; IIc só o esqueleto | Têm | Iguais ao AppleWin | AUD-P-004; manual 12 |
| No-Slot Clock no ][ e ][+ (soquete F8) não funciona com ProDOS-8 2.5.0a8 | Funciona | Igual ao AppleWin | manual 12 |
| `rand()` do Disk II (latch de drive vazio, bits aleatórios de WOZ) | Aleatório | `rand()` da glibc com `srand(1)` (igual ao Oráculo); no Windows o `RAND_MAX` é 32767 | D6 do diferencial de CPU |
| Atualização dos cartões por quadro (17.030 ciclos) no frontend | Contínua | O AppleWin atualiza por lote de 1 ms; até 16 ms de diferença no fim do atraso do motor | disk-differential.md, L6 |

## 5. Limitações de produto vigentes (sem equivalente a comparar)

Os itens abaixo são escolhas de produto ou recursos exclusivos do Windows. Estão também no [manual 12](../manual/12-diferencas-em-relacao-ao-applewin.md).

| Item | Estado |
|---|---|
| Interface em inglês; manual em português; F1 abre o manual embutido no lugar do CHM | NOT_APPLICABLE como paridade |
| Configuração em TOML (não Registro/INI); `DELREG.INF` não existe; `.reg` e INI importam por `-conf` e pelo comando da interface | NOT_APPLICABLE |
| Registro das extensões de disco no sistema; menu "Send to CiderPress"; `-hook-alt-tab`, `-no-hook-system-key` | NOT_APPLICABLE (Windows) |
| ThunderClock Plus emulado na biblioteca e sem tipo de carta escolhível; o AppleWin também não o oferece | NOT_APPLICABLE (AUD-P-015) |
| Camada headless, bloco de extensão no YAML, sockets UDP e servidor TCP no Uthernet II, Ctrl+F11 como passo | Extensões; o bloco de extensão precisa continuar legível por um AppleWin real (importação reversa validada com `aws-for-oracle.py`) |
| Pacotes sem assinatura Authenticode e sem notarização | Veja a seção 2.1 |
| O programa não procura `disks/` sozinho; o usuário abre os discos pelo diálogo ou por `-d1` | Escolha de projeto |

## 6. Divergências intencionais

_Nenhuma registrada._

Uma Divergência intencional é um ponto em que o AppleII-Rust segue o hardware real em vez do AppleWin. Como o AppleWin vence (ADR 0004), um item só entra aqui quando houver, juntos:

1. **prova de hardware**: medição, documentação do fabricante ou teste em máquina real que mostre que o AppleWin difere do hardware;
2. **o comportamento do AppleWin e o do AppleII-Rust descritos**, com o SHA do AppleWin e o teste que demonstra a diferença;
3. **o motivo** de seguir o hardware, e o Modelo e a configuração a que se aplica;
4. **decisão do dono do projeto**.

Um bug do AppleWin não é, por si só, uma Divergência intencional, e uma extensão que não muda o comportamento comparável também não é. **Nenhum dos itens abaixo preenche os quatro requisitos**; ficam aqui como candidatos, sem decisão:

| Candidato | Falta | Observação |
|---|---|---|
| Z80: F3/F5 de LDIR/CPIR/INIR/OTIR quando repetem vêm do byte alto do PC, com os ajustes de PF/HF (AUD-P-014) | A prova é de terceiros (vetores SingleStepTests/z80, 1.604.000 de 1.604.000, e ZEXALL 65/65), não uma medição própria em hardware. Não houve execução do `Z80VICE` do AppleWin para mostrar o desvio. Sem decisão do dono | A SoftCard roda o Z80 do Rust; o comportamento é observável em CP/M |
| Quantum de 23 ciclos do alto-falante (D-3) | O Rust já usa clk/44100 exato e por isso é um FAIL contra o Oráculo; não há medição em hardware, e o dono não decidiu | Hoje está em FAIL, não em divergência intencional |
| Os 10 FAIL do depurador (seção 1.4) | São pedidos de melhoria em cima de bugs do AppleWin; exigiriam decisão do dono | O Rust reproduz o AppleWin |

As melhorias de qualidade que não alteram o comportamento comparável (gravação atômica de disco e de save-state, recusa de arquivos hostis, erros visíveis de E/S, ROM do usuário inválida com fallback) não são Divergências intencionais: o AppleWin não tem o mesmo comportamento a comparar. Elas têm testes próprios.

## 7. Limites do Oráculo

- O Oráculo executável é o fork do audetto (22 commits antes do `a8f0d1d`); não é o AppleWin do Windows. Os arquivos de vídeo do fork e do clone de referência são idênticos (`NTSC.cpp`, `RGBMonitor.cpp` e outros), então a defasagem não afeta o vídeo. O executável oficial 1.32.1.0 (Wine) serve só para gerar save-states.
- Sem entrada de teclado nem troca de disco em execução; um só cartão de áudio por execução (`--audio-slot`); CPU a 1 MHz; sem relógio real (o No-Slot Clock devolve a hora do hospedeiro); `g_bFullSpeed` nunca roda; sem janela, DirectSound nem diálogos.
- O PCM do AY do próprio Oráculo não é reprodutível entre fatiamentos (calibração em audio-differential.md).
