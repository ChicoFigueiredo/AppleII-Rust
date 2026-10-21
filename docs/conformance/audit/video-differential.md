# Diferencial de vídeo: framebuffer contra o AppleWin (HU-0201, HU-0202)

- Data: 2026-10-21. Rust: `apple2-video` neste branch. Oráculo: fork Linux do audetto (`a1e20c0`), núcleo AppleWin `3e8054b`.
- Política de comparação (definida antes da primeira execução): [test-oracles.md, "Vídeo"](../test-oracles.md). Resumo: framebuffer inteiro de 600x420 (ou 680x436 com VidHD), RGBA8, diferença máxima de canal = 0, sem tolerância; MAE e caixa envolvente só como diagnóstico.
- Achados do plano que este registro fecha: AUD-P-001 (#1443), AUD-P-002, AUD-P-003, AUD-P-050; AUD-P-004 permanece NOT_APPLICABLE (paridade).

## Resultado

1.573 comparações oráculo x Rust, todas com o oráculo repetido duas vezes com o mesmo SHA-256 (calibração, `--selfcheck`): **1.573 PASS, 0 FAIL, 0 BLOCKED**. O caminho até aqui achou três divergências do Rust (DIV-001 a DIV-003, corrigidas) e uma de RAM de power-on (DIV-004, resolvida pela frente de CPU), mais três achados do lado do oráculo.

Há 305 casos em `tests/conformance/cases/` (`video-*.toml`, `authority = "applewin"`); o `conformance run --all` dá PASS em todos.

### Matriz modo x Tipo de vídeo

Cada célula soma as comparações do grupo (PASS n/N). Colunas: tipo de vídeo do AppleWin a 60 Hz (t0 mono custom, t1 cor idealizada, t2 carta RGB, t3 monitor NTSC, t4 Color TV, t5 B&W TV, t6 âmbar, t7 verde, t8 branco) e, à direita, a 50 Hz (PAL) para t1 a t5. Em t2, N conta as três cartas (Video-7 SL7, Le Chat Mauve EVE e Feline).

| Grupo de cenas | t0 | t1 | t2 | t3 | t4 | t5 | t6 | t7 | t8 | PAL t1 | PAL t2 | PAL t3 | PAL t4 | PAL t5 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Texto 40/80 colunas (normal, inverso, flash em quadros 4 e 20, MouseText, II+, //e) | PASS 18/18 | PASS 18/18 | PASS 54/54 | PASS 18/18 | PASS 18/18 | PASS 18/18 | PASS 18/18 | PASS 18/18 | PASS 18/18 | PASS 18/18 | PASS 54/54 | PASS 18/18 | PASS 18/18 | PASS 18/18 |
| LORES (inclui misto, página 2, II+) | PASS 4/4 | PASS 4/4 | PASS 12/12 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 12/12 | PASS 4/4 | PASS 4/4 | PASS 4/4 |
| DLORES (inclui misto) | PASS 2/2 | PASS 2/2 | PASS 6/6 | PASS 2/2 | PASS 2/2 | PASS 2/2 | PASS 2/2 | PASS 2/2 | PASS 2/2 | PASS 2/2 | PASS 6/6 | PASS 2/2 | PASS 2/2 | PASS 2/2 |
| HIRES (padrões de artefato, bit 7, ruído, misto, página 2, II+) | PASS 6/6 | PASS 6/6 | PASS 18/18 | PASS 6/6 | PASS 6/6 | PASS 6/6 | PASS 6/6 | PASS 6/6 | PASS 6/6 | PASS 6/6 | PASS 18/18 | PASS 6/6 | PASS 6/6 | PASS 6/6 |
| DHIRES (barras, ruído, misto, 80STORE+PAGE2) | PASS 4/4 | PASS 4/4 | PASS 12/12 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 12/12 | PASS 4/4 | PASS 4/4 | PASS 4/4 |
| Modos da carta RGB (AN3: 140, 160, misto, 560; DHGR, DHGR misto, HGR, texto 80) | PASS 14/14 | PASS 14/14 | PASS 42/42 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 42/42 | PASS 14/14 | PASS 14/14 | PASS 14/14 |
| Troca de modo no meio da linha (TEXT/GR, LORES/HIRES, 40/80, FULL/MIXED, PAGE, DHIRES; inclui #1443) | PASS 14/14 | PASS 14/14 | PASS 42/42 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 14/14 | PASS 42/42 | PASS 14/14 | PASS 14/14 | PASS 14/14 |
| Troca de modo ou de página no meio do quadro (flash com mudança de modo, page flip, 80 colunas + LORES) | PASS 5/5 | PASS 5/5 | PASS 15/15 | PASS 5/5 | PASS 5/5 | PASS 5/5 | PASS 5/5 | PASS 5/5 | PASS 5/5 | PASS 5/5 | PASS 15/15 | PASS 5/5 | PASS 5/5 | PASS 5/5 |
| Modos legados com VidHD (centrados em 640x400) | PASS 9/9 | PASS 9/9 | PASS 27/27 | PASS 9/9 | PASS 9/9 | PASS 9/9 | PASS 9/9 | PASS 9/9 | PASS 9/9 | - | - | - | - | - |
| Super Hi-Res com VidHD (320, 640, controle misto, preenchimento de cor) | PASS 4/4 | PASS 4/4 | PASS 12/12 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | PASS 4/4 | - | - | - | - | - |
| Boot da ROM, RAM padrão do AppleWin (II, II+, //e, //e Enhanced) | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 |
| Boot da ROM, RAM zerada (`--memclear 0`; II, II+, //e, //e Enhanced) | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 | PASS 8/8 |

Notas de leitura:

- Os modos com carta RGB só existem em t2; os outros tipos renderizam as mesmas cenas pelo caminho NTSC/mono. Todos conferem.
- "Boot" são 8 comparações por tipo (4 modelos x 30 e 120 quadros), sem disco; "RAM padrão" é o padrão `FF FF 00 00` do AppleWin com o sorteio fixo do oráculo, "RAM zerada" roda os dois lados com `--memclear 0`.
- Não há comparação de SHR em PAL nem de VidHD em II+: o oráculo e o Rust só têm aux RAM na //e para essa carta nesta bateria (ver "Lacunas").

## Divergências

| Id | Onde | Resultado | Causa raiz | Correção e teste |
|---|---|---|---|---|
| DIV-001 | Texto 80 colunas em Color TV, 1o quadro | corrigida | O AppleWin calcula o endereço da linha 0 em `NTSC_VideoInit` -> `VideoReinitialize(true)` (`NTSC.cpp:2364`, `:2426-2440`) com o modo de power-on (TEXT40). O `Renderer` se configurava no primeiro `update`, depois dos efeitos do 1o opcode; a linha 0 do 1o quadro começava 1 pixel antes em 80 colunas (-3 em vez de -2) e o resíduo da linha intermediária acima dela (`updateFramebufferTVSingleScanline`, `*pLine1Prev`) diferia em 1 pixel | `Renderer::init` (chamado por `run_frame`) configura o scanner antes do 1o opcode. `tests/first_frame.rs` |
| DIV-002 | SHR (VidHD), todas as linhas | corrigida | `VidHDCard::UpdateSHRCell` (`VidHD.cpp:130`) escreve 16 pixels em **uma** linha de framebuffer (`g_pScanLines[2*vert]`); a linha seguinte fica preta. O Rust preenchia as duas | `shr_cell` escreve só a linha 2*y. `tests/vidhd.rs` ajustado (a asserção antiga "duas linhas" estava errada em relação ao AppleWin) |
| DIV-003 | SHR (VidHD), linha 0 do quadro em que o SHR liga | corrigida | `NTSC_SetVideoMode` não recalcula `g_pVideoAddress` (só o fim de linha o faz: `updateVideoScannerHorzEOL_SHR`, `NTSC.cpp:722-728`), e `updateScreenSHR` escreve a partir desse ponteiro. A linha 0 desse quadro sai de onde o modo legado centrado deixou o ponteiro (x=60, y=26) e, como o framebuffer do AppleWin é bottom-up, os últimos 40 pixels transbordam para a linha de cima, não para a de baixo. O Rust calculava o destino por `vert`/`horz` e recalculava o ponteiro ao trocar de modo | `shr_cell` usa e avança o ponteiro (`advance` modela o wrap bottom-up), `eol_shr` recalcula o endereço, e `set_shr` não o recalcula. `tests/vidhd_roundtrip.rs::first_shr_line_starts_at_the_legacy_pointer_and_wraps_upwards` |
| DIV-004 | Boot da ROM do II e do //e Enhanced | resolvida (pela frente de CPU, HU-0103) | O Rust ligava com a RAM toda em zero. O AppleWin enche a RAM com `FF FF 00 00` (`MemReset`, `g_nMemoryClearType = MIP_FF_FF_00_00`) e semeia `$4E/$4F` (RNDL/RNDH) com o relógio; o oráculo fixa esses últimos em `$20`. A tela do II original (a ROM não a limpa) e a fase do cursor piscante do //e Enhanced (o laço de espera de tecla parte de RNDL) dependem disso | O diferencial apontou a causa antes da integração, com duas hipóteses testadas e confirmadas: (1) com `--memclear 0` no oráculo o boot do II dava o mesmo SHA do Rust; (2) com a cena `main 004E 00 00` + `entry FA62` (RAM zerada) o //e Enhanced a 120 quadros também dava o mesmo SHA. A correção está no `apple2-core` (`RamInit::APPLEWIN_DETERMINISTIC`, de outra frente, fora da fronteira de arquivos desta); com ela, `scene-fb` e o harness usam o mesmo padrão de RAM do oráculo e os 56 FAIL de boot passaram a PASS |

Todas as correções foram validadas pela matriz inteira, não só pelo caso que as achou.

### Achados do lado do oráculo (não são defeitos do Rust)

| Id | Achado | Tratamento |
|---|---|---|
| O-1 | O framebuffer do AppleWin é um DIB bottom-up (`NTSC_VideoInit` mapeia a linha y para a linha `H-1-y`) | `framebuffer.cpp` inverte as linhas na saída (RGBA top-down, como o Rust) |
| O-2 | `RGBMonitor.cpp` (`UpdateDuochromeCell` e outros) grava `RGBQUAD` e lê como `UINT32`: comportamento indefinido que o GCC `-O2` explora; o texto em carta RGB saía com cores de lixo (stack lida) | `build-audetto-oracle.sh` compila com `-fno-strict-aliasing`, que reproduz o comportamento do MSVC com que o AppleWin é desenvolvido. Antes da correção, 258 das 1.206 comparações do 1o lote falhavam, todas em t2 |
| O-3 | `--state x.aws.yaml` (nunca tinha rodado) carrega, mas o fork (3e8054b) recusa a unidade "Game I/O Connector" versão 4 que o port (a8f0d1d) grava, e o oráculo passa a dar boot em vez de continuar o estado (`PC=FCB1`) | `tools/conformance/oracle/aws-for-oracle.py` remove o bloco de extensão e essa unidade. Validado de ponta a ponta com `examples/aws_state.rs`: estado salvo pelo Rust após 2 quadros de `hires-noise`, carregado no oráculo e no Rust, mais 2 quadros: framebuffers idênticos byte a byte |
| O-4 | Os arquivos de vídeo do fork e do clone de referência do port são idênticos (`NTSC.cpp`, `NTSC.h`, `NTSC_CharSet.cpp`, `RGBMonitor.cpp/.h`, `Video.h`, `VidHD.cpp/.h`; `Video.cpp` só difere no código de captura de tela em PNG) | A defasagem de 22 commits entre o fork e o `a8f0d1d` não afeta o vídeo |

## Cenas e entradas

- 59 cenas "scene v1" em `tests/conformance/inputs/video/*.scene`, geradas por `crates/apple2-video/examples/gen_scenes.rs` a partir de `crates/apple2-video/tools/scenes.rs` (RAM e código 6502 escritos à mão, sem ROM de usuário). `tests/scene_files.rs` garante que os arquivos são os gerados.
- O oráculo e o Rust carregam os mesmos bytes logo após o power-on, apontam o PC para `$0300` e rodam do ciclo 0 até o fim do quadro N (primeira instrução que cruza `N x ciclos por quadro`); os ciclos finais coincidem nos dois lados em todas as comparações.
- As cenas temporizadas usam laços de exatamente 65 ciclos (uma linha) com NOPs, para a troca de modo cair numa posição horizontal fixa de cada linha (`midline-hgr-text-N`, `midline-1443-*`), ou delays de milhares de ciclos para trocar no meio do quadro.
- #1443 (AUD-P-001): `midline-1443-hgr-then-text` e `midline-1443-text-then-hgr`. O resultado do oráculo e do Rust coincide em PAL e NTSC nos tipos 1 a 5: a Color TV PAL renderiza igual à NTSC (o color burst não é morto na troca para TEXT) e o monitor composto PAL difere (mata o burst). `tests/pal_1443.rs` fixa esse comportamento.

## AUD-P-xxx

| Achado | Resultado | Evidência |
|---|---|---|
| AUD-P-001 (#1443, Color TV PAL) | PASS | casos `video-midline-1443-*-pal` (t1 a t5) e `tests/pal_1443.rs` |
| AUD-P-002 (NTSC sem oráculo pixel a pixel) | PASS | 1.573 comparações bit a bit, t3, t4 e t5 incluídos; diferença máxima de canal 0 em todas |
| AUD-P-003 (flash, misto, page flip, bordas) | PASS | texto em quadros 4 e 20, `flash-mode-change`, `pageflip-midframe`, cenas `*-mixed`, `*-page2`; bordas incluídas em todas (região `full`) |
| AUD-P-004 (VidHD sem 80x45 e sem cor de borda) | NOT_APPLICABLE (paridade) | O AppleWin também não suporta (`VidHD.cpp:27-33`); não foi implementado nem testado |
| AUD-P-050 (VidHD sobrevive a save/load) | PASS | `tests/vidhd_roundtrip.rs` (SHR e modo legado, antes e depois do `.aws.yaml`) e o experimento `--state` (O-3) |

## Desempenho

- `Machine::step` + `Renderer::update` em Color TV (`tests/perf.rs`, ignorado por padrão): **10,2 MHz** de tempo de CPU com o renderizador de `main` e com o deste branch, na mesma máquina, ambos sob carga de outros processos (`docs/pendencias.md` registra ~15 MHz, de outra medição). O caminho quente dos modos não SHR não mudou; as mudanças são `Renderer::init` (uma vez) e o ponteiro do SHR.
- `crates/apple2-core/tests/no_alloc.rs` (zero alocações por passo) continua verde. Novo: `crates/apple2-video/tests/no_alloc.rs`, que garante zero alocações por quadro renderizado em todos os 9 tipos de vídeo, no boot e em três cenas pesadas (HGR, DHGR e troca de modo no meio da linha).

## Lacunas

- Modelos: o oráculo roda `apple2`, `apple2p`, `apple2e` e `apple2ee`; Pravets, TK3000, Base64A e J-Plus não entraram nesta bateria.
- VidHD em II/II+ (aux RAM da própria carta), SHR em PAL, Video-7 e Feline além dos 4 modos AN3 e do texto 80, e telas capturadas do software real (corpus) ficam para uma próxima rodada.
- Os casos de boot dependem da RAM de power-on: a semente de `$4E/$4F` é a do oráculo (`$20`), não a aleatória do AppleWin real.

## Como reproduzir

```sh
# 1) oráculo (usa ORACLE_CACHE; compilar na árvore principal substitui o binário compartilhado)
tools/conformance/oracle/build-audetto-oracle.sh
# 2) cenas (só se mudarem)
cargo run --release -p apple2-video --example gen_scenes
# 3) diferencial completo (oráculo x Rust), com calibração do oráculo
cargo build --release -p apple2-conformance
python3 tools/conformance/oracle/video-differential.py --jobs 8 --cards 0,1,2 --selfcheck
#    uma cena isolada: --scene text80 --types 4 --hz 60; o que falha deixa o RGBA dos dois lados em
#    target/video-differential/{ora,rust} e o diagnóstico (pixels, caixa, MAE) em results.json
# 4) regenerar os casos (só escreve os que passaram)
python3 tools/conformance/oracle/video-differential.py --cards 0,1,2 --write-cases tests/conformance/cases
# 5) casos no harness
./target/release/conformance run --all --report target/conformance-report
```
