# Fonte matricial e tabelas de caracteres da Impressora virtual LX-310

> **Aviso: aproximação.** Os glifos deste projeto são desenhos próprios (GPL-2.0-or-later), no
> estilo geral das matrizes de 9 agulhas. **Não são** a ROM da Epson, nem foram copiados de
> manuais ou de bitmaps de ROM. Eles podem diferir visivelmente dos caracteres de uma LX-310 real
> (ADR 0008). Correções são bem-vindas (ver "Como corrigir um glifo").

Código: `crates/apple2-printer/src/font.rs` (API), `src/font/glyphs.txt` (desenhos),
`src/font/tables.rs` (Unicode das metades superiores). Testes: `tests/font_*.rs`, golden em
`tests/golden/font/`.

## Geometria

| | Draft | NLQ |
|---|---|---|
| Linhas (agulhas) | 9, espaçadas 1/72" | 18 (duas passadas), 1/144" |
| Colunas | 1/120" | 1/240" |
| Célula de 10 CPI | 12 colunas | 24 colunas |
| `baseline_row` | 8 | 16 |

`Glyph.columns[i]` é a máscara de linhas da coluna `i` (bit 0 = topo). Todo glifo devolvido tem a
largura da célula de 10 CPI (`cell_columns`); outras larguras (condensado, expandido,
proporcional) são tratadas pelo layout. Em NLQ, a linha `2r` e a `2r+1` correspondem às duas
passadas (linha `r` do Draft).

Layout vertical (Draft): maiúsculas e dígitos nas linhas 1..=7; linha 0 reservada ao acento das
maiúsculas; altura-x (minúsculas) nas linhas 3..=7; descendentes (g j p q y) têm o bojo
deslocado para as linhas 2..=6 e a cauda nas linhas 7..=8 (técnica das impressoras de 9 agulhas).
A cedilha usa a linha 8. Nas maiúsculas acentuadas o acento é de 1 linha; nas minúsculas, de 2.

## Regra de adjacência (meio-ponto)

Uma mesma agulha não dispara em colunas consecutivas. Todo glifo, em Draft **e** em NLQ, cumpre
`columns[i] & columns[i+1] == 0` (teste `no_adjacent_dots_in_any_glyph_either_quality`).

- **Draft:** garantido pela construção (colunas do desenho de 5 colunas vão para as colunas
  `2c+1`) e por um filtro final que descarta o ponto repetido (`enforce_no_adjacent`).
- **NLQ:** a ESC/P reference (`ESC *`, modos de densidade) mostra que modos de 120/240 dpi na
  mesma linha de agulha proíbem pontos adjacentes ("Adjacent dot printing: No"); o texto não
  documenta a regra interna do NLQ. **Decisão adotada (conservadora, a verificar contra uma
  impressora real):** vale a mesma regra por linha em NLQ. Na prática todos os pontos NLQ caem em
  colunas pares (resolução efetiva horizontal 1/120") e o ganho do NLQ vem da resolução vertical
  dobrada (duas passadas) e da ligação suave dos traços.

## Como os glifos são gerados

1. Cada caractere-base é um bitmap de 5 colunas em `glyphs.txt`
   (`<char> <linha-topo> <linhas "." / "#">`).
2. Letras acentuadas (À..ÿ, Ÿ) são **compostas por regra**: letra-base (i vira ı sem ponto) +
   marca (agudo, grave, circunflexo, til, trema, anel, cedilha).
3. Caracteres de moldura (U+2500..) e blocos (░▒▓█▀▄▌▐) são **gerados por regra**, não
   desenhados: todos os pontos em colunas pares da célula de 12 (período 2, que se repete sem
   emenda entre células); vertical simples na coluna 6, dupla nas 4 e 8; horizontal simples na
   linha 4, dupla nas 3 e 5. Linhas verticais ocupam as 9 agulhas; o espaçamento de linha do
   papel é maior que a altura da cabeça (como na impressora real), então pode haver vazio entre
   linhas de texto.
4. **Draft:** ponto `(c, r)` do bitmap vai para a coluna `2c+1`, linha `r`.
5. **NLQ:** cada ponto Draft `(x, r)` vira um par vertical em `(2x, 2r..2r+1)`; pontos a 2
   colunas de distância (horizontal e diagonais) são ligados por um ponto intermediário
   (`2x+2`), de modo que os traços ficam contínuos.
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

1. Edite a linha do caractere em `src/font/glyphs.txt` (5 colunas; ver cabeçalho do arquivo).
   Para acentos, mude a regra em `compose()` (`font.rs`). Para moldura/blocos, `box_or_block()`.
2. `UPDATE_GOLDEN=1 cargo test -p apple2-printer --test font_golden` regenera os arquivos de
   `tests/golden/font/` (arte `#`/`.`); revise o `git diff` visualmente.
3. `cargo test -p apple2-printer` valida cobertura, adjacência, geometria e determinismo.
4. Desenhe do zero: não copie bitmaps de ROM de impressoras nem de manuais.

Fontes de referência consultadas: manual LX-310
(<https://support2.epson.net/manuals/english/sidm/lx_310/index.html>) e ESC/P reference
(<https://files.support.epson.com/pdf/general/escp2ref.pdf>), apenas para comportamento e
tabelas, nunca para formas dos glifos.
