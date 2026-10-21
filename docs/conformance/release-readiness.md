# Prontidão para release

Este documento avalia os Gates de prontidão (G0 a G3) e a nota pela rubrica Verônica, com os critérios do §18 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). Um Gate só é atingido quando **todos** os seus critérios objetivos estão em PASS. Um Gate com algum critério BLOCKED e nenhum FAIL é BLOCKED; qualquer FAIL o torna FAIL. Nota alta não substitui um Gate.

**A nota e as avaliações deste documento são autoavaliação; a revisão independente vem depois.** Quem implementou o trabalho avaliado não pode aprová-lo (regra do §18).

## Estado atual

Data: 2026-10-21. SHA avaliado: `bb01e3415c45fcb3d7e1103e788cc6cec2279e5f` (`main`), mais os documentos desta entrega (reavaliação depois da onda final do G0; a versão anterior avaliou `22dbf2b`). Ambiente: Linux x86_64 (Ubuntu 26.04.1, `rustc 1.98.1`). Números e relatórios: [relatorio-final.md](relatorio-final.md).

| Gate | Nome | Estado | Antes |
|---|---|---|---|
| G0 | Inventário | **PASS** (autoavaliação) | FAIL |
| G1 | Alpha técnica | **BLOCKED** (Windows nativo, macOS e CI) | BLOCKED |
| G2 | Beta funcional | **FAIL** (e parte BLOCKED) | FAIL |
| G3 | Stable candidata | **FAIL** | FAIL |

Nota (Verônica, 0 a 100): **81**, autoavaliação (antes: 76). **Recomendação objetiva pelo §18: alpha técnica**; o dono declarou a entrega como **release candidate para Linux** (`0.2.0-rc.1`, pré-lançamento). A seção seguinte avalia os Gates nesse escopo e diz o que falta para sustentar o rótulo. A tabela acima é a do projeto inteiro, com Windows e macOS contados como pendentes.

## Escopo declarado da release: Linux x86_64

Decisão do dono: a entrega é um **release candidate para Linux x86_64** (`v0.2.0-rc.1`, branch local `release/v0.2.0-rc.1`, pré-lançamento). **Windows e macOS saem como BLOCKED/pendentes, com o motivo "teste manual do dono em VM"** (VirtualBox, a partir do clone do repositório, pelo roteiro [manual-tests/plataformas.md](manual-tests/plataformas.md)). Eles **não contam como FAIL do escopo Linux**: nada neles foi medido, nem para bem nem para mal. Os pacotes de Windows e macOS continuam sendo gerados pelo `release.yml` quando o dono o disparar, e o texto da release os marca como **não verificados nativamente**.

Estado dos Gates no escopo Linux (critério a critério abaixo). Cada Gate tem duas colunas, conforme a #241 (o único FAIL P1 da matriz, 1 pixel na borda do Color Monitor PAL; outro agente tenta corrigi-la) seja ou não corrigida:

