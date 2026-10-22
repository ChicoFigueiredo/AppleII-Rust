# Exportação PDF da Impressora virtual

Módulo: `crates/apple2-printer/src/pdf.rs` (feature `pdf`, ligada por padrão). O PDF é escrito à mão
(sem `pdf-writer`); a única dependência é `flate2` (MIT/Apache-2.0) para comprimir os fluxos de
conteúdo. Poppler (`pdfinfo`, `pdftoppm`, `pdftotext`) é usado só nos testes.

## API

- `pdf::export(&PageDocument, &RenderOptions, &PdfMeta, &PageSelection) -> Vec<u8>`
- `pdf::export_with_report(...) -> (Vec<u8>, ExportReport)`: informa páginas, caracteres do texto
  invisível omitidos e caracteres mapeados pela fonte extra (nada é silencioso, R02).
- `pdf::export_to_path(path, doc, opts, meta, pages, overwrite)` e `pdf::write_atomic(path, bytes,
  overwrite)` (feature `fs`): arquivo temporário oculto no mesmo diretório, `fsync`, depois
  `hard_link` (sem `overwrite`, falha com `AlreadyExists` sem corrida) ou `rename` (com `overwrite`).
  Qualquer falha remove o temporário. Nomes com espaço, acento, `#`, Unicode e emoji funcionam, pois
  o caminho nunca é interpretado.
- `PageSelection`: `All`, `Range { start, end }`, `List(Vec<usize>)`. Seleção vazia gera uma página
  em branco (um PDF precisa de pelo menos uma). A página em andamento (`last_in_progress`) entra como
  está: o documento recebido já é o instantâneo consistente.

## O que o PDF contém

1. **Páginas** com `MediaBox = Page.width x Page.height`, convertidos a 30 unidades = 1 pt (4 casas
   decimais, ex.: A4 = 595,2667 x 841,9 pt). Páginas de tamanhos diferentes mantêm cada uma o seu.
2. **Papel** desenhado sob a tinta, conforme `RenderOptions.paper.kind`.
3. **Pontos**: cada `Dot` como círculo vetorial.
4. **Camada de texto invisível** (modo de renderização 3) a partir de `Page.text`.
5. **Propriedades**: Info (Title, Author, Subject, Creator, Producer, datas se informadas, mais as
   chaves `Apple2Model`, `PrinterProfile`, `Note`, `Apple2RustVersion`) e XMP (`dc:`, `xmp:`, `pdf:`
   e o espaço `a2:` do projeto). Sempre escritas; textos não ASCII em UTF-16BE.
6. **Capa** opcional (`PdfMeta.cover`, desligada por padrão): folha de rosto com título, data,
   máquina, impressora, autor e nota, no tamanho do papel configurado.

O exportador nunca lê o relógio: a data vem de `PdfMeta.date`; sem ela, a saída é idêntica byte a
byte entre duas exportações.

## Pontos e fita

- Sistema de coordenadas da página: 1 unidade = 1/2160", y para baixo (uma matriz `cm` de 1/30). As
  coordenadas dos pontos são escritas como os inteiros exatos do documento, sem arredondar.
- Cada ponto é um traço de comprimento zero com ponta redonda (`1 J`) e espessura
  `DOT_DIAMETER` (`pub const` em `pdf.rs`, a prévia da Central usa o mesmo valor) = 30 unidades =
  0,353 mm = 1 pt. A agulha de uma cabeça de 9 agulhas tem cerca de 0,3 mm e a tinta se espalha nas
  fibras do papel, então o ponto impresso mede ~0,33–0,36 mm; 30 unidades igualam o passo vertical
  das agulhas (1/72"), de modo que os traços verticais do Draft saem contínuos como numa impressão
  real (com 25 unidades, 0,29 mm, ficavam frestas e o texto saía claro demais). É **aproximação**:
  não encontrei o diâmetro da agulha nos manuais da LX-310 consultados e não copiei nada da Epson.
  Todos os pontos de um balde vão em um único caminho (a cada 4000
  pontos, um novo), o que mantém o arquivo pequeno (25 páginas esparsas ficam bem abaixo de 200 KB).
- **Fita nova (`Ribbon::New`)**: preto uniforme e opaco. Pontos repetidos na mesma posição
  (ênfase, dupla impressão) coincidem e não mudam o visual; as posições repetidas são desenhadas uma vez.
- **Fita usada (`Ribbon::Worn`)**: cada batida é desenhada com opacidade entre 0,40 e 1,00 (8 níveis
  `ExtGState`); batidas repetidas na mesma posição são camadas separadas e a opacidade se acumula
  (escurece). Há ainda uma variação lenta por faixa de 1/6". A semente é `PdfMeta.ribbon_seed` (o
  chamador deve passar o hash dos bytes do trabalho, conforme o ADR 0008) ou, se ausente, um hash
  das posições dos pontos da página. Nunca há aleatoriedade real nem dependência de hora; mesma
  entrada, mesmos bytes.
