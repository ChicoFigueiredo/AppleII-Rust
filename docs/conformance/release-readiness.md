# Prontidão para release

Este documento avalia os Gates de prontidão (G0 a G3) e a nota pela rubrica Verônica, com os critérios do §18 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). Um Gate só é atingido quando **todos** os seus critérios objetivos estão em PASS. Um Gate com algum critério BLOCKED e nenhum FAIL é BLOCKED; qualquer FAIL o torna FAIL. Nota alta não substitui um Gate.

**A nota e as avaliações deste documento são autoavaliação; a revisão independente vem depois.** Quem implementou o trabalho avaliado não pode aprová-lo (regra do §18).

## Estado atual

Data: 2026-10-21. SHA avaliado: `22dbf2b7a4c57d9b6ede7be39a577e93196c1a3f` (`main`), mais os documentos desta entrega. Ambiente: Linux x86_64 (Ubuntu 26.04.1, `rustc 1.98.1`). Números e relatórios: [relatorio-final.md](relatorio-final.md).

| Gate | Nome | Estado |
|---|---|---|
| G0 | Inventário | **FAIL** (1 critério) |
| G1 | Alpha técnica | **BLOCKED** (Windows nativo, macOS e CI) |
| G2 | Beta funcional | **FAIL** (e parte BLOCKED) |
| G3 | Stable candidata | **FAIL** |

Nota (Verônica, 0 a 100): **76**, autoavaliação. **Recomendação: alpha** (`0.2.0-alpha.1`, como pré-lançamento).

## Regras de evidência

- A evidência vem do Linux local e do Wine. Windows nativo e macOS ficam BLOCKED até o dono autorizar uma execução de CI (a CI é só manual, porque a cota está esgotada) ou executar os roteiros.
- Testes que exigem hardware físico (gamepad, serial, pcap, escuta de áudio) têm roteiros manuais e ficam BLOCKED até o dono executá-los e registrar o resultado.
- O G3 depende de um parecer de agente revisor independente, aprovado pelo dono.
- A recomendação de release (alpha, beta ou stable candidate) não é inflada: segue o maior Gate atingido, com as ressalvas escritas.

## G0: Inventário

| Critério | Resultado | Evidência |
|---|---|---|
| 100% das issues do AppleWin (abertas e fechadas) coletadas, com contagem e data | **PASS** | 1.115 issues (358 abertas, 757 fechadas) e 401 PRs, coleta de 2026-10-21T00:47Z no HEAD `a8f0d1d` ([issue-audit.md](issue-audit.md)). Os itens são de um instantâneo; nada mudou desde então no SHA fixado |
| Aplicabilidade classificada para cada issue | **PASS** | 1.115 de 1.115 linhas com aplicabilidade, categoria e `mapeamento_rust` ([applewin-issue-matrix.csv](applewin-issue-matrix.csv)). 323 linhas ainda têm só a triagem original, sem revisão individual |
| Cada issue tem evidência ou justificativa; nenhuma fechada em PASS sem teste | **FAIL** | A segunda metade passa: nenhuma issue fechada está em PASS sem teste. A primeira não: das 524 UNVERIFIED, **509 não têm evidência nem justificativa** além da classificação (P2: 235, P3: 274). As outras 15 UNVERIFIED e as 205 PASS, 87 BLOCKED, 13 FAIL e 286 NOT_APPLICABLE têm |
| Divergências identificadas registradas, nenhuma omitida | **PASS** | [gaps-and-known-limitations.md](gaps-and-known-limitations.md), [capability-matrix.md](capability-matrix.md), os `audit/*.md` |
| Amostra estratificada validada por auditor independente; toda diferença provoca o reexame da classe | **PASS** | [issue-audit-review.md](issue-audit-review.md): 465 issues (337 obrigatórias e 128 aleatórias), semente `20261020`; 168 correções; 334 linhas reexaminadas por classe. Concordância na parte aleatória: 95 % em aplicabilidade, 91 % em qualquer campo |
| [baseline.md](baseline.md) com todos os SHAs congelados | **PASS** | quatro SHAs completos, data, toolchain |

Para o G0 chegar a PASS basta documentar a justificativa das 509 linhas (por exemplo, "planejada na triagem do EP-11, não selecionada") ou executar os testes delas. Nenhuma foi preenchida de forma genérica só para fechar o Gate.

## G1: Alpha técnica