| Gate | Sem a #241 corrigida | Com a #241 corrigida |
|---|---|---|
| G0 Inventário | **PASS** | **PASS** |
| G1 Alpha técnica (Linux) | **BLOCKED**: falta só a CI do GitHub no Linux (nunca acionada; decisão 6) | **BLOCKED**: idem |
| G2 Beta funcional (Linux) | **FAIL**: matriz incompleta, 5 FAIL de PCM/SSI, 7 P0 sem aceite, hardware sem execução | **FAIL**: idem (a #241 não é critério do G2 além de "zero FAIL P0") |
| G3 Stable candidata (Linux) | **FAIL**: FAIL P1 (#241 e 5 casos de PCM/SSI), parecer independente ausente | **FAIL**: FAIL P1 (5 casos de PCM/SSI, até a decisão 4) e parecer independente ausente. Se o dono aceitar a decisão 4 e o parecer sair, o que sobra é a CI e o aceite dos 7 P0 |

### G1 no escopo Linux

| Critério | Resultado | Evidência |
|---|---|---|
| Smoke tests e CI verdes (Linux) | **BLOCKED** (CI); smoke **PASS** | `smoke-linux.sh` e `test-package.sh` PASS (17 verificações em 20/10/2026; o pacote `0.2.0-rc.1` foi gerado e testado de novo em 21/10/2026, veja o relatório final, item 6). A CI do GitHub nunca foi acionada: cota esgotada, só o dono autoriza (decisão 6). `tools/package/linux.sh` reproduz localmente os passos do `release.yml` |
| Boot básico dos Modelos, por Modelo | **PASS** | igual ao G1 do projeto inteiro |
| CPU, memória, vídeo, disco e save-state com regressões P0 em PASS | **PASS** | 23 de 30 P0 PASS, 0 FAIL, 7 BLOCKED |
| `cargo fmt`, `clippy -D warnings`, `cargo test --workspace --release` verdes | **PASS** | 1.577 passam, 0 falham, 22 ignorados |
| Falhas conhecidas destacadas nas notas da release | **PASS** | CHANGELOG da `release/v0.2.0-rc.1`, "Limitações conhecidas" |

### G2 no escopo Linux

| Critério | Resultado | Evidência |
|---|---|---|
| Zero FAIL P0 aplicável | **PASS** | 0 de 30 |
| Zero UNVERIFIED P0 sem bloqueio formal aceito | **BLOCKED** | 7 P0 BLOCKED (#4, #70, #400, #422, #564, #640, #1126) sem aceite do dono (decisão 5). São do núcleo ou do frontend e valem para o Linux |
| Fixtures reais críticas aprovadas | **PASS** | 88 PASS, 0 FAIL; 27 BLOCKED com causa |
| Matriz dos componentes essenciais por Modelo | **FAIL** | clones (Pravets, TK3000, Base64A, J-Plus) sem comparação de vídeo; VidHD em ][/][+ e SHR em PAL sem comparação. Não depende do sistema operacional |
| Áudio, entrada e interface com evidência (Linux) | **FAIL** (e BLOCKED) | Áudio: 5 casos de PCM/SSI FAIL; audição humana BLOCKED. Entrada: testes Rust e Xvfb PASS; gamepad, teclado regional e Wayland nativo BLOCKED (roteiros) |
| Recuperação de estado e de discos | **PASS** | como no G2 do projeto inteiro |

### G3 no escopo Linux

| Critério | Sem a #241 corrigida | Com a #241 corrigida |
|---|---|---|
| Zero FAIL P0 e P1 no escopo declarado | **FAIL**: #241 (issue P1) e 5 casos de PCM/SSI (P1 no harness: `audio-ay-{tone,envelope,noise}-pcm`, `audio-speaker-clicks-pcm`, `audio-ssi263-phonemes-p333`). Nenhum outro FAIL P0/P1 | **FAIL** pelos 5 casos de PCM/SSI, a menos que o dono decida (decisão 4) declarar o PCM como limitação conhecida fora do escopo, caso em que passa |
| Pacote instalado testado | **PASS** (Linux) | **PASS** (Linux) |
| Nenhuma perda de dados nos cenários cobertos | **PASS** | **PASS** |
| Parecer independente aprovado pelo dono | **BLOCKED** | **BLOCKED** |
| P2 e P3 publicados e escopo explícito | **PASS** | **PASS** |

"Pacote instalado testado" (Linux): o pacote foi extraído para um diretório fora da árvore e rodado de dentro (`test-package.sh`), em 20/10/2026 e de novo em 21/10/2026. `install.sh` em outras distribuições e a instalação do `.desktop` e do MIME num ambiente gráfico limpo não foram testados.

**Os FAIL P0/P1 do escopo Linux, por inteiro:** a #241 (P1) e os 5 casos de PCM/SSI. Não há outro FAIL P0 nem P1.

**Recomendação por variante** (a recomendação completa está em "Release final"):

- *Sem a #241 corrigida*: o rótulo de release candidate é do dono, mas pelo §18 o estado objetivo é **alpha técnica** (G2 e G3 em FAIL). Critérios que faltam para sustentar "release candidate": zero FAIL P1 (#241 e PCM/SSI), parecer independente, CI, aceite dos 7 P0 e matriz completa.
- *Com a #241 corrigida*: continua abaixo de beta (G2 FAIL). Critérios que faltam: decisão 4 (PCM/SSI), parecer independente, CI no Linux, aceite dos 7 P0 e matriz completa. Quando a #241 entrar, este documento precisa dos números novos (a matriz passa a 480 PASS e 13 FAIL se for só ela).

## Regras de evidência

- A evidência vem do Linux local e do Wine. Windows nativo e macOS ficam BLOCKED ("teste manual do dono em VM") até o dono executar os roteiros ou autorizar uma execução de CI (a CI é só manual, porque a cota está esgotada).
- Testes que exigem hardware físico (gamepad, serial, pcap, escuta de áudio) têm roteiros manuais e ficam BLOCKED até o dono executá-los e registrar o resultado.
- O G3 depende de um parecer de agente revisor independente, aprovado pelo dono.
- A recomendação de release (alpha, beta ou stable candidate) não é inflada: segue o maior Gate atingido, com as ressalvas escritas.

## G0: Inventário

| Critério | Resultado | Evidência |
|---|---|---|
| 100% das issues do AppleWin (abertas e fechadas) coletadas, com contagem e data | **PASS** | 1.115 issues (358 abertas, 757 fechadas) e 401 PRs, coleta de 2026-10-21T00:47Z no HEAD `a8f0d1d` ([issue-audit.md](issue-audit.md)). Os itens são de um instantâneo; nada mudou desde então no SHA fixado |
| Aplicabilidade classificada para cada issue | **PASS** | 1.115 de 1.115 linhas com aplicabilidade, categoria e `mapeamento_rust` ([applewin-issue-matrix.csv](applewin-issue-matrix.csv)). Cada linha tem resultado individual (849) ou nota do auditor (266 sem prioridade, NOT_APPLICABLE) |
| Cada issue tem estado e evidência ou justificativa; nenhuma fechada em PASS sem teste | **PASS** | `apply_results.py --check`: "Matriz em dia"; PASS 479, FAIL 14, BLOCKED 133, NOT_APPLICABLE 489, **UNVERIFIED 0**. Verificado por consulta à matriz: 0 linhas sem `teste`/`evidencia`/`justificativa_na`, 0 PASS sem `teste`, 0 NOT_APPLICABLE sem justificativa, 0 FAIL ou BLOCKED sem evidência. Resultados em [results/](results/) (`p23-*.csv` da onda final) |
| Divergências identificadas registradas, nenhuma omitida | **PASS** | [gaps-and-known-limitations.md](gaps-and-known-limitations.md) (inclusive a seção "Divergências do AppleWin pendentes de decisão"), [capability-matrix.md](capability-matrix.md), os `audit/*.md` |
| Amostra estratificada validada por auditor independente; toda diferença provoca o reexame da classe | **PASS** (triagem) | [issue-audit-review.md](issue-audit-review.md): 465 issues, semente `20261020`; 168 correções; 334 linhas reexaminadas por classe. **Ressalva**: os 849 resultados da onda final foram produzidos e aplicados por agentes implementadores e não têm amostra independente; isso é o CA-07 da HU-0001 e mantém a #15 em BLOCKED |
| [baseline.md](baseline.md) com todos os SHAs congelados | **PASS** | quatro SHAs completos, data, toolchain |

G0 = **PASS** pelo §18 ("100% coletadas, aplicabilidade classificada e evidência ou justificativa para cada caso; divergências não omitidas"). Vale como autoavaliação até a revisão independente da onda final. Antes: FAIL, porque 509 linhas UNVERIFIED não tinham justificativa. Nenhuma justificativa foi genérica; cada uma cita teste, código do AppleWin ou causa do bloqueio.

## G1: Alpha técnica (projeto inteiro: Linux, Windows e macOS)

| Critério | Resultado | Evidência |
|---|---|---|
| Smoke tests e CI verdes nos três SO | **BLOCKED** | Linux nativo: PASS (`smoke-linux.sh`, pacote extraído, 17 verificações; 20/10/2026). Windows via Wine 10.0: PASS (PNG idêntico ao do Linux em 5 Modelos e no DOS 3.3; 20/10/2026), mas **Wine não é Windows**. **Windows nativo: BLOCKED. macOS: BLOCKED.** CI: nunca executada nesta fase (cota esgotada, só com autorização do dono); `release.yml` nunca rodou ([relatorio-plataformas.md](platform/relatorio-plataformas.md)) |
| Boot básico dos Modelos suportados, com resultado por Modelo | **PASS** | Traces idênticos ao Oráculo em ][, ][+, //e e //e Enhanced; boot headless em `apple2`, `apple2p`, `apple2jp`, `apple2e`, `apple2ee` no Linux e no Wine; estados reais de Pravets 82/8M/8A, Base64A, TK3000 //e e J-Plus batem em 100.000 ciclos. Vídeo dos clones: sem comparação de framebuffer |
| CPU, memória, vídeo, disco e save-state com regressões P0 automatizadas e em PASS | **PASS** | As 30 issues P0: 23 PASS, 7 BLOCKED (mecanismos sintéticos passam), 0 FAIL. Os P0 achados na auditoria têm regressão automatizada em PASS (AUD-C-010, AUD-C-060, AUD-C-001/003, AUD-C-035, AUD-C-022/033). Save-state real: 88 PASS, 0 FAIL |
| `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace --release` verdes | **PASS** | Em 2026-10-21, Linux: `cargo fmt --all -- --check` sem diferenças; `cargo clippy --workspace --all-targets -- -D warnings` sem avisos; `cargo test --workspace --release --no-fail-fast`: 1.577 passam, 0 falham, 22 ignorados. Conformidade: 773 PASS, 5 FAIL, 0 BLOCKED em 778 casos (os 5 FAIL são divergências conhecidas de PCM/SSI) |
| Falhas conhecidas destacadas nas notas da release | **PASS** | Seção "Limitações conhecidas" do CHANGELOG da branch `release/v0.2.0-rc.1`, com link para o [relatório final](relatorio-final.md) (a release não foi publicada) |

G1 = **BLOCKED**: nenhum critério está em FAIL, mas o primeiro depende de Windows nativo, macOS e CI, que não foram executados. Destravar: decisão 6 abaixo e o teste manual de Windows e macOS pelo dono (decisão 7).

## G2: Beta funcional (projeto inteiro)

| Critério | Resultado | Evidência |
|---|---|---|
| Zero FAIL P0 aplicável | **PASS** | 0 das 30 P0 em FAIL na matriz. Antes: FAIL, por `Saturn64`, `Saturn128` e `SmartPort-firmware`; hoje os três batem no trace de 100.000 ciclos (os Saturn por correção do Oráculo, o SmartPort por correção do firmware do DIB, 132.1) |
| Zero UNVERIFIED P0 sem bloqueio formal aceito | **BLOCKED** | 0 P0 UNVERIFIED. Mas 7 P0 estão BLOCKED (#4, #70, #400, #422, #564, #640, #1126) e nenhum bloqueio foi aceito formalmente pelo dono; aguardam o aceite ou as fixtures (decisão 5) |
| Fixtures reais críticas (AppleWin-Test) aprovadas | **PASS** (críticas); 27 BLOCKED | 88 de 115 estados reais PASS e **0 FAIL**, entre eles os da HU-0002 (`Saturn64`, `Saturn128`, `LC-RamWrite-Test`, `SSI263-4x_phonemes_playing_continuously`, `HDD-32MB`, `AS-S1_Turn the disk over(PAL)`). Os 27 BLOCKED têm causa (18 por imagem ausente do corpus, 3 SSC, 2 hora real, 2 Shift Key Mod, 1 Z80, 1 Uthernet). WOZ, DSK, HDD e `.do.gz` reais bootam idênticos ao Oráculo (disco: 56 casos PASS) |
| Matriz dos componentes essenciais por Modelo completa | **FAIL** | [capability-matrix.md](capability-matrix.md) existe e cobre os componentes, mas o vídeo dos clones (Pravets, TK3000, Base64A, J-Plus), VidHD em ][/][+ e SHR em PAL seguem sem comparação, e Windows/macOS BLOCKED |
| Testes de áudio, entrada e interface com evidência nos três SO | **FAIL** (e BLOCKED) | Áudio: eventos PASS no Linux; **PCM em FAIL (5 casos)**; audição humana BLOCKED. Entrada e interface: testes Rust e Xvfb no Linux e Wine; Windows nativo, macOS e hardware (gamepad, teclado regional) BLOCKED |
| Recuperação de estado e de discos validada | **PASS** | Gravação atômica, queda simulada, flush em `ApplySettings`/`LoadState`/SIGTERM, fuzz de WOZ/NIB/DSK/YAML/2MG/gz/zip, limites de tamanho: `atomic_write.rs`, `crash_safety.rs`, `audit_persistence.rs`, `fuzz_parsers.rs`, `fuzz_loader.rs`. Queda real de energia e HDD de mais de 2 MiB: ressalvas no [item 8 do relatório](relatorio-final.md) |

G2 = **FAIL**, com parte BLOCKED. Dois critérios saíram do vermelho (zero FAIL P0 e fixtures críticas); os que seguram são a matriz incompleta, o áudio, os 3 SO e os 7 P0 sem aceite.

## G3: Stable candidata (projeto inteiro)

| Critério | Resultado | Evidência |
|---|---|---|
| Zero FAIL P0 e P1 no escopo declarado | **FAIL** | P1 em FAIL: **#241** (Color Monitor PAL). Saíram dos FAIL a #381 e a #1176. Os 5 casos FAIL de PCM/SSI são P1 no harness (issues #7, #27, #28 do backlog) |
| Testes entre plataformas repetidos a partir do pacote instalado | **BLOCKED** | Só o pacote Linux foi extraído e testado; Windows e macOS sem pacote testado; nada repetido |
| Nenhuma perda de dados nos cenários cobertos | **PASS** | Cenários cobertos em [relatorio-final.md](relatorio-final.md), item 8. Fora do coberto: queda real de energia, HDD de mais de 2 MiB (atualizado no lugar), duas instâncias na mesma imagem (UNVERIFIED) |
| Parecer independente de agente revisor, aprovado pelo dono | **BLOCKED** | Ainda não existe. A auditoria independente cobriu só a triagem de issues (HU-0001), não o conjunto nem a onda final |
| Problemas P2 e P3 publicados e escopo explícito | **PASS** | P2 e P3: 327 PASS, 13 FAIL, 52 BLOCKED, 211 NOT_APPLICABLE, 0 UNVERIFIED, em [gaps-and-known-limitations.md](gaps-and-known-limitations.md), no CHANGELOG e no manual 12 |

G3 = **FAIL**.

## Rubrica Verônica

Nota de 0 a 100, baseada em evidências. Autoavaliação feita por quem consolidou a entrega; a revisão independente vem depois e pode alterar qualquer linha.

| Dimensão | Pontos | Antes | Nota | Justificativa |
|---|---:|---:|---:|---|
| Especificação e critérios de aceite | 25 | 20 | 22 | Plano e backlog importável (13 épicos, 30 histórias e 36 histórias de issues abertas, #2 a #80); ADR 0004 e 0005; políticas de comparação registradas antes das execuções ([test-oracles.md](test-oracles.md)); Gates definidos; as 1.115 issues com estado e evidência (0 UNVERIFIED). Faltam: revisão independente da onda final, critérios de aceite de interface sem Oráculo de janela, as divergências a favor do usuário sem decisão (nove decisões pendentes) |
| Corretude e evidência | 25 | 18 | 20 | Evidência forte contra o Oráculo: 10,22 milhões de vetores de CPU, 7 traces, 1.573 telas, 103 milhões de instruções de disco, 88 save-states reais sem nenhum FAIL, eventos de áudio idênticos, 773 de 778 casos de conformidade. Descontos: 5 casos de PCM/SSI em FAIL, #241, 14 FAIL na matriz (10 são bugs do AppleWin reproduzidos), 133 BLOCKED, Windows/macOS/hardware sem execução, Oráculo limitado (fork, sem entrada, uma carta de áudio). Atenuante honesto: 5 dos 9 FAIL de save-state eram defeito do Oráculo, não do port |
| Testes e regressão | 15 | 12 | 12 | 1.577 testes e 778 casos de conformidade executados (0 testes falhando); regressão por issue (849 resultados aplicados) e fuzz de analisadores; o runner passa a comparar `trace_rows_sha256`. Descontos: CI nunca executada, sem repetição entre plataformas, fuzz de longa duração e hardware não executados, vetores 65x02/Z80 não refeitos nesta rodada |
| Arquitetura e manutenção | 10 | 8 | 8 | Núcleo headless e determinístico (ADR 0001), 18 crates, zero alocações por passo e por quadro verificado por teste, contratos de harness e Oráculo documentados, patches do Oráculo versionados. Descontos: dado de resultado obsoleto achado nesta execução (FAIL antigo da #1340 vencendo o PASS pela precedência), `CardCtx::floating_bus` eager |
| Segurança, dados e resiliência | 10 | 8 | 8 | Gravação atômica, fuzz, limites de tamanho e descompressão, handlers de sinal, erros de E/S visíveis. Descontos: HDD grande gravado no lugar, AUD-C-037 aberto, pacotes sem assinatura, queda real não testada |
| Higiene da entrega | 5 | 4 | 4 | Commits em português com `[skip ci]`, SBOM, SHA256SUMS e licenças de terceiros preparados, nenhuma CI acionada, nenhuma tag. Desconto: `release.yml` nunca executado |
| Eficiência/retrabalho | 10 | 6 | 7 | Os três itens que estavam em "duas hipóteses sem êxito" (#1176, mouse/VBL e Saturn) fecharam, o último por descobrir que o defeito era do Oráculo. Seguem abertos D-2 (PCM) e #241, ambos com duas hipóteses descartadas. Houve regeneração de relatórios obsoletos e um relatório de importação regerado com caminhos errados (descartado) |
| **Total** | **100** | **76** | **81** | |

Leitura da nota:

- O aceite de uma história exige nota **igual ou maior que 96**, todos os Critérios de aceite materiais em PASS, nenhum bloqueador crítico e revisão por quem **não** implementou a alteração. Esta nota é do conjunto, não de uma história, e está bem abaixo de 96: o trabalho **não** está aceito como completo.
- Nota de 90 a 95 pede uma correção focada; **89 ou menos**, ou repetição de tentativas sem hipótese nova, interrompe o trabalho e leva a replanejar. O plano está em [backlog.md](backlog.md) e no item 5 do [relatório final](relatorio-final.md).

## Estado das histórias HU (#15 a #44)

Depois da reavaliação, cada HU recebeu a label `estado:` do estado real, por `tools/conformance/update_hu_issues.py` (regra: PASS só com todos os CA em PASS). **14 PASS, 5 FAIL, 11 BLOCKED.** Nenhuma foi fechada.

| Estado | Histórias |
|---|---|
| PASS | HU-0101, 0102, 0103, 0202, 0301, 0303, 0402, 0602, 1101, 1201, 1202, 1203, 1204, 1205 |
| FAIL | HU-0201 (#241), 0501 e 0502 (PCM/SSI), 0702 (#1396), 0801 (depurador) |
| BLOCKED | HU-0001 (CA-07), 0002 (CI/JUnit), 0302, 0401, 0601, 0701, 0703, 0802, 0901, 0902, 1001 |

## Release final

**Recomendação objetiva (§18): alpha técnica; o rótulo de release candidate para Linux é do dono e vai como pré-lançamento.** A versão `0.2.0-rc.1` está preparada na branch local `release/v0.2.0-rc.1` (versão do workspace, CHANGELOG com "release candidate para Linux; Windows e macOS sem verificação nativa" e "Limitações conhecidas", `Info.plist` com `CFBundleVersion` numérico, texto da release que marca os pacotes de Windows e macOS como não verificados). O `release.yml` trata o sufixo `-rc.1` como pré-lançamento (`prerelease: true`). O pull request `[release]`, a tag e o push só são feitos por ordem do dono.

O que sustenta a entrega: G0 PASS; G1 no Linux só espera a CI; nenhuma regressão P0 conhecida; o pacote Linux foi gerado e testado. O que impede chamá-la de beta ou stable candidate pelo §18, **qual critério falta**:

- G2 (Linux): matriz incompleta, 5 FAIL de PCM/SSI, 7 P0 BLOCKED sem aceite, hardware sem execução.
- G3 (Linux): zero FAIL P1 (a #241 e os 5 de PCM/SSI) e o parecer independente.
- Com a #241 corrigida, falta ainda a decisão 4 (PCM/SSI), o parecer independente, a CI e o aceite dos 7 P0.

Artefatos do pacote Linux (em `target/release-candidate/`, fora do Git): os hashes estão no [relatório final](relatorio-final.md), item 6.

## Decisões pendentes do dono

Nenhuma foi tomada pelo time. Cada linha traz as opções e uma recomendação; o detalhe de cada uma está no [relatório final](relatorio-final.md), item 11.

| # | Decisão | Opções | Recomendação | Destrava |
|---|---|---|---|---|
| 1 | Os 10 FAIL do depurador e a #458 (bugs do AppleWin `a8f0d1d`) | Manter a paridade; corrigir todos como divergência; corrigir só os baratos | Corrigir #114, #245, #458, #466, #468, #1381; manter #713, #1145, #1377; adiar #1426 e #1431 | 11 FAIL da matriz; HU-0801 |
| 2 | #1396: Alt+F2 apaga o Open Apple no AppleWin; o port mantém | Manter e registrar; reproduzir o AppleWin; opção de linha de comando | Manter e registrar como divergência a favor do usuário | 1 FAIL; HU-0702 |
| 3 | #916 (Pravets correto no port) e #855 (`DebuggerAutoRun` funciona no port) | Registrar como divergência a favor do usuário; reverter; registrar e corrigir o AppleWin | Registrar (e, se quiser, enviar a correção ao AppleWin); idem #280/#358/#1374, #932, #1448 | seção de pendentes do gaps |
| 4 | As 5 divergências de PCM/SSI | Aceitar como limitação conhecida; copiar o quantum do AppleWin | Aceitar como limitação conhecida | 5 casos FAIL; HU-0501 e HU-0502 |
| 5 | Os 7 P0 BLOCKED | Aceite formal; obter fixtures; misto | Misto: aceitar #4, #564, #640; obter fixtures de #70, #400, #422, #1126 se houver origem legal | G2 (critério P0) |
| 6 | Autorizar uma execução manual de CI (`ci.yml` e `conformance.yml`, nos 3 SO) | Autorizar uma de cada; esperar a cota; não rodar | Autorizar uma de cada, no SHA final; `release.yml` não | G1 |
| 7 | O dono executa os roteiros de Windows e macOS (VMs do VirtualBox, a partir do clone do repositório) | Executar os roteiros e registrar o resultado; esperar a CI; publicar sem verificação nativa | Executar os roteiros; até lá os dois ficam BLOCKED e marcados como não verificados na release | Windows e macOS, HU-0901, HU-0302 |
| 8 | Assinatura e notarização dos pacotes | Pré-lançamento sem assinar; assinar antes do beta; programa gratuito de assinatura para código aberto | Pré-lançamento sem assinar (aviso no CHANGELOG); decidir antes do beta | HU-0902; G3 |
| 9 | Abrir o PR `[release]` da v0.2.0-rc.1 | Agora; depois da CI (decisão 6), dos roteiros (decisão 7) e da revisão independente; não abrir | Depois da CI no Linux, da #241 resolvida e da revisão independente; Windows e macOS seguem marcados como não verificados | publicação do pré-lançamento |
