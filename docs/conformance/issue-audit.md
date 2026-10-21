# Auditoria de issues upstream do AppleWin

Gerado por `tools/conformance/summarize_issue_matrix.py` a partir de `docs/conformance/applewin-issue-matrix.csv`. Não editar à mão.

## Coleta

Metadados indisponíveis (`issues.json` ausente); rode `tools/conformance/fetch_upstream_issues.py`.

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
| UNVERIFIED | 849 | 76.1 |
| NOT_APPLICABLE | 266 | 23.9 |

## Taxas

Taxa geral: PASS 0 / aplicáveis 849 = 0.0%. Aplicável = estado diferente de NOT_APPLICABLE. Issues ainda não classificadas contam como aplicáveis.

### Por modelo

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| todos | 1004 | 755 | 0 | 0 | 755 | 0.0% |
| apple2e | 59 | 52 | 0 | 0 | 52 | 0.0% |
| apple2e-enhanced | 59 | 49 | 0 | 0 | 49 | 0.0% |
| apple2plus | 29 | 27 | 0 | 0 | 27 | 0.0% |
| apple2 | 26 | 24 | 0 | 0 | 24 | 0.0% |
| apple2jplus | 11 | 9 | 0 | 0 | 9 | 0.0% |
| pravets82 | 7 | 7 | 0 | 0 | 7 | 0.0% |
| pravets8m | 7 | 7 | 0 | 0 | 7 | 0.0% |
| apple2c | 6 | 4 | 0 | 0 | 4 | 0.0% |
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
| disco | 159 | 131 | 0 | 0 | 131 | 0.0% |
| video | 137 | 112 | 0 | 0 | 112 | 0.0% |
| debugger | 136 | 124 | 0 | 0 | 124 | 0.0% |
| outra | 120 | 36 | 0 | 0 | 36 | 0.0% |
| ui | 83 | 49 | 0 | 0 | 49 | 0.0% |
| config | 58 | 42 | 0 | 0 | 42 | 0.0% |
| cpu | 55 | 44 | 0 | 0 | 44 | 0.0% |
| teclado | 53 | 39 | 0 | 0 | 39 | 0.0% |
| memoria | 48 | 44 | 0 | 0 | 44 | 0.0% |
| mockingboard | 47 | 42 | 0 | 0 | 42 | 0.0% |
| joystick | 41 | 36 | 0 | 0 | 36 | 0.0% |
| audio | 32 | 25 | 0 | 0 | 25 | 0.0% |
| savestate | 32 | 29 | 0 | 0 | 29 | 0.0% |
| hdd | 28 | 21 | 0 | 0 | 21 | 0.0% |
| serial | 20 | 19 | 0 | 0 | 19 | 0.0% |
| mouse | 16 | 13 | 0 | 0 | 13 | 0.0% |
| rede | 12 | 10 | 0 | 0 | 10 | 0.0% |
| relogio | 10 | 10 | 0 | 0 | 10 | 0.0% |
| ssi263 | 10 | 9 | 0 | 0 | 9 | 0.0% |
| impressora | 9 | 6 | 0 | 0 | 6 | 0.0% |
| z80 | 8 | 7 | 0 | 0 | 7 | 0.0% |
| vidhd | 1 | 1 | 0 | 0 | 1 | 0.0% |

### Por aplicabilidade

| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |
|---|---:|---:|---:|---:|---:|---:|
| runtime | 388 | 388 | 0 | 0 | 388 | 0.0% |
| feature-futura | 146 | 146 | 0 | 0 | 146 | 0.0% |
| ui | 122 | 122 | 0 | 0 | 122 | 0.0% |
| pergunta-suporte | 118 | 0 | 0 | 0 | 0 | n/a |
| debugger | 86 | 86 | 0 | 0 | 86 | 0.0% |
| formato | 77 | 77 | 0 | 0 | 77 | 0.0% |
| plataforma-windows | 61 | 0 | 0 | 0 | 0 | n/a |
| build | 33 | 0 | 0 | 0 | 0 | n/a |
| documentacao | 30 | 30 | 0 | 0 | 30 | 0.0% |
| duplicada | 28 | 0 | 0 | 0 | 0 | n/a |
| invalida | 26 | 0 | 0 | 0 | 0 | n/a |
