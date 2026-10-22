# Fonte matricial e tabelas de caracteres da Impressora virtual LX-310

> **Aviso: aproximação.** Os glifos deste projeto são desenhos próprios (GPL-2.0-or-later), no
> estilo geral das matrizes de 9 agulhas. **Não são** a ROM da Epson, nem foram copiados de
> manuais ou de bitmaps de ROM. Eles podem diferir visivelmente dos caracteres de uma LX-310 real
> (ADR 0008). Correções são bem-vindas (ver "Como corrigir um glifo").

Código: `crates/apple2-printer/src/font.rs` (API), `src/font/draft.txt` (desenhos Draft),
`src/font/nlq.txt` (traços NLQ), `src/font/legacy5.txt` (símbolos raros ainda no desenho da v1; ¿ º ° § ± ¹ ² ³ µ × ÷ » foram redesenhados no Draft 9×12 e no NLQ 18×24),
`src/font/tables.rs` (Unicode das metades superiores). Testes: `tests/font_*.rs`, golden em
`tests/golden/font/`. Amostra visual: `cargo run -p apple2-printer --example amostra_fonte -- saida.pdf`.

## Geometria

| | Draft | NLQ |
|---|---|---|
| Linhas (agulhas) | 9, espaçadas 1/72" | 18 (duas passadas), 1/144" |
| Colunas | 1/120" (meio-ponto permitido) | 1/240" |
| Célula de 10 CPI | 12 colunas | 24 colunas |
| `baseline_row` | 8 | 16 |

`Glyph.columns[i]` é a máscara de linhas da coluna `i` (bit 0 = topo). Todo glifo devolvido tem a
largura da célula de 10 CPI (`cell_columns`); outras larguras (condensado, expandido,
proporcional) são tratadas pelo layout.

**Draft (v2).** Os glifos são desenhados direto na grade 9 x 12, e os pontos caem em colunas pares
e ímpares (meio-ponto), o que dá diagonais e curvas mais suaves que a v1 (5 colunas ampliadas).
Maiúsculas e dígitos ocupam as linhas 1..=7 (7 agulhas acima da base; a linha 0 é do acento);
altura-x nas linhas 3..=7; g j p q y têm o bojo subido uma linha e a cauda nas linhas 7..=8.
Letras ocupam até 9 colunas (1..=9, ou 1..=10 em m, w, M, W), deixando 2 a 3 colunas vazias entre
letras. Acento de maiúscula: 1 linha (linha 0); de minúscula: 2 linhas (3 no anel); cedilha na linha 8.

**NLQ (v2).** Os glifos são desenhados nativamente na grade 18 x 24 como **traços** (linhas e arcos
elípticos, formato em `nlq.txt`) rasterizados em pontos, no estilo "Roman": hastes grossas (duas
colunas), traços finos (uma coluna) e serifas leves em pés e topos. Maiúsculas nas linhas 2..=14,
altura-x 6..=14, descendentes até a 17, acentos de maiúscula nas linhas 0..=1. Como as duas passadas
sobrepõem pontos (o ponto impresso mede ~0,35 mm contra 1/240" = 0,1 mm de passo de coluna e 1/144"
= 0,18 mm de passo de linha), os traços parecem contínuos. Letras acentuadas são o glifo-base
mais a marca (traços), também nativos. Caracteres de moldura e blocos, e uns poucos símbolos raros
(letras gregas, matemática, ¼ ½ ¾ © ® ¤ ƒ ¶ Æ æ Ø ø Œ œ Ð ð Þ þ), **não** têm desenho NLQ próprio:
são derivados do Draft (cada ponto vira um par vertical, com ponto intermediário nas diagonais).
Esses símbolos raros também ficam no desenho de 5 colunas da v1 (pendência conhecida).

## Regra de adjacência (meio-ponto)

Uma mesma agulha não dispara em colunas consecutivas. Todo glifo, em Draft **e** em NLQ, cumpre
`columns[i] & columns[i+1] == 0` (teste `no_adjacent_dots_in_any_glyph_either_quality`). Em ambos o
filtro final `enforce_no_adjacent` descarta, da esquerda para a direita, o ponto repetido.

- **Draft:** a regra vale por linha (agulha); pontos em colunas vizinhas só em linhas diferentes
  (diagonais). Horizontais saem com um ponto a cada 2 colunas.
- **NLQ:** a ESC/P reference (`ESC *`, "Adjacent dot printing: No") não documenta a regra interna do
  NLQ. **Decisão adotada (conservadora, a verificar numa impressora real):** vale a mesma regra por
  linha de 1/144" a 1/240". Diferente da v1, os pontos NLQ usam colunas pares **e** ímpares: a regra
  só proíbe vizinhas na mesma linha, então diagonais avançam uma coluna por linha e horizontais saem
  com um ponto a cada 2 colunas (sobrepostos pelo diâmetro do ponto).

## Como os glifos são gerados

1. Draft: cada caractere-base é uma grade de linhas `.`/`#` com 10 colunas (colunas 1..=10 da
   célula de 12) em `draft.txt` (`<char> <linha-topo> <linhas>`).
2. NLQ: cada caractere-base é um programa de traços em `nlq.txt` (comandos `m l M l e E`, ver o
   cabeçalho do arquivo); maiúsculos `M L E` são grossos (duas colunas, x-1 e x+1).
