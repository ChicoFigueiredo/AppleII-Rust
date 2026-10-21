# Revisão independente da matriz de issues do AppleWin (HU-0001)

Auditor: `auditor-sonnet`, independente dos 10 agentes Haiku de triagem. Escopo: `docs/conformance/applewin-issue-matrix.csv` (1.115 issues). Só leitura em `gh api` e em `issues.json`; nenhum teste foi executado, então nenhum estado `PASS` ou `FAIL` foi atribuído.

## 1. Semente e método

- Semente do sorteio: **`random.seed(20261020)`** (Python 3, `random.sample`), aplicada na ordem dos estratos (lote, aplicabilidade) ordenados.
- Lotes: faixas contíguas de 111 issues por posição na matriz (lote 01 = #1 a #111 ... lote 10 = #1331 a #1529, 116 issues).
- Parte obrigatória (337 issues, 100% conferidas): as 43 `P0`; as 136 `NOT_APPLICABLE`; as que têm rótulo upstream `invalid` (83), `duplicate` (29) ou `question` (121), sem contar as já incluídas.
- Parte aleatória (128 issues): das 778 restantes, sorteio estratificado por lote × aplicabilidade (57 estratos), proporcional ao tamanho do estrato e com mínimo de 1 por estrato.
- Total da amostra: **465 issues** (337 obrigatórias + 128 aleatórias).
- Conferência: título, rótulos, corpo (até 330 caracteres, mais o corpo completo quando preciso) e, para as 233 issues com rótulo invalid/duplicate/question e para todas as P0, os comentários via `gh api` (primeiro e últimos comentários). Verificado em cada linha: aplicabilidade, categoria, modelo, `mapeamento_rust` (existência do arquivo e adequação do módulo), estado e prioridade pelo critério do guia (P0 = perda de dados, crash ou estado errado de emulação em recurso comum).
- Critério adotado para P0 (aplicado de forma uniforme): P0 só para (a) crash/hang do próprio emulador, (b) perda ou corrupção de dados, (c) estado errado de núcleo em recurso comum (LC/MMU, save-state, reset, semântica de CPU). Falha de jogo/demo no guest por temporização de periférico fica em P1.
- Critério para `NOT_APPLICABLE`: `pergunta-suporte` quando o relato não descreve defeito do emulador (erro de uso, software ou imagem de terceiros defeituosos, comportamento correto confirmado); `invalida` quando falta informação ou não houve reprodução; `duplicada` quando o mantenedor aponta a original (justificativa "duplicata de #N"); `plataforma-windows` quando a causa é Win32/DirectDraw/GDI/shell sem caminho análogo no port. Crash de causa Win32 só vira N/A se o port não tem caminho análogo; caso contrário fica como runtime/debugger (ex.: #268).
- Formato do CSV preservado (mesmas colunas e ordem, LF). `revisado_por` = `auditor-sonnet (amostra)` ou `auditor-sonnet (reexame de classe)`; nas linhas alteradas, `evidencia` traz "corrigido: campo antes→depois — motivo".

## 2. Tamanho da amostra por estrato

| Lote | Obrigatórias | Aleatórias | Total |
|---|---:|---:|---:|
| 01 | 20 | 14 | 34 |
| 02 | 27 | 15 | 42 |
| 03 | 27 | 13 | 40 |
| 04 | 30 | 13 | 43 |
| 05 | 39 | 13 | 52 |
| 06 | 38 | 12 | 50 |
| 07 | 48 | 11 | 59 |
| 08 | 31 | 13 | 44 |
| 09 | 36 | 12 | 48 |
| 10 | 41 | 12 | 53 |
| **Total** | 337 | 128 | 465 |

Por aplicabilidade original: runtime 185, pergunta-suporte 48, plataforma-windows 44, ui 39, feature-futura 39, build 33, formato 28, debugger 27, documentacao 11, invalida 8, duplicada 3.

Parte aleatória por aplicabilidade: runtime 52, feature-futura 22, ui 21, debugger 14, formato 12, documentacao 7.

## 3. Concordância (auditor × triagem Haiku)

Concordância = campo igual antes e depois da revisão. As colunas `estado_verificacao` e `prioridade` seguem quase sempre a aplicabilidade (uma issue que vira N/A perde prioridade e muda de estado). A linha "aleatória" é a estimativa não enviesada da qualidade geral; os estratos obrigatórios foram escolhidos justamente por serem de risco (P0, rótulos de descarte) e por isso concordam menos.

### Por tipo de estrato

| Estrato | n | aplicabilidade | categoria | modelo | mapeamento_rust | estado_verificacao | prioridade | qualquer campo |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Amostra total | 465 | 73% | 98% | 99% | 98% | 74% | 72% | 69% |
| aleatória | 128 | 95% | 98% | 98% | 98% | 98% | 97% | 91% |
| P0 | 43 | 79% | 88% | 100% | 91% | 81% | 67% | 56% |
| NOT_APPLICABLE | 136 | 98% | 100% | 100% | 99% | 99% | 99% | 98% |
| rótulo invalid/duplicate/question | 158 | 31% | 98% | 100% | 99% | 32% | 31% | 30% |

### Por lote (todas as linhas da amostra)

| Estrato | n | aplicabilidade | categoria | modelo | mapeamento_rust | estado_verificacao | prioridade | qualquer campo |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| lote 01 | 34 | 76% | 97% | 100% | 97% | 79% | 79% | 74% |
| lote 02 | 42 | 67% | 98% | 100% | 100% | 69% | 67% | 62% |
| lote 03 | 40 | 80% | 100% | 98% | 98% | 80% | 78% | 75% |
| lote 04 | 43 | 72% | 95% | 100% | 95% | 72% | 72% | 67% |
| lote 05 | 52 | 65% | 100% | 100% | 100% | 71% | 67% | 62% |
| lote 06 | 50 | 72% | 98% | 100% | 98% | 72% | 66% | 66% |
| lote 07 | 59 | 75% | 100% | 98% | 100% | 75% | 75% | 73% |
| lote 08 | 44 | 66% | 98% | 100% | 98% | 70% | 66% | 66% |
| lote 09 | 48 | 67% | 96% | 100% | 96% | 67% | 67% | 62% |
| lote 10 | 53 | 87% | 96% | 98% | 98% | 89% | 89% | 83% |

### Por aplicabilidade original (todas as linhas da amostra)

| Estrato | n | aplicabilidade | categoria | modelo | mapeamento_rust | estado_verificacao | prioridade | qualquer campo |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| runtime | 185 | 55% | 95% | 98% | 96% | 58% | 53% | 46% |
| pergunta-suporte | 48 | 98% | 100% | 100% | 100% | 100% | 100% | 98% |
| plataforma-windows | 44 | 95% | 100% | 100% | 98% | 98% | 98% | 95% |
| ui | 39 | 59% | 97% | 100% | 100% | 62% | 62% | 59% |
| feature-futura | 39 | 72% | 100% | 100% | 100% | 72% | 72% | 72% |
| build | 33 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| formato | 28 | 71% | 100% | 100% | 100% | 71% | 71% | 71% |
| debugger | 27 | 78% | 100% | 100% | 100% | 78% | 78% | 78% |
| documentacao | 11 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| invalida | 8 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| duplicada | 3 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |

### Parte aleatória por lote

| Estrato | n | aplicabilidade | categoria | modelo | mapeamento_rust | estado_verificacao | prioridade | qualquer campo |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| lote 01 (aleatória) | 14 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| lote 02 (aleatória) | 15 | 80% | 100% | 100% | 100% | 87% | 87% | 80% |
| lote 03 (aleatória) | 13 | 100% | 100% | 92% | 100% | 100% | 100% | 92% |
| lote 04 (aleatória) | 13 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| lote 05 (aleatória) | 13 | 77% | 100% | 100% | 100% | 92% | 92% | 77% |
| lote 06 (aleatória) | 12 | 100% | 92% | 100% | 92% | 100% | 92% | 92% |
| lote 07 (aleatória) | 11 | 100% | 100% | 91% | 100% | 100% | 100% | 91% |
| lote 08 (aleatória) | 13 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| lote 09 (aleatória) | 12 | 100% | 92% | 100% | 92% | 100% | 100% | 92% |
| lote 10 (aleatória) | 12 | 100% | 100% | 92% | 100% | 100% | 100% | 92% |

### Parte aleatória por aplicabilidade original

| Estrato | n | aplicabilidade | categoria | modelo | mapeamento_rust | estado_verificacao | prioridade | qualquer campo |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| runtime (aleatória) | 52 | 94% | 96% | 94% | 96% | 100% | 98% | 85% |
| feature-futura (aleatória) | 22 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| ui (aleatória) | 21 | 86% | 100% | 100% | 100% | 86% | 86% | 86% |
| debugger (aleatória) | 14 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| formato (aleatória) | 12 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| documentacao (aleatória) | 7 | 100% | 100% | 100% | 100% | 100% | 100% | 100% |


Leitura:

- **Parte aleatória (não enviesada)**: 91% das linhas sem nenhuma alteração; por campo: aplicabilidade 95%, categoria 98%, modelo 98%, mapeamento 98%, estado 98%, prioridade 97%.
- **Rótulos `invalid`/`duplicate`/`question`**: só 30% sem alteração. A triagem leu só o início do corpo e deixou como `runtime`/`feature-futura` quase tudo que o mantenedor fechou como "não é defeito do emulador", "não reproduzido" ou duplicata. É o padrão sistemático desta auditoria.
- **P0**: 56% sem alteração; a prioridade concordou em 67% (a triagem usou P0 para qualquer "crash", inclusive de jogo no guest, erro de imagem de terceiros e crash exclusivo de Win32/DirectDraw).
- **NOT_APPLICABLE**: 98% de concordância; os `plataforma-windows`/`build` estão corretos. Os ajustes foram duplicatas (justificativa padronizada) e #268, que é um crash real de buffer no `tf` do depurador e virou teste.
- **Lotes**: na parte aleatória, os lotes 02 e 05 têm 3 discordâncias cada (`ui` com causa Win32 e pedidos de recurso classificados como runtime) e os demais no máximo 1; não há lote inteiro fora do padrão. Na amostra total, a variação por lote acompanha a quantidade de issues obrigatórias (rótulos de descarte, P0).
- **Aplicabilidade `ui`** na parte aleatória: 14% de discordância (3 de 21), acima do limite de 10% -> reexame da classe (item 4).

## 4. Classes reexaminadas

Gatilho: discordância relevante (> 10%) numa aplicabilidade ou padrão sistemático. Em todas as classes abaixo as linhas foram relidas por inteiro (não só as da amostra) e marcadas `auditor-sonnet (reexame de classe)`.

| Classe | Linhas | Alteradas | Padrão encontrado |
|---|---:|---:|---|
| rótulo `invalid` | 83 | 57 | "invalid" mantido como runtime/formato; maioria é erro de uso, imagem ruim, software de terceiros ou sem reprodução -> `pergunta-suporte`/`invalida` |
| rótulo `duplicate` | 29 | 28 | duplicatas mantidas como runtime/ui/feature-futura -> `duplicada` com "duplicata de #N" (originais achadas nos comentários via `gh api`; #1356 sem original explícita, mantida) |
| rótulo `question` | 121 | 37 | perguntas de uso respondidas pelo mantenedor -> `pergunta-suporte`; as que revelam comportamento de hardware a verificar foram mantidas |
| classe P0 (43 linhas + varredura de títulos com crash/freeze/hang/corrupt/assert) | 87 | 27 | 14 P0 rebaixados/anulados (crash de DirectDraw, imagem de terceiros, guest); 1 elevado (#700, estado do LC); varredura nas P1-P3 não achou crash de host a elevar |
| aplicabilidade `ui` com causa Win32/DirectDraw/GDI/hook de teclado (varredura por palavras-chave) | 70 | 19 | tela cheia DirectDraw, Alt+Esc, PrintScreen, hook de teclado, XP/Vista/Win7/8 -> `plataforma-windows` |
| `runtime` em aberto só com rótulo `enhancement` (pedidos sem comportamento de referência) | 48 | 8 | pedido de recurso inexistente (SwyftCard, modem, Grappler+, Votrax, 40/41 trilhas...) -> `feature-futura` |
| `runtime`/`formato`/`debugger` com categoria `outra` ou `mapeamento_rust` = `nenhum` | 25 | 16 | falta de categoria/mapeamento nas issues de LC/memória, serial, relógio, SSI263 |
| modelo das issues de CPU NMOS (264, 271, 288, 313, 404, 801, 1360) | 7 | 6 | o Apple //e não-enhanced também tem NMOS 6502 (#490); acrescentado `apple2e` |

Estado de reexame fora da amostra: 128 linhas fora da amostra foram relidas por classe, das quais 21 foram alteradas. As demais classes (documentacao, build, formato, debugger, feature-futura) ficaram com 100% de concordância na amostra aleatória e não foram reabertas.

Limite do reexame: as classes de varredura (ui, enhancement, crash) foram lidas por título e início do corpo; as de rótulo, por comentários. Não há garantia de que issues fora desses filtros estejam livres de erros semelhantes.

## 5. Mudanças (antes → depois)

Total: **168 linhas alteradas** (147 na amostra e 21 no reexame de classe). Movimentos de aplicabilidade: runtime → pergunta-suporte: 58; ui → plataforma-windows: 18; runtime → invalida: 12; runtime → feature-futura: 9; runtime → duplicada: 8; feature-futura → duplicada: 8; ui → duplicada: 5; ui → pergunta-suporte: 4; formato → pergunta-suporte: 4; formato → invalida: 3; feature-futura → pergunta-suporte: 3; debugger → invalida: 2; debugger → pergunta-suporte: 2; plataforma-windows → duplicada: 1; debugger → duplicada: 1; plataforma-windows → debugger: 1; formato → duplicada: 1; pergunta-suporte → duplicada: 1; debugger → plataforma-windows: 1; runtime → debugger: 1; ui → invalida: 1; ui → runtime: 1.

| # | Lote | Origem | Mudança | Motivo |
|---:|---:|---|---|---|
| 11 | 01 | amostra | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P0→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Crash em tela cheia 640x480 DirectDraw no Windows 2000/98 (modo removido no 1.26); sem caminho análogo no port.) |
| 16 | 01 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Boot lento do Maniac Mansion é esperado em velocidade autêntica; fechada sem defeito.) |
| 22 | 01 | amostra | aplicabilidade runtime→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | duplicata de #558 (comentário do mantenedor) (N/A: duplicata de #558) |
| 39 | 01 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Lentidão causada por driver de som defeituoso do relator; sem defeito do emulador.) |
| 40 | 01 | amostra | aplicabilidade feature-futura→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #45 (comentário do mantenedor) (N/A: duplicata de #45) |
| 46 | 01 | amostra | aplicabilidade plataforma-windows→duplicada | duplicata de #556 (comentário do mantenedor) (N/A: duplicata de #556, onde a correção foi feita) |
| 55 | 01 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | sem informação reproduzível (N/A: Não reproduzida pelo mantenedor com as cinco versões do Aztec.) |
| 58 | 01 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Diálogo de abrir arquivo fora da tela em tela cheia DirectDraw; API Win32.) |
| 66 | 01 | amostra | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P0→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Falha de Lock() de superfície DirectDraw em tela cheia (Win98/XP); API Win32 sem equivalente no port.) |
| 70 | 01 | amostra | categoria outra→serial; mapeamento_rust apple2-core/src/machine.rs→apple2-hostio/src/ssc.rs;apple2-core/src/machine.rs | categoria outra→serial: crash do Pascal ligado à porta serial (correção 1.16.1 e contorno via COM1) |
| 94 | 01 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Diretório de trabalho após duplo clique no shell do Windows; sem equivalente no port.) |
| 102 | 01 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Saída da tela cheia no XP com dois monitores (DirectDraw); específico do host Windows.) |
| 130 | 02 | classe | aplicabilidade runtime→feature-futura | runtime→feature-futura: pedido de recurso ainda inexistente, sem comportamento de referência |
| 131 | 02 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Imagem .dsk do Raster Blaster corrompida; outra imagem funciona.) |
| 139 | 02 | amostra | prioridade P0→P1 | P0→P1: travamento do guest (hlt) por layout de trilhas NIB; causa na imagem |
| 140 | 02 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Alt+Esc é capturado pelo Windows antes do emulador; correção em #556; sem equivalente no port.) |
| 141 | 02 | amostra | aplicabilidade runtime→feature-futura | runtime→feature-futura: pedido de recurso ainda inexistente, sem comportamento de referência |
| 144 | 02 | classe | aplicabilidade runtime→feature-futura | runtime→feature-futura: pedido de recurso ainda inexistente, sem comportamento de referência |
| 145 | 02 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Alt+Esc é capturado pelo Windows antes do emulador; correção em #556; sem equivalente no port.) |
| 162 | 02 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Resíduo de vídeo 2x→1x no Windows XP; renderização do host.) |
| 169 | 02 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Cores Double Lo-Res verificadas em hardware real como corretas; sem defeito.) |
| 170 | 02 | amostra | aplicabilidade debugger→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | sem informação reproduzível (N/A: Soft switches presentes no depurador; não reproduzida.) |
| 172 | 02 | amostra | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Tela cheia DirectDraw no Windows 8 (tela preta/janela deslocada); API Win32.) |
| 174 | 02 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Distorção do beep causada por interrupção periódica do Cybernoid; comportamento esperado.) |
| 176 | 02 | amostra | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Alt+Esc é capturado pelo Windows antes do emulador; correção em #556; sem equivalente no port.) |
| 177 | 02 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Paleta corrompida em tela cheia no Windows 8 (DirectDraw); específico do host.) |
| 179 | 02 | amostra | categoria outra→relogio | categoria outra→relogio: dia da semana do NSC |
| 180 | 02 | amostra | aplicabilidade debugger→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #406 (comentário do mantenedor) (N/A: duplicata de #406) |
| 185 | 02 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: ProDOS não lê discos DOS 3.3; comportamento esperado do software, não do emulador.) |
| 187 | 02 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Uso de CPU explicado pelo Wine; relator retirou o relato.) |
| 195 | 02 | amostra | aplicabilidade feature-futura→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #104 (comentário do mantenedor) (N/A: duplicata de #104) |
| 197 | 02 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: AppleSoft mascara o bit alto; dúvida de uso.) |
| 203 | 02 | amostra | aplicabilidade formato→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | sem informação reproduzível (N/A: Não reproduzida; relator conseguiu criar os discos.) |
| 208 | 02 | classe | aplicabilidade runtime→feature-futura | runtime→feature-futura: pedido de recurso ainda inexistente, sem comportamento de referência |
| 211 | 02 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Problemas de tela cheia 32-bit no Windows 8.1 (DirectDraw); específico do host.) |
| 212 | 02 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Problemas de tela cheia 32-bit no Windows 7 (DirectDraw); específico do host.) |
| 218 | 02 | amostra | aplicabilidade ui→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Pausa por ESC é recurso dos jogos; fechada como spec.) |
| 264 | 03 | classe | modelo apple2;apple2plus;apple2jplus→apple2;apple2plus;apple2jplus;apple2e | modelo: o Apple //e não-enhanced também tem NMOS 6502 (confirmado em #490) |
| 268 | 03 | amostra | aplicabilidade plataforma-windows→debugger; mapeamento_rust nenhum→apple2-debugger/src/cmd_exec.rs; estado_verificacao NOT_APPLICABLE→UNVERIFIED; prioridade (vazio)→P2 | NA→debugger: o relato é crash real em CmdTraceFile com caminho >80 caracteres; caso vira teste no port (não é só troca de sprintf) |
| 271 | 03 | amostra | modelo apple2;apple2plus;apple2jplus→apple2;apple2plus;apple2jplus;apple2e | modelo: o Apple //e não-enhanced também tem NMOS 6502 (confirmado em #490) |
| 277 | 03 | amostra | prioridade P0→P2 | P0→P2: corrida de dados na thread SSI263/DirectSound em build debug; o port não tem esse desenho; vira teste de fim de fonema durante o Play |
| 288 | 03 | classe | modelo apple2;apple2plus;apple2jplus→apple2;apple2plus;apple2jplus;apple2e | modelo: o Apple //e não-enhanced também tem NMOS 6502 (confirmado em #490) |
| 289 | 03 | amostra | aplicabilidade ui→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #168 (comentário do mantenedor) (N/A: duplicata de #168) |
| 300 | 03 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P0→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Imagem de disco do WeeGUI corrompida na origem (confirmado pelo autor); não é defeito do emulador.) |
| 306 | 03 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Dúvida de uso do mouse no Graphics Magician; resolvida.) |
| 310 | 03 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Comportamento de Ctrl-C do DOS 3.3 confirmado em hardware real; emulador correto.) |
| 313 | 03 | amostra | aplicabilidade runtime→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | duplicata de #292 (comentário do mantenedor) (N/A: duplicata de #292) |
| 333 | 03 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | sem informação reproduzível (N/A: Informação insuficiente (aplicativo não informado).) |
| 335 | 03 | amostra | aplicabilidade ui→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #211 (comentário do mantenedor) (N/A: duplicata de #211) |
| 359 | 04 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Rotina Apple Split não inicializa $1600; resultado esperado no //e.) |
| 368 | 04 | amostra | aplicabilidade formato→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | duplicata de #348 (comentário do mantenedor) (N/A: duplicata de #348) |
| 380 | 04 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Aviso do registro da tecla PrintScreen via API Win32; sem equivalente no port.) |
| 393 | 04 | amostra | aplicabilidade ui→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Lançador externo (Launchbox) mal configurado; sem defeito.) |
| 396 | 04 | amostra | aplicabilidade feature-futura→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Pedido mal interpretado; teclas de cursor para joystick já suportadas.) |
| 400 | 04 | amostra | categoria outra→memoria; mapeamento_rust apple2-core/src/memory.rs→apple2-core/src/language_card.rs;apple2-core/src/memory.rs;apple2-cards/src/mouse.rs | categoria outra→memoria: causa em MemSetPaging/Language Card; mapeamento corrigido |
| 404 | 04 | classe | modelo apple2;apple2plus→apple2;apple2plus;apple2jplus;apple2e | modelo: o Apple //e não-enhanced também tem NMOS 6502 (confirmado em #490) |
| 416 | 04 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Dúvida de uso da impressora (PR#1); resolvida.) |
| 421 | 04 | amostra | aplicabilidade ui→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | duplicata de #276 (comentário do mantenedor) (N/A: duplicata de #276) |
| 422 | 04 | amostra | categoria outra→memoria; mapeamento_rust apple2-core/src/machine.rs→apple2-core/src/memory.rs;apple2-cards/src/mouse.rs | categoria outra→memoria: IRQ do mouse durante código em ROM interna; paginação de memória |
| 430 | 04 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Recurso de salvar do jogo funciona; sem defeito.) |
| 432 | 04 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Lentidão do cursor vem do jogo (joystick vs teclado); sem defeito.) |
| 444 | 04 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Space Raiders V2 não é compatível com //e; problema do software.) |
| 457 | 04 | amostra | aplicabilidade debugger→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Comportamento de stepping é por design; dúvida de uso.) |
| 465 | 04 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Apple ][ original não tem autostart ROM; cai no Monitor, comportamento esperado.) |
| 474 | 04 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P0→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Comportamento esperado: LDA $C500 com cartão Z80 no slot 5 troca a CPU; disco ligado põe o emulador em full speed.) |
| 489 | 05 | amostra | aplicabilidade pergunta-suporte→duplicada | duplicata de #224 (comentário do mantenedor) (N/A: duplicata de #224) |
| 490 | 05 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Opcode $FF é INS na NMOS 6502 (//e não-enhanced); emulador correto.) |
| 495 | 05 | amostra | prioridade P0→P1 | P0→P1: crash não reproduzido pelo mantenedor, sem causa conhecida |
| 497 | 05 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: geos.hdv sem driver de mouse configurado; sem defeito.) |
| 500 | 05 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Emulador correto (BB); o defeito estava no emulador Sweet 16 usado pelo relator.) |
| 528 | 05 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Dúvida de uso do Caps Lock/minúsculas; resolvida.) |
| 530 | 05 | amostra | aplicabilidade ui→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #357 (comentário do mantenedor) (N/A: duplicata de #357) |
| 534 | 05 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | sem informação reproduzível (N/A: Não reproduzida; atribuída a RDP/driver de som do host.) |
| 540 | 05 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Discussão de desenvolvimento sobre função de depuração a remover; sem defeito.) |
| 545 | 05 | amostra | aplicabilidade feature-futura→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #394 (comentário do mantenedor) (N/A: duplicata de #394) |
| 547 | 05 | amostra | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Aviso falso do registro da tecla PrintScreen via API Win32; sem equivalente no port.) |
| 550 | 05 | amostra | duplicata de #544 (comentário do mantenedor) |  (N/A: duplicata de #544) |
| 560 | 05 | amostra | duplicata de #561 (comentário do mantenedor) |  (N/A: duplicata de #561; #562) |
| 562 | 05 | classe | aplicabilidade runtime→feature-futura | runtime→feature-futura: pedido de recurso ainda inexistente, sem comportamento de referência |
| 565 | 05 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: No Slot Clock vs Thunderclock; comportamento esperado.) |
| 566 | 05 | amostra | aplicabilidade runtime→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | duplicata de #357 (comentário do mantenedor) (N/A: duplicata de #357) |
| 568 | 05 | amostra | aplicabilidade runtime→feature-futura | runtime→feature-futura: pedido de recurso ainda inexistente, sem comportamento de referência |
| 569 | 05 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Atraso de 100 ms no message pump com hook de teclado do Windows; sem equivalente no port.) |
| 575 | 05 | amostra | aplicabilidade runtime→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | duplicata de #523 (comentário do mantenedor) (N/A: duplicata de #523) |
| 578 | 05 | amostra | duplicata de #616 (comentário do mantenedor) |  (N/A: duplicata de #616) |
| 582 | 05 | amostra | prioridade P0→P1 | P0→P1: falha do jogo por temporização do latch do Disk II, não crash do emulador |
| 584 | 05 | amostra | aplicabilidade feature-futura→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #104 (comentário do mantenedor) (N/A: duplicata de #104) |
| 598 | 05 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Atraso de teclado sob Wine/macOS, específico do host Win32.) |
| 606 | 05 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Defeito do próprio jogo Eliminator (teclas fixas); sem defeito do emulador.) |
| 607 | 05 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | sem informação reproduzível (N/A: Sem dados para reprodução; sem resposta do relator.) |
| 613 | 05 | amostra | aplicabilidade runtime→feature-futura | runtime→feature-futura: pedido de recurso ainda inexistente, sem comportamento de referência |
| 635 | 06 | amostra | aplicabilidade runtime→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | duplicata de #583 (comentário do mantenedor) (N/A: duplicata de #583) |
| 645 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Pergunta de desenvolvimento respondida; slot 0 já é inicializado como os demais.) |
| 655 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Sem disco o controlador tenta ler para sempre como no hardware real; esperado.) |
| 660 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Defeito do Phasor Music Player (ocorre também em hardware real).) |
| 665 | 06 | amostra | aplicabilidade runtime→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | duplicata de #1386 (comentário do mantenedor) (N/A: duplicata de #1386) |
| 667 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Comportamento normal do //e sem disco; dúvida de uso.) |
| 700 | 06 | amostra | categoria outra→memoria; mapeamento_rust nenhum→apple2-core/src/language_card.rs; prioridade P1→P0 | P1→P0, categoria outra→memoria: INC $C08B (dupla leitura) não habilitava escrita do LC em 6502; estado errado de memória em recurso comum |
| 703 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Imagem crackeada com defeito; sem defeito do emulador.) |
| 704 | 06 | amostra | prioridade P0→P1 | P0→P1: escrita em WOZ ainda não suportada (recurso ausente); jogo trava no guest |
| 708 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Programa BASIC invade a memória HGR; erro de uso.) |
| 716 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Falha de áudio resolvida por driver/dispositivo de som do host.) |
| 727 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Pergunta de desenvolvimento respondida (borda do NTSC).) |
| 728 | 06 | amostra | prioridade P0→P2 | P0→P2: Won't fix; código executado a partir do espaço de soft switches |
| 736 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Discussão de desenvolvimento respondida; sem defeito.) |
| 738 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Imagem .dsk só suporta volume 254; use .nib; comportamento esperado.) |
| 739 | 06 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: As três imagens WOZ funcionam; o defeito era da imagem.) |
| 759 | 06 | amostra | aplicabilidade ui→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #743 (comentário do mantenedor) (N/A: duplicata de #743) |
| 760 | 07 | amostra | aplicabilidade runtime→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #9 (comentário do mantenedor) (N/A: duplicata de #9) |
| 766 | 07 | amostra | aplicabilidade formato→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P0→(vazio) | sem informação reproduzível (N/A: Relato incompleto e sem resposta do autor; fechada como invalid.) |
| 782 | 07 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Campo bootSectorFormat da imagem WOZ estava errado; imagem corrigida.) |
| 792 | 07 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: PR#0 comporta-se assim também em hardware real; sem defeito.) |
| 797 | 07 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P0→(vazio) | sem informação reproduzível (N/A: Sem informação reproduzível; a imagem WOZ funciona; fechada como invalid.) |
| 798 | 07 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | sem informação reproduzível (N/A: Sem versão nem passos; fechada sem resposta do relator.) |
| 801 | 07 | amostra | modelo apple2;apple2plus;apple2jplus→apple2;apple2plus;apple2jplus;apple2e | modelo: o Apple //e não-enhanced também tem NMOS 6502 (confirmado em #490) |
| 820 | 07 | amostra | aplicabilidade debugger→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P0→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Vazamento de DC do GDI em FrameDrawDiskStatus (causa do crash ao restaurar janela); API Win32.) |
| 835 | 07 | amostra | aplicabilidade feature-futura→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Pergunta respondida: -videorom já permite ROM alternativa.) |
| 865 | 07 | amostra | aplicabilidade runtime→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | duplicata de #650 (comentário do mantenedor) (N/A: duplicata de #650) |
| 887 | 07 | amostra | aplicabilidade feature-futura→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Pergunta respondida: -h1 já carrega imagem de 800K.) |
| 907 | 07 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Demo espera interrupção de um Mockingboard ausente; defeito do software.) |
| 909 | 07 | amostra | aplicabilidade feature-futura→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #9 (comentário do mantenedor) (N/A: duplicata de #9) |
| 922 | 07 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | sem informação reproduzível (N/A: Imagens WOZ funcionam para o mantenedor; não reproduzida.) |
| 935 | 07 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Crack com defeito; sem defeito do emulador.) |
| 957 | 07 | amostra | aplicabilidade debugger→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | sem informação reproduzível (N/A: Não reproduzida; relator usava versão antiga.) |
| 965 | 08 | classe | aplicabilidade ui→plataforma-windows; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | causa específica do host Win32/DirectDraw, sem caminho análogo no port (N/A: Troca do modo de vídeo do Windows ao sair; específico do host.) |
| 989 | 08 | amostra | aplicabilidade runtime→debugger; categoria cpu→debugger; mapeamento_rust apple2-cpu/src/lib.rs→apple2-debugger/src/disasm.rs; prioridade P1→P2 | cpu→debugger: o relato de CPU era inválido (emulador correto); a correção real foi no endereço-alvo NMOS do depurador |
| 1008 | 08 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Comportamento confirmado como correto para o cartão RGB (modo 140 cores).) |
| 1019 | 08 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | sem informação reproduzível (N/A: Informação insuficiente; sem resposta do relator.) |
| 1034 | 08 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | sem informação reproduzível (N/A: Sem passos reproduzíveis; sem resposta do relator.) |
| 1045 | 08 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | sem informação reproduzível (N/A: Informação insuficiente; sem resposta do relator.) |
| 1063 | 08 | amostra | aplicabilidade debugger→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Recurso já existente (brk all on); dúvida de uso.) |
| 1068 | 08 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Usuário estava no Monitor; dúvida de uso.) |
| 1087 | 08 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Jogo D-Generation não suporta os modos do cartão RGB; defeito do software.) |
| 1107 | 08 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Defeito do software (Total Replay) reconhecido pelo relator.) |
| 1117 | 08 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Relator conseguiu criar o disco; dúvida de uso.) |
| 1123 | 08 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Imagem com bytes extras (cópia ruim); sem defeito.) |
| 1124 | 08 | amostra | aplicabilidade feature-futura→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #853 (comentário do mantenedor) (N/A: duplicata de #853) |
| 1147 | 08 | amostra | aplicabilidade formato→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | sem informação reproduzível (N/A: Problema da máquina do relator; não reproduzido.) |
| 1151 | 08 | amostra | aplicabilidade formato→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Bytes $00 nas trilhas .NIB formatadas são intencionais; sem defeito.) |
| 1152 | 08 | amostra | aplicabilidade runtime→feature-futura; prioridade P2→P3 | runtime→feature-futura: EVE do Le Chat Mauve não suportado ($C0Bx pendente); ajuda atualizada |
| 1195 | 09 | amostra | aplicabilidade feature-futura→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #780 (comentário do mantenedor) (N/A: duplicata de #780) |
| 1196 | 09 | amostra | categoria outra→relogio; mapeamento_rust apple2-core/src/model.rs→apple2-cards/src/nsc.rs;apple2-core/src/model.rs | categoria outra→relogio: o utilitário procura um No-Slot Clock |
| 1232 | 09 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | sem informação reproduzível (N/A: Não reproduzida com a imagem WOZ; sem resposta do relator.) |
| 1243 | 09 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Limitação de imagens DSK/NIB (use WOZ); segundo item esperado.) |
| 1244 | 09 | amostra | aplicabilidade feature-futura→duplicada; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | duplicata de #105 (comentário do mantenedor) (N/A: duplicata de #105) |
| 1245 | 09 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Dúvida de uso resolvida (boot pelo slot 5).) |
| 1246 | 09 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Comportamento do firmware de 80 colunas do //e (RDKEY); sem defeito.) |
| 1259 | 09 | amostra | aplicabilidade ui→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Escala da janela guardada no registro; use Ctrl+F6; dúvida de uso.) |
| 1263 | 09 | amostra | aplicabilidade ui→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Escala de tela do Windows 11 do relator; sem defeito.) |
| 1266 | 09 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: One on One trava em 65C02; problema do software, não do emulador.) |
| 1280 | 09 | amostra | aplicabilidade formato→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Imagens .po em ordem DOS (.do); resolvida renomeando; dúvida de uso.) |
| 1283 | 09 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Balanço do alto-falante do host; resolvido pelo relator.) |
| 1291 | 09 | amostra | aplicabilidade ui→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | sem informação reproduzível (N/A: Falta de detalhes; tooltips dos drives já existem.) |
| 1298 | 09 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Deslocamento de meio ponto dos pixels azuis é esperado (HGR com bit alto).) |
| 1299 | 09 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Efeito de strobing esperado (taxa de atualização do jogo vs varredura).) |
| 1316 | 09 | amostra | aplicabilidade formato→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P0→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Corrupção do registro na máquina do relator (confirmado por ele); sem defeito do emulador.) |
| 1318 | 09 | amostra | categoria mockingboard→ssi263; mapeamento_rust apple2-audio/src/mockingboard.rs→apple2-audio/src/ssi263.rs;apple2-audio/src/mockingboard.rs | categoria mockingboard→ssi263; mapeamento corrigido: o defeito é a fala (SSI263) repetindo o fonema |
| 1320 | 09 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Erro do relator ao digitar o nome no prompt do Merlin.) |
| 1332 | 10 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P3→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Valor zero em E6 é o esperado na cold boot em hardware real.) |
| 1348 | 10 | amostra | aplicabilidade runtime→invalida; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | sem informação reproduzível (N/A: Não reproduzida; sem resposta do relator.) |
| 1354 | 10 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: PR#6 não reinicia a máquina inteira; comportamento esperado do controlador de disco.) |
| 1360 | 10 | amostra | modelo apple2;apple2plus→apple2;apple2plus;apple2jplus;apple2e | modelo: o Apple //e não-enhanced também tem NMOS 6502 (confirmado em #490) |
| 1362 | 10 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Perda de teclas ocorre também em hardware real; sem defeito do emulador.) |
| 1417 | 10 | classe | aplicabilidade runtime→feature-futura; categoria outra→serial | runtime→feature-futura: pedido de recurso ainda inexistente, sem comportamento de referência |
| 1439 | 10 | amostra | aplicabilidade formato→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P2→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Erro no makefile do relator (volume ProDOS de 143KB).) |
| 1450 | 10 | amostra | aplicabilidade runtime→pergunta-suporte; estado_verificacao UNVERIFIED→NOT_APPLICABLE; prioridade P1→(vazio) | relato sem defeito do emulador (pergunta/erro de uso/software de terceiros) (N/A: Disco de boot do Wizardry precisa estar protegido contra escrita; sem defeito.) |
| 1491 | 10 | amostra | aplicabilidade ui→runtime; categoria disco→config | ui→runtime, categoria disco→config: -s6 empty -d1 gera bad_cast (crash) por inserir disco sem verificar a placa; lógica de cartas/slots, não de interface |
| 1520 | 10 | amostra | categoria outra→memoria; mapeamento_rust nenhum→apple2-core/src/language_card.rs | categoria outra→memoria, mapeamento nenhum→language_card.rs: LC/Saturn iniciavam com RAM gravável (correção 1.32.1.0) |

