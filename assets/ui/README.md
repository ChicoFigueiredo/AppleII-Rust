# Ícones da interface

**Origem:** redesenho vetorial dos bitmaps do AppleWin (`resource/*.BMP`, GPL), mantendo
símbolos e cores ("mesmos símbolos, traço moderno"). Desenhados à mão em SVG simples (sem
`<style>`, classes, texto, imagens ou referências externas; glifos convertidos em caminhos).
**Licença:** GPL-2.0-or-later, como o restante do projeto.

Fonte da verdade: `assets/ui/toolbar/*.svg` (embutidos com `include_bytes!` em
`crates/applewin/src/icons.rs`; rasterizados com `resvg` por tamanho de pixel e cacheados).

| Arquivo | Uso |
|---|---|
| `help.svg` | botão F1 (Ajuda) |
| `run.svg` | F2 (Reiniciar) em Apple ][/][+/J-Plus/IIe/IIc |
| `run-pravets.svg` | F2 nos modelos Pravets 82/8M/8A |
| `run-tk3000.svg` | F2 no TK3000 //e |
| `run-base64a.svg` | F2 no Base64A |
| `drive1.svg`, `drive2.svg` | F3/F4 (drives 1 e 2) |
| `drive-swap.svg` | F5 (trocar drives) |
| `fullscreen.svg` | F6 (tela cheia) |
| `debug.svg` | F7 (depurador) |
| `setup.svg` | F8 (configuração) |
| `led-off/read/write/protected.svg` | LED de cada drive (parado, leitura, escrita, protegido) |
| `caps-off/on/on-lat/on-p8.svg` | indicador de Caps Lock na barra de status (`on-lat` ainda não usado, como no AppleWin) |

Nota: `run.svg` reproduz a maçã de seis listras do RUN.BMP do AppleWin; a maçã arco-íris é
marca da Apple Inc. (ver aviso de marcas do projeto).

Teste: `cargo test -p applewin --test icons` valida e renderiza tudo em 24/48/128 px;
com `ICONS_OUT=<dir>` (e `ICONS_REF=<dir de PNGs dos BMPs originais>`) salva PNGs e uma folha
de contato.
