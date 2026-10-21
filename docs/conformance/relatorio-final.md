# Relatório final de conformidade

Relatório do §21 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md), em português, com os 10 itens na ordem do plano. Esta é a **reavaliação** depois da onda final: as 1.115 issues com estado final (resultados `p23-*.csv`), a GH#1176 (reset do 6522), a GH#1340 (disco rápido com som), as correções de entrada, configuração, interface e núcleo, os 9 save-states reais corrigidos, o firmware do HDD 132.1 e o runner que compara `trace_rows_sha256`. Todos os números vêm de execuções feitas em 21/10/2026 sobre o SHA do item 1; nada vem de memória. Onde um número vem de um relatório gerado antes (vetores de CPU e Z80, smokes de plataforma, desempenho), a data do relatório é citada. A nota do item 9 é **autoavaliação; a revisão independente vem depois**.

## 1. Baseline com SHAs e versões

| Repositório | Referência | SHA completo |
|---|---|---|
| AppleII-Rust (código avaliado) | `main` de 21/10/2026 (`git rev-parse main`) | `bb01e3415c45fcb3d7e1103e788cc6cec2279e5f` |
| AppleII-Rust (versão anterior deste relatório) | `main` antes da onda final | `22dbf2b7a4c57d9b6ede7be39a577e93196c1a3f` |
| AppleII-Rust (ponto de partida da auditoria) | `main` de 20/10/2026 | `6da60488a2c61b82099d6a7aabece21125eb0963` |
| AppleII-Rust (última versão publicada) | tag `v0.1.0` (workspace `0.1.0`) | `87110ff15ef35f58db882ab46c08f69296f7250d` |
| AppleWin (Oráculo de código) | commit `a8f0d1d`, 17/10/2026 | `a8f0d1d40f551ad9e010522c29c2891e33c88275` |
| AppleWin (release de referência) | tag `v1.32.1.0` (5 commits antes de `a8f0d1d`) | `daf813dcaced7469bbbc16f23905cfd75abe8496` |
| AppleWin-Test (corpus, sem licença; cache local) | HEAD de `master` em 20/10/2026 | `4b113ce8cff1426bda5595cac9cb04776adfbb5d` |
| audetto/AppleWin (Oráculo executável, Linux) | HEAD de `master` em 20/10/2026; núcleo AppleWin `3e8054b`; `audetto-version.patch` leva a versão a 1.32.1.0 | `a1e20c0362f392dc25f1a3b218d2f84d4f1a25da` |

Versões: AppleII-Rust `0.1.0` (tag) e `0.2.0-alpha.1` (preparada na branch local `release/v0.2.0-alpha.1`, sem tag nem publicação). AppleWin `1.32.1.0` mais 5 commits. Detalhes e a comparação de commits: [baseline.md](baseline.md).

Ambiente de execução: Ubuntu 26.04.1 LTS, kernel 7.0.0-38, x86_64, AMD Ryzen 7 3700X (16 threads, 125 GiB), `rustc 1.98.1 (48a229cea 2026-09-01)`, Wine 10.0 para o Windows via `cargo-xwin`. A máquina estava compartilhada com outros processos, o que afeta tempos, não resultados.

## 2. Tabela-resumo

