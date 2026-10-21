# Prontidão para release

Este documento define os Gates de prontidão (G0 a G3) e a rubrica Verônica de nota. Os critérios vêm do §18 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). Um Gate só é atingido quando **todos** os seus critérios objetivos estão em PASS. Nota alta não substitui um Gate.

## Estado atual

Data: 2026-10-20. Nenhum Gate foi avaliado.

| Gate | Nome | Estado |
|---|---|---|
| G0 | Inventário | não avaliado |
| G1 | Alpha técnica | não avaliado |
| G2 | Beta funcional | não avaliado |
| G3 | Stable candidata | não avaliado |

Quando forem avaliados, o estado de cada Gate será PASS, FAIL ou BLOCKED, com o relatório que o sustenta. Um Gate com algum critério BLOCKED é BLOCKED, não PASS.

## Regras de evidência

- A evidência vem do Linux local e do Wine. Windows nativo e macOS ficam BLOCKED até o dono autorizar uma execução de CI (a CI é só manual, porque a cota está esgotada).
- Testes que exigem hardware físico (gamepad, serial, pcap, escuta de áudio) têm roteiros manuais e ficam BLOCKED até o dono executá-los e registrar o resultado.
- O G3 depende de um parecer de agente revisor independente, aprovado pelo dono.
- A recomendação de release (alpha, beta ou stable candidate) não é inflada: segue o maior Gate atingido.

## G0: Inventário

- [ ] 100% das issues do AppleWin (abertas e fechadas) coletadas, com a contagem e a data da coleta.
- [ ] Aplicabilidade classificada para cada issue (runtime, plataforma Windows, interface, periférico, documentação, feature futura).
- [ ] Cada issue tem evidência ou justificativa; nenhuma issue fechada está em PASS sem reprodução ou teste.
- [ ] Divergências identificadas estão registradas, nenhuma omitida (veja [gaps-and-known-limitations.md](gaps-and-known-limitations.md)).
- [ ] Amostra estratificada validada por auditor independente; toda diferença na amostra provoca o reexame da classe inteira.
- [ ] [baseline.md](baseline.md) com todos os SHAs congelados.

## G1: Alpha técnica

- [ ] Smoke tests e CI verdes nos três sistemas operacionais (Linux local e Wine; Windows nativo e macOS dependem de autorização do dono).
- [ ] Boot básico dos Modelos suportados, com resultado por Modelo.
- [ ] CPU, memória, vídeo, disco e save-state com regressões P0 automatizadas e em PASS.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace --release` verdes.
- [ ] Falhas conhecidas destacadas nas notas da release.

## G2: Beta funcional

- [ ] Zero FAIL P0 aplicável.
- [ ] Zero UNVERIFIED P0 sem bloqueio formal aceito pelo dono.
- [ ] Fixtures reais críticas (AppleWin-Test) aprovadas.
- [ ] Matriz de componentes essenciais por Modelo completa.
- [ ] Testes de áudio, entrada e interface com evidência nos três sistemas (os que exigem hardware físico dependem do roteiro manual).
- [ ] Recuperação de estado e de discos validada.

## G3: Stable candidata

- [ ] Zero FAIL P0 e P1 no escopo declarado.
- [ ] Testes entre plataformas repetidos a partir do pacote instalado.
- [ ] Nenhuma perda de dados nos cenários cobertos.
- [ ] Parecer independente de agente revisor, aprovado pelo dono.
- [ ] Problemas P2 e P3 publicados e escopo explícito.

## Rubrica Verônica

Nota de 0 a 100 por história, baseada em evidências.

| Dimensão | Pontos |
|---|---:|
| Especificação e critérios de aceite | 25 |
| Corretude e evidência | 25 |
| Testes e regressão | 15 |
| Arquitetura e manutenção | 10 |
| Segurança, dados e resiliência | 10 |
| Higiene da entrega | 5 |
| Eficiência e retrabalho | 10 |
| **Total** | **100** |

Aceite de uma história exige, ao mesmo tempo:

- nota **igual ou maior que 96**;
- todos os Critérios de aceite materiais em PASS;
- nenhum bloqueador crítico;
- revisão por pessoa ou agente que **não** implementou a alteração.

Nota de 90 a 95 pede uma correção focada e nova revisão. Nota de 89 ou menos, ou repetição de tentativas sem hipótese nova, interrompe o trabalho e leva a replanejar.

## Notas por história

Nenhuma história foi avaliada ainda.

## Release final

A versão passa a 0.2.0-alpha ou 0.2.0-beta, conforme o parecer. O pull request `[release]` só é aberto por ordem do dono.
