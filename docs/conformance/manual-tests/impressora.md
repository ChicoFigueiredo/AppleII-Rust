# Roteiros manuais: Impressora virtual LX-310 (T-025 e T-018 nativos)

- Escopo: HU-IMP-032 (latência até a tela), HU-IMP-011 e HU-IMP-020 (janela da Central: foco, segundo monitor, DPI), T-018 e T-025 de [../../printer/matriz-de-testes.md](../../printer/matriz-de-testes.md); gate G2.
- Estado: **T-025 BLOCKED** até o dono executar nos sistemas abaixo e preencher os campos. **T-018**: evidência nativa Linux já registrada (seção "Evidência já obtida"); Windows e macOS **BLOCKED**.
- Não marque PASS sem rodar na máquina real. Sem evidência humana, a matriz fica em BLOCKED. Este ambiente de desenvolvimento é Linux (Wayland, Vulkan); não há Windows nem macOS nativos aqui.

Como preencher: escreva `PASS`, `FAIL` ou `N/A` em cada passo; em `FAIL`, anote a mensagem exata do `stderr`, a captura de tela e o que viu. Rode sempre de um terminal. Cada execução tem o cabeçalho de evidência abaixo.

## Cabeçalho de evidência (uma cópia por sistema e por execução)

| Campo | Valor |
|---|---|
| Data e hora (ISO 8601, fuso) | |
| Executor | |
| Sistema e versão (ex.: Windows 11 23H2, macOS 14.5 Apple Silicon, Ubuntu 24.04 Wayland/X11) | |
| Versão do programa (`apple2-rust --version`) e commit ou pacote | |
| GPU e driver | |
| Monitores: resolução, escala (%) e posição de cada um | |
| Pasta do Arquivo de impressões usada (`APPLE2_RUST_PRINTER_ARCHIVE`, ou a padrão) | |
| Anexos (capturas de tela, `stderr`, vídeo) | |

## Preparo comum

1. Abra um terminal na pasta do projeto (ou do pacote) e use um Arquivo de impressões vazio, para contar páginas sem ruído. Linux e macOS: `export APPLE2_RUST_PRINTER_ARCHIVE=$(mktemp -d)`. Windows (PowerShell): `$env:APPLE2_RUST_PRINTER_ARCHIVE = (New-Item -ItemType Directory -Path "$env:TEMP\\imp-$(Get-Random)").FullName`.
2. Ligue a sonda de latência: Linux e macOS `export APPLE2_RUST_PRINTER_LATENCY=1`; Windows `$env:APPLE2_RUST_PRINTER_LATENCY = "1"`. Ao fechar o programa pelo botão da janela, o `stderr` imprime uma linha `printer latency (N samples): ... P50 ... P95 ...`. Copie-a para o campo de latência.
3. Abra o programa com o DOS 3.3 e a Carta de impressora paralela no slot 1: `apple2-rust -model apple2ee -s1 parallel -d1 "disks/DOS 3.3 System Master - 680-0210-A.dsk"` (ou monte o disco pela interface e escolha a Carta paralela no slot 1 em F8).
4. Programa de teste (digite depois do boot, no prompt `]`; CAPS LOCK ligado):

```
NEW
PR#1
10 FOR I=1 TO 600:PRINT I;" LINHA DE TESTE DA IMPRESSAO":NEXT
RUN
```

   Isso imprime 600 linhas (cerca de 8 Páginas de formulário contínuo). Para repetir, `RUN` de novo. Para encerrar a saída para a impressora, `PR#0`.
5. A janela da Central abre sozinha no primeiro byte, sem roubar o foco do teclado do Apple II (nas plataformas em que o sistema respeita isso; veja o cabeçalho de `printer_window.rs`). O botão "Printer" na barra lateral abre e fecha a Central à mão.

## T-025: tela cheia do Apple II, Central no segundo monitor, foco, DPI, fechar e reabrir

Pré-requisito: dois monitores (se só houver um, execute os passos A1 a A5 e C1 a C3 e marque os de segundo monitor como `N/A`, registrando isso no cabeçalho).