Versão condensada. A tabela completa, com uma linha por componente e Modelo, está em [capability-matrix.md](capability-matrix.md).

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| CPU 6502 / 65C02 | ][, ][+, //e, //e Enhanced | PASS | 7 traces de 0,5 a 5 milhões de ciclos idênticos ao Oráculo; 10,22 milhões de vetores SingleStepTests; determinismo 10 x 2M | Diferenças AppleWin x hardware catalogadas (ciclo fantasma, RMB/SMB, WAI/STP) copiadas do AppleWin | #17 | P0 |
| Memória (LC, Saturn, RamWorks, AUX, floating bus) | ][+, //e, //e Enhanced | PASS | `audit_memoria.rs`, a2audit, estados `LC-*`, `RamWorks_r4`, `Saturn64`, `Saturn128`, `Saturn128-Bank3-2` (todos PASS no trace de 100.000 ciclos) | Nenhuma medida | #18, #19 | P0 |
| Vídeo | ][, ][+, //e, //e Enhanced | PASS (1.573 de 1.573 comparações pixel a pixel); FAIL (1 cena PAL) | 583 casos `video-*` | #241: 1 pixel no Color Monitor PAL. Clones sem comparação de framebuffer | #20, #21, #46 | P0 |
| Disco (Disk II, WOZ, 2MG, gz/zip, HDD/SmartPort) | ][+, //e, //e Enhanced | PASS | 56 casos `disk-*`, 103.146.754 instruções comparadas; firmware do HDD 132.1 | HDC no ][+ não comparável; NIB real e INIT/FORMAT sem Oráculo | #22, #23, #24 | P0 |
| Save-state | vários | PASS (88 de 115 reais; atômico; fuzz); BLOCKED (27); **0 FAIL** | [savestate-import.md](../../tests/conformance/reports/savestate-import.md); 87 casos `savestate-*` | Nenhuma no trace. O teste de formato acusa 1 a 8 campos em quase todos os arquivos (o mais comum: bit B do P, normalizado de propósito) | #25, #26 | P0 |
| Áudio: eventos e IRQ do 6522 | ][+, //e | PASS | 34 casos `audio-*` PASS, inclusive `audio-issue-1176-last-cycle` (corrigido) | nenhuma | #27, #28 | P1 |
| Áudio: PCM do AY e do alto-falante, SSI263 +666 | ][+, //e | FAIL (5 casos) | `audio-ay-{tone,envelope,noise}-pcm`, `audio-speaker-clicks-pcm`, `audio-ssi263-phonemes-p333` | blocos e quantum de tempo do AppleWin; 23 ciclos por amostra | #7, #27, #28 | P1 |
| Z80 SoftCard | ][+, //e | PASS (vetores de terceiros); BLOCKED (sistema/CP/M) | 1.604.000 vetores; ZEXDOC/ZEXALL 65/65 (relatório de 20/10/2026) | F3/F5 de blocos repetindo segue o hardware (candidata) | #29 | P1 |
| Cartas (mouse, 4Play, SNES MAX, relógios, SSC, Uthernet) | ][+, //e | PASS (mouse, 4Play, SNES MAX nos estados reais); BLOCKED (rede/serial reais) | `entrada-cartas.csv`, `p23-entrada.csv`, `p23-perifericos.csv`; `dazzledraw1` x3 e `SNES_MAX-slot4` PASS | #1396 (Alt+F2): o port mantém o Open Apple | #30, #31, #32, #33 | P1 |
| Depurador | ][+, //e Enhanced | PASS 73; FAIL 11; BLOCKED 7; N/A 39 (130 issues) | `results/depurador.csv`, `results/p23-depurador.csv` | Os 11 FAIL (10 mais a #458) são comportamentos do AppleWin reproduzidos | #34 | P1 |
| Interface, linha de comando, configuração | todos | PASS (testes próprios, 49 issues de UI e 73 de configuração verificadas); sem Oráculo de janela | `p23-ui.csv`, `p23-config-outras.csv`, `hu0702_input.rs`, `cli.rs` | Divergências a favor do usuário aguardam decisão (item 3 do gaps) | #35 | P1 |
| Linux x86_64 | Linux | PASS (smoke, pacote extraído; 20/10/2026) | [relatorio-plataformas.md](platform/relatorio-plataformas.md) | Wayland nativo e dispositivos: manual | #36 | P0 |
| Windows (Wine) / Windows nativo | Windows | PASS (Wine, 20/10/2026) / BLOCKED (nativo) | [windows-wine.md](platform/windows-wine.md) | Wine não é Windows | #36 | P0 |
| macOS | macOS | BLOCKED | sem hardware | tudo | #36 | P0 |
| Extensões: rewind, DAP, laboratório, WASM | Enhanced //e | IMPLEMENTED | crates próprios | sem equivalente no AppleWin | #40 a #44 | P2 a P3 |

## 3. Sumário por status das issues upstream

Coleta de 21/10/2026 (00:47 UTC) no HEAD do AppleWin `a8f0d1d`: 1.516 itens (1.115 issues e 401 PRs). Issues: **358 abertas e 757 fechadas**. Fonte: [applewin-issue-matrix.csv](applewin-issue-matrix.csv) e [issue-audit.md](issue-audit.md). `python3 tools/conformance/apply_results.py --check` confirma "Matriz em dia" (849 issues com resultado aplicado, 0 linhas por alterar). **Não resta nenhuma issue UNVERIFIED**: cada uma das 1.115 tem estado e `teste`, `evidencia` ou `justificativa_na` preenchidos (849 por resultado individual, 266 sem prioridade como NOT_APPLICABLE com justificativa da triagem).

| Estado de verificação | Abertas | Fechadas | Total |
|---|---:|---:|---:|
| PASS | 89 | 390 | **479** |
| FAIL | 13 | 1 | **14** |
| BLOCKED | 40 | 93 | **133** |
| NOT_APPLICABLE | 216 | 273 | **489** |
| UNVERIFIED | 0 | 0 | **0** |
| Total | 358 | 757 | **1.115** |

Antes da onda final (relatório sobre `22dbf2b`): 205 PASS, 13 FAIL, 87 BLOCKED, 286 NOT_APPLICABLE, 524 UNVERIFIED. Taxa de PASS sobre as aplicáveis (todas menos NOT_APPLICABLE): 479 de 626 = 76,5 % (antes 24,7 %). Nenhuma issue fechada está em PASS sem teste (0 PASS sem teste na matriz).

Por prioridade:

| Prioridade | Total | PASS | FAIL | BLOCKED | NOT_APPLICABLE | UNVERIFIED |
|---|---:|---:|---:|---:|---:|---:|
| P0 | 30 | 23 | 0 | 7 | 0 | 0 |
| P1 | 216 | 129 | 1 | 74 | 12 | 0 |
| P2 | 276 | 185 | 6 | 37 | 48 | 0 |
| P3 | 327 | 142 | 7 | 15 | 163 | 0 |
| sem prioridade | 266 | 0 | 0 | 0 | 266 | 0 |

Os 14 FAIL: **#241** (P1, Color Monitor PAL), **#1396** (P2, Alt+F2 apaga o Open Apple no AppleWin e o port mantém), **#458** e **#781** (P3) e **10 do depurador** (#114, #245, #466, #468, #713, #1145, #1377, #1381, #1426, #1431; P2 e P3). Saíram dos FAIL desde o relatório anterior: **#381** (VBL do mouse), **#1176** (reset do 6522); a **#1340** passou de FAIL a PASS com a correção `fe50e4b`. Os 7 P0 BLOCKED não mudaram: **#4, #70, #400, #422, #564, #640, #1126**; todos fechados no AppleWin e todos dependentes de software de terceiros sem fixture ou de relógio e janela reais. BLOCKED por categoria (133): disco 54, vídeo 16, Mockingboard 12, CPU 9, áudio 9, memória 6, depurador 5, serial 4, teclado 3, HDD 3, e o resto em parcelas de 1 a 2 (outra, relógio, save-state, rede, SSI263, Z80, joystick). A maioria é software de terceiros sem fixture licenciada; o restante é comportamento de frontend sem Oráculo headless ou hardware.

Auditoria independente da triagem: [issue-audit-review.md](issue-audit-review.md) (amostra de 465 issues, concordância de 91 % em qualquer campo na parte aleatória; 168 correções; 334 linhas reexaminadas por classe). **Os 849 resultados da onda final são autoavaliação de quem os produziu e não têm amostra independente.**

## 4. Top 10 problemas críticos

### 4.1 Os 10 abertos, em ordem de gravidade

Cada um com passos para reproduzir, causa raiz e estado. SHA observado: `bb01e34`.

| # | Problema | Passos para reproduzir | Causa raiz | Estado |
|---|---|---|---|---|
| 1 | **Windows nativo e macOS nunca foram executados; a CI nunca foi executada (P0, G1)** | Não há o que reproduzir: `tools/package/smoke-wine.sh` roda sob Wine, que não é Windows; `ci.yml`, `conformance.yml` e `release.yml` jamais rodaram nesta fase. Roteiro: [manual-tests/plataformas.md](manual-tests/plataformas.md) | Sem hardware nem licença de SDK da Apple; cota de CI esgotada | **Aberto, BLOCKED**. Decisões 6, 7 e 8 |
| 2 | **7 P0 BLOCKED (#4, #70, #400, #422, #564, #640, #1126)** | `grep ',BLOCKED,' docs/conformance/results/*.csv` mostra a causa de cada um. Os mecanismos sintéticos de #400, #422, #564 e #640 passam em testes Rust | Precisam de software de terceiros sem fixture (UCSD Pascal, entre outros), do relógio real ou de um Oráculo de janela e de troca de disco ao vivo | **Aberto**. Decisão 5 |
| 3 | **PCM do AY e do alto-falante, SSI263 +666 (P1, 5 casos FAIL)** | `conformance run --case audio-ay-tone-pcm` (e `-envelope-pcm`, `-noise-pcm`, `audio-speaker-clicks-pcm`, `audio-ssi263-phonemes-p333`) | O AppleWin gera o PCM em blocos que dependem do fatiamento de tempo do hospedeiro, com 23 ciclos por amostra no alto-falante e `PeriodicUpdate` de ~1000 ciclos; o Rust gera cada amostra no ciclo exato. O próprio Oráculo muda de fase com o fatiamento | **Aberto**. Núcleo do AY e eventos idênticos. Decisão 4 |
| 4 | **#241: 1 pixel de borda no Color Monitor PAL (P1)** | Cena `tests/conformance/inputs/video/issue-0241-floatbus-dlores-text80.scene` no diferencial de vídeo, PAL + Color Monitor + `TEXT/GR` por linha + texto na página 2: pixel (18,18), Oráculo (225,239,255), Rust preto | Não localizada; duas hipóteses descartadas (zeragem em `pal_mid_line_fixups`, `addr -= 2` na troca TEXT para gráfico). NTSC e Color TV PAL passam | **Aberto**, FAIL |
| 5 | **11 FAIL do depurador (#114, #245, #458, #466, #468, #713, #1145, #1377, #1381, #1426, #1431)** | `cargo test -p apple2-debugger --release -- --ignored` (os testes `#[ignore = "FAIL ..."]` reproduzem o comportamento que a issue pede mudar) | São bugs do próprio AppleWin `a8f0d1d` (`SYMUSER SAVE` no-op, `CD ..`, 43 de 48 linhas na cópia, aspas simples, tabelas de símbolos) que o port reproduz por regra (ADR 0004) | **Aberto, aguardando o dono**. Decisão 1 |
| 6 | **#1396: Alt+F2 apaga o Open Apple no AppleWin; no port, não (P2)** | `cargo test -p applewin --release --test p23_entrada -- --ignored gh1396` | O F2 do AppleWin chama `JoyReset()` (Utilities.cpp:554-555, Joystick.cpp:703); `Command::PowerCycle` do port não toca os botões | **Aberto, aguardando o dono**. Decisão 2 |
| 7 | **27 save-states reais BLOCKED** | `savestate-compare.py` sobre o corpus: 18 por imagem ausente (`DiskFormat/*`, `AZTEC`), 3 SSC por TCP/serial, 2 por hora real, 2 `skm-*`, 1 Z80, 1 Uthernet | Falta imagem no corpus, ou o Oráculo não tem o recurso (Shift Key Mod, SSC, hora real) | **Aberto**. Sem decisão pendente; precisa de fixtures |
| 8 | **Risco de dados: HDD acima de 2 MiB gravado no lugar; imagens abertas por save-state de terceiros (AUD-C-037); duas instâncias na mesma imagem** | Interromper a escrita num HDD de 32 MB: o bloco pode ficar pela metade (não há renomeação atômica); abrir um `.aws.yaml` de terceiros que cita uma imagem local | Escolha de projeto (atualização bloco a bloco com `sync_data`); igual ao AppleWin; sem trava de arquivo | **Aberto** (risco médio; UNVERIFIED para as duas instâncias). Veja o item 8 |
| 9 | **Pacotes sem assinatura nem notarização; `release.yml` nunca executou** | Instalar o pacote no Windows (SmartScreen) ou no macOS (Gatekeeper); [assinatura.md](platform/assinatura.md) | Sem certificado Authenticode nem Developer ID | **Aberto**. Decisão 8 |
| 10 | **Divergências a favor do usuário já presentes, sem decisão (#855, #916, #280/#358/#1374, #932, #1448, #1396)** | Rodar os testes citados na seção "Divergências do AppleWin pendentes de decisão" do [gaps](gaps-and-known-limitations.md) | O port faz diferente do AppleWin onde o AppleWin tem defeito; a ADR 0004 diz que o AppleWin vence enquanto o dono não aprovar uma Divergência intencional | **Aberto, aguardando o dono**. Decisões 2 e 3 |

### 4.2 Os corrigidos (achados da auditoria e fechados nesta rodada)

| # | Problema | Causa raiz | Correção e regressão |
|---|---|---|---|
| a | **AUD-C-060 (P0)**: importar um save-state real do AppleWin (#1520, `Cycles This Frame: 26190`) travava a emulação (`run_frame()` não terminava) | O carregador copiava o contador cru (maior que 17.030) e `tick()` fechava o quadro com `==` | Reduzido módulo o quadro e `>=`; `audit_cycles_this_frame.rs` |
| b | **AUD-C-010 (P0)**: save-state hostil (`Length: -1`, bomba zlib) derrubava o carregador | `Length` como `u64` sem teto; descompressão sem limite | Tetos de 64 MiB e 256 MiB; `fuzz_loader.rs`, `audit_robustness.rs` |
| c | **Saturn (AUD-C-001/003, #1486, #1495, P0)**: a contagem de bancos configurada se perdia | `Saturn128K { banks }` virava tipo sem campo | `MachineConfig.saturn_banks` até a carta e o save-state (`7e5c46f`); `audit_saturn_banks.rs`. Os estados reais `Saturn64` e `Saturn128` passam hoje porque o Oráculo lia lixo de heap (`MALLOC_PERTURB_=255` no script); não era defeito do port |
| d | **Gravação não atômica de save-state e de imagens (AUD-C-022, AUD-C-033)** | `set_len(0)` e `write_all` no mesmo arquivo | Temporário, `sync_all`, `rename`, `fsync` do diretório; `atomic_write.rs`, `crash_safety.rs` |
| e | **`write_block` de 2 TB (AUD-C-035)** | Um bloco após o fim crescia a imagem sem limite | Recusa blocos a partir de 65.536; `fuzz_parsers.rs` |
| f | **Escrita pendente de disco perdida (AUD-C-030/031/034)** | `rebuild`/`load_machine` sem `disks.flush` | Flush antes de reconstruir, carregar e no SIGTERM; `audit_persistence.rs`, `sigterm.rs` |
| g | **Tecla presa (AUD-P-028)** | O evento de soltar não chegava ao teclado | Soltar sempre chega; `hu0702_input.rs` |
| h | **Panics e alocação gigante no depurador** | Fatiamento por byte em UTF-8; intervalo invertido | `4cd5132`, `d2e9962`; `fuzz_cmdline.rs` |
| i | **Fechado nesta rodada, P1: IRQ do 6522 um opcode depois (#1176)** | `Reset()` do 6522 não fechava o portão `m_canAssertIRQ` dos eventos de timer, e um evento de T1 sem IER marcava "IRQ no último ciclo do opcode" | `2927c25`; `audio-issue-1176-last-cycle` PASS (sem `known_divergence`), mais `6522-one-shot` e `SSI263-test` (que na verdade dependiam da ROM do SSC no Oráculo) |
| j | **Fechado nesta rodada, P1: IRQ de VBL do mouse 7 ciclos antes depois de carregar (#381) e firmware do DIB do HDD** | O evento recalculava a posição do quadro somando de novo os ciclos do opcode; o DIB do SmartPort dizia 130.0 em vez de 132.1 | `bf17216`, `75b0956`; testes em `mouse.rs` e `hdd.rs`; `dazzledraw1` x3 e `SmartPort-firmware` PASS |
| k | **Fechado nesta rodada, P2: GH#1340, disco rápido acelerava com o som ativo** | `Emulator::effective_full_speed` ignorava `Spkr_IsActive` e o Mockingboard | `fe50e4b`; `gh1340_full_speed_sound.rs` (8 testes) |

Dos 9 arquivos de save-state em FAIL no relatório anterior, **4 eram defeito do port** (VBL do mouse, 3 arquivos, e firmware do SmartPort, 1 arquivo; 2 causas) e **5 eram defeito do Oráculo** (heap sem zerar nos bancos Saturn, 2 arquivos; `SNESMAX.cpp` sem tratador de E/S, 1; ROM do SSC ausente, 2). As três correções do Oráculo (`MALLOC_PERTURB_`, `audetto-snesmax.patch`, `audetto-ssc-rom.patch`) corrigem o instrumento de medida e estão versionadas em `tools/conformance/oracle/`.

Risco que não é um defeito achado, mas pesa na recomendação: **Windows e macOS nativos nunca foram executados** (item 1 acima).

## 5. Backlog importável

- [backlog.md](backlog.md) e [backlog.json](backlog.json): 13 épicos (EP-00 a EP-12) e 30 histórias (HU-0001 a HU-1205), com prioridade, onda, Gate e número da issue no GitHub, gerados de `docs/prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md` por `tools/conformance/backlog.py` (sincronização idempotente com `gh`).
- Issues no repositório ChicoFigueiredo/AppleII-Rust: **#2 a #14** (épicos), **#15 a #44** (histórias HU) e **#45 a #80** (36 histórias do EP-11, derivadas das 358 issues abertas do AppleWin: [upstream-open-selected.md](upstream-open-selected.md), triagem completa em [upstream-open-issues-triage.md](upstream-open-issues-triage.md)). Cada história tem Critérios de aceite, dependências e rótulos.
- **Estado das histórias HU (#15 a #44)**: a label `estado:UNVERIFIED` foi trocada pela do estado real por `tools/conformance/update_hu_issues.py` (idempotente, com `--dry-run`): 14 PASS, 5 FAIL, 11 BLOCKED. `estado:PASS` só vale com todos os CA em PASS. Nenhuma issue foi fechada: o fechamento é decisão do dono, depois da revisão independente. O campo "Estado atual" do corpo das issues continua o do `backlog.py`.
- Fila da onda 1 (246 issues P0/P1 de runtime, formato e depurador): [wave1-candidates.md](wave1-candidates.md); a maior parte está hoje em PASS.
- Pendências a abrir como histórias, se o dono quiser: PCM (política v3 do PCM); #241 PAL; fixtures dos 7 P0 BLOCKED; execução em Windows e macOS nativos; as nove decisões da seção abaixo.

## 6. Testes executados e não executados

### Executados (Linux x86_64 nativo, SHA do item 1, em 21/10/2026)

| Execução | Comando | Resultado |
|---|---|---|
| Testes do workspace | `cargo test --workspace --release --no-fail-fast` | **1.577 passam, 0 falham, 22 ignorados** (184 suítes: 167 executáveis de teste e 17 doctests; exit 0). Os 22 ignorados: vetores pesados que exigem download de 2,5 GB (65x02) e 1,3 GB (Z80 e ZEX), Klaus estendido, 11 testes que reproduzem FAIL (9 do depurador, #458 e #1396; comportamento do AppleWin), 3 medidas de tempo (`emulated_speed`, `disk_boot_speed`, `frames_per_second`), a carga do corpus AppleWin-Test, um gerador manual, um reprodutor de SNES MAX e um doctest |
| Casos de conformidade | `cargo build --release -p apple2-conformance && ./target/release/conformance run --all --report target/conformance-report` | **778 casos: 773 PASS, 5 FAIL, 0 BLOCKED** (com o corpus AppleWin-Test no cache; sem ele, parte dos casos fica BLOCKED). Por área: vídeo 583, save-state 87, disco 56, áudio 39 (34 PASS e 5 FAIL), CPU/boot/determinismo/framebuffer 13. Os 5 FAIL são os casos com divergência conhecida: `audio-ay-tone-pcm`, `audio-ay-envelope-pcm`, `audio-ay-noise-pcm`, `audio-speaker-clicks-pcm` e `audio-ssi263-phonemes-p333`. O comando sai com código 3 enquanto houver FAIL |
| Matriz de issues | `python3 tools/conformance/apply_results.py --check` e `summarize_issue_matrix.py` | "Matriz em dia": PASS 479, FAIL 14, BLOCKED 133, NOT_APPLICABLE 489, UNVERIFIED 0 (849 resultados aplicados). Achado e corrigido nesta execução: `p23-perifericos.csv` ainda trazia o FAIL medido da #1340 antes da correção e, pela precedência FAIL > PASS, mantinha a issue em FAIL; a linha foi trocada por PASS com o texto original preservado |
| Importação de save-states reais | `tools/conformance/savestate-compare.py` sobre os 3 estados do Wine e os 112 do corpus (100.000 ciclos cada), agregado como em `savestate-report.py` | **88 PASS, 0 FAIL, 27 BLOCKED** (115 arquivos), igual ao [relatório versionado](../../tests/conformance/reports/savestate-import.md). Na leitura crua, 8 dos 27 aparecem como FAIL do trace (`NoSlotClockTest*`, `skm-*`, `SSC-*`, `CPM2.2(56k)-3`); o `savestate-report.py` os reclassifica como BLOCKED por causa conhecida (hora real, Shift Key Mod ausente no Oráculo, TCP/serial, Z80). Regerar o relatório com caminhos absolutos estraga os casos TOML; o relatório versionado foi mantido |
| Vetores 65x02 | `cargo test -p apple2-cpu --release --test singlestep_65x02 -- --ignored` | 10.220.000 de vetores, 0 inesperados (relatório versionado de 20/10/2026, [65x02-singlestep.md](../../tests/conformance/reports/65x02-singlestep.md)); **não refeito** nesta rodada (2,5 GB de download) |
| Vetores Z80 e ZEXALL | `cargo test -p apple2-cards --release --test z80_singlestep -- --ignored` | 1.604.000 de 1.604.000; ZEXDOC e ZEXALL 65/65 (relatórios de 20/10/2026, [z80-singlestep.md](../../tests/conformance/reports/z80-singlestep.md), [z80-zex.md](../../tests/conformance/reports/z80-zex.md)); **não refeito** |
| Corpus AppleWin-Test (carga de 1 quadro) | `applewin_test_corpus.rs` | 94 OK, 18 OK com avisos, de 112 (relatório de 20/10/2026, [applewin-test-load.md](../../tests/conformance/reports/applewin-test-load.md)); carregar não é PASS de fidelidade |
| Windows via Wine 10.0 | `tools/package/smoke-wine.sh` | PASS (relatório de 20/10/2026, [windows-wine.md](platform/windows-wine.md)); **não refeito** |
| Linux: pacote e smoke | `tools/package/smoke-linux.sh`, `test-package.sh` | PASS, 17 verificações (relatório de 20/10/2026, [relatorio-plataformas.md](platform/relatorio-plataformas.md)); **não refeito** |
| Desempenho | `apple2-bench` | [performance.md](../../tests/conformance/reports/performance.md), em máquina compartilhada; não refeito |

`cargo clippy --workspace --all-targets -- -D warnings`: sem avisos (exit 0). `cargo fmt --all -- --check`: sem diferenças (exit 0). Ambos em 21/10/2026 sobre o SHA do item 1 mais as alterações de documentos e do script desta entrega.

Relatórios por sistema operacional: Linux nativo, [relatorio-plataformas.md](platform/relatorio-plataformas.md); Windows (Wine), [windows-wine.md](platform/windows-wine.md); Windows nativo e macOS, roteiros em [manual-tests/plataformas.md](manual-tests/plataformas.md) (não executados). Os smokes de Linux e Wine são de 20/10/2026 (commit base `516c123`); os do SHA atual não foram refeitos, só as suítes de testes e de conformidade.

### Não executados

- **Windows nativo e macOS (Intel e Apple Silicon)**: build, smoke, GUI, áudio, DPI, gamepad, permissões, Gatekeeper, SmartScreen. BLOCKED.
- **CI do GitHub e do GitLab**: nenhuma execução foi acionada (cota esgotada; só o dono autoriza). `release.yml` nunca rodou.
- **Hardware físico**: gamepad, captura de mouse, som audível, serial real, pcap/Npcap, impressora, teclado regional ([manual-tests/](manual-tests/)).
- Fuzzing de longa duração (`cargo-fuzz`); queda real de energia; Wayland nativo.
- Jogos e software de terceiros sem fixture (a maior parte das 133 issues BLOCKED).
- 27 save-states do corpus (imagens ausentes, rede, hora real, Shift Key Mod, Z80).
- Revisão independente dos 849 resultados da onda final.
- Fixtures de AppleWin real em Windows (só o AppleWin 1.32.1.0 sob Wine gerou save-states).

## 7. Comparação diferencial AppleWin x Rust

Oráculo executável: fork audetto no SHA `a1e20c0` (item 1). Política de comparação definida antes das execuções em [test-oracles.md](test-oracles.md): estado digital comparado de forma exata; áudio e vídeo analógicos com limiares registrados antes.

| Área | Resultado | Registro |
|---|---|---|
| **CPU: traces** | 7 traces idênticos em todos os campos (`cycle, pc, op, a, x, y, sp, p`): ][+ (0,5 e 2 milhões de ciclos), //e Enhanced (0,5 e 2 milhões), ][ original (2 milhões), //e (2 milhões) e //e Enhanced com boot do DOS 3.3 (5 milhões de ciclos, 1.821.912 instruções) | [cpu-trace-differential.md](audit/cpu-trace-differential.md). Sete divergências de ambiente e convenção, todas resolvidas; nenhuma era erro do núcleo 6502 |
| **CPU: vetores** | 10.220.000 de vetores SingleStepTests/65x02, 0 inesperados: 3.521.157 estritos e as diferenças AppleWin x hardware catalogadas (D0 a D10) | [65x02-singlestep.md](../../tests/conformance/reports/65x02-singlestep.md) |
| **Vídeo** | **1.573 telas** comparadas com o framebuffer inteiro (600x420 ou 680x436 com VidHD), diferença máxima de canal 0, Oráculo calibrado contra ele mesmo: 1.573 PASS, 0 FAIL. Hoje são 583 casos `video-*` no harness (a onda final acrescentou estilos de vídeo, o Mac LC e as cenas de issues). A exceção que permanece é a cena #241 em Color Monitor PAL (1 pixel), fora das 1.573 | [video-differential.md](audit/video-differential.md) |
| **Disco** | 56 casos `disk-*`; **103.146.754 instruções** comparadas e SHA-256 das imagens ao final; 9 WOZ reais, DSK, `.do.gz`, `.zip`, 2MG, HDD de 32 MB. Com o firmware do DIB em 132.1 as referências `disk-*hdd*` saíram idênticas. O defeito do Rust achado no caminho foi o `write_block` de 2 TB | [disk-differential.md](audit/disk-differential.md) |
| **Save-states reais** | 115 arquivos (112 do AppleWin-Test e 3 do AppleWin 1.32.1.0 sob Wine), 100.000 ciclos por arquivo: **88 PASS, 0 FAIL, 27 BLOCKED**. O trace compara linha a linha e o runner passa a comparar também `trace_rows_sha256`. O estado salvo pelo Rust e carregado no Oráculo também bate (`oracle_roundtrip.rs`, 30.000 ciclos) | [savestate-import.md](../../tests/conformance/reports/savestate-import.md); esquema em [savestate-schema.md](savestate-schema.md) |
| **Áudio (eventos)** | Escritas 6522/AY, timers, IRQ (inclusive o último ciclo do opcode, #1176), cliques do alto-falante, soquetes e `6522a-bad`: eventos idênticos incluindo o ciclo; SSI263 e SC-01: duração de fonema dentro da tolerância medida no Oráculo; núcleo do AY contra `AY8910.cpp`: ~330 mil amostras idênticas | [audio-differential.md](audit/audio-differential.md) |
| **Áudio (PCM)** | **5 FAIL**: PCM do AY (tom, envelope, ruído; D-2), PCM do alto-falante (D-3) e SSI263 fase +666 (D-4) | idem |
| **Z80** | 1.604.000 de 1.604.000 vetores SingleStepTests/z80; ZEXDOC e ZEXALL 65/65. É comparação com vetores de terceiros, não com o Z80VICE do AppleWin | [z80-singlestep.md](../../tests/conformance/reports/z80-singlestep.md), [z80-zex.md](../../tests/conformance/reports/z80-zex.md) |
| **Depurador** | 130 issues: 73 PASS, 11 FAIL (bugs do AppleWin reproduzidos), 7 BLOCKED, 39 N/A; 251 comandos (249 IMPLEMENTED, 2 N/A); fuzz da linha de comando | [results/depurador.csv](results/depurador.csv), [results/p23-depurador.csv](results/p23-depurador.csv), [debugger-commands.md](audit/debugger-commands.md) |

Artefatos de diferença: o harness deixa o RGBA dos dois lados e o diagnóstico (pixels, caixa envolvente, MAE) em `target/video-differential` quando algo falha; os traces de referência estão em `tests/conformance/reference-traces/`, os eventos e PCM do Oráculo em `tests/conformance/reference-audio/`. As imagens de referência dos 3 save-states de Wine estão em `tests/conformance/inputs/applewin-real/*.png`.

## 8. Matriz de risco de corrupção de disco e de save-state

Impacto: perda ou corrupção de dados do usuário. Os riscos "residuais" são os que a mitigação não elimina. Esta matriz não mudou na onda final: nenhum código de gravação foi alterado; as linhas foram reconferidas contra os testes que passaram em 21/10/2026.

| Cenário | Impacto | Antes (`6da6048`) | Mitigação | Evidência | Risco residual |
|---|---|---|---|---|---|
| Queda no meio da regravação de disquete (DSK/DO/PO/NIB/WOZ/2MG, `.gz`, `.zip`) | Alto | Truncava a imagem | Temporário + `sync_all` + `rename` + `fsync` do diretório, links seguidos, permissões preservadas | `store.rs`, `crash_safety.rs`, `integrity.rs` | Baixo: erro de E/S injetado, sem queda real de energia |
| Queda no meio da gravação do HDD (imagens maiores que 2 MiB, até 32 MB) | Alto | Truncava | Atualização no lugar, bloco a bloco com `sync_data`; sem renomear | `hdd.rs` testes | **Médio**: o arquivo nunca é substituído atomicamente; uma queda pode deixar um bloco escrito pela metade |
| Queda no meio da gravação do save-state | Alto | Destruía o último save válido | `write_atomic` (temporário no mesmo diretório, `sync_all`, `rename`, sync do diretório; `MOVEFILE_REPLACE_EXISTING` no Windows) | `atomic_write.rs`, `audit_persistence.rs` | Baixo; Windows nativo não testado |
| Save-state hostil (`Length`, bomba zlib, YAML truncado ou mutado) | Alto (travar ou abortar) | Panic/alocação gigante | Tetos de 64 MiB e 256 MiB, validação de linhas, fuzz | `fuzz_loader.rs`, `audit_robustness.rs` | Baixo; `cargo-fuzz` de longa duração não executado |
| Save-state de terceiros aponta imagens e as abre para leitura e escrita (AUD-C-037) | Médio (altera arquivo local) | Igual ao AppleWin | Nenhuma ainda | `state_io.rs` | **Médio** (UNVERIFIED): proposta é abrir somente leitura fora do diretório do estado |
| Imagens malformadas ou gigantes (WOZ, gzip, zip, 2MG, nomes com `../`) | Alto | `write_block` de 2 TB, bombas | Limites antes de ler, orçamento de descompressão, recusa de blocos > 32 MB, fuzz | `audit_images.rs`, `fuzz_parsers.rs` | Baixo |
| Trocar configuração ou carregar estado com trilha suja | Alto | Perdia a escrita | `disks.flush` antes de `rebuild` e `load_machine`; flush ao parar o motor, ejetar, sair | `audit_persistence.rs`, `audit_disk_flush.rs` | Baixo; o frontend atualiza os cartões por quadro, não por lote de 1 ms (até 16 ms de diferença no atraso do motor) |
| Encerramento por sinal (SIGTERM, SIGINT, SIGHUP) | Médio | Sem tratamento | Handler pede saída pela via normal; segundo sinal encerra na hora | `sigterm.rs` | Baixo; Windows nativo não testado |
| Strings do YAML (`\\`, `#`, Unicode, espaços) mudando a cada save | Médio | Perdia uma contrabarra por ciclo | Escape sempre; round-trip determinístico | `audit_robustness.rs`, `roundtrip_determinism.rs`, `oracle_roundtrip.rs` | Baixo |
| Duas instâncias abrindo a mesma imagem | Alto | Sem trava de arquivo | Nenhuma | não testado | **UNVERIFIED** |
| Estado importado com o Rust divergindo do AppleWin | Baixo para dados; médio para fidelidade | 9 arquivos divergiam | Os 9 passaram a bater no trace (2 correções do port, 7 do Oráculo) | `savestate-import.md` | Baixo para o trace; o teste de formato ainda acusa campos não modelados (o mais comum é o bit B do P, normalizado de propósito) |

## 9. Nota e Gates

**Autoavaliação; a revisão independente vem depois.** Cálculo e justificativa por dimensão em [release-readiness.md](release-readiness.md).

| Dimensão | Pontos | Nota (antes) | Nota |
|---|---:|---:|---:|
| Especificação e critérios de aceite | 25 | 20 | 22 |
| Corretude e evidência | 25 | 18 | 20 |
| Testes e regressão | 15 | 12 | 12 |
| Arquitetura e manutenção | 10 | 8 | 8 |
| Segurança, dados e resiliência | 10 | 8 | 8 |
| Higiene da entrega | 5 | 4 | 4 |
| Eficiência/retrabalho | 10 | 6 | 7 |
| **Total** | **100** | **76** | **81** |

A nota está bem abaixo de 96: o trabalho **não** está aceito como completo (veja a leitura da nota em [release-readiness.md](release-readiness.md)).

| Gate | Estado | Motivo |
|---|---|---|
| G0 Inventário | **PASS** (autoavaliação) | 1.115 de 1.115 issues coletadas, classificadas e com estado; 0 UNVERIFIED; todas com evidência ou justificativa; divergências registradas. Ressalva: os 849 resultados da onda final não têm amostra independente |
| G1 Alpha técnica | BLOCKED | Smoke em Windows nativo e macOS e CI nunca executados; o resto é PASS |
| G2 Beta funcional | FAIL (e BLOCKED em parte) | Matriz de componentes incompleta (clones, VidHD em ][, SHR em PAL) e áudio com 5 FAIL; 7 P0 BLOCKED sem aceite formal; evidência de áudio/entrada/GUI nos 3 SO BLOCKED. Zero FAIL P0 e fixtures reais críticas passam |
| G3 Stable candidata | FAIL | FAIL P1 (#241), sem pacote instalado nos 3 SO, sem parecer independente |

Todos os critérios de cada Gate, com PASS, FAIL ou BLOCKED e evidência: [release-readiness.md](release-readiness.md).

## 10. Recomendação de release

**alpha.** Publicar, se o dono decidir, apenas como **pré-lançamento `v0.2.0-alpha.1`**, com a seção "Limitações conhecidas" do CHANGELOG, e sem chamá-la de beta nem de candidata a estável. O que mudou desde o relatório anterior (o inventário passou a PASS, 0 FAIL em save-states, FAIL P1 de 3 para 1) aproxima a beta, mas não a atinge: o G2 não está atingido (matriz incompleta, 5 FAIL de áudio, 7 P0 BLOCKED sem aceite, evidência dos 3 SO ausente). Antes de usar o rótulo "alpha técnica" sem ressalva, o G1 pede o smoke em Windows e macOS nativos (o que o dono pode executar pelos roteiros, pela VM Windows ou autorizando um `workflow_dispatch`). A versão `0.2.0-alpha.1` está preparada na branch local `release/v0.2.0-alpha.1`; nenhuma tag, PR ou push foi feito.

## 11. Decisões pendentes do dono

Nove decisões que só o dono pode tomar. Nenhuma foi tomada pelo time. Cada item traz as opções e uma recomendação, para o dono aceitar ou trocar. A recomendação não vale como aprovação.

| # | Decisão | Opções | Recomendação |
|---|---|---|---|
| 1 | **Os 10 FAIL do depurador e a #458**: bugs do próprio AppleWin `a8f0d1d` (#114, #245, #458, #466, #468, #713, #1145, #1377, #1381, #1426, #1431) | (a) Manter a paridade e deixar os FAIL como "bug do AppleWin reproduzido". (b) Corrigir todos como Divergência registrada na ADR. (c) Corrigir só os que são defeito sem uso legítimo e sem mudar scripts existentes | **(c)**: corrigir #114, #245, #458, #466, #468 e #1381 (um teste por item, registro no gaps); manter #713 (muda a semântica das aspas em scripts), #1145 e #1377 (dados do AppleWin) e adiar #1426 e #1431 (exigem expor o LC por banco no núcleo) |
| 2 | **#1396**: Alt+F2 apaga o Open Apple no AppleWin; o port mantém o Open Apple | (a) Manter o port como está e registrar como divergência a favor do usuário. (b) Reproduzir o AppleWin (zerar os botões no F2). (c) Oferecer opção de linha de comando | **(a)**: a própria issue pede que Alt+F2 funcione para pular o boot do HDD-1; o teste `#[ignore]` fica como registro |
| 3 | **#916** (conversão Pravets correta no port, quebrada no AppleWin por causa do encoding das tabelas) e **#855** (`DebuggerAutoRun` funciona no port, não no AppleWin): divergências a favor do usuário já presentes | (a) Registrar as duas como "Divergência a favor do usuário", com emenda na ADR 0004. (b) Reverter para o comportamento do AppleWin. (c) Registrar e enviar correção ao AppleWin | **(a) e, se o dono quiser, (c)**: ambas implementam o que a issue pede e o defeito só existe no C++; reverter apagaria uma correção sem benefício. O mesmo vale para #280/#358/#1374 (tela cheia), #932 e #1448, listadas no gaps |
| 4 | **As 5 divergências de PCM/SSI**, que vêm do fatiamento de tempo do hospedeiro no AppleWin | (a) Aceitar como limitação conhecida (os eventos são idênticos). (b) Copiar o quantum do AppleWin (blocos e 23 ciclos por amostra) | **(a)**: o próprio Oráculo muda de fase com o fatiamento, copiar fixaria um artefato do hospedeiro e pioraria o áudio sem prova de hardware; registrar os 5 casos como `known_divergence` aceita |
| 5 | **Os 7 P0 BLOCKED** (#4, #70, #400, #422, #564, #640, #1126), que precisam de software de terceiros ou do relógio real | (a) Aceite formal do bloqueio, com os mecanismos sintéticos como evidência. (b) Obter as fixtures e o Oráculo de janela. (c) Misto | **(c)**: aceite formal de #4, #564 e #640 (frontend e tempo real, testes Rust passam); obter fixtures de #70, #400, #422 e #1126 só se houver origem legal. Sem o aceite, o G2 não fecha |
| 6 | **Autorizar uma execução manual de CI** (`ci.yml` e `conformance.yml`, nos 3 SO) para destravar o G1 | (a) Autorizar uma execução de cada, no SHA final. (b) Esperar a cota renovar. (c) Não rodar | **(a)**, uma única vez, depois que as branches forem mescladas. O `release.yml` fica de fora. Sem isso o G1 continua BLOCKED |
| 7 | **VM Windows 11 do VirtualBox** do dono para o roteiro manual de Windows nativo | (a) Usar a VM (o dono liga e dá acesso; o time executa o roteiro [plataformas.md](manual-tests/plataformas.md)). (b) Esperar a CI. (c) Não testar | **(a)**: evidência nativa sem gastar a cota; complementa o item 6, porque a CI não cobre DPI, SmartScreen nem áudio real |
| 8 | **Assinatura e notarização** dos pacotes | (a) Alpha sem assinatura e com aviso no CHANGELOG. (b) Authenticode e Apple Developer ID antes do beta. (c) Programa de assinatura gratuita para código aberto no Windows e Developer ID no macOS | **(a) agora, (b) ou (c) antes do beta**: o aviso já está no CHANGELOG; o custo e o certificado são decisão do dono |
| 9 | **Abrir o PR `[release]` da v0.2.0-alpha.1** | (a) Abrir agora. (b) Abrir depois das decisões 6 e 7 e da revisão independente. (c) Não abrir | **(b)**: o PR levaria o G1 em BLOCKED e uma nota de autoavaliação; com Windows e macOS nativos e o parecer independente a mensagem do alpha fica honesta. Tag só depois do merge |