| Critério | Resultado | Evidência |
|---|---|---|
| Smoke tests e CI verdes nos três SO | **BLOCKED** | Linux nativo: PASS (`smoke-linux.sh`, pacote extraído, 17 verificações). Windows via Wine 10.0: PASS (PNG idêntico ao do Linux em 5 Modelos e no DOS 3.3), mas **Wine não é Windows**. **Windows nativo: BLOCKED. macOS: BLOCKED.** CI: nunca executada nesta fase (cota esgotada, só com autorização do dono); `release.yml` nunca rodou ([relatorio-plataformas.md](platform/relatorio-plataformas.md)) |
| Boot básico dos Modelos suportados, com resultado por Modelo | **PASS** | Traces idênticos ao Oráculo em ][, ][+, //e e //e Enhanced; boot headless em `apple2`, `apple2p`, `apple2jp`, `apple2e`, `apple2ee` no Linux e no Wine; estados reais de Pravets 82/8M/8A, Base64A, TK3000 //e e J-Plus batem em 100.000 ciclos. Vídeo dos clones: UNVERIFIED |
| CPU, memória, vídeo, disco e save-state com regressões P0 automatizadas e em PASS | **PASS** | As 30 issues P0: 23 PASS, 7 BLOCKED (mecanismos sintéticos passam), 0 FAIL. Os P0 achados na auditoria têm regressão automatizada em PASS (AUD-C-010, AUD-C-060, AUD-C-001/003, AUD-C-035, AUD-C-022/033). Ressalva: veja os FAIL de importação em G2 |
| `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace --release` verdes | **PASS** | Em 2026-10-21, Linux: `cargo fmt --all -- --check` sem diferenças; `cargo clippy --workspace --all-targets -- -D warnings` sem avisos; `cargo test --workspace --release --no-fail-fast`: 1.400 passam, 0 falham, 20 ignorados. Conformidade: 585 PASS, 6 FAIL, 0 BLOCKED em 591 casos |
| Falhas conhecidas destacadas nas notas da release | **PASS** | Seção "Limitações conhecidas" do CHANGELOG da branch `release/v0.2.0-alpha.1`, com link para o [relatório final](relatorio-final.md) (a release não foi publicada) |

G1 = **BLOCKED**: nenhum critério está em FAIL, mas o primeiro depende de Windows nativo, macOS e CI, que não foram executados.

## G2: Beta funcional

| Critério | Resultado | Evidência |
|---|---|---|
| Zero FAIL P0 aplicável | **FAIL** | Na matriz de issues, 0 das 30 P0 estão em FAIL. Mas as áreas P0 têm FAIL medidos: `Saturn64.aws.yaml` e `Saturn128.aws.yaml` (EP-01/EP-04, HU-0102/HU-0401) divergem do Oráculo no ciclo +406, e `SmartPort-firmware` (EP-03, P0) no ciclo +4.373 ([savestate-import.md](../../tests/conformance/reports/savestate-import.md)). Causa raiz não investigada |
| Zero UNVERIFIED P0 sem bloqueio formal aceito | **BLOCKED** | 0 P0 UNVERIFIED. Mas 7 P0 estão BLOCKED (#4, #70, #400, #422, #564, #640, #1126) e nenhum bloqueio foi aceito formalmente pelo dono; aguardam o aceite ou as fixtures |
| Fixtures reais críticas (AppleWin-Test) aprovadas | **FAIL** | 79 de 115 estados reais PASS; **9 FAIL** (Saturn x2, SmartPort, SNES MAX, mouse x3, Mockingboard x2) e 27 BLOCKED. WOZ, DSK, HDD e `.do.gz` reais do corpus bootam idênticos ao Oráculo (disco: 56 casos PASS) |
| Matriz dos componentes essenciais por Modelo completa | **FAIL** | [capability-matrix.md](capability-matrix.md) existe e cobre os componentes, mas o vídeo dos clones (Pravets, TK3000, Base64A, J-Plus), VidHD em ][/][+ e SHR em PAL estão UNVERIFIED, e Windows/macOS BLOCKED |
| Testes de áudio, entrada e interface com evidência nos três SO | **BLOCKED** | Áudio: eventos PASS no Linux; PCM em FAIL; audição humana BLOCKED. Entrada e interface: testes Rust e Xvfb no Linux e Wine; Windows nativo, macOS e hardware (gamepad, teclado regional) BLOCKED |
| Recuperação de estado e de discos validada | **PASS** | Gravação atômica, queda simulada, flush em `ApplySettings`/`LoadState`/SIGTERM, fuzz de WOZ/NIB/DSK/YAML/2MG/gz/zip, limites de tamanho: `atomic_write.rs`, `crash_safety.rs`, `audit_persistence.rs`, `fuzz_parsers.rs`, `fuzz_loader.rs`. Queda real de energia e HDD de mais de 2 MiB: ressalvas no [item 8 do relatório](relatorio-final.md) |

G2 = **FAIL**, com parte BLOCKED.

## G3: Stable candidata

| Critério | Resultado | Evidência |
|---|---|---|
| Zero FAIL P0 e P1 no escopo declarado | **FAIL** | P1 em FAIL: **#241** (Color Monitor PAL), **#381** (mouse, 3 estados reais) e **#1176** (6522). Mais os FAIL P0/P1 de importação listados em G2. Os 5 FAIL de PCM/SSI são P1 |
| Testes entre plataformas repetidos a partir do pacote instalado | **BLOCKED** | Só o pacote Linux foi extraído e testado; Windows e macOS sem pacote testado; nada repetido |
| Nenhuma perda de dados nos cenários cobertos | **PASS** | Cenários cobertos em [relatorio-final.md](relatorio-final.md), item 8. Fora do coberto: queda real de energia, HDD de mais de 2 MiB (atualizado no lugar), duas instâncias na mesma imagem (UNVERIFIED) |
| Parecer independente de agente revisor, aprovado pelo dono | **BLOCKED** | Ainda não existe. A auditoria independente cobriu só a triagem de issues (HU-0001), não o conjunto |
| Problemas P2 e P3 publicados e escopo explícito | **PASS** | 524 UNVERIFIED (278 P3 e 246 P2) e as limitações em [gaps-and-known-limitations.md](gaps-and-known-limitations.md), no CHANGELOG e no manual 12 |

G3 = **FAIL**.

## Rubrica Verônica

Nota de 0 a 100, baseada em evidências. Autoavaliação feita por quem consolidou a entrega; a revisão independente vem depois e pode alterar qualquer linha.

| Dimensão | Pontos | Nota | Justificativa |
|---|---:|---:|---|
| Especificação e critérios de aceite | 25 | 20 | Plano e backlog importável (13 épicos, 30 histórias e 36 histórias de issues abertas, #2 a #80); ADR 0004 e 0005; políticas de comparação registradas antes das execuções ([test-oracles.md](test-oracles.md)); Gates definidos. Faltam: 509 issues sem justificativa, 323 triagens sem revisão individual, critérios de aceite de interface sem Oráculo |
| Corretude e evidência | 25 | 18 | Evidência forte contra o Oráculo: 10,22 milhões de vetores de CPU, 7 traces, 1.573 telas, 103 milhões de instruções de disco, 79 save-states reais, eventos de áudio idênticos. Descontos: 6 casos de conformidade em FAIL, 9 save-states reais em FAIL (2 com causa não investigada), 87 issues BLOCKED, 524 UNVERIFIED, Windows/macOS/hardware sem execução, Oráculo limitado (fork, sem entrada, uma carta de áudio) |
| Testes e regressão | 15 | 12 | 1.400 testes e 591 casos de conformidade executados (0 testes falhando). Regressão por issue (325 resultados aplicados) e fuzz de analisadores. Descontos: CI nunca executada, sem repetição entre plataformas, fuzz de longa duração e hardware não executados |
| Arquitetura e manutenção | 10 | 8 | Núcleo headless e determinístico (ADR 0001), 18 crates, zero alocações por passo e por quadro verificado por teste, contratos de harness e Oráculo documentados. Descontos: dívida de documentos desatualizados achada nesta consolidação (relatório de importação com 12 FAIL em vez de 9, linhas do wave0 antigas), `CardCtx::floating_bus` eager |
| Segurança, dados e resiliência | 10 | 8 | Gravação atômica, fuzz, limites de tamanho e descompressão, handlers de sinal, erros de E/S visíveis. Descontos: HDD grande gravado no lugar, AUD-C-037 aberto, pacotes sem assinatura, queda real não testada |
| Higiene da entrega | 5 | 4 | Commits em português com `[skip ci]`, SBOM, SHA256SUMS e licenças de terceiros preparados, nenhuma CI acionada, nenhuma tag. Desconto: `release.yml` nunca executado |
| Eficiência/retrabalho | 10 | 6 | Várias frentes paralelas conflitaram em arquivos e no cache compartilhado do Oráculo (L2 do diferencial de disco), regeneração de relatórios obsoletos, três itens abertos com "duas hipóteses sem êxito" (D-2, #1176, Saturn não investigado). Nenhum loop de correção sem hipótese nova |
| **Total** | **100** | **76** | |

Leitura da nota:

- O aceite de uma história exige nota **igual ou maior que 96**, todos os Critérios de aceite materiais em PASS, nenhum bloqueador crítico e revisão por quem **não** implementou a alteração. Esta nota é do conjunto, não de uma história, e está bem abaixo de 96: o trabalho **não** está aceito como completo.
- Nota de 90 a 95 pede uma correção focada; **89 ou menos**, ou repetição de tentativas sem hipótese nova, interrompe o trabalho e leva a replanejar. O plano está em [backlog.md](backlog.md) e no item 5 do [relatório final](relatorio-final.md).

## Release final

**Recomendação: alpha.** A versão `0.2.0-alpha.1` está preparada na branch local `release/v0.2.0-alpha.1` (versão do workspace, CHANGELOG com "Limitações conhecidas"); o `release.yml` trata o sufixo `-alpha.1` como pré-lançamento (`prerelease: true`). O pull request `[release]`, a tag e o push só são feitos por ordem do dono.