3. Letras acentuadas (À..ÿ, Ÿ) são **compostas por regra** (`compose_draft` e `nlq_mark`):
   letra-base (i vira ı sem ponto) + marca (agudo, grave, circunflexo, til, trema, anel, cedilha).
4. Caracteres de moldura (U+2500..) e blocos (░▒▓█▀▄▌▐) são **gerados por regra** na grade 9 x 12:
   pontos em colunas pares (período 2, repete sem emenda entre células); vertical simples na coluna 6,
   dupla nas 4 e 8; horizontal simples na linha 4, dupla nas 3 e 5; em NLQ, derivados do Draft.
5. **Moldura contínua entre linhas.** Os verticais ocupam as 9 agulhas, mas o espaçamento de linha
   de 1/6" (12 x 1/72") deixa 3 linhas sem impressão entre uma linha de texto e a seguinte (como na
   impressora real, que só emenda com espaçamento de até 9/72"). Para o Arquivo/PDF fiel, o glifo
   não cresce: a função pública `line_bridge(upper, lower)` devolve as colunas cujo pino dispara na
   última linha do glifo de cima e na primeira do de baixo, e o **layout deve continuar esses
   pontos pelo vão** (um ponto por 1/72" no Draft, por 1/144" no NLQ). A amostra
   (`examples/amostra_fonte.rs`) faz isso e a moldura sai contínua.
6. **Itálico** (`CharTable::Italic`, também `italicize()` para ESC 4): cisalhamento
   determinístico. A linha Draft `r` desloca-se `(9 - r) / 4` colunas para a direita (2,2,1,1,1,1,
   0,0,0); em NLQ usa-se a linha Draft `r/2` e o dobro do deslocamento. Molduras e blocos não
   são inclinados.

## Tabelas

`to_unicode(table, country, code)` serve de camada de texto invisível do PDF e de fonte do
glifo. `None` para 0x00..0x1F, 0x7F e códigos indefinidos. A parte baixa (0x20..0x7E) é ASCII
com as 12 posições nacionais do ESC R (`country`), aplicado a **todas** as tabelas (a ESC/P
reference não restringe o ESC R a uma tabela).

### Variantes nacionais (ESC R n = 0..=12)

Fonte: Epson ESC/P Reference Manual, comando ESC R, tabela "International character sets"
(<https://files.support.epson.com/pdf/general/escp2ref.pdf>, p. C-82). Códigos alterados: 0x23,
0x24, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x60, 0x7B, 0x7C, 0x7D, 0x7E. Conjuntos: 0 EUA, 1 França,
2 Alemanha, 3 Reino Unido, 4 Dinamarca I, 5 Suécia, 6 Itália, 7 Espanha I (0x23 = Pt, U+20A7),
8 Japão, 9 Noruega, 10 Dinamarca II, 11 Espanha II, 12 América Latina. `country` > 12 é tratado
como 12.

### Metades superiores

- **PC437, PC850, PC860** (0x80..0xFF completos): mapeamentos padrão das páginas de código IBM
  (CP437/CP850/CP860, Unicode Consortium / codecs do Python). 0xFF = NBSP.
- **BRASCII** (0xA0..0xFF): ABNT NBR 9611:1991, equivalente a ISO 8859-1 exceto 0xD7 = Œ e
  0xF7 = œ. Fonte: <https://en.wikipedia.org/wiki/BraSCII> (cita a norma; a página não lista a
  faixa 0x80..0x9F, deixada indefinida aqui). **Ressalva:** a variante exata que a LX-310 chama
  de BRASCII pode diferir; não foi possível confirmar na ROM/manual.
- **ABICOMP** (0xA0..0xDF): <https://en.wikipedia.org/wiki/ABICOMP_character_set>. A página
  deixa vazias 0x80..0x9F e 0xE0..0xFF; ficam indefinidas (`None`). 0xA0 = NBSP (inferido).
- **Itálico** (ESC t 0): 0x80..0x9F indefinidos (a ESC/P reference diz que nenhum caractere é
  definido ali); 0xA0..0xFF tomados como ISO 8859-1 (**suposição**; o conteúdo real da ROM
  itálica não está documentado nas fontes consultadas).

Os controles imprimíveis 0x01..0x1F (smileys etc., via ESC ( ^) não são suportados.

## Como contribuir / corrigir um glifo

1. Draft: edite a linha do caractere em `src/font/draft.txt` (10 colunas; sem `##` na mesma linha).
   NLQ: edite o programa de traços em `src/font/nlq.txt`. Para acentos, mude `draft_mark()` /
   `nlq_mark()` (`font.rs`). Para moldura/blocos, `box_or_block()`.
2. `cargo run -p apple2-printer --example amostra_fonte -- /tmp/a.pdf` e olhe o PDF ampliado.
3. `UPDATE_GOLDEN=1 cargo test -p apple2-printer --test font_golden` regenera os arquivos de
   `tests/golden/font/` (arte `#`/`.`); revise o `git diff` visualmente.
4. `cargo test -p apple2-printer` valida cobertura, adjacência, geometria, determinismo, letras
   Draft com meio-ponto (não confinadas a colunas pares) e continuidade da moldura.
5. Desenhe do zero: não copie bitmaps de ROM de impressoras nem de manuais.

Fontes de referência consultadas: manual LX-310
(<https://support2.epson.net/manuals/english/sidm/lx_310/index.html>) e ESC/P reference
(<https://files.support.epson.com/pdf/general/escp2ref.pdf>), apenas para comportamento e
tabelas, nunca para formas dos glifos.
