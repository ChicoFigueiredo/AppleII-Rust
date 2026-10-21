# Auditoria de issues upstream do AppleWin

Gerado por `tools/conformance/summarize_issue_matrix.py` a partir de `docs/conformance/applewin-issue-matrix.csv`. Não editar à mão.

## Coleta

- Data da coleta (UTC): 2026-10-21T00:47:54Z
- SHA do upstream (HEAD de AppleWin/AppleWin): `a8f0d1d40f551ad9e010522c29c2891e33c88275`
- Itens coletados: 1516 (1115 issues e 401 PRs)
- Issues: 358 abertas e 757 fechadas
- PRs: 11 abertos, 324 mesclados, 66 fechados sem merge
- Issues fechadas com commit de correção identificado: 445 (141 por evento `closed`, 304 por `referenced`)
- Issues com PR vinculado (heurístico): 16

## Contagens (total de issues na matriz: 1115)

### Estado upstream

| Valor | Quantidade | % |
|---|---:|---:|
| closed | 757 | 67.9 |
| open | 358 | 32.1 |

### Aplicabilidade

| Valor | Quantidade | % |
|---|---:|---:|
| runtime | 388 | 34.8 |
| feature-futura | 146 | 13.1 |
| ui | 122 | 10.9 |
| pergunta-suporte | 118 | 10.6 |
| debugger | 86 | 7.7 |
| formato | 77 | 6.9 |
| plataforma-windows | 61 | 5.5 |
| build | 33 | 3.0 |
| documentacao | 30 | 2.7 |
| duplicada | 28 | 2.5 |
| invalida | 26 | 2.3 |

### Categoria

| Valor | Quantidade | % |
|---|---:|---:|
| disco | 159 | 14.3 |
| video | 137 | 12.3 |
| debugger | 136 | 12.2 |
| outra | 120 | 10.8 |
| ui | 83 | 7.4 |
| config | 58 | 5.2 |
| cpu | 55 | 4.9 |
| teclado | 53 | 4.8 |
| memoria | 48 | 4.3 |
| mockingboard | 47 | 4.2 |
| joystick | 41 | 3.7 |
| audio | 32 | 2.9 |
| savestate | 32 | 2.9 |
| hdd | 28 | 2.5 |
| serial | 20 | 1.8 |
| mouse | 16 | 1.4 |
| rede | 12 | 1.1 |
| relogio | 10 | 0.9 |
| ssi263 | 10 | 0.9 |
| impressora | 9 | 0.8 |
| z80 | 8 | 0.7 |
| vidhd | 1 | 0.1 |

Observação: uma issue com vários modelos conta em cada um, então a soma pode exceder o total.

### Modelo

| Valor | Quantidade | % |
|---|---:|---:|
| todos | 1004 | 90.0 |
| apple2e | 59 | 5.3 |
| apple2e-enhanced | 59 | 5.3 |
| apple2plus | 29 | 2.6 |
| apple2 | 26 | 2.3 |
| apple2jplus | 11 | 1.0 |
| pravets82 | 7 | 0.6 |
| pravets8m | 7 | 0.6 |
| apple2c | 6 | 0.5 |
| base64a | 3 | 0.3 |
| pravets8a | 2 | 0.2 |
| apple2e-platinum | 1 | 0.1 |
| apple3 | 1 | 0.1 |
| laser128 | 1 | 0.1 |
| pravetz8c | 1 | 0.1 |
| tk3000 | 1 | 0.1 |

### Estado de verificação

| Valor | Quantidade | % |
|---|---:|---:|
| NOT_APPLICABLE | 489 | 43.9 |
| PASS | 486 | 43.6 |
| BLOCKED | 134 | 12.0 |
| FAIL | 6 | 0.5 |

## Taxas

Taxa geral: PASS 486 / aplicáveis 626 = 77.6%. Aplicável = estado diferente de NOT_APPLICABLE. Issues ainda não classificadas contam como aplicáveis.

### PASS por tipo de oráculo

Só o tipo `executavel` é "concorda com o AppleWin rodando" (README de conformidade). Os demais são evidência de natureza diferente e não se somam a ele sem dizer o tipo.

| Tipo | Significado | PASS | % dos PASS |
|---|---|---:|---:|
| executavel | comparação com o AppleWin rodando (fork do audetto ou Wine) | 147 | 30.2 |
| cpp-transcrito | teste Rust que reproduz o C++ do a8f0d1d citando arquivo:linha | 199 | 40.9 |
| especificacao | hardware, a2audit ou vetores públicos | 5 | 1.0 |
| fixture-real | fixture real, sem comparação com o AppleWin | 7 | 1.4 |
| regressao | teste do próprio port, sem fonte externa (inclui Divergência aprovada) | 128 | 26.3 |
| **total** | | **486** | |