| # | Passo | Resultado esperado | Resultado |
|---|---|---|---|
| A1 | Com o programa no monitor 1, aperte F6 (tela cheia do Apple II). | O Apple II ocupa o monitor 1 inteiro, proporção preservada, sem tela preta. | |
| A2 | Rode o programa de teste (preparo, item 4). | A Central abre sozinha **sem tirar o monitor 1 da tela cheia** e sem pegar o foco: continuar digitando no teclado ainda chega ao Apple II (digite `PR#0` e Enter com a impressão terminada, e veja o eco na tela cheia). | |
| A3 | Arraste a Central para o monitor 2 e redimensione. | A janela segue o mouse, redesenha sem piscar, a página aparece inteira (zoom "ajustar à largura"). | |
| A4 | Rode `RUN` de novo e olhe a Central no monitor 2 enquanto imprime. | O papel é preenchido linha a linha, sem atraso perceptível; a tela cheia do Apple II não perde quadros nem áudio. | |
| A5 | Clique na Central e digite texto qualquer (inclusive Esc, F6 e Ctrl+C). | O que se digita na Central **não chega** ao Apple II nem sai da tela cheia (nenhuma tecla aparece no monitor 1). Clique de volta no monitor 1: o teclado volta a valer para o Apple II. | |
| B1 | DPI: mude a escala do SO do monitor 2 (ex.: 100% para 150% ou 200%) com a Central aberta. | A Central mantém a nitidez e a proporção da página; o texto da interface continua legível; sem corte. Anote as escalas. | |
| B2 | Arraste a Central entre monitores de escalas diferentes (se tiver). | Redesenha na escala nova sem borrão duradouro nem tamanho absurdo. | |
| C1 | Feche a Central pelo botão da janela, com o Apple II ainda em tela cheia. | Só a Central fecha; o monitor 1 continua em tela cheia, a emulação segue rodando. | |
| C2 | Reabra pelo botão "Printer" (a barra lateral some na tela cheia: saia com F6, clique em "Printer" e volte com F6). | A Central reabre **na mesma posição e tamanho** (monitor 2), com as Páginas e o zoom de antes. | |
| C3 | Feche o programa pelo botão da janela principal. | Sai sem demora, sem falha; o `stderr` mostra a linha de latência. Código de saída 0. | |

Latência (cole da linha do `stderr`; meta de HU-IMP-032: P95 até 250 ms do byte à tela para trabalho pequeno; a medida inclui os ~20 ms de lote da Carta):

| Campo | Valor |
|---|---|
| Amostras | |
| Do byte na Carta até o snapshot: P50 / P95 | |
| Do byte na Carta até o quadro apresentado: P50 / P95 / máx. | |
| Observação (com a Central fechada o tempo corre até a reabertura; use uma execução sem fechar a Central) | |

## T-018: fechar a Central no meio de 100 linhas e reabrir (nada se perde)

| # | Passo | Resultado esperado | Resultado |
|---|---|---|---|
| D1 | Rode o programa de teste. Quando a Central mostrar cerca de 3 Páginas, feche-a pelo botão da janela. Anote a hora. | A emulação e a impressão continuam: o contador de bytes do Apple II segue, sem erro no `stderr`. | |
| D2 | Espere o `RUN` terminar (a tela volta ao prompt `]`). | A impressão termina normalmente com a Central fechada. | |
| D3 | Reabra a Central pelo botão "Printer". | O Trabalho aparece completo: todas as ~8 Páginas, a última linha é `600 LINHA DE TESTE DA IMPRESSAO`, nenhuma Página em branco no meio, sem diagnóstico de perda. | |
| D4 | Compare com o Arquivo: abra `manifest.json` do Trabalho na pasta do Arquivo. | `pages` igual ao que a Central mostra, `total_bytes` igual ao tamanho de `raw.bin`, `diagnostics_total` 0. | |
| D5 | Repita D1 a D3 fechando a Central **três vezes** em momentos diferentes do mesmo `RUN`. | Mesmo resultado; nenhuma linha duplicada ou faltando entre as Páginas. | |

Campos de evidência: Páginas vistas ao fechar, Páginas ao reabrir, `pages` e `total_bytes` do manifesto.

## Evidência já obtida (Linux, automatizada; não substitui os roteiros acima)

- 22/10/2026, Ubuntu (kernel 7.0, WSL), Wayland, Vulkan (NVIDIA GeForce RTX 4080 SUPER), build `--release` da branch `feat/imp-rodada3`. Com `APPLE2_RUST_PRINTER_T018=10,16,26` (dev: fecha a Central em 10 s, reabre em 16 s, sai em 26 s) e o programa de teste acima: Central fechada com **3 Páginas (51.489 pontos)**; com ela fechada o modelo cresceu para **5 Páginas**; reaberta, **8 Páginas (174.955 pontos)**, janela aberta. O Arquivo registrou `pages: 8`, `total_bytes: 14941` (igual a `raw.bin`, 472 linhas), `diagnostics_total: 0`. Isso prova T-018 no Linux nativo; não prova o foco nem o segundo monitor.
- Latência ponta a ponta, 3 execuções, 1.074 a 1.089 amostras cada: veja [../../printer/desempenho.md](../../printer/desempenho.md).
