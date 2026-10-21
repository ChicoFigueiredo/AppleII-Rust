# Prontidão para release

Este documento avalia os Gates de prontidão (G0 a G3) e a nota pela rubrica Verônica, com os critérios do §18 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). Um Gate só é atingido quando **todos** os seus critérios objetivos estão em PASS. Um Gate com algum critério BLOCKED e nenhum FAIL é BLOCKED; qualquer FAIL o torna FAIL. Nota alta não substitui um Gate.

**A nota e as avaliações deste documento são autoavaliação.** O [parecer independente](parecer-independente.md) de 21/10/2026 deu 77 de 100 e recomendou alpha técnica; a resposta a cada achado está na seção 12 do [relatório final](relatorio-final.md). Quem implementou o trabalho avaliado não pode aprová-lo (regra do §18): a aprovação do parecer é do dono.

**Sobre "Oráculo" neste documento:** o oráculo executável é o fork do audetto (`a1e20c0`) com núcleo AppleWin `3e8054b`, 22 commits antes do `a8f0d1d`, com os patches `audetto-trace`, `audetto-audio`, `audetto-snesmax`, `audetto-ssc-rom` e `audetto-version`; o `a8f0d1d` é a referência de código. Os PASS por issue têm um tipo de oráculo (`executavel`, `cpp-transcrito`, `especificacao`, `fixture-real`, `regressao`): dos 486 PASS, 147 são comparação com o AppleWin rodando, 199 C++ transcrito com arquivo:linha, 5 especificação, 7 fixture real e 128 teste do próprio port.

## Estado atual

Data: 2026-10-21. SHA avaliado: `469bf941deab932ab228169a7ffebddae1fc6854` (`main`), mais a conversão da #1396 em Divergência aprovada (teste novo, sem mudar código de emulação), a revisão do parecer independente (coluna `tipo_oraculo`, #1490 de PASS a BLOCKED, um teste reforçado) e os documentos desta entrega. Esta é a reavaliação depois da #241 corrigida, das seis Divergências aprovadas do depurador e da ADR 0006 ([divergências aprovadas e limitações aceitas](../adr/0006-divergencias-aprovadas-pelo-dono.md)). Ambiente: Linux x86_64 (Ubuntu 26.04.1, `rustc 1.98.1`). Números e relatórios: [relatorio-final.md](relatorio-final.md).

| Gate | Nome | Estado | Antes |
|---|---|---|---|
| G0 | Inventário | **PASS** (autoavaliação, com a ressalva do parecer) | PASS |
| G1 | Alpha técnica | **BLOCKED** (Windows nativo, macOS e CI) | BLOCKED |
| G2 | Beta funcional | **FAIL** (e parte BLOCKED) | FAIL |
| G3 | Stable candidata | **BLOCKED** (zero FAIL P0/P1 não aceito; 5 FAIL P1 aceitos pelo dono como Limitação aceita (ADR 0006); faltam Windows/macOS e a aprovação do parecer independente pelo dono) | FAIL |

Nota (Verônica, 0 a 100): **84**, autoavaliação (antes: 81). **Recomendação objetiva pelo §18: alpha técnica**, agora sem FAIL P0 nem P1 em issue; o dono declarou a entrega como **release candidate para Linux** (`0.2.0-rc.1`, pré-lançamento). A seção seguinte avalia os Gates nesse escopo e diz o que falta para sustentar o rótulo. A tabela acima é a do projeto inteiro, com Windows e macOS contados como pendentes.

Uma **Limitação aceita** conta nos Gates como FAIL aceito pelo dono, não como PASS (ADR 0006): os 5 casos de PCM/SSI continuam FAIL no harness e nos relatórios. Por isso o critério de G3 "zero FAIL P0/P1" está escrito, nesta página e no relatório final, como "zero FAIL P0/P1 não aceito; 5 FAIL P1 aceitos pelo dono como Limitação aceita (ADR 0006)".

## Escopo declarado da release: Linux x86_64

