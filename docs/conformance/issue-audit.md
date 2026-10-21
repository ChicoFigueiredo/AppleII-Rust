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
| UNVERIFIED | 407 | 36.5 |
| NOT_APPLICABLE | 343 | 30.8 |
| PASS | 255 | 22.9 |
| BLOCKED | 96 | 8.6 |
| FAIL | 14 | 1.3 |

## Taxas

Taxa geral: PASS 255 / aplicáveis 772 = 33.0%. Aplicável = estado diferente de NOT_APPLICABLE. Issues ainda não classificadas contam como aplicáveis.

### Por modelo

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| todos | 1004 | 680 | 215 | 12 | 362 | 31.6% |
| apple2e | 59 | 51 | 30 | 1 | 16 | 58.8% |
| apple2e-enhanced | 59 | 47 | 25 | 0 | 17 | 53.2% |
| apple2plus | 29 | 27 | 13 | 2 | 12 | 48.1% |
| apple2 | 26 | 24 | 12 | 2 | 10 | 50.0% |
| apple2jplus | 11 | 9 | 5 | 0 | 4 | 55.6% |
| pravets82 | 7 | 7 | 3 | 0 | 4 | 42.9% |
| pravets8m | 7 | 7 | 3 | 0 | 4 | 42.9% |
| apple2c | 6 | 4 | 1 | 0 | 3 | 25.0% |
| base64a | 3 | 3 | 0 | 0 | 3 | 0.0% |
| pravets8a | 2 | 2 | 1 | 0 | 1 | 50.0% |
| apple2e-platinum | 1 | 1 | 0 | 0 | 1 | 0.0% |
| apple3 | 1 | 1 | 0 | 0 | 1 | 0.0% |
| laser128 | 1 | 1 | 0 | 0 | 1 | 0.0% |
| pravetz8c | 1 | 1 | 0 | 0 | 1 | 0.0% |
| tk3000 | 1 | 1 | 0 | 0 | 1 | 0.0% |

### Por categoria

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| disco | 159 | 130 | 28 | 0 | 72 | 21.5% |
| video | 137 | 101 | 28 | 1 | 60 | 27.7% |
| debugger | 136 | 86 | 72 | 9 | 0 | 83.7% |
| outra | 120 | 35 | 1 | 0 | 33 | 2.9% |
| ui | 83 | 49 | 0 | 0 | 49 | 0.0% |
| config | 58 | 42 | 2 | 0 | 40 | 4.8% |
| cpu | 55 | 44 | 19 | 0 | 20 | 43.2% |
| teclado | 53 | 39 | 6 | 0 | 30 | 15.4% |
| memoria | 48 | 44 | 17 | 2 | 19 | 38.6% |
| mockingboard | 47 | 41 | 28 | 1 | 0 | 68.3% |
| joystick | 41 | 36 | 4 | 0 | 32 | 11.1% |
| audio | 32 | 16 | 7 | 0 | 0 | 43.8% |
| savestate | 32 | 29 | 9 | 1 | 17 | 31.0% |
| hdd | 28 | 21 | 3 | 0 | 17 | 14.3% |
| serial | 20 | 14 | 10 | 0 | 0 | 71.4% |
| mouse | 16 | 13 | 2 | 0 | 11 | 15.4% |
| rede | 12 | 6 | 4 | 0 | 0 | 66.7% |
| relogio | 10 | 8 | 6 | 0 | 0 | 75.0% |
| ssi263 | 10 | 6 | 4 | 0 | 0 | 66.7% |
| impressora | 9 | 4 | 4 | 0 | 0 | 100.0% |
| z80 | 8 | 7 | 1 | 0 | 6 | 14.3% |
| vidhd | 1 | 1 | 0 | 0 | 1 | 0.0% |

### Por aplicabilidade

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| runtime | 388 | 367 | 158 | 2 | 129 | 43.1% |
| feature-futura | 146 | 114 | 11 | 0 | 103 | 9.6% |
| ui | 122 | 119 | 2 | 1 | 115 | 1.7% |
| pergunta-suporte | 118 | 0 | 0 | 0 | 0 | n/a |
| debugger | 86 | 78 | 62 | 10 | 0 | 79.5% |
| formato | 77 | 77 | 21 | 1 | 44 | 27.3% |
| plataforma-windows | 61 | 0 | 0 | 0 | 0 | n/a |
| build | 33 | 0 | 0 | 0 | 0 | n/a |
| documentacao | 30 | 17 | 1 | 0 | 16 | 5.9% |
| duplicada | 28 | 0 | 0 | 0 | 0 | n/a |
| invalida | 26 | 0 | 0 | 0 | 0 | n/a |