| Prioridade | PASS | executavel | cpp-transcrito | especificacao | fixture-real | regressao |
|---|---:|---:|---:|---:|---:|---:|
| P0 | 23 | 8 | 5 | 2 | 0 | 8 |
| P1 | 130 | 86 | 28 | 2 | 1 | 13 |
| P2 | 189 | 47 | 94 | 0 | 0 | 48 |
| P3 | 144 | 6 | 72 | 1 | 6 | 59 |

### Por modelo

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| todos | 1004 | 545 | 413 | 4 | 0 | 75.8% |
| apple2e | 59 | 50 | 44 | 1 | 0 | 88.0% |
| apple2e-enhanced | 59 | 45 | 40 | 0 | 0 | 88.9% |
| apple2plus | 29 | 23 | 21 | 2 | 0 | 91.3% |
| apple2 | 26 | 21 | 19 | 2 | 0 | 90.5% |
| apple2jplus | 11 | 9 | 9 | 0 | 0 | 100.0% |
| pravets82 | 7 | 6 | 6 | 0 | 0 | 100.0% |
| pravets8m | 7 | 6 | 6 | 0 | 0 | 100.0% |
| apple2c | 6 | 3 | 3 | 0 | 0 | 100.0% |
| base64a | 3 | 3 | 3 | 0 | 0 | 100.0% |
| pravets8a | 2 | 1 | 1 | 0 | 0 | 100.0% |
| apple2e-platinum | 1 | 0 | 0 | 0 | 0 | n/a |
| apple3 | 1 | 0 | 0 | 0 | 0 | n/a |
| laser128 | 1 | 0 | 0 | 0 | 0 | n/a |
| pravetz8c | 1 | 1 | 1 | 0 | 0 | 100.0% |
| tk3000 | 1 | 0 | 0 | 0 | 0 | n/a |

### Por categoria

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| disco | 159 | 111 | 57 | 0 | 0 | 51.4% |
| video | 137 | 75 | 59 | 0 | 0 | 78.7% |
| debugger | 136 | 86 | 78 | 3 | 0 | 90.7% |
| outra | 120 | 11 | 9 | 0 | 0 | 81.8% |
| ui | 83 | 29 | 29 | 0 | 0 | 100.0% |
| config | 58 | 34 | 33 | 0 | 0 | 97.1% |
| cpu | 55 | 38 | 29 | 0 | 0 | 76.3% |
| teclado | 53 | 25 | 22 | 0 | 0 | 88.0% |
| memoria | 48 | 40 | 32 | 2 | 0 | 80.0% |
| mockingboard | 47 | 41 | 29 | 0 | 0 | 70.7% |
| joystick | 41 | 24 | 23 | 0 | 0 | 95.8% |
| audio | 32 | 16 | 7 | 0 | 0 | 43.8% |
| savestate | 32 | 25 | 22 | 1 | 0 | 88.0% |
| hdd | 28 | 19 | 16 | 0 | 0 | 84.2% |
| serial | 20 | 14 | 10 | 0 | 0 | 71.4% |
| mouse | 16 | 9 | 9 | 0 | 0 | 100.0% |
| rede | 12 | 6 | 4 | 0 | 0 | 66.7% |
| relogio | 10 | 8 | 6 | 0 | 0 | 75.0% |
| ssi263 | 10 | 6 | 4 | 0 | 0 | 66.7% |
| impressora | 9 | 4 | 4 | 0 | 0 | 100.0% |
| z80 | 8 | 5 | 4 | 0 | 0 | 80.0% |
| vidhd | 1 | 0 | 0 | 0 | 0 | n/a |

### Por aplicabilidade

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| runtime | 388 | 339 | 245 | 0 | 0 | 72.3% |
| feature-futura | 146 | 46 | 45 | 0 | 0 | 97.8% |
| ui | 122 | 83 | 67 | 1 | 0 | 80.7% |
| pergunta-suporte | 118 | 0 | 0 | 0 | 0 | n/a |
| debugger | 86 | 78 | 67 | 5 | 0 | 85.9% |
| formato | 77 | 74 | 56 | 0 | 0 | 75.7% |
| plataforma-windows | 61 | 0 | 0 | 0 | 0 | n/a |
| build | 33 | 0 | 0 | 0 | 0 | n/a |
| documentacao | 30 | 6 | 6 | 0 | 0 | 100.0% |
| duplicada | 28 | 0 | 0 | 0 | 0 | n/a |
| invalida | 26 | 0 | 0 | 0 | 0 | n/a |