Decisão do dono: a entrega é um **release candidate para Linux x86_64** (`v0.2.0-rc.1`, branch local `release/v0.2.0-rc.1`, pré-lançamento). **Windows e macOS saem como BLOCKED/pendentes.** Eles **não contam como FAIL do escopo Linux**: nada neles foi medido, nem para bem nem para mal. Uma sessão do dono no Windows 11 nativo está rodando o roteiro W1 a W10 (veja "Windows nativo: em execução" no [relatório final](relatorio-final.md), item 6); macOS segue sem execução. Os pacotes de Windows e macOS continuam sendo gerados pelo `release.yml` quando o dono o disparar, e o texto da release os marca como **não verificados nativamente**. A CI e o PR `[release]` são disparados pelo dono.

Estado dos Gates no escopo Linux (critério a critério abaixo). A #241 já passa, e o antigo "sem a #241 corrigida" deixou de existir.

| Gate | Estado | O que falta |
|---|---|---|
| G0 Inventário | **PASS** | nada |
| G1 Alpha técnica (Linux) | **BLOCKED** | só a CI do GitHub no Linux (nunca acionada; o dono a dispara) |
| G2 Beta funcional (Linux) | **FAIL** (e parte BLOCKED) | matriz de componentes incompleta (clones, VidHD em ][/][+, SHR em PAL), 7 P0 BLOCKED sem aceite do dono, hardware sem execução (gamepad, teclado regional, Wayland, audição) |
| G3 Stable candidata (Linux) | **BLOCKED** | a aprovação do parecer independente pelo dono. Critério de FAIL: zero FAIL P0/P1 não aceito; 5 FAIL P1 aceitos pelo dono como Limitação aceita (ADR 0006). Depende também do G2, que está em FAIL |

### G1 no escopo Linux

| Critério | Resultado | Evidência |
|---|---|---|
| Smoke tests e CI verdes (Linux) | **BLOCKED** (CI); smoke **PASS** | `smoke-linux.sh` e `test-package.sh` PASS (o pacote `0.2.0-rc.1` foi gerado e testado de novo em 21/10/2026 sobre esta entrega, veja o relatório final, item 6). A CI do GitHub nunca foi acionada: cota esgotada, só o dono dispara. `tools/package/linux.sh` reproduz localmente os passos do `release.yml` |
| Boot básico dos Modelos, por Modelo | **PASS** | igual ao G1 do projeto inteiro |
| CPU, memória, vídeo, disco e save-state com regressões P0 em PASS | **PASS** | 23 de 30 P0 PASS (8 `executavel`, 5 `cpp-transcrito`, 2 `especificacao`, 8 `regressao`), 0 FAIL, 7 BLOCKED (23,3 % dos P0) |
| `cargo fmt`, `clippy -D warnings`, `cargo test --workspace --release` verdes | **PASS** | 1.585 passam, 0 falham, 15 ignorados |
| Falhas conhecidas destacadas nas notas da release | **PASS** | CHANGELOG da `release/v0.2.0-rc.1`, "Limitações conhecidas", "Limitações aceitas" e "Divergências aprovadas" |

### G2 no escopo Linux

