# Lacunas e limitações conhecidas

Estado medido na `main` `469bf941deab932ab228169a7ffebddae1fc6854` (21/10/2026, depois da #241 corrigida, das seis Divergências aprovadas do depurador e da ADR 0006; a medição de testes e da matriz inclui a conversão da #1396, veja a seção 7). Esta página lista tudo o que hoje está em FAIL ou BLOCKED, o que continua UNVERIFIED, as diferenças entre o AppleWin e o hardware que o AppleII-Rust copia do AppleWin, as Divergências intencionais (nenhuma), as **Divergências aprovadas pelo dono** (seção 7, ADR 0006) e as **Limitações aceitas** (seção 8, ADR 0006). Os números por componente estão na [matriz de capacidades](capability-matrix.md), e o panorama no [relatório final](relatorio-final.md).

Como ler:

- **FAIL**: um teste executado discorda do Oráculo. Cada FAIL tem o caso ou o teste que o mostra.
- **BLOCKED**: o teste não pôde ser executado, com a causa. Não é PASS nem teste pulado.
- **UNVERIFIED**: ninguém comparou. `IMPLEMENTED` nunca vale PASS.
- O Oráculo é o fork Linux do audetto (`a1e20c0`) rodando headless; o AppleWin de referência é o `a8f0d1d` (ADR 0004: onde o AppleWin diverge do hardware, o AppleWin vence enquanto não houver Divergência intencional aprovada).

## 1. FAIL vigentes

Na matriz de issues restam **6 FAIL** (de 1.115): #713, #1145, #1377 (depurador, paridade mantida pelo dono), #1426 e #1431 (depurador, adiadas) e #781 (cursor de espera). No harness (`conformance run --all`) restam **5 FAIL**, os casos de PCM/SSI, que são **Limitação aceita** (seção 8): continuam FAIL nos relatórios e nunca contam como PASS. Não há FAIL P0 nem P1 em issue.

### 1.1 Áudio: PCM do AY, PCM do alto-falante e SSI263 (5 casos): Limitação aceita

Os 5 casos continuam FAIL no harness (`known_divergence`) e fazem o `conformance run --all` terminar com código 3. O dono os aceitou em 21/10/2026 (ADR 0006). Tabela com a causa de cada um na seção 8.

### 1.2 Vídeo: #241, pixel do Color Monitor PAL: corrigida (PASS)

A #241 ("Emulator's floating bus is ahead by 2 cycles", issue Rust #46) era o último FAIL P1 em issue: 1 pixel de borda (x=18, y=18) no Color Monitor a 50 Hz na cena `tests/conformance/inputs/video/issue-0241-floatbus-dlores-text80.scene`. A correção é `de3ac3b` (histórico de sinal `0xFFF` no primeiro quadro, como o AppleWin); a cena passa em NTSC, Color TV PAL e Color Monitor PAL, e a matriz marca a #241 como PASS (commit `68c582f`).

### 1.3 Importação de save-states reais do AppleWin: 0 FAIL (os 9 da versão anterior foram resolvidos)

Fonte: [savestate-import.md](../../tests/conformance/reports/savestate-import.md), medido de novo em 21/10/2026 com `tools/conformance/savestate-compare.py` sobre os 112 estados do AppleWin-Test (SHA `4b113ce`) e os 3 estados do AppleWin 1.32.1.0 sob Wine: **88 PASS, 0 FAIL, 27 BLOCKED (115 arquivos)**. A versão anterior desta página listava 9 FAIL; a causa de cada um:

| Arquivo | Causa | De quem era |
|---|---|---|
| `v2/dazzledraw1-mouse-rom-page0`, `...-page6`, `RGB card/dazzledraw1` | O primeiro evento de VBL do mouse vem 12.480 ciclos (192 linhas) depois do carregamento, e o evento recalculava a posição do quadro somando de novo os ciclos do opcode (7 ciclos antes) | port |
| `v2/SmartPort-firmware/SmartPort-firmware` | O DIB devolvia a versão do firmware 130.0; o `version.h` do `a8f0d1d` dá 132.1 (o Oráculo ganhou `audetto-version.patch`) | port |
| `v2/Memory/Saturn64`, `Saturn128` | O fork aloca os bancos com `new BYTE[16K]` sem zerar e o YAML só lista `0000: 00`; o Oráculo lia lixo de heap (`$E000` = `$46`/`$32`). `MALLOC_PERTURB_=255` no `run-audetto-oracle.sh` | Oráculo |
| `v2/SNES_MAX-slot4` | O `SNESMAX.cpp` do fork é um esqueleto sem tratador de E/S (`audetto-snesmax.patch`) | Oráculo |
| `v2/Mockingboard/6522-one-shot`, `SSI263-test` | A ROM do SSC não existia no fork (varredura de slots lia `$C204`); `audetto-ssc-rom.patch` a instala. Não era o reset do 6522 | Oráculo |

O teste de formato (`conformance check-import`, segunda coluna do relatório) continua acusando 1 a 8 campos em quase todos os arquivos; o mais comum é o bit B do registrador P (`0x37` no arquivo contra `0x27` na máquina), normalizado de propósito. Não afeta o trace e não foi investigado campo a campo nesta rodada.

Os 27 BLOCKED do mesmo relatório (18 `DiskFormat/*` e `AZTEC` por imagem ausente; 3 SSC por TCP/serial do hospedeiro; 2 No-Slot Clock por hora real; 2 `skm-*` porque o Oráculo não tem o Shift Key Mod; `CPM2.2(56k)-3` por Z80 ativo; `contiki` por Uthernet recusado pelo Oráculo) estão listados na seção 2. Na leitura crua do `savestate-compare.py`, 8 desses 27 aparecem como FAIL do trace; o `savestate-report.py` os reclassifica como BLOCKED por causa conhecida.

### 1.4 Depurador: 5 FAIL, todos comportamento do AppleWin mantido ou adiado

Fonte: [results/depurador.csv](results/depurador.csv), testes `crates/apple2-debugger/tests/matriz_depurador_b.rs`. Das 11 issues que estavam em FAIL (o Rust reproduzia bugs do AppleWin `a8f0d1d`), o dono aprovou em 21/10/2026 a correção de seis (#114, #245, #458, #466, #468, #1381; hoje PASS, veja a seção 7). As 5 restantes seguem em FAIL porque a issue pede um comportamento diferente do AppleWin e o port faz o mesmo que o AppleWin. Os testes `#[ignore = "FAIL ..."]` reproduzem cada um com `cargo test -p apple2-debugger --release -- --ignored`.

| Issue upstream | Comportamento (igual no AppleWin e no Rust) | Decisão do dono (21/10/2026) | Issue Rust |
|---:|---|---|---|
| #713 | Aspas simples = bit alto ligado e duplas = desligado (Debug.cpp:6199-6255); a issue quer a convenção do Merlin32 | **Paridade mantida** (mudaria a semântica de scripts existentes) | #34 |
| #1145 | `APPLE2E.SYM` é idêntico ao do AppleWin: fora de ordem e com nomes repetidos | **Paridade mantida** (dado do AppleWin) | #34 |
| #1377 | O startup carrega só 4 das 9 tabelas de símbolos | **Paridade mantida** | #34 |
| #1426 | `M1 L2/D000` com o LC desligado não mostra o banco 2 do LC (`read_prefixed` só distingue main/aux); o núcleo não expõe o backing do LC por banco/slot | **Adiada** (exige expor o LC por banco no núcleo) | #34 |
| #1431 | Mesma causa de #1426, para LC/Saturn por prefixo de slot/banco | **Adiada** | #34 |

Os 7 BLOCKED do depurador (#269 Esc, #345 GR2, #483 clique no P, #740 modo misto, #746 repintura ao arrastar a janela, #974 teclado UK, mais 1 da onda final) pertencem ao frontend e não têm Oráculo headless. Os 39 NOT_APPLICABLE são pedidos futuros sem comportamento de referência ou defeitos do Win32. Total: 130 issues, **79 PASS**, 5 FAIL, 7 BLOCKED, 39 NOT_APPLICABLE.

### 1.5 Outros FAIL da matriz

| Issue | Estado | O que acontece | Teste |
|---:|---|---|---|
| #781 (P3, fechada) | FAIL | O AppleWin mostra o cursor de espera ao carregar um save-state grande (SaveState.cpp:394); o port não muda o cursor: a carga roda na thread de emulação sem sinal para a interface. Sem efeito na emulação; corrigir exige canal da emulação para a interface e não é testável sem janela | sem teste (causa documentada em `results/p23-nucleo.csv`) |

A #1396 (Alt+F2 e o Open Apple) deixou de ser FAIL: virou Divergência aprovada e PASS (seção 7).

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

Os BLOCKED por issue na matriz são **133** (40 abertas, 93 fechadas), sem mudança desde a onda final (eram 87 antes dela, porque issues UNVERIFIED que dependem de software de terceiros, de janela ou de hardware passaram a BLOCKED com causa). Por categoria: disco 54, vídeo 16, Mockingboard 12, CPU 9, áudio 9, memória 6, depurador 5, serial 4, teclado 3, HDD 3, e 12 em parcelas de 1 a 2 (outra, relógio, save-state, rede, SSI263, Z80, joystick). Por causa, a contagem da versão anterior (87 issues; a divisão das 46 novas não foi refeita) foi:

| Causa | Quantidade | Exemplos |
|---|---:|---|
| Software de terceiros sem fixture licenciada e com proveniência | 60 | disco 28 (#29 Ultima I, #51, #125...), áudio 11 (#19 Merlin, #72, #608, #647, #699...), vídeo 6 (#6 Karateka, #37, #371, #409, #680, #711), CPU 5 (#133 Chessmaster, #204, #206, #290, #1126), memória 6 (#222, #225, #297, #400, #422, #1126), entrada/cartas 5 (#70 UCSD Pascal, #71 Drol, #103, #127, #707) |
| Comportamento do frontend sem Oráculo headless (pacing, janela, DirectSound, cliques de UI) | 24 | áudio 7 (#95, #240, #456, #813, #1159, #1172, #1173), depurador 6, vídeo 5 (#352, #363, #397, #405, #722), disco 3 (#640, #831, #1078), CPU #4, save-state #564 e #486 |
| Outros | 3 | #100, #1104 (áudio), #372 (vídeo) |

(#1126 aparece nos resultados de CPU e de memória com o mesmo estado. Por heurística de palavras-chave sobre a evidência das 133, cerca de três quintos são software de terceiros sem fixture; o resto é comportamento de frontend ou de plataforma sem Oráculo headless e hardware físico.) Para o G2 os BLOCKED P0 importam: **#4, #70, #400, #422, #564, #640, #1126** (7 issues P0 BLOCKED, todas fechadas upstream). Os mecanismos sintéticos de #400, #422, #564 e #640 passam em testes Rust; faltam o Oráculo ou a fixture.

Também BLOCKED, fora da contagem por issue:

- 27 estados reais do AppleWin-Test (seção 1.3), dos quais 18 por imagens ausentes do corpus (`DiskFormat/*` e `AZTEC`).
- Nenhum dos 133 BLOCKED tem aceite formal do dono; para os 7 P0 a decisão está na seção 11 do [relatório final](relatorio-final.md) (decisão 5, ainda pendente).
- HDC no ][+ contra o Oráculo: o fork é 22 commits mais antigo e não tem o patch de Shift do GH#1234 (disk-differential.md, L1). O modo bloco no //e está comparado.
- Z80 SoftCard (CP/M) no sistema contra o Oráculo: o trace do Oráculo cobre só a CPU 6502 e não existe fixture com SoftCard e Phasor juntos (#72).
- Uthernet I/II e SSC em rede/serial reais; estados com TCP.
- Hot swap, entrada de teclado e troca de lado de disco no Oráculo: ele não insere disco com a máquina ligada nem aceita teclas (L4 do diferencial de disco).

## 3. UNVERIFIED

- **Issues upstream**: nenhuma das 1.115 está UNVERIFIED na matriz (eram 524, P2 e P3, antes da onda final). Os 849 resultados individuais estão em [results/](results/); a [triagem](upstream-open-issues-triage.md) escolheu 36 das 358 abertas para histórias (#45 a #80). Esses resultados são autoavaliação e não têm amostra independente.
- **Componentes**: vídeo dos clones (Pravets, TK3000, Base64A, J-Plus) além do trace de CPU; interface (janela de configuração página a página, menu do drive, status de trilha e setor, Ctrl+Shift+V): testes Rust e leitura do C++, sem Oráculo de janela; Disk II em outro slot que não o 6 e mais de uma carta; INIT/FORMAT e firmware de 13 setores; Mockingboard com SSI263 somado (`Mixer`), MEGA Audio e SD Music, SSI263 em modo Phasor e Echo+; VidHD em ][/][+ e SHR em PAL; Uthernet II com sessões múltiplas e servidor TCP.
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
| Amostragem do alto-falante | Contínua | 23 ciclos por amostra no AppleWin; clk/44100 no Rust (FAIL no harness, Limitação aceita, seção 8) | D-3 |
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

Um bug do AppleWin não é, por si só, uma Divergência intencional, e uma extensão que não muda o comportamento comparável também não é. **Nenhum dos itens abaixo preenche os quatro requisitos**; ficam aqui como candidatos. As Divergências aprovadas pelo dono (seção 7) são outra classe: dispensam a prova de hardware, mas só valem para o que não é emulação de hardware.

| Candidato | Falta | Observação |
|---|---|---|
| Z80: F3/F5 de LDIR/CPIR/INIR/OTIR quando repetem vêm do byte alto do PC, com os ajustes de PF/HF (AUD-P-014) | A prova é de terceiros (vetores SingleStepTests/z80, 1.604.000 de 1.604.000, e ZEXALL 65/65), não uma medição própria em hardware. Não houve execução do `Z80VICE` do AppleWin para mostrar o desvio. Sem decisão do dono | A SoftCard roda o Z80 do Rust; o comportamento é observável em CP/M |
| Quantum de 23 ciclos do alto-falante (D-3) | O Rust já usa clk/44100 exato e por isso é um FAIL contra o Oráculo; não há medição em hardware | Limitação aceita pelo dono (seção 8), não divergência intencional |
| Os 5 FAIL do depurador que sobraram (seção 1.4) | Pedidos de melhoria em cima de bugs do AppleWin; o dono manteve a paridade (#713, #1145, #1377) ou adiou (#1426, #1431) | O Rust reproduz o AppleWin |
| As Divergências aprovadas da seção 7 | Não são Divergências intencionais: sem prova de hardware (o AppleWin tem um defeito de software), mas com decisão explícita do dono (ADR 0006) | Registradas na seção 7 |

As melhorias de qualidade que não alteram o comportamento comparável (gravação atômica de disco e de save-state, recusa de arquivos hostis, erros visíveis de E/S, ROM do usuário inválida com fallback) não são Divergências intencionais: o AppleWin não tem o mesmo comportamento a comparar. Elas têm testes próprios.

## 7. Divergências aprovadas (ADR 0006)

Uma **Divergência aprovada** é um ponto em que o AppleII-Rust faz diferente do AppleWin `a8f0d1d` por decisão explícita do dono, sem prova de hardware, porque o AppleWin tem ali um defeito de software que só prejudica o usuário ([ADR 0006](../adr/0006-divergencias-aprovadas-pelo-dono.md), [GLOSSARY.md](../../GLOSSARY.md)). Não vale para a emulação do hardware (CPU, memória, vídeo, disco, áudio, cartas). Todas foram decididas pelo dono em **21/10/2026**; cada uma tem teste que passa e está em PASS na matriz. Um item novo exige decisão nova do dono; um agente não pode aprovar sozinho. A versão para o usuário está no [manual 12](../manual/12-diferencas-em-relacao-ao-applewin.md).

| Issue | O que faz o AppleWin `a8f0d1d` | O que faz o AppleII-Rust | Teste | Decisão |
|---|---|---|---|---|
| #916 (P2) conversão Pravets na impressora | As tabelas de conversão de latim para cirílico estão em UTF-8 (Pravets.cpp:210-214, bytes `C3 80`); a conversão sai quebrada | Converte para CP-1251 (Pravets82 `a` = `$C0`; Pravets8A minúsculas e maiúsculas) | `crates/apple2-hostio/tests/audit_perifericos.rs::gh916_pravets_conversion_maps_latin_to_cp1251_cyrillic` | 21/10/2026 |
| #855 (P2) `DebuggerAutoRun` | `DebugInitialize` roda antes de `MemInitialize` (AppleWin.cpp:921-924); `bsave` e `bpx` do `DebuggerAutoRun.txt` não surtem efeito | O depurador é inicializado com a máquina pronta; o script grava os 256 bytes e registra o breakpoint | `crates/apple2-debugger/tests/p23_depurador.rs::gh855_autorun_script_can_bsave_and_set_breakpoints` | 21/10/2026 |
| #1396 (P2) Alt+F2 e o Open Apple | O F2 é `ResetMachineState`, que chama `JoyReset()` e apaga o Open Apple (Utilities.cpp:554-555, Joystick.cpp:703); Alt+F2 não funciona | Mantém o Open Apple apertado depois do F2; Alt+F2 funciona (por exemplo, para pular o boot do HDD-1) | `crates/applewin/tests/p23_entrada.rs::gh1396_alt_f2_keeps_open_apple_pressed_approved_divergence` | 21/10/2026 |
| #280, #358, #1374 (P2/P3) tela cheia | Usa só a escala inteira e deixa bordas; em #358 a imagem era a de 640x480 | A tela Apple ocupa toda a altura (1080 de 1080), mantém 560:384 e fica centralizada; "Integer scaling" reproduz a escala do AppleWin | `crates/applewin/tests/p23_ui.rs::p23_fullscreen_fills_the_display` | 21/10/2026 |
| #117 (P3) widescreen | Defeito aberto na tela cheia widescreen | Nunca estica: `min(largura/560, altura/384)` com barras laterais | `crates/applewin/src/ui.rs::fit_tests::widescreen_keeps_the_4_3_ratio_and_centres_with_side_bars` | 21/10/2026 |
| #932 (P3) escala da janela | `fit_scale` usa só o monitor primário (`SM_CXSCREEN`) | Usa o monitor onde a janela está (`window.current_monitor`) | `p23_ui.rs::p23_window_scale_and_small_screens` | 21/10/2026 |
| #1448 (P2) "Fastest" em ][ e ][+ | Defeito aberto do "Fastest" no ][ e no ][+ | Converte 40 em `Speed::Max` em qualquer Modelo | `p23_ui.rs::p23_fastest_speed_on_every_model` | 21/10/2026 |
| #114 (P2) | `SYMUSER LOAD filename.sym` sem aspas não faz nada (`CmdSymbolsLoad` só lê `TYPE_QUOTED_2`, Debugger_Symbols.cpp:826) | Carrega o arquivo também sem aspas | `matriz_depurador_b.rs::gh114_symuser_load_accepts_an_unquoted_file_name` | 21/10/2026 |
| #245 (P2) | `SYMUSER SAVE` é um no-op sem mensagem (Debugger_Symbols.cpp:1130) | Grava a tabela em linhas `ADDR NOME`, avisa, e o `LOAD` restaura | `matriz_depurador_b.rs::gh245_symuser_save_then_load_round_trips` | 21/10/2026 |
| #458 (P3) Ctrl+PrintScreen no depurador | Copia só 43 das 48 linhas (`DEBUG_VIRTUAL_TEXT_HEIGHT`, Debugger_Display.h:64, Debugger_Display.cpp:728); as 5 últimas do console se perdem | Copia as 48 linhas | `p23_depurador.rs::gh458_ctrl_prtscn_text_has_the_whole_screen` | 21/10/2026 |
| #466 (P3) | `SYM CMDNTB.PR#` dá Syntax Error (o `#` final é operador do tokenizador) | Trata o `#` que fecha o nome como parte dele e acha o símbolo | `matriz_depurador_b.rs::gh466_symbol_names_with_hash_can_be_looked_up` | 21/10/2026 |
| #468 (P3) | `CD ..` faz `dir.join("..")` e o `PWD` mostra `.../y/..` | Resolve `..` e volta ao diretório pai | `matriz_depurador_b.rs::gh468_cd_dotdot_removes_the_last_directory` | 21/10/2026 |
| #1381 (P3) | `DA 2004` com `B_2100` existente mostra `da A_2101-1` (o `\|\| pData` de Debugger_Disassembler.cpp:318) | Mostra `da B_2100`, o símbolo exato, e só tenta alvo+1 se não houver símbolo exato | `matriz_depurador_b.rs::gh1381_da_prefers_the_exact_data_symbol` | 21/10/2026 |

Não são divergências (o dono manteve a paridade com o AppleWin): #713, #1145 e #1377 do depurador (seção 1.4). Adiadas: #1426 e #1431. Outras issues em que o port segue o AppleWin apesar de a issue pedir diferente: #520 (reset do RamWorks mantém o banco), #1313 (ruído do AY), #967 (ciclo fantasma), #961 (HLT/KIL), #266 (HOMEMOUSE). Estão na seção 4 ou nos CSV de resultados.

## 8. Limitações aceitas (ADR 0006)

Uma **Limitação aceita** é um FAIL conhecido que o dono decidiu não bloquear a release ([ADR 0006](../adr/0006-divergencias-aprovadas-pelo-dono.md)). Continua FAIL nos relatórios e no harness (`known_divergence`), com a causa registrada, e nunca é contada como PASS. Decisão do dono: **21/10/2026**, os 5 casos de PCM/SSI. Neles os eventos de 6522, AY e SSI263 são idênticos ao AppleWin, ciclo a ciclo; só as amostras diferem, porque o AppleWin gera o som em fatias de tempo do hospedeiro e o próprio Oráculo muda de fase com o fatiamento (M0 falha entre `--audio-slice 1000` e `250`). O núcleo do AY é idêntico amostra a amostra quando a linha do tempo é a mesma (`ay_core_oracle.rs`). Rode `conformance run --case ID`; o `conformance run --all` termina com 5 FAIL e código 3. O sexto caso da versão anterior, `audio-issue-1176-last-cycle`, passa desde `2927c25`.

| Caso | Issue Rust | Diferença medida | Causa raiz |
|---|---|---|---|
| `audio-ay-tone-pcm` (D-2) | #27 | frequência 8,77 % > 4,00 %; RMS 4,45 % > 2,00 % | O AppleWin gera o PCM em blocos de `(int)(44100 / (clk/intervalo + 0.5))` amostras (43 onde seriam 43,2: -0,4 % de tempo de AY por bloco) e o primeiro bloco sai ~1000 ciclos depois do ciclo 0. O Rust gera cada amostra no ciclo exato |
| `audio-ay-envelope-pcm` (D-2) | #27 | frequência 5,88 % > 1,00 % (16 contra 17 cruzamentos numa janela quase muda) | idem |
| `audio-ay-noise-pcm` (D-2) | #27 | frequência 58,33 % > 40,00 % | idem; o LFSR muda a taxa de cruzamentos com a fase |
| `audio-speaker-clicks-pcm` (D-3) | #7 | RMS 2,46 % > 2,00 %; 39.140 quadros no Oráculo contra 38.893 no Rust | `SetClksPerSpkrSample` (Speaker.cpp:221-229) usa 23 ciclos inteiros por amostra (clk/44100 = 23,14); o AppleWin compensa regulando a velocidade da CPU pelo buffer. O hardware não tem esse quantum |
| `audio-ssi263-phonemes-p333` (D-4) | #28 | primeiro fonema 1.683 ciclos > limiar 1.670 (13 ciclos, 0,8 %) | O fim do fonema só é visto pelo AppleWin em `PeriodicUpdate` (~1000 ciclos) e o primeiro `Update` pré-carrega 25 % do buffer; o Rust agenda `início + comprimento`. Na fase 0 o caso PASS; nos 12 fonemas seguintes e nas fixtures o Rust fica dentro da tolerância |

Efeito para o usuário: o som é igual ao ouvido, mas não é bit a bit idêntico ao do AppleWin. Copiar os blocos e o quantum do AppleWin fixaria um artefato do hospedeiro e pioraria o áudio sem prova de hardware.

## 9. Limites do Oráculo

- O Oráculo executável é o fork do audetto (22 commits antes do `a8f0d1d`); não é o AppleWin do Windows. Os arquivos de vídeo do fork e do clone de referência são idênticos (`NTSC.cpp`, `RGBMonitor.cpp` e outros), então a defasagem não afeta o vídeo. O executável oficial 1.32.1.0 (Wine) serve só para gerar save-states.
- Sem entrada de teclado nem troca de disco em execução; um só cartão de áudio por execução (`--audio-slot`); CPU a 1 MHz; sem relógio real (o No-Slot Clock devolve a hora do hospedeiro); `g_bFullSpeed` nunca roda; sem janela, DirectSound nem diálogos.
- O PCM do AY do próprio Oráculo não é reprodutível entre fatiamentos (calibração em audio-differential.md).