- **Cor** (`InkColor`): preto (0,0,0), azul (0,05; 0,12; 0,55), roxo (0,35; 0,10; 0,50). O campo
  `Dot.ink` é ignorado: a LX-310 só usa o índice 0 e a cor vem das opções.

## Papel

- `GreenBar` / `BlueBar`: faixas alternadas de 1/2" (3 linhas a 6 lpi) entre as duas tiras de trator,
  começando por uma faixa branca. É o padrão do "green bar paper" que recordo (faixas de cerca de
  1/2" verdes sobre branco, cor verde-clara); a cor exata (0,80; 0,92; 0,80 / 0,80; 0,87; 0,95) é
  aproximação minha. Conferir com uma amostra física seria o ideal.
- Todas as folhas contínuas: tiras de 1/2" em cada lado com furos de 5/32" de diâmetro, passo de
  1/2", centro a 1/4" da borda; linhas de picote tracejadas ao longo das tiras e na borda superior e
  inferior (entre folhas). Os furos mostram um cinza claro (o fundo), não transparência real.
- `PlainContinuous`: igual, sem faixas. `A4` e `Letter`: branco liso.
- A decoração é desenhada antes dos pontos e nunca altera a posição deles (teste de regressão).

## Texto invisível

Fonte padrão Courier (Type1 não embutida), codificação WinAnsi, `Tr 3`. Cada `TextRun` vira um `Tj`
posicionado na linha de base, com `Tc` ajustando o avanço de 0,6 em para o `advance` do passo
(pitch) e tamanho da fonte = `height`. Acentos do português saem direto de WinAnsi (Ação, coração).

Caracteres fora de WinAnsi (caixas do PC437, grego, etc.): uma segunda fonte Courier cujos glifos
são todos brancos recebe códigos 1..255 na ordem de aparição, e um `ToUnicode` devolve o caractere
real. Passados 255 caracteres distintos, o excedente vira espaço no texto invisível e é contado em
`ExportReport.omitted_chars`; os pontos não são afetados. Caracteres de controle viram espaço.

## Como validar

```sh
cargo test -p apple2-printer --test pdf_export --test pdf_raster --test pdf_files
pdfinfo -f 1 -l 99 arquivo.pdf      # contagem e tamanho de cada página
pdftotext -enc UTF-8 arquivo.pdf -  # busca de texto
pdftoppm -r 150 -gray arquivo.pdf x # raster
```

Os testes de raster comparam com goldens em `crates/apple2-printer/tests/golden/pdf/*.pgm` (cinza,
72 e 150 dpi), pixel a pixel, tolerância zero. Se o poppler mudar o antialiasing entre versões, o
golden falha de forma legítima: reconferir a amostragem (testes `dot_and_background_samples`,
`dot_diameter_is_about_029_mm`, `green_bar_pattern_holes_and_perforation`, que independem de
antialiasing fino) e regenerar com `UPDATE_GOLDEN=1`. Goldens gerados com poppler 26.01.0. Se
`pdfinfo`/`pdftoppm`/`pdftotext` faltarem, os testes falham com "BLOCKED", nunca passam em silêncio.

## O que é fiel e o que é aproximação

- **Fiel**: tamanho da página, posição de cada ponto (inteiros em 1/2160"), ordem/contagem de
  batidas, texto pesquisável, propriedades.
- **Aproximação**: diâmetro e forma do ponto (círculo perfeito de 0,35 mm (`DOT_DIAMETER`)), aparência da fita usada,
  cores da fita, cores e proporções do papel, furos como discos cinza, picote como traço.