| Critério | Resultado | Evidência |
|---|---|---|
| Zero FAIL P0 aplicável | **PASS** | 0 de 30 |
| Zero UNVERIFIED P0 sem bloqueio formal aceito | **BLOCKED** | 7 P0 BLOCKED (#4, #70, #400, #422, #564, #640, #1126) sem aceite do dono (decisão 5, ainda pendente). São do núcleo ou do frontend e valem para o Linux |
| Fixtures reais críticas aprovadas | **PASS** | 88 PASS (76,5 % dos 115), 0 FAIL; 27 BLOCKED (23,5 %) com causa. O PASS compara o fluxo de CPU em 100.000 ciclos (0,1 s); memória e vídeo não entram |
| Matriz dos componentes essenciais por Modelo | **FAIL** | clones (Pravets, TK3000, Base64A, J-Plus) sem comparação de vídeo; VidHD em ][/][+ e SHR em PAL sem comparação. Não depende do sistema operacional |
| Áudio, entrada e interface com evidência (Linux) | **BLOCKED** (com Limitação aceita) | Áudio: eventos PASS; os 5 casos de PCM/SSI são **Limitação aceita** (FAIL aceito pelo dono, não PASS); audição humana BLOCKED. Entrada: testes Rust e Xvfb PASS; gamepad, teclado regional e Wayland nativo BLOCKED (roteiros) |
| Recuperação de estado e de discos | **PASS** | como no G2 do projeto inteiro |

### G3 no escopo Linux

| Critério | Resultado | Evidência |
|---|---|---|
| Zero FAIL P0 e P1 no escopo declarado | **zero FAIL P0/P1 não aceito; 5 FAIL P1 aceitos pelo dono como Limitação aceita (ADR 0006)** | Nenhum FAIL P0 nem P1 em issue (a #241 passa). Os 5 casos de PCM/SSI (P1 no harness) seguem FAIL no harness e são Limitação aceita (ADR 0006, 21/10/2026): FAIL aceito pelo dono (dispensa), não PASS |
| Pacote instalado testado | **PASS** (Linux) | `test-package.sh`, pacote extraído fora da árvore e rodado de dentro |
| Nenhuma perda de dados nos cenários cobertos | **PASS** | [relatório final](relatorio-final.md), item 8 |
| Parecer independente aprovado pelo dono | **BLOCKED** | O parecer existe ([parecer-independente.md](parecer-independente.md), 77/100, alpha técnica); falta a aprovação do dono |
| P2 e P3 publicados e escopo explícito | **PASS** | P2 e P3 no [gaps](gaps-and-known-limitations.md), no CHANGELOG e no manual 12 |

"Pacote instalado testado" (Linux): o pacote foi extraído para um diretório fora da árvore e rodado de dentro (`test-package.sh`). `install.sh` em outras distribuições e a instalação do `.desktop` e do MIME num ambiente gráfico limpo não foram testados.

**Não há FAIL P0 nem P1 em issue no escopo Linux.** Os únicos FAIL P0/P1 no harness são as 5 Limitações aceitas de PCM/SSI (FAIL aceitos pelo dono, não PASS).

**Recomendação para o escopo Linux** (completa em "Release final"): o rótulo de release candidate é do dono. Pelo §18 o estado objetivo continua **alpha técnica** (G1 só espera a CI; G2 em FAIL). Para chegar a beta no Linux falta a matriz completa, o aceite dos 7 P0, a CI e a execução dos roteiros de hardware; para stable candidate, ainda o parecer independente.

## Regras de evidência

- A evidência vem do Linux local e do Wine. Windows nativo (execução do dono em andamento) e macOS ficam BLOCKED até o dono registrar os resultados ou disparar uma execução de CI (a CI é só manual, porque a cota está esgotada).
- Testes que exigem hardware físico (gamepad, serial, pcap, escuta de áudio) têm roteiros manuais e ficam BLOCKED até o dono executá-los e registrar o resultado.
- O G3 depende de um parecer de agente revisor independente, aprovado pelo dono.
- A recomendação de release (alpha, beta ou stable candidate) não é inflada: segue o maior Gate atingido, com as ressalvas escritas.
- Uma Limitação aceita (ADR 0006) é FAIL aceito pelo dono: continua FAIL nos relatórios e nunca vira PASS. Uma Divergência aprovada (ADR 0006) é PASS quando o teste do comportamento do port passa.

## G0: Inventário

| Critério | Resultado | Evidência |
|---|---|---|
| 100% das issues do AppleWin (abertas e fechadas) coletadas, com contagem e data | **PASS** | 1.115 issues (358 abertas, 757 fechadas) e 401 PRs, coleta de 2026-10-21T00:47Z no HEAD `a8f0d1d` ([issue-audit.md](issue-audit.md)). Os itens são de um instantâneo; nada mudou desde então no SHA fixado |
| Aplicabilidade classificada para cada issue | **PASS** | 1.115 de 1.115 linhas com aplicabilidade, categoria e `mapeamento_rust` ([applewin-issue-matrix.csv](applewin-issue-matrix.csv)). Cada linha tem resultado individual (849) ou nota do auditor (266 sem prioridade, NOT_APPLICABLE) |
| Cada issue tem estado e evidência ou justificativa; nenhuma fechada em PASS sem teste | **PASS** | `apply_results.py --check`: "Matriz em dia"; PASS 486 (147 `executavel`, 199 `cpp-transcrito`, 5 `especificacao`, 7 `fixture-real`, 128 `regressao`), FAIL 6, BLOCKED 134, NOT_APPLICABLE 489, **UNVERIFIED 0**. Verificado por consulta à matriz: 0 linhas sem `teste`/`evidencia`/`justificativa_na`, 0 PASS sem `teste`, 0 NOT_APPLICABLE sem justificativa, 0 FAIL ou BLOCKED sem evidência. Resultados em [results/](results/) (`p23-*.csv` da onda final) |
| Divergências identificadas registradas, nenhuma omitida | **PASS** | [gaps-and-known-limitations.md](gaps-and-known-limitations.md) (inclusive as seções "Divergências aprovadas" e "Limitações aceitas", ADR 0006), [capability-matrix.md](capability-matrix.md), os `audit/*.md` |
| Amostra estratificada validada por auditor independente; toda diferença provoca o reexame da classe | **PASS** (triagem); reexame da classe "PASS só em teste Rust" feito | [issue-audit-review.md](issue-audit-review.md): 465 issues, semente `20261020`; 168 correções; 334 linhas reexaminadas por classe. O [parecer independente](parecer-independente.md) amostrou 80 dos 849 resultados (semente `20261021`): 92,5 % de concordância, mas 87,2 % na classe PASS; a classe foi reexaminada ([relatório final](relatorio-final.md), seção 12): 164 PASS de `p23-disco`, `p23-config-outras`, `p23-nucleo` e `p23-entrada` mais 52 de outros arquivos; 1 PASS virou BLOCKED (#1490), 1 teste foi reforçado (#452). **Ressalva**: os outros 769 resultados não têm amostra independente; isso é o CA-07 da HU-0001 e mantém a #15 em BLOCKED |
| [baseline.md](baseline.md) com todos os SHAs congelados | **PASS** | quatro SHAs completos, data, toolchain |

G0 = **PASS** pelo §18 ("100% coletadas, aplicabilidade classificada e evidência ou justificativa para cada caso; divergências não omitidas"). Vale como autoavaliação até a revisão independente da onda final. Antes: FAIL, porque 509 linhas UNVERIFIED não tinham justificativa. Nenhuma justificativa foi genérica; cada uma cita teste, código do AppleWin ou causa do bloqueio.

## G1: Alpha técnica (projeto inteiro: Linux, Windows e macOS)

| Critério | Resultado | Evidência |
|---|---|---|
| Smoke tests e CI verdes nos três SO | **BLOCKED** | Linux nativo: PASS (`smoke-linux.sh`, pacote extraído; 20/10/2026 e de novo no pacote `0.2.0-rc.1`). Windows via Wine 10.0: PASS (PNG idêntico ao do Linux em 5 Modelos e no DOS 3.3; 20/10/2026), mas **Wine não é Windows**. **Windows nativo: BLOCKED (execução do dono no Windows 11 em andamento, roteiro W1 a W10). macOS: BLOCKED.** CI: nunca executada nesta fase (cota esgotada, só com autorização do dono); `release.yml` nunca rodou ([relatorio-plataformas.md](platform/relatorio-plataformas.md)) |
| Boot básico dos Modelos suportados, com resultado por Modelo | **PASS** | Traces idênticos ao fork do audetto (núcleo `3e8054b`, 22 commits antes do `a8f0d1d`) em ][, ][+, //e e //e Enhanced; boot headless em `apple2`, `apple2p`, `apple2jp`, `apple2e`, `apple2ee` no Linux e no Wine; estados reais de Pravets 82/8M/8A, Base64A, TK3000 //e e J-Plus batem em 100.000 ciclos. Vídeo dos clones: sem comparação de framebuffer |
| CPU, memória, vídeo, disco e save-state com regressões P0 automatizadas e em PASS | **PASS** | As 30 issues P0: 23 PASS, 7 BLOCKED (mecanismos sintéticos passam), 0 FAIL. Os P0 achados na auditoria têm regressão automatizada em PASS (AUD-C-010, AUD-C-060, AUD-C-001/003, AUD-C-035, AUD-C-022/033). Save-state real: 88 PASS, 0 FAIL |
| `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace --release` verdes | **PASS** | Em 2026-10-21, Linux: `cargo fmt --all -- --check` sem diferenças; `cargo clippy --workspace --all-targets -- -D warnings` sem avisos; `cargo test --workspace --release --no-fail-fast`: 1.585 passam, 0 falham, 15 ignorados. Conformidade: 774 PASS, 5 FAIL, 0 BLOCKED em 779 casos (os 5 FAIL são as Limitações aceitas de PCM/SSI) |
| Falhas conhecidas destacadas nas notas da release | **PASS** | Seções "Limitações conhecidas", "Limitações aceitas" e "Divergências aprovadas" do CHANGELOG da branch `release/v0.2.0-rc.1`, com link para o [relatório final](relatorio-final.md) (a release não foi publicada) |

G1 = **BLOCKED**: nenhum critério está em FAIL, mas o primeiro depende de Windows nativo, macOS e CI, que não foram executados (a CI é disparada pelo dono; o teste nativo no Windows está em andamento). Destravar: CI (decisão 6 abaixo) e o teste de Windows e macOS pelo dono (decisão 7).

## G2: Beta funcional (projeto inteiro)

| Critério | Resultado | Evidência |
|---|---|---|
| Zero FAIL P0 aplicável | **PASS** | 0 das 30 P0 em FAIL na matriz. Antes: FAIL, por `Saturn64`, `Saturn128` e `SmartPort-firmware`; hoje os três batem no trace de 100.000 ciclos (os Saturn por correção do Oráculo, o SmartPort por correção do firmware do DIB, 132.1) |
| Zero UNVERIFIED P0 sem bloqueio formal aceito | **BLOCKED** | 0 P0 UNVERIFIED. Mas 7 P0 estão BLOCKED (#4, #70, #400, #422, #564, #640, #1126) e nenhum bloqueio foi aceito formalmente pelo dono; aguardam o aceite ou as fixtures (decisão 5) |
| Fixtures reais críticas (AppleWin-Test) aprovadas | **PASS** (críticas); 27 BLOCKED | 88 de 115 estados reais PASS e **0 FAIL**, entre eles os da HU-0002 (`Saturn64`, `Saturn128`, `LC-RamWrite-Test`, `SSI263-4x_phonemes_playing_continuously`, `HDD-32MB`, `AS-S1_Turn the disk over(PAL)`). Os 27 BLOCKED têm causa (18 por imagem ausente do corpus, 3 SSC, 2 hora real, 2 Shift Key Mod, 1 Z80, 1 Uthernet). WOZ, DSK, HDD e `.do.gz` reais bootam idênticos ao Oráculo (disco: 56 casos PASS) |
| Matriz dos componentes essenciais por Modelo completa | **FAIL** | [capability-matrix.md](capability-matrix.md) existe e cobre os componentes, mas o vídeo dos clones (Pravets, TK3000, Base64A, J-Plus), VidHD em ][/][+ e SHR em PAL seguem sem comparação, e Windows/macOS BLOCKED |
| Testes de áudio, entrada e interface com evidência nos três SO | **FAIL** (e BLOCKED) | Áudio: eventos PASS no Linux; **PCM em FAIL aceito pelo dono (5 casos, Limitação aceita)**; audição humana BLOCKED. Entrada e interface: testes Rust e Xvfb no Linux e Wine; Windows nativo, macOS e hardware (gamepad, teclado regional) BLOCKED |
| Recuperação de estado e de discos validada | **PASS** | Gravação atômica, queda simulada, flush em `ApplySettings`/`LoadState`/SIGTERM, fuzz de WOZ/NIB/DSK/YAML/2MG/gz/zip, limites de tamanho: `atomic_write.rs`, `crash_safety.rs`, `audit_persistence.rs`, `fuzz_parsers.rs`, `fuzz_loader.rs`. Queda real de energia e HDD de mais de 2 MiB: ressalvas no [item 8 do relatório](relatorio-final.md) |

G2 = **FAIL**, com parte BLOCKED. Os que seguram são a matriz incompleta, a evidência nos 3 SO (e o PCM, Limitação aceita, que não conta como PASS) e os 7 P0 sem aceite.

## G3: Stable candidata (projeto inteiro)

| Critério | Resultado | Evidência |
|---|---|---|
| Zero FAIL P0 e P1 no escopo declarado | **zero FAIL P0/P1 não aceito; 5 FAIL P1 aceitos pelo dono como Limitação aceita (ADR 0006)** | Nenhum FAIL P0 nem P1 em issue: a **#241** passa (`de3ac3b`). Os 5 casos FAIL de PCM/SSI são P1 no harness (issues #7, #27, #28 do backlog) e Limitação aceita (ADR 0006): FAIL aceito pelo dono, não PASS |
| Testes entre plataformas repetidos a partir do pacote instalado | **BLOCKED** | Só o pacote Linux foi extraído e testado; Windows (execução do dono em andamento) e macOS sem pacote testado; nada repetido |
| Nenhuma perda de dados nos cenários cobertos | **PASS** | Cenários cobertos em [relatorio-final.md](relatorio-final.md), item 8. Fora do coberto: queda real de energia, HDD de mais de 2 MiB (atualizado no lugar), duas instâncias na mesma imagem (UNVERIFIED) |
| Parecer independente de agente revisor, aprovado pelo dono | **BLOCKED** | O parecer foi entregue em 21/10/2026 (nota 77, alpha técnica) e respondido na seção 12 do relatório final; falta a aprovação do dono |
| Problemas P2 e P3 publicados e escopo explícito | **PASS** | P2 e P3: 333 PASS, 6 FAIL, 53 BLOCKED, 211 NOT_APPLICABLE, 0 UNVERIFIED, em [gaps-and-known-limitations.md](gaps-and-known-limitations.md), no CHANGELOG e no manual 12 |

G3 = **BLOCKED** no projeto inteiro: o único critério com FAIL é o dos 5 casos de PCM/SSI, aceito pelo dono (zero FAIL P0/P1 não aceito; 5 FAIL P1 aceitos pelo dono como Limitação aceita (ADR 0006)); faltam o pacote testado em Windows e macOS e a aprovação do parecer independente, e o G2 não foi atingido.

## Rubrica Verônica

Nota de 0 a 100, baseada em evidências. Autoavaliação feita por quem consolidou a entrega; a revisão independente vem depois e pode alterar qualquer linha. A subida de 81 para 84 vem de: decisões do dono tomadas e registradas (ADR 0006), o último FAIL P1 em issue corrigido (#241) e 6 FAIL do depurador corrigidos. Nada subiu por Windows, macOS, CI, hardware ou pelos 7 P0, que seguem sem execução.

| Dimensão | Pontos | Antes | Nota | Justificativa |
|---|---:|---:|---:|---|
| Especificação e critérios de aceite | 25 | 22 | 23 | Plano e backlog importável (13 épicos, 30 histórias e 36 histórias de issues abertas); ADR 0004, 0005 e 0006; políticas de comparação registradas antes das execuções ([test-oracles.md](test-oracles.md)); Gates definidos; as 1.115 issues com estado e evidência (0 UNVERIFIED); as divergências a favor do usuário e os 5 casos de PCM/SSI deixaram de estar sem decisão. Faltam: revisão independente da onda final, critérios de aceite de interface sem Oráculo de janela, aceite dos 7 P0 e a decisão sobre assinatura |
| Corretude e evidência | 25 | 20 | 21 | Evidência forte contra o Oráculo (fork do audetto, núcleo `3e8054b`): 10,22 milhões de vetores de CPU, 7 traces, 1.573 telas (inclusive o Color Monitor PAL da #241), 103 milhões de instruções de disco, 88 save-states reais sem nenhum FAIL, eventos de áudio idênticos, 774 de 779 casos de conformidade. Descontos: 5 casos de PCM/SSI em FAIL (aceitos pelo dono, mas não PASS), 6 FAIL na matriz (3 do depurador por paridade mantida, 2 adiados, #781), 134 BLOCKED, Windows/macOS/hardware sem execução, Oráculo limitado (fork 22 commits antes do `a8f0d1d`, sem entrada, uma carta de áudio), e só 147 dos 486 PASS por issue (30,2 %) comparam com o AppleWin rodando. As Divergências aprovadas são comportamento a favor do usuário, sem prova de hardware |
| Testes e regressão | 15 | 12 | 12 | 1.585 testes e 779 casos de conformidade executados (0 testes falhando); regressão por issue (849 resultados aplicados, cada PASS com tipo de oráculo) e fuzz de analisadores; o runner compara `trace_rows_sha256`. Descontos: CI nunca executada, sem repetição entre plataformas, fuzz de longa duração e hardware não executados, vetores 65x02/Z80 não refeitos nesta rodada |
| Arquitetura e manutenção | 10 | 8 | 8 | Núcleo headless e determinístico (ADR 0001), 18 crates, zero alocações por passo e por quadro verificado por teste, contratos de harness e Oráculo documentados, patches do Oráculo versionados. Descontos: `CardCtx::floating_bus` eager; dado de resultado obsoleto achado na rodada anterior (precedência FAIL sobre PASS) |
| Segurança, dados e resiliência | 10 | 8 | 8 | Gravação atômica, fuzz, limites de tamanho e descompressão, handlers de sinal, erros de E/S visíveis. Descontos: HDD grande gravado no lugar, AUD-C-037 aberto, pacotes sem assinatura, queda real não testada |
| Higiene da entrega | 5 | 4 | 4 | Commits em português com `[skip ci]`, SBOM, SHA256SUMS e licenças de terceiros, nenhuma CI acionada, nenhuma tag, nenhum push. Desconto: `release.yml` nunca executado |
| Eficiência/retrabalho | 10 | 7 | 8 | A #241, que tinha duas hipóteses descartadas, fechou; as seis correções do depurador entraram em um passe. Segue aberto o PCM, agora aceito em vez de resolvido |
| **Total** | **100** | **81** | **84** | |

Leitura da nota:

- O aceite de uma história exige nota **igual ou maior que 96**, todos os Critérios de aceite materiais em PASS, nenhum bloqueador crítico e revisão por quem **não** implementou a alteração. Esta nota é do conjunto, não de uma história, e está abaixo de 96: o trabalho **não** está aceito como completo.
- Nota de 90 a 95 pede uma correção focada; **89 ou menos**, ou repetição de tentativas sem hipótese nova, interrompe o trabalho e leva a replanejar. O plano está em [backlog.md](backlog.md) e no item 5 do [relatório final](relatorio-final.md).

## Estado das histórias HU (#15 a #44)

Estado pelo critério do `tools/conformance/update_hu_issues.py` (PASS só com todos os CA em PASS), ajustado à mão depois da #241 e da #1396. **As labels `estado:` no GitHub não foram atualizadas nesta entrega** (nenhuma chamada à API); a última aplicação foi a da rodada anterior (14 PASS, 5 FAIL, 11 BLOCKED). Estado esperado agora: **16 PASS, 3 FAIL, 11 BLOCKED**. Nenhuma foi fechada.

| Estado | Histórias |
|---|---|
| PASS | HU-0101, 0102, 0103, 0201 (#241), 0202, 0301, 0303, 0402, 0602, 0702 (#1396), 1101, 1201, 1202, 1203, 1204, 1205 |
| FAIL | HU-0501 e 0502 (PCM/SSI: Limitação aceita, continua FAIL), 0801 (depurador: #713, #1145, #1377 por paridade, #1426 e #1431 adiadas) |
| BLOCKED | HU-0001 (CA-07), 0002 (CI/JUnit), 0302, 0401, 0601, 0701, 0703, 0802, 0901, 0902, 1001 |

## Release final

**Recomendação objetiva (§18): alpha técnica; o rótulo de release candidate para Linux é do dono e vai como pré-lançamento.** Não há FAIL P0 nem P1 em issue; o que separa o Linux de beta é a matriz de componentes incompleta, o aceite dos 7 P0, a CI e o hardware sem execução; de stable candidate, o parecer independente. A versão `0.2.0-rc.1` está preparada na branch local `release/v0.2.0-rc.1` (versão do workspace, CHANGELOG com "release candidate para Linux; Windows e macOS sem verificação nativa", "Limitações aceitas" e "Divergências aprovadas", `Info.plist` com `CFBundleVersion` numérico, texto da release que marca os pacotes de Windows e macOS como não verificados). O `release.yml` trata o sufixo `-rc.1` como pré-lançamento (`prerelease: true`). A CI, o pull request `[release]`, a tag e o push só são feitos pelo dono.

O que sustenta a entrega: G0 PASS (com a ressalva do parecer); G1 no Linux só espera a CI; nenhuma regressão P0 conhecida; zero FAIL P0/P1 em issue (e 5 FAIL P1 do harness aceitos pelo dono); o pacote Linux foi gerado e testado. O que impede chamá-la de beta ou stable candidate pelo §18, **qual critério falta**:

- G2 (Linux): matriz incompleta, 7 P0 BLOCKED sem aceite, hardware sem execução.
- G3 (Linux): a aprovação do parecer independente pelo dono (e o G2).
- Windows nativo (execução do dono em andamento) e macOS: pendentes; não contam como FAIL no escopo Linux.

Artefatos do pacote Linux (em `target/release-candidate/`, fora do repositório): os hashes estão no [relatório final](relatorio-final.md), item 6.

## Decisões do dono

Em 21/10/2026 o dono decidiu as de número 1 a 4; a execução da CI e o PR `[release]` ficam com ele. Detalhe no [relatório final](relatorio-final.md), item 11.

| # | Decisão | Resultado | Estado |
|---|---|---|---|
| 1 | Os 10 FAIL do depurador e a #458 | Corrigir #114, #245, #458, #466, #468, #1381 (Divergências aprovadas); manter a paridade em #713, #1145, #1377; adiar #1426 e #1431 | **Decidida** (ADR 0006) |
| 2 | #1396: Alt+F2 | Divergência aprovada (o port mantém o Open Apple) | **Decidida** (ADR 0006) |
| 3 | #916, #855, tela cheia (#280, #358, #1374, #117, #932, #1448) | Divergências aprovadas | **Decidida** (ADR 0006) |
| 4 | As 5 divergências de PCM/SSI | Limitação aceita (continuam FAIL nos relatórios) | **Decidida** (ADR 0006) |
| 5 | Os 7 P0 BLOCKED | Recomendação: aceite de #4, #564 e #640; fixtures de #70, #400, #422, #1126 se houver origem legal | **Pendente** |
| 6 | Executar `ci.yml` e `conformance.yml` nos 3 SO | O dono dispara | **Pendente (dono)** |
| 7 | Roteiros de Windows e macOS | Windows 11 nativo em execução (W1 a W10, branch `test/windows-nativo`); macOS sem execução | **Em andamento** |
| 8 | Assinatura e notarização | Pré-lançamento sem assinar, com aviso no CHANGELOG; decidir antes do beta | **Pendente** |
| 9 | Abrir o PR `[release]` da v0.2.0-rc.1 | O dono dispara o PR; recomendação: depois da CI no Linux e da revisão independente | **Pendente (dono)** |
