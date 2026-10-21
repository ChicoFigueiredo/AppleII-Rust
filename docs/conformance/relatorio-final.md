# Relatório final de conformidade

Relatório do §21 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md), em português, com os 10 itens na ordem do plano. Esta é a **reavaliação final** depois de: a #241 corrigida (zero FAIL P1 em issue), as 6 Divergências aprovadas do depurador (#114, #245, #458, #466, #468, #1381), a [ADR 0006](../adr/0006-divergencias-aprovadas-pelo-dono.md) e as decisões do dono de 21/10/2026. Todos os números vêm de execuções feitas em 21/10/2026 sobre o SHA do item 1; nada vem de memória. Onde um número vem de um relatório gerado antes (vetores de CPU e Z80, smokes de plataforma, desempenho), a data do relatório é citada. A nota do item 9 é **autoavaliação; a revisão independente ainda falta e será feita por outro agente**.

**Decisões do dono (21/10/2026, ADR 0006):** os 5 casos de PCM/SSI viram **Limitação aceita** (continuam FAIL nos relatórios); #916, #855, #1396 e a tela cheia (#280, #358, #1374, #117, #932, #1448) viram **Divergências aprovadas**; #713, #1145 e #1377 mantêm a paridade com o AppleWin; #1426 e #1431 ficam adiadas; a CI e o PR `[release]` são disparados pelo dono. A #1396 passou de FAIL a PASS nesta entrega (teste do comportamento aprovado no lugar do `#[ignore]`).

**Escopo declarado da release: Linux x86_64** (decisão do dono: release candidate para Linux, `v0.2.0-rc.1`). **Windows e macOS ficam BLOCKED/pendentes**, pelos roteiros de [manual-tests/plataformas.md](manual-tests/plataformas.md); não contam como FAIL do escopo Linux, e os pacotes dos dois, gerados pelo `release.yml` quando o dono o disparar, são marcados no texto da release como não verificados nativamente. Uma sessão do dono no Windows 11 nativo está rodando o roteiro W1 a W10 (item 6, "Windows nativo: em execução"). Os Gates são dados no projeto inteiro e no escopo Linux.

## 1. Baseline com SHAs e versões

| Repositório | Referência | SHA completo |
|---|---|---|
| AppleII-Rust (código avaliado) | `main` de 21/10/2026 (`git rev-parse main`) | `469bf941deab932ab228169a7ffebddae1fc6854` |
| AppleII-Rust (versão anterior deste relatório) | `main` da reavaliação anterior | `bb01e3415c45fcb3d7e1103e788cc6cec2279e5f` |
| AppleII-Rust (antes da onda final) | `main` | `22dbf2b7a4c57d9b6ede7be39a577e93196c1a3f` |
| AppleII-Rust (ponto de partida da auditoria) | `main` de 20/10/2026 | `6da60488a2c61b82099d6a7aabece21125eb0963` |
| AppleII-Rust (última versão publicada) | tag `v0.1.0` (workspace `0.1.0`) | `87110ff15ef35f58db882ab46c08f69296f7250d` |
| AppleWin (Oráculo de código) | commit `a8f0d1d`, 17/10/2026 | `a8f0d1d40f551ad9e010522c29c2891e33c88275` |
| AppleWin (release de referência) | tag `v1.32.1.0` (5 commits antes de `a8f0d1d`) | `daf813dcaced7469bbbc16f23905cfd75abe8496` |
| AppleWin-Test (corpus, sem licença; cache local) | HEAD de `master` em 20/10/2026 | `4b113ce8cff1426bda5595cac9cb04776adfbb5d` |
| audetto/AppleWin (Oráculo executável, Linux) | HEAD de `master` em 20/10/2026; núcleo AppleWin `3e8054b`; `audetto-version.patch` leva a versão a 1.32.1.0 | `a1e20c0362f392dc25f1a3b218d2f84d4f1a25da` |

Versões: AppleII-Rust `0.1.0` (tag) e `0.2.0-rc.1` (preparada na branch local `release/v0.2.0-rc.1`, sem tag nem publicação). AppleWin `1.32.1.0` mais 5 commits. Detalhes e a comparação de commits: [baseline.md](baseline.md).

Ambiente de execução: Ubuntu 26.04.1 LTS, kernel 7.0.0-38, x86_64, AMD Ryzen 7 3700X (16 threads, 125 GiB), `rustc 1.98.1 (48a229cea 2026-09-01)`, Wine 10.0 para o Windows via `cargo-xwin`. A máquina estava compartilhada com outros processos, o que afeta tempos, não resultados.

## 2. Tabela-resumo

