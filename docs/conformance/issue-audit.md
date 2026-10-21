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
| runtime | 475 | 42.6 |
| ui | 151 | 13.5 |
| feature-futura | 148 | 13.3 |
| debugger | 90 | 8.1 |
| formato | 85 | 7.6 |
| pergunta-suporte | 48 | 4.3 |
| plataforma-windows | 44 | 3.9 |
| build | 33 | 3.0 |
| documentacao | 30 | 2.7 |
| invalida | 8 | 0.7 |
| duplicada | 3 | 0.3 |

### Categoria

| Valor | Quantidade | % |
|---|---:|---:|
| disco | 160 | 14.3 |
| video | 137 | 12.3 |
| debugger | 135 | 12.1 |
| outra | 128 | 11.5 |
| ui | 83 | 7.4 |
| config | 57 | 5.1 |
| cpu | 56 | 5.0 |
| teclado | 53 | 4.8 |
| mockingboard | 48 | 4.3 |
| memoria | 44 | 3.9 |
| joystick | 41 | 3.7 |
| audio | 32 | 2.9 |
| savestate | 32 | 2.9 |
| hdd | 28 | 2.5 |
| serial | 18 | 1.6 |
| mouse | 16 | 1.4 |
| rede | 12 | 1.1 |
| impressora | 9 | 0.8 |
| ssi263 | 9 | 0.8 |
| relogio | 8 | 0.7 |
| z80 | 8 | 0.7 |
| vidhd | 1 | 0.1 |

Observação: uma issue com vários modelos conta em cada um, então a soma pode exceder o total.

### Modelo

| Valor | Quantidade | % |
|---|---:|---:|
| todos | 1004 | 90.0 |
| apple2e-enhanced | 59 | 5.3 |
| apple2e | 53 | 4.8 |
| apple2plus | 29 | 2.6 |
| apple2 | 26 | 2.3 |
| apple2jplus | 9 | 0.8 |
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
| UNVERIFIED | 979 | 87.8 |
| NOT_APPLICABLE | 136 | 12.2 |

## Taxas

Taxa geral: PASS 0 / aplicáveis 979 = 0.0%. Aplicável = estado diferente de NOT_APPLICABLE. Issues ainda não classificadas contam como aplicáveis.

### Por modelo

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| todos | 1004 | 871 | 0 | 0 | 871 | 0.0% |
| apple2e-enhanced | 59 | 56 | 0 | 0 | 56 | 0.0% |
| apple2e | 53 | 52 | 0 | 0 | 52 | 0.0% |
| apple2plus | 29 | 29 | 0 | 0 | 29 | 0.0% |
| apple2 | 26 | 26 | 0 | 0 | 26 | 0.0% |
| apple2jplus | 9 | 9 | 0 | 0 | 9 | 0.0% |
| pravets82 | 7 | 7 | 0 | 0 | 7 | 0.0% |
| pravets8m | 7 | 7 | 0 | 0 | 7 | 0.0% |
| apple2c | 6 | 6 | 0 | 0 | 6 | 0.0% |
| base64a | 3 | 3 | 0 | 0 | 3 | 0.0% |
| pravets8a | 2 | 2 | 0 | 0 | 2 | 0.0% |
| apple2e-platinum | 1 | 1 | 0 | 0 | 1 | 0.0% |
| apple3 | 1 | 1 | 0 | 0 | 1 | 0.0% |
| laser128 | 1 | 1 | 0 | 0 | 1 | 0.0% |
| pravetz8c | 1 | 1 | 0 | 0 | 1 | 0.0% |
| tk3000 | 1 | 1 | 0 | 0 | 1 | 0.0% |

### Por categoria

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| disco | 160 | 155 | 0 | 0 | 155 | 0.0% |
| video | 137 | 135 | 0 | 0 | 135 | 0.0% |
| debugger | 135 | 130 | 0 | 0 | 130 | 0.0% |
| outra | 128 | 54 | 0 | 0 | 54 | 0.0% |
| ui | 83 | 63 | 0 | 0 | 63 | 0.0% |
| config | 57 | 47 | 0 | 0 | 47 | 0.0% |
| cpu | 56 | 54 | 0 | 0 | 54 | 0.0% |
| teclado | 53 | 52 | 0 | 0 | 52 | 0.0% |
| mockingboard | 48 | 47 | 0 | 0 | 47 | 0.0% |
| memoria | 44 | 44 | 0 | 0 | 44 | 0.0% |
| joystick | 41 | 39 | 0 | 0 | 39 | 0.0% |
| audio | 32 | 28 | 0 | 0 | 28 | 0.0% |
| savestate | 32 | 30 | 0 | 0 | 30 | 0.0% |
| hdd | 28 | 24 | 0 | 0 | 24 | 0.0% |
| serial | 18 | 17 | 0 | 0 | 17 | 0.0% |
| mouse | 16 | 16 | 0 | 0 | 16 | 0.0% |
| rede | 12 | 10 | 0 | 0 | 10 | 0.0% |
| impressora | 9 | 8 | 0 | 0 | 8 | 0.0% |
| ssi263 | 9 | 9 | 0 | 0 | 9 | 0.0% |
| relogio | 8 | 8 | 0 | 0 | 8 | 0.0% |
| z80 | 8 | 8 | 0 | 0 | 8 | 0.0% |
| vidhd | 1 | 1 | 0 | 0 | 1 | 0.0% |

### Por aplicabilidade

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| runtime | 475 | 475 | 0 | 0 | 475 | 0.0% |
| ui | 151 | 151 | 0 | 0 | 151 | 0.0% |
| feature-futura | 148 | 148 | 0 | 0 | 148 | 0.0% |
| debugger | 90 | 90 | 0 | 0 | 90 | 0.0% |
| formato | 85 | 85 | 0 | 0 | 85 | 0.0% |
| pergunta-suporte | 48 | 0 | 0 | 0 | 0 | n/a |
| plataforma-windows | 44 | 0 | 0 | 0 | 0 | n/a |
| build | 33 | 0 | 0 | 0 | 0 | n/a |
| documentacao | 30 | 30 | 0 | 0 | 30 | 0.0% |
| invalida | 8 | 0 | 0 | 0 | 0 | n/a |
| duplicada | 3 | 0 | 0 | 0 | 0 | n/a |