## 6. P0 finais, ordenados por risco

Critério da ordem: perda de dados primeiro; depois crash/corrupção do host; depois estado errado de núcleo (LC/MMU); depois comportamento de recurso comum. Todos são `UNVERIFIED`: cada um precisa de teste executado antes de qualquer PASS/FAIL.

| Ordem | # | Título | Categoria | Caminho do port afetado |
|---:|---:|---|---|---|
| 1 | 1444 | Write disk image to disk periodically | disco | `apple2-storage/src/store.rs`, `apple2-storage/src/disk2.rs` |
| 2 | 73 | Save changes to disk image | disco | `apple2-storage/src/disk2.rs` |
| 3 | 119 | Catalog update not flushed | disco | `apple2-storage/src/store.rs` |
| 4 | 664 | Mounting .woz file blocks writing to .dsk images as well. | disco | `apple2-storage/src/disk2.rs`, `apple2-storage/src/store.rs` |
| 5 | 1262 | loading save-state in A2osX actives wrong stack page | savestate | `apple2-savestate/src/apple2.rs` |
| 6 | 564 | Save State on Exit not called on restart | savestate | `applewin/src/emu.rs` |
| 7 | 1402 | Bad WOZ crashes AppleWin | disco | `apple2-storage/src/woz.rs` |
| 8 | 1285 | CPU emulation: AppleWin crashes when doing a 16-bit read from 6502 address 0xFFFF | memoria | `apple2-core/src/memory.rs` |
| 9 | 1126 | emulated-memory corruption sometimes crashes AppleWin | memoria | `apple2-core/src/memory.rs` |
| 10 | 1495 | Saturn128 in AppleWin | memoria | `apple2-core/src/language_card.rs` |
| 11 | 395 | LC write-enabled state is not maintained correctly (1.26.2.0) | memoria | `apple2-core/src/language_card.rs` |
| 12 | 392 | a2audit errors | memoria | `apple2-core/src/language_card.rs`, `apple2-core/src/memory.rs` |
| 13 | 700 | ProDOS8 2.5.0 alpha6 not working | memoria | `apple2-core/src/language_card.rs` |
| 14 | 1520 | Can't start any Merlin version - v1.32.0.0 | memoria | `apple2-core/src/language_card.rs` |
| 15 | 400 | Quark Catalyst V3 & BeagleWrite crash | memoria | `apple2-core/src/language_card.rs`, `apple2-core/src/memory.rs`, `apple2-cards/src/mouse.rs` |
| 16 | 422 | Apple // Bejeweled crash with version 1.26.2.3 | memoria | `apple2-core/src/memory.rs`, `apple2-cards/src/mouse.rs` |
| 17 | 164 | MMU no good when no cards in slot-4 & 5 | memoria | `apple2-core/src/slots.rs` |
| 18 | 1486 | Saturn 128K card in slot 0 does not use configured size | memoria | `apple2-core/src/language_card.rs`, `apple2-config/src/card.rs` |
| 19 | 70 | 1.14.2 crashes with USCD Pascal | serial | `apple2-hostio/src/ssc.rs`, `apple2-core/src/machine.rs` |
| 20 | 1491 | Odd behaviour of inserting disks in empty slots | config | `applewin/src/disk_ui.rs`, `apple2-config/src/cmdline.rs` |
| 21 | 186 | ProDOS Order 2IMG crashing | disco | `apple2-storage/src/image.rs` |
| 22 | 821 | Fix the ASSERT in `Disk2InterfaceCard::ReadTrack()` | disco | `apple2-storage/src/disk2.rs` |
| 23 | 640 | With Drive-1 empty, start from logo screen: inserting a disk doesn't boot | disco | `apple2-storage/src/disk2.rs`, `applewin/src/disk_ui.rs` |
| 24 | 4 | Emulation Speed Control problem | cpu | `apple2-core/src/scheduler.rs` |
| 25 | 622 | Crash when entering debugger from logo screen | debugger | `apple2-debugger/src/disasm.rs` |
| 26 | 226 | (Debugger)[1.25] Bad arguments to SAVE crash in ConfigSave_PrepareHeader() | debugger | `apple2-debugger/src/cmd_mem.rs` |
| 27 | 349 | [Debugger] NTSC LOAD or SAVE without filename crash | debugger | `apple2-debugger/src/cmd_misc.rs` |
| 28 | 64 | Crash from debugger for cmd: source asdf.txt | debugger | `apple2-debugger/src/commands.rs` |
| 29 | 109 | Debugger RUN cmd doesn't work & crashes AppleWin | debugger | `apple2-debugger/src/commands.rs` |
| 30 | 1204 | Crash due to debugger command line length | debugger | `apple2-debugger/src/console.rs` |

De 43 P0 iniciais restaram 30: 14 saíram (11, 66, 300, 474, 766, 797, 820, 1316 viraram N/A; 139, 277, 495, 582, 704, 728 foram rebaixadas) e 1 entrou (700). Essa lista alimenta a onda 1 (`docs/conformance/wave1-candidates.md`).

## 7. Observações e pontos em aberto

- A matriz atual só contém `UNVERIFIED` e `NOT_APPLICABLE`; esta auditoria não mudou isso.
- Issues de WOZ ficaram ora como `runtime`, ora como `formato` na triagem; ambas as classes entram na onda 1, então não foi reclassificado em bloco.
- Várias issues N/A por "comportamento correto confirmado" (#169, #310, #490, #500, #1298, #1332) são úteis como casos de teste de regressão do port; ficam fora da onda 1 por não descreverem defeito.
- #1356 tem rótulo `duplicate` mas o comentário só sugere relação com #1172; mantida como runtime/cpu P1.
- #46, #140, #145, #176 tratam de Alt+Esc capturado pelo Windows; #46 aponta #556 como correção. Se o port definir outra tecla para Open Apple, #556 cobre o teste.
