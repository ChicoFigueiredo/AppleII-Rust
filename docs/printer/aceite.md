# Aceite da Impressora virtual LX-310

Dois níveis: testes **automáticos** sem interface gráfica (rodam em qualquer máquina) e um **roteiro manual** com programas da época (issue #104), que precisa de discos de terceiros.

## 1. Testes automáticos

Ponta a ponta: uma Máquina real com a Generic Printer no slot 1, um programa BASIC digitado pelo teclado emulado, os eventos de captura entregues ao `Spool` com o motor `Lx310`, e o Arquivo de impressões numa pasta temporária.

```sh
cargo test -p apple2-hostio --test printer_aceite_basic --test printer_aceite_graficos --test printer_aceite_apps
```

| Teste | O que prova |
|---|---|
| `printer_aceite_basic::applesoft_listing_one_job_two_pages_pdf_and_goldens` | Applesoft com `PR#1`, ESC E (negrito), ESC - (sublinhado), ESC M (12 cpi), SI (condensado), ESC x 1 (NLQ), FF e `PR#0`: um único Trabalho, fechado por inatividade (a Carta recebe `update`), 2 Páginas, SHA-256 do `raw.bin` igual ao do fluxo, texto invisível com todas as linhas, zero diagnósticos, PDF de 2 Páginas com estrutura válida, golden do documento lógico |
| `printer_aceite_basic::applesoft_eight_bit_transport_...` | Documenta o limite do Applesoft em 8 bits (tudo chega com o bit 7 ligado) |
| `printer_aceite_basic::integer_basic_list_...` | `LIST` do Integer BASIC numa Apple ][ vira Página, texto e PDF |
| `printer_aceite_graficos::eight_bit_transport_prints_the_top_pin_seven_bit_loses_it` | ESC K com bytes de 0x80 a 0xFF: em 8 bits cada coluna sai inteira; em 7 bits cinco pontos da agulha de cima somem (uma agulha, 30 unidades, de diferença no topo) |
| `printer_aceite_apps::*` | AppleWorks, Print Shop e dBase II: **BLOCKED** sem disco (veja a seção 2) |

Os fixtures são gerados pelo projeto, sem software de terceiros, em `tests/fixtures/printer/`:

- `*.raw`: bytes que a Carta entregou; `aceite_applesoft.raw.sha256`: o SHA-256 deles;
- `*.doc.txt`: resumo estável do documento lógico (Páginas, pontos por Página, hash das posições dos pontos, textos), calculado com células de teste no lugar da fonte, para não mudar quando a fonte for retocada.

Se uma mudança legítima alterar um golden, regenere com `UPDATE_GOLDEN=1 cargo test -p apple2-hostio --test printer_aceite_basic --test printer_aceite_graficos` e confira o diff antes de aceitar.

### Achado: BASIC e o Transporte de 8 bits

O Applesoft liga o bit 7 de **todo** caractere que manda para `PR#1`, controles inclusos (ESC chega como `0x9B`). Em 8 bits a LX-310 não reconhece o comando e imprime símbolos; por isso os testes de listagem BASIC usam o Transporte de **7 bits**, e os gráficos usam uma rotina em linguagem de máquina (como um driver faz) para controlar o bit 7. Também é preciso `PRINT CHR$(9);"255N"` para o firmware da Carta não inserir espaços. Detalhes e recomendação ao usuário em [README.md](README.md), seção 4.

## 2. Roteiro manual com programas da época

### Preparação

1. Instale a Generic Printer no slot 1 (F8, aba Slots), destino **Virtual printer + file**, e deixe aberta a opção "Open the printer window...".
2. Deixe cada disco em `target/conformance-cache/printer-apps/<programa>/`, **fora do Git**:

   | Programa | Pasta | Conteúdo esperado |
   |---|---|---|
   | AppleWorks | `target/conformance-cache/printer-apps/appleworks/` | disco(s) de boot e de programa (`.dsk`, `.po`, `.woz`...) |
   | Print Shop | `target/conformance-cache/printer-apps/printshop/` | disco do programa e de gráficos |
   | dBase II | `target/conformance-cache/printer-apps/dbase2/` | disco CP/M com `DBASE.COM` e um disco de dados |

   Nenhum desses discos é distribuído com o projeto. Os testes `printer_aceite_apps` imprimem `BLOCKED: disco ausente em <pasta>` quando a pasta está vazia, e passam sem falhar.
3. Para cada programa, anote no relatório o Transporte usado. Se o texto sair como símbolos ou os acentos errados, repita com o outro Transporte.
4. Critério geral: o Trabalho aparece no Arquivo de impressões, a prévia mostra o conteúdo, **Salvar PDF** produz um arquivo que abre num leitor, e a lista de diagnósticos está vazia ou só tem itens explicáveis.

### AppleWorks

1. Inicie o AppleWorks (disco no drive 1, slot 6). Modelo: Apple //e Enhanced, 128K.
2. No menu principal escolha **Other Activities**, depois **Specify information about your printer(s)** e acrescente uma impressora. Escolha **Epson** (FX-80 ou o modelo mais próximo da lista); em *interface*, **Apple Parallel Interface Card** (ou "Other", se o seu disco não listar), **slot 1**. A quantidade de colunas e o *line feed after carriage return* são perguntados: responda **No** para o LF (o firmware da Carta já acrescenta o LF; veja o resultado e inverta se as linhas se sobrepuserem).
3. Crie um documento de processador de texto com pelo menos 70 linhas (para ter 2 Páginas), usando negrito e sublinhado (pelos comandos de formatação do próprio programa).
4. Imprima: **Print** (Open-Apple P), destino **Printer**.
5. Resultado esperado: um Trabalho com 2 ou mais Páginas, texto legível, negrito mais escuro, sublinhado contínuo, quebra de Página no ponto certo. Transporte **7 bits** costuma ser o certo para texto.

### Print Shop

1. Inicie o Print Shop (disco do programa; faz boot no slot 6).
2. Na primeira execução ele pede a configuração: **Graphics/Printer setup**. Em *Printer*, escolha **Epson FX** (famílias FX/MX/RX); em *Interface*, **Apple Parallel** (Apple Parallel Interface Card ou o item "Grappler" se o disco só tiver esse), **slot 1**.
3. Crie um cartão simples (**Greeting Card**) ou uma faixa (**Banner**) com um gráfico do disco e um texto, e imprima.
4. Resultado esperado: gráficos de bit-image (ESC L ou ESC K), com a imagem reconhecível e proporção coerente; sem faixas em branco entre as linhas de gráfico. Transporte **8 bits**: com 7 bits o desenho perde a agulha de cima e fica "esburacado". Repita com os dois e compare.

### dBase II (CP/M, Z80 SoftCard)

1. Máquina: Apple //e (ou ][+), **Z80 SoftCard** no slot 4 (ou o seu) e a Generic Printer no slot 1. Disco CP/M no drive 1.
2. No CP/M, o `LST:` é o dispositivo de impressão, e o driver da SoftCard usa o slot da impressora configurado no `CONFIG` do disco (impressora paralela no slot 1). Confira com `PIP LST:=A:ARQUIVO.TXT` (um arquivo de texto qualquer) antes de abrir o dBase.
3. Entre no dBase II (`DBASE`), crie um banco simples (`CREATE`, `APPEND`) e emita `SET PRINT ON`, depois `LIST` (ou `DISPLAY ALL`) e `SET PRINT OFF`.
4. Resultado esperado: a listagem em colunas no Trabalho, sem caracteres trocados. O CP/M manda texto de 7 bits, em geral seguro nos dois Transportes. Se as linhas se sobrepuserem, o LF não está sendo enviado: ajuste no `CONFIG` do CP/M.

### O que registrar

Para cada programa: data, versão do disco (nome do arquivo e SHA-256), Transporte, Papel, número de Páginas, diagnósticos e, se possível, o PDF salvo. Falhas viram issue com o `raw.bin` do Trabalho anexado (ele reproduz o problema sem o disco).

Este roteiro **não é automatizado**: exige navegar menus de software de terceiros, e o projeto não distribui esses discos (estado: PARTIAL). Os testes `printer_aceite_apps` só avisam se o disco está presente.