Versão condensada. A tabela completa, com uma linha por componente e Modelo, está em [capability-matrix.md](capability-matrix.md).

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| CPU 6502 / 65C02 | ][, ][+, //e, //e Enhanced | PASS | 7 traces de 0,5 a 5 milhões de ciclos idênticos ao Oráculo; 10,22 milhões de vetores SingleStepTests; determinismo 10 x 2M | Diferenças AppleWin x hardware catalogadas (ciclo fantasma, RMB/SMB, WAI/STP) copiadas do AppleWin | #17 | P0 |
| Memória (LC, Saturn, RamWorks, AUX, floating bus) | ][+, //e, //e Enhanced | PASS | `audit_memoria.rs`, a2audit, estados `LC-*`, `RamWorks_r4`, `Saturn64`, `Saturn128`, `Saturn128-Bank3-2` (todos PASS no trace de 100.000 ciclos) | Nenhuma medida | #18, #19 | P0 |
| Vídeo | ][, ][+, //e, //e Enhanced | PASS (1.573 de 1.573 comparações pixel a pixel e a cena #241 no Color Monitor PAL) | 584 casos `video-*` | Nenhuma medida. Clones sem comparação de framebuffer | #20, #21, #46 | P0 |
| Disco (Disk II, WOZ, 2MG, gz/zip, HDD/SmartPort) | ][+, //e, //e Enhanced | PASS | 56 casos `disk-*`, 103.146.754 instruções comparadas; firmware do HDD 132.1 | HDC no ][+ não comparável; NIB real e INIT/FORMAT sem Oráculo | #22, #23, #24 | P0 |
| Save-state | vários | PASS (88 de 115 reais; atômico; fuzz); BLOCKED (27); **0 FAIL** | [savestate-import.md](../../tests/conformance/reports/savestate-import.md); 87 casos `savestate-*` | Nenhuma no trace. O teste de formato acusa 1 a 8 campos em quase todos os arquivos (o mais comum: bit B do P, normalizado de propósito) | #25, #26 | P0 |
| Áudio: eventos e IRQ do 6522 | ][+, //e | PASS | 34 casos `audio-*` PASS, inclusive `audio-issue-1176-last-cycle` (corrigido) | nenhuma | #27, #28 | P1 |
| Áudio: PCM do AY e do alto-falante, SSI263 +666 | ][+, //e | FAIL (5 casos), **Limitação aceita** (ADR 0006) | `audio-ay-{tone,envelope,noise}-pcm`, `audio-speaker-clicks-pcm`, `audio-ssi263-phonemes-p333` | blocos e quantum de tempo do AppleWin; 23 ciclos por amostra | #7, #27, #28 | P1 |
| Z80 SoftCard | ][+, //e | PASS (vetores de terceiros); BLOCKED (sistema/CP/M) | 1.604.000 vetores; ZEXDOC/ZEXALL 65/65 (relatório de 20/10/2026) | F3/F5 de blocos repetindo segue o hardware (candidata) | #29 | P1 |
| Cartas (mouse, 4Play, SNES MAX, relógios, SSC, Uthernet) | ][+, //e | PASS (mouse, 4Play, SNES MAX nos estados reais); BLOCKED (rede/serial reais) | `entrada-cartas.csv`, `p23-entrada.csv`, `p23-perifericos.csv`; `dazzledraw1` x3 e `SNES_MAX-slot4` PASS | #1396 (Alt+F2): Divergência aprovada, o port mantém o Open Apple | #30, #31, #32, #33 | P1 |
| Depurador | ][+, //e Enhanced | PASS 79; FAIL 5; BLOCKED 7; N/A 39 (130 issues) | `results/depurador.csv`, `results/p23-depurador.csv` | Seis Divergências aprovadas (#114, #245, #458, #466, #468, #1381). Os 5 FAIL (#713, #1145, #1377 paridade mantida; #1426, #1431 adiadas) são comportamentos do AppleWin reproduzidos | #34 | P1 |
| Interface, linha de comando, configuração | todos | PASS (testes próprios, 49 issues de UI e 73 de configuração verificadas); sem Oráculo de janela | `p23-ui.csv`, `p23-config-outras.csv`, `hu0702_input.rs`, `cli.rs` | Divergências aprovadas pelo dono (seção 7 do gaps) | #35 | P1 |
| Linux x86_64 | Linux | PASS (smoke, pacote extraído; 20/10/2026) | [relatorio-plataformas.md](platform/relatorio-plataformas.md) | Wayland nativo e dispositivos: manual | #36 | P0 |
| Windows (Wine) / Windows nativo | Windows | PASS (Wine, 20/10/2026) / BLOCKED (nativo: execução do dono no Windows 11 em andamento) | [windows-wine.md](platform/windows-wine.md) | Wine não é Windows | #36 | P0 |
| macOS | macOS | BLOCKED (sem execução) | sem hardware | tudo | #36 | P0 |
| Extensões: rewind, DAP, laboratório, WASM | Enhanced //e | IMPLEMENTED | crates próprios | sem equivalente no AppleWin | #40 a #44 | P2 a P3 |

## 3. Sumário por status das issues upstream

Coleta de 21/10/2026 (00:47 UTC) no HEAD do AppleWin `a8f0d1d`: 1.516 itens (1.115 issues e 401 PRs). Issues: **358 abertas e 757 fechadas**. Fonte: [applewin-issue-matrix.csv](applewin-issue-matrix.csv) e [issue-audit.md](issue-audit.md). `python3 tools/conformance/apply_results.py --check` confirma "Matriz em dia" (849 issues com resultado aplicado, 0 linhas por alterar). **Não resta nenhuma issue UNVERIFIED**: cada uma das 1.115 tem estado e `teste`, `evidencia` ou `justificativa_na` preenchidos (849 por resultado individual, 266 sem prioridade como NOT_APPLICABLE com justificativa da triagem).

| Estado de verificação | Abertas | Fechadas | Total |
|---|---:|---:|---:|
| PASS | 97 | 390 | **487** |
| FAIL | 5 | 1 | **6** |
| BLOCKED | 40 | 93 | **133** |
| NOT_APPLICABLE | 216 | 273 | **489** |
| UNVERIFIED | 0 | 0 | **0** |
| Total | 358 | 757 | **1.115** |

Antes (relatório sobre `bb01e34`): 479 PASS, 14 FAIL, 133 BLOCKED, 489 NOT_APPLICABLE; antes da onda final (`22dbf2b`): 205 PASS, 13 FAIL, 87 BLOCKED, 286 NOT_APPLICABLE, 524 UNVERIFIED. Taxa de PASS sobre as aplicáveis (todas menos NOT_APPLICABLE): 487 de 626 = 77,8 % (antes 76,5 % e 24,7 %). Nenhuma issue fechada está em PASS sem teste (0 PASS sem teste na matriz).

Por prioridade:

| Prioridade | Total | PASS | FAIL | BLOCKED | NOT_APPLICABLE | UNVERIFIED |
|---|---:|---:|---:|---:|---:|---:|
| P0 | 30 | 23 | 0 | 7 | 0 | 0 |
| P1 | 216 | 130 | 0 | 74 | 12 | 0 |
| P2 | 276 | 189 | 2 | 37 | 48 | 0 |
| P3 | 327 | 145 | 4 | 15 | 163 | 0 |
| sem prioridade | 266 | 0 | 0 | 0 | 266 | 0 |

Os 6 FAIL: **#713, #1145, #1377** (depurador, P3, paridade com o AppleWin mantida por decisão do dono), **#1426 e #1431** (depurador, P2, adiadas) e **#781** (P3, cursor de espera). Saíram dos FAIL desde o relatório anterior: **#241** (P1, histórico de sinal do primeiro quadro, `de3ac3b`), as seis Divergências aprovadas do depurador (**#114, #245, #458, #466, #468, #1381**) e **#1396** (Divergência aprovada, teste do comportamento aprovado). **Não há FAIL P0 nem P1 em issue.** Os 7 P0 BLOCKED não mudaram: **#4, #70, #400, #422, #564, #640, #1126**; todos fechados no AppleWin e todos dependentes de software de terceiros sem fixture ou de relógio e janela reais. BLOCKED por categoria (133): disco 54, vídeo 16, Mockingboard 12, CPU 9, áudio 9, memória 6, depurador 5, serial 4, teclado 3, HDD 3, e o resto em parcelas de 1 a 2 (outra, relógio, save-state, rede, SSI263, Z80, joystick). A maioria é software de terceiros sem fixture licenciada; o restante é comportamento de frontend sem Oráculo headless ou hardware.

Auditoria independente da triagem: [issue-audit-review.md](issue-audit-review.md) (amostra de 465 issues, concordância de 91 % em qualquer campo na parte aleatória; 168 correções; 334 linhas reexaminadas por classe). **Os 849 resultados da onda final são autoavaliação de quem os produziu e não têm amostra independente.**

## 4. Top 10 problemas críticos

### 4.1 Os 8 abertos, em ordem de gravidade

Cada um com passos para reproduzir, causa raiz e estado. SHA observado: `469bf94`.

| # | Problema | Passos para reproduzir | Causa raiz | Estado |
|---|---|---|---|---|
| 1 | **Windows nativo e macOS nunca foram executados (fora do escopo Linux; Windows 11 nativo em execução pelo dono); a CI nunca foi executada (P0, G1)** | Não há o que reproduzir: `tools/package/smoke-wine.sh` roda sob Wine, que não é Windows; `ci.yml`, `conformance.yml` e `release.yml` jamais rodaram nesta fase. Roteiro: [manual-tests/plataformas.md](manual-tests/plataformas.md) | Sem hardware nem licença de SDK da Apple; cota de CI esgotada | **Aberto, BLOCKED**. Decisões 6, 7 e 8; a CI é disparada pelo dono. Não conta como FAIL no escopo Linux |
| 2 | **7 P0 BLOCKED (#4, #70, #400, #422, #564, #640, #1126)** | `grep ',BLOCKED,' docs/conformance/results/*.csv` mostra a causa de cada um. Os mecanismos sintéticos de #400, #422, #564 e #640 passam em testes Rust | Precisam de software de terceiros sem fixture (UCSD Pascal, entre outros), do relógio real ou de um Oráculo de janela e de troca de disco ao vivo | **Aberto**. Decisão 5 |
| 3 | **PCM do AY e do alto-falante, SSI263 +666 (P1 no harness, 5 casos FAIL): Limitação aceita** | `conformance run --case audio-ay-tone-pcm` (e `-envelope-pcm`, `-noise-pcm`, `audio-speaker-clicks-pcm`, `audio-ssi263-phonemes-p333`) | O AppleWin gera o PCM em blocos que dependem do fatiamento de tempo do hospedeiro, com 23 ciclos por amostra no alto-falante e `PeriodicUpdate` de ~1000 ciclos; o Rust gera cada amostra no ciclo exato. O próprio Oráculo muda de fase com o fatiamento | **Aceita pelo dono (ADR 0006, 21/10/2026)**; continua FAIL no harness e nos relatórios. Núcleo do AY e eventos idênticos |
| 4 | **5 FAIL do depurador: #713, #1145, #1377 (paridade mantida) e #1426, #1431 (adiadas)** | `cargo test -p apple2-debugger --release -- --ignored` (os testes `#[ignore = "FAIL ..."]` reproduzem o comportamento que a issue pede mudar) | São comportamentos do próprio AppleWin `a8f0d1d` (aspas simples, ordem e tabelas de símbolos, mini dump `M` do LC por banco/slot) que o port reproduz | **Decidido em 21/10/2026**: paridade mantida (#713, #1145, #1377) ou adiado (#1426, #1431, exigem expor o LC por banco no núcleo) |
| 5 | **27 save-states reais BLOCKED** | `savestate-compare.py` sobre o corpus: 18 por imagem ausente (`DiskFormat/*`, `AZTEC`), 3 SSC por TCP/serial, 2 por hora real, 2 `skm-*`, 1 Z80, 1 Uthernet | Falta imagem no corpus, ou o Oráculo não tem o recurso (Shift Key Mod, SSC, hora real) | **Aberto**. Sem decisão pendente; precisa de fixtures |
| 6 | **Risco de dados: HDD acima de 2 MiB gravado no lugar; imagens abertas por save-state de terceiros (AUD-C-037); duas instâncias na mesma imagem** | Interromper a escrita num HDD de 32 MB: o bloco pode ficar pela metade (não há renomeação atômica); abrir um `.aws.yaml` de terceiros que cita uma imagem local | Escolha de projeto (atualização bloco a bloco com `sync_data`); igual ao AppleWin; sem trava de arquivo | **Aberto** (risco médio; UNVERIFIED para as duas instâncias). Veja o item 8 |
| 7 | **Pacotes sem assinatura nem notarização; `release.yml` nunca executou** | Instalar o pacote no Windows (SmartScreen) ou no macOS (Gatekeeper); [assinatura.md](platform/assinatura.md) | Sem certificado Authenticode nem Developer ID | **Aberto**. Decisão 8 |
| 8 | **Parecer independente do conjunto e dos 849 resultados da onda final** | Não há o que reproduzir | Os resultados foram produzidos e aplicados por quem implementou | **Aberto**: outro agente fará a revisão independente depois desta entrega |

### 4.2 Os corrigidos (achados da auditoria e fechados nas rodadas até esta)

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
| l | **Fechado nesta entrega, P1: #241, 1 pixel de borda no Color Monitor PAL** | O histórico de sinal do primeiro quadro (`0xFFF`) não era o do AppleWin | `de3ac3b`; cena `video-issue-0241-*` PASS em NTSC, Color TV PAL e Color Monitor PAL |
| m | **Fechado nesta entrega, P2/P3: seis comandos do depurador (#114, #245, #458, #466, #468, #1381)** | Reproduziam defeitos do AppleWin | `6409ce9`; testes em `matriz_depurador_b.rs` e `p23_depurador.rs`; Divergências aprovadas (ADR 0006) |
| k | **Fechado nesta rodada, P2: GH#1340, disco rápido acelerava com o som ativo** | `Emulator::effective_full_speed` ignorava `Spkr_IsActive` e o Mockingboard | `fe50e4b`; `gh1340_full_speed_sound.rs` (8 testes) |

Dos 9 arquivos de save-state em FAIL no relatório anterior, **4 eram defeito do port** (VBL do mouse, 3 arquivos, e firmware do SmartPort, 1 arquivo; 2 causas) e **5 eram defeito do Oráculo** (heap sem zerar nos bancos Saturn, 2 arquivos; `SNESMAX.cpp` sem tratador de E/S, 1; ROM do SSC ausente, 2). As três correções do Oráculo (`MALLOC_PERTURB_`, `audetto-snesmax.patch`, `audetto-ssc-rom.patch`) corrigem o instrumento de medida e estão versionadas em `tools/conformance/oracle/`.

Risco que não é um defeito achado, mas pesa na recomendação: **Windows e macOS nativos nunca foram executados** (item 1 acima; o Windows 11 está em execução pelo dono).

## 5. Backlog importável

- [backlog.md](backlog.md) e [backlog.json](backlog.json): 13 épicos (EP-00 a EP-12) e 30 histórias (HU-0001 a HU-1205), com prioridade, onda, Gate e número da issue no GitHub, gerados de `docs/prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md` por `tools/conformance/backlog.py` (sincronização idempotente com `gh`).
- Issues no repositório ChicoFigueiredo/AppleII-Rust: **#2 a #14** (épicos), **#15 a #44** (histórias HU) e **#45 a #80** (36 histórias do EP-11, derivadas das 358 issues abertas do AppleWin: [upstream-open-selected.md](upstream-open-selected.md), triagem completa em [upstream-open-issues-triage.md](upstream-open-issues-triage.md)). Cada história tem Critérios de aceite, dependências e rótulos.
- **Estado das histórias HU (#15 a #44), na rodada anterior**: a label `estado:UNVERIFIED` foi trocada pela do estado real por `tools/conformance/update_hu_issues.py` (idempotente, com `--dry-run`): 14 PASS, 5 FAIL, 11 BLOCKED. `estado:PASS` só vale com todos os CA em PASS. Nenhuma issue foi fechada: o fechamento é decisão do dono, depois da revisão independente. O campo "Estado atual" do corpo das issues continua o do `backlog.py`.
- Fila da onda 1 (246 issues P0/P1 de runtime, formato e depurador): [wave1-candidates.md](wave1-candidates.md); a maior parte está hoje em PASS.
- Pendências a abrir como histórias, se o dono quiser: fixtures dos 7 P0 BLOCKED; execução em Windows e macOS nativos; #1426 e #1431 (LC por banco no núcleo); as decisões 5 a 9 da seção 11. As labels `estado:` das histórias HU no GitHub não foram atualizadas nesta entrega: estado esperado 16 PASS, 3 FAIL, 11 BLOCKED (HU-0201 e HU-0702 passam a PASS).

## 6. Testes executados e não executados

### Executados (Linux x86_64 nativo, SHA do item 1 mais a conversão da #1396, em 21/10/2026)

| Execução | Comando | Resultado |
|---|---|---|
| Testes do workspace | `cargo test --workspace --release --no-fail-fast` | **1.584 passam, 0 falham, 15 ignorados** (185 suítes: 168 executáveis de teste e 17 doctests; exit 0). Os 15 ignorados: vetores pesados que exigem download de 2,5 GB (65x02) e 1,3 GB (Z80 e ZEX), Klaus estendido, 4 testes do depurador que reproduzem FAIL (#713, #1145, #1377 e #1426/#1431; comportamento do AppleWin), 3 medidas de tempo (`emulated_speed`, `disk_boot_speed`, `frames_per_second`), a carga do corpus AppleWin-Test, um gerador manual, um reprodutor de SNES MAX e um doctest. Eram 22 na rodada anterior: saíram os 6 do depurador corrigidos e o `#[ignore]` da #1396 (agora um teste que passa) |
| Casos de conformidade | `cargo build --release -p apple2-conformance && ./target/release/conformance run --all --report target/conformance-report` | **779 casos: 774 PASS, 5 FAIL, 0 BLOCKED** (com o corpus AppleWin-Test no cache; sem ele, parte dos casos fica BLOCKED). Por área: vídeo 584, save-state 87, disco 56, áudio 39 (34 PASS e 5 FAIL), CPU/boot/determinismo/framebuffer 13. Os 5 FAIL são as **Limitações aceitas** (ADR 0006): `audio-ay-tone-pcm`, `audio-ay-envelope-pcm`, `audio-ay-noise-pcm`, `audio-speaker-clicks-pcm` e `audio-ssi263-phonemes-p333`. O comando sai com código 3 enquanto houver FAIL |
| Matriz de issues | `python3 tools/conformance/apply_results.py --check` e `summarize_issue_matrix.py` | "Matriz em dia" depois de aplicar a conversão da #1396 (`apply_results.py` mudou 1 linha; antes, `--check` acusava a matriz desatualizada, como esperado): PASS 487, FAIL 6, BLOCKED 133, NOT_APPLICABLE 489, UNVERIFIED 0 (849 resultados aplicados). Por prioridade e por estado no item 3 |
| Importação de save-states reais | `tools/conformance/savestate-compare.py` sobre os 3 estados do Wine e os 112 do corpus (100.000 ciclos cada), agregado como em `savestate-report.py` | **88 PASS, 0 FAIL, 27 BLOCKED** (115 arquivos), igual ao [relatório versionado](../../tests/conformance/reports/savestate-import.md). Na leitura crua do `savestate-compare.py`: 88 PASS, 19 BLOCKED e 8 FAIL do trace (`NoSlotClockTest*`, `skm-*`, `SSC-*`, `CPM2.2(56k)-3`); o `savestate-report.py` reclassifica esses 8 como BLOCKED por causa conhecida (hora real, Shift Key Mod ausente no Oráculo, TCP/serial, Z80). O relatório e os casos TOML regerados foram descartados (caminhos absolutos estragam os casos) e o relatório versionado foi mantido |
| Vetores 65x02 | `cargo test -p apple2-cpu --release --test singlestep_65x02 -- --ignored` | 10.220.000 de vetores, 0 inesperados (relatório versionado de 20/10/2026, [65x02-singlestep.md](../../tests/conformance/reports/65x02-singlestep.md)); **não refeito** nesta rodada (2,5 GB de download) |
| Vetores Z80 e ZEXALL | `cargo test -p apple2-cards --release --test z80_singlestep -- --ignored` | 1.604.000 de 1.604.000; ZEXDOC e ZEXALL 65/65 (relatórios de 20/10/2026, [z80-singlestep.md](../../tests/conformance/reports/z80-singlestep.md), [z80-zex.md](../../tests/conformance/reports/z80-zex.md)); **não refeito** |
| Corpus AppleWin-Test (carga de 1 quadro) | `applewin_test_corpus.rs` | 94 OK, 18 OK com avisos, de 112 (relatório de 20/10/2026, [applewin-test-load.md](../../tests/conformance/reports/applewin-test-load.md)); carregar não é PASS de fidelidade |
| Windows via Wine 10.0 | `tools/package/smoke-wine.sh` | PASS (relatório de 20/10/2026, [windows-wine.md](platform/windows-wine.md)); **não refeito** |
| Linux: pacote e smoke | `tools/package/smoke-linux.sh`, `test-package.sh` | PASS, 17 verificações (relatório de 20/10/2026, [relatorio-plataformas.md](platform/relatorio-plataformas.md)); **não refeito** |
| Pacote Linux do release candidate | `tools/package/linux.sh` (build release, smoke headless, licenças, `package.sh linux`, SBOM, SHA256SUMS) e `tools/package/test-package.sh` | **PASS**, 17 verificações (pacote extraído fora da árvore e rodado de dentro; 3 discos montam e bootam; manual embutido), gerado e testado de novo em 21/10/2026 sobre o commit da branch `release/v0.2.0-rc.1` (versão `0.2.0-rc.1`). Artefatos em `target/release-candidate/` da worktree de trabalho (fora do repositório); SBOM CycloneDX 1.5 com 452 componentes; 492 licenças de terceiros dentro do pacote. SHA-256: `applewin-0.2.0-rc.1-linux-x64.tar.gz` = `6b3b8ce5b298091e2cdb33aeb08a243088af8f23fdc4e30259d0e72bcb2f72a3`; `applewin-0.2.0-rc.1.cdx.json` = `41f1b7fc2b1631f7d366aaf5626a414395778505a6267f93bf6410a97fc367c6` (conferidos por `sha256sum --check` do `SHA256SUMS`). O SBOM é reprodutível com `SOURCE_DATE_EPOCH=1792589714` (o mesmo hash em duas gerações); o tarball não é bit a bit reprodutível (a primeira geração deu `ef4505d6...`, a segunda, a registrada aqui, sobre o mesmo conteúdo), então o hash vale para o arquivo em `target/release-candidate/`, que é o que foi testado. O hash do tarball muda se o pacote for refeito (o manual e o código vão dentro do binário); os hashes da rodada anterior (`3e93238a...` e `4e95cfc1...`) valiam para o pacote antigo |
| Desempenho | `apple2-bench` | [performance.md](../../tests/conformance/reports/performance.md), em máquina compartilhada; não refeito |

`cargo clippy --workspace --all-targets -- -D warnings`: sem avisos (exit 0). `cargo fmt --all -- --check`: sem diferenças (exit 0). Ambos em 21/10/2026 sobre o SHA do item 1 mais a conversão da #1396 e os documentos desta entrega, e de novo na branch `release/v0.2.0-rc.1` (versão `0.2.0-rc.1`). A suíte de testes completa foi rodada sobre o SHA do item 1 mais a conversão da #1396, não na branch de release (que só acrescenta versão, CHANGELOG e empacotamento).

Relatórios por sistema operacional: Linux nativo, [relatorio-plataformas.md](platform/relatorio-plataformas.md); Windows (Wine), [windows-wine.md](platform/windows-wine.md); Windows nativo e macOS, roteiros em [manual-tests/plataformas.md](manual-tests/plataformas.md) (não executados). Os smokes de Linux e Wine são de 20/10/2026 (commit base `516c123`); os do SHA atual não foram refeitos, só as suítes de testes e de conformidade.

### Windows nativo: em execução

Uma sessão do dono num **Windows 11 nativo** está rodando o roteiro W1 a W10 de [manual-tests/plataformas.md](manual-tests/plataformas.md) e vai publicar o resultado na branch `test/windows-nativo`. **A ser preenchido** depois da execução: build e testes no Windows, smoke headless e com janela, DPI por monitor, manifesto, console, WASAPI, XInput, SmartScreen e antivírus, com o commit testado e as falhas encontradas. Até lá o Windows nativo continua BLOCKED e fora do escopo Linux; nenhum resultado dessa execução foi usado neste relatório.

### Não executados

- **Windows nativo (execução do dono em andamento) e macOS (Intel e Apple Silicon)**: build, smoke, GUI, áudio, DPI, gamepad, permissões, Gatekeeper, SmartScreen. BLOCKED.
- **CI do GitHub e do GitLab**: nenhuma execução foi acionada (cota esgotada; só o dono dispara). `release.yml` nunca rodou.
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
| **Vídeo** | **1.573 telas** comparadas com o framebuffer inteiro (600x420 ou 680x436 com VidHD), diferença máxima de canal 0, Oráculo calibrado contra ele mesmo: 1.573 PASS, 0 FAIL. Hoje são 584 casos `video-*` no harness (a onda final acrescentou estilos de vídeo, o Mac LC e as cenas de issues). A cena #241 em Color Monitor PAL, que diferia em 1 pixel, passa desde `de3ac3b` | [video-differential.md](audit/video-differential.md) |
| **Disco** | 56 casos `disk-*`; **103.146.754 instruções** comparadas e SHA-256 das imagens ao final; 9 WOZ reais, DSK, `.do.gz`, `.zip`, 2MG, HDD de 32 MB. Com o firmware do DIB em 132.1 as referências `disk-*hdd*` saíram idênticas. O defeito do Rust achado no caminho foi o `write_block` de 2 TB | [disk-differential.md](audit/disk-differential.md) |
| **Save-states reais** | 115 arquivos (112 do AppleWin-Test e 3 do AppleWin 1.32.1.0 sob Wine), 100.000 ciclos por arquivo: **88 PASS, 0 FAIL, 27 BLOCKED**. O trace compara linha a linha e o runner passa a comparar também `trace_rows_sha256`. O estado salvo pelo Rust e carregado no Oráculo também bate (`oracle_roundtrip.rs`, 30.000 ciclos) | [savestate-import.md](../../tests/conformance/reports/savestate-import.md); esquema em [savestate-schema.md](savestate-schema.md) |
| **Áudio (eventos)** | Escritas 6522/AY, timers, IRQ (inclusive o último ciclo do opcode, #1176), cliques do alto-falante, soquetes e `6522a-bad`: eventos idênticos incluindo o ciclo; SSI263 e SC-01: duração de fonema dentro da tolerância medida no Oráculo; núcleo do AY contra `AY8910.cpp`: ~330 mil amostras idênticas | [audio-differential.md](audit/audio-differential.md) |
| **Áudio (PCM)** | **5 FAIL, Limitação aceita (ADR 0006)**: PCM do AY (tom, envelope, ruído; D-2), PCM do alto-falante (D-3) e SSI263 fase +666 (D-4) | idem |
| **Z80** | 1.604.000 de 1.604.000 vetores SingleStepTests/z80; ZEXDOC e ZEXALL 65/65. É comparação com vetores de terceiros, não com o Z80VICE do AppleWin | [z80-singlestep.md](../../tests/conformance/reports/z80-singlestep.md), [z80-zex.md](../../tests/conformance/reports/z80-zex.md) |
| **Depurador** | 130 issues: 79 PASS, 5 FAIL (paridade mantida ou adiadas), 7 BLOCKED, 39 N/A; seis Divergências aprovadas corrigidas (ADR 0006); 251 comandos (249 IMPLEMENTED, 2 N/A); fuzz da linha de comando | [results/depurador.csv](results/depurador.csv), [results/p23-depurador.csv](results/p23-depurador.csv), [debugger-commands.md](audit/debugger-commands.md) |

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

**Autoavaliação; a revisão independente ainda falta (outro agente a fará).** Cálculo e justificativa por dimensão em [release-readiness.md](release-readiness.md).

| Dimensão | Pontos | Nota (antes) | Nota |
|---|---:|---:|---:|
| Especificação e critérios de aceite | 25 | 22 | 23 |
| Corretude e evidência | 25 | 20 | 21 |
| Testes e regressão | 15 | 12 | 12 |
| Arquitetura e manutenção | 10 | 8 | 8 |
| Segurança, dados e resiliência | 10 | 8 | 8 |
| Higiene da entrega | 5 | 4 | 4 |
| Eficiência/retrabalho | 10 | 7 | 8 |
| **Total** | **100** | **81** | **84** |

A nota está abaixo de 96: o trabalho **não** está aceito como completo (veja a leitura da nota em [release-readiness.md](release-readiness.md)). A subida de 3 pontos vem das decisões do dono tomadas, da #241 corrigida e das seis correções do depurador; Windows, macOS, CI, hardware e os 7 P0 seguem sem execução e não contribuíram.

Gates do **projeto inteiro** (Linux, Windows e macOS; Windows e macOS contam como pendentes):

| Gate | Estado | Motivo |
|---|---|---|
| G0 Inventário | **PASS** (autoavaliação) | 1.115 de 1.115 issues coletadas, classificadas e com estado; 0 UNVERIFIED; todas com evidência ou justificativa; divergências registradas. Ressalva: os 849 resultados da onda final não têm amostra independente |
| G1 Alpha técnica | BLOCKED | Smoke em Windows nativo e macOS e CI nunca executados; o resto é PASS |
| G2 Beta funcional | FAIL (e BLOCKED em parte) | Matriz de componentes incompleta (clones, VidHD em ][, SHR em PAL); 7 P0 BLOCKED sem aceite formal; evidência de áudio/entrada/GUI nos 3 SO BLOCKED (e 5 Limitações aceitas de PCM/SSI, que não contam como PASS). Zero FAIL P0 e fixtures reais críticas passam |
| G3 Stable candidata | BLOCKED | Nenhum critério em FAIL (zero FAIL P0/P1 em issue; os 5 PCM/SSI são Limitação aceita). Faltam o pacote testado em Windows e macOS e o parecer independente; o G2 não foi atingido |

Gates no **escopo declarado, Linux x86_64** (Windows e macOS fora, BLOCKED, sem contar como FAIL). A #241 já passa, então não há mais variantes. Critério a critério em [release-readiness.md](release-readiness.md):

| Gate | Estado | O que falta |
|---|---|---|
| G0 | **PASS** | nada |
| G1 | **BLOCKED** | só a CI do GitHub no Linux (nunca acionada; o dono a dispara). Smoke, `fmt`, `clippy`, testes e pacote estão em PASS |
| G2 | **FAIL** | matriz incompleta (clones, VidHD em ][/][+, SHR em PAL), 7 P0 sem aceite, hardware (gamepad, teclado regional, Wayland, audição) |
| G3 | **BLOCKED** | só o parecer independente. Zero FAIL P0/P1 passa, com 5 Limitações aceitas (FAIL aceito pelo dono, não PASS). Depende também do G2 |

Não há FAIL P0 nem P1 em issue no escopo Linux. Os únicos FAIL P0/P1 no harness são as 5 Limitações aceitas de PCM/SSI.

Todos os critérios de cada Gate, com PASS, FAIL ou BLOCKED e evidência: [release-readiness.md](release-readiness.md).

## 10. Recomendação de release

**Objetiva pelo §18: alpha técnica.** O dono declarou a entrega como **release candidate para Linux x86_64** (`v0.2.0-rc.1`); isso é um rótulo dele, e o time o publica **só como pré-lançamento** (`prerelease: true`), sem chamá-lo de beta nem de stable candidate, com "Limitações conhecidas", "Limitações aceitas" e "Divergências aprovadas" no CHANGELOG e com os pacotes de Windows e macOS marcados como **não verificados nativamente**.

O rótulo é mais sustentável que na rodada anterior, mas ainda não é sustentado pelos Gates: não há FAIL P0 nem P1 em issue, o G1 do Linux espera só a CI e o G3 do Linux espera só o parecer independente; o G2 do Linux segue em FAIL. **Critérios que faltam**:

- G1 (Linux): a CI do GitHub no Linux, disparada pelo dono.
- G2 (Linux): matriz de componentes completa (clones, VidHD em ][/][+, SHR em PAL), aceite dos 7 P0 (decisão 5) e os roteiros de hardware.
- G3 (Linux): parecer independente (outro agente o fará depois desta entrega) aprovado pelo dono.
- Windows nativo (execução do dono em andamento) e macOS: pendentes; o texto da release os marca como não verificados.

O que mudou desde a rodada anterior: a #241 e seis comandos do depurador corrigidos, nove decisões do dono registradas (ADR 0006), nota de 81 para 84. Isso consolida o alpha e aproxima o Linux de beta, mas não o atinge. A versão `0.2.0-rc.1` está preparada na branch local `release/v0.2.0-rc.1`; nenhuma tag, PR, push ou CI foi feito ou disparado. O PR `[release]` e a CI são do dono.

## 11. Decisões do dono

Em 21/10/2026 o dono decidiu as de número 1 a 4 (registradas na [ADR 0006](../adr/0006-divergencias-aprovadas-pelo-dono.md)). Das demais, a CI e o PR `[release]` ficam com ele. A recomendação não vale como aprovação.

| # | Decisão | Resultado | Estado |
|---|---|---|---|
| 1 | **Os 10 FAIL do depurador e a #458** (#114, #245, #458, #466, #468, #713, #1145, #1377, #1381, #1426, #1431) | Corrigir #114, #245, #458, #466, #468 e #1381 como Divergências aprovadas; manter a paridade em #713 (muda a semântica das aspas em scripts), #1145 e #1377 (dados do AppleWin); adiar #1426 e #1431 (exigem expor o LC por banco no núcleo) | **Decidida** |
| 2 | **#1396**: Alt+F2 | Divergência aprovada: o port mantém o Open Apple (o AppleWin o apaga em `JoyReset`) | **Decidida** |
| 3 | **#916** (Pravets em CP-1251), **#855** (`DebuggerAutoRun` com a máquina pronta) e a **tela cheia** (#280, #358, #1374, #117, #932, #1448) | Divergências aprovadas | **Decidida** |
| 4 | **As 5 divergências de PCM/SSI** | Limitação aceita: continuam FAIL nos relatórios e no harness (`known_divergence`), nunca contadas como PASS | **Decidida** |
| 5 | **Os 7 P0 BLOCKED** (#4, #70, #400, #422, #564, #640, #1126) | Opções: (a) aceite formal com os mecanismos sintéticos como evidência; (b) obter fixtures e Oráculo de janela; (c) misto. Recomendação: **(c)**, aceitar #4, #564 e #640 (frontend e tempo real, testes Rust passam) e obter fixtures de #70, #400, #422 e #1126 só se houver origem legal. Sem o aceite, o G2 não fecha | **Pendente** |
| 6 | **Executar a CI** (`ci.yml` e `conformance.yml`, nos 3 SO) para destravar o G1 | O dono dispara (recomendação anterior: uma execução de cada, no SHA final, sem o `release.yml`) | **Pendente (dono)** |
| 7 | **Roteiros de Windows e macOS** | Windows 11 nativo em execução (W1 a W10, branch `test/windows-nativo`); macOS sem execução | **Em andamento** |
| 8 | **Assinatura e notarização** dos pacotes | Recomendação: pré-lançamento sem assinatura e com aviso no CHANGELOG; Authenticode e Developer ID (ou programa gratuito de código aberto no Windows) antes do beta | **Pendente** |
| 9 | **Abrir o PR `[release]` da v0.2.0-rc.1** | O dono dispara. Recomendação: depois da CI no Linux e da revisão independente; Windows e macOS entram marcados como não verificados; tag só depois do merge | **Pendente (dono)** |
