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
| `printer_aceite_basic::applesoft_seven_bit_fixed_...` e `..._eight_bit_transport_...` | 7 bits fixo = Automático via firmware; 8 bits fixo estraga o BASIC (tudo chega com o bit 7 ligado) |
| `printer_aceite_basic::integer_basic_list_...` | `LIST` do Integer BASIC numa Apple ][ vira Página, texto e PDF |
| `printer_aceite_graficos::auto_direct_driver_keeps_bit_7_firmware_path_loses_it` | ESC K com bytes de 0x80 a 0xFF: driver direto em Automático sai inteiro; pelo firmware cinco pontos da agulha de cima somem (uma agulha, 30 unidades, de diferença no topo) |
| `printer_aceite_apps::*` | AppleWorks, Print Shop e dBase II: **BLOCKED** sem disco (veja a seção 2) |
| `printer_aceite_dbase2::dbase_ii_cpm_lst_prints_one_job_with_text_layer_and_pdf` | dBase II no CP/M da Z80 SoftCard (slot 4, carta no slot 1): `SET PRINT ON` + `LIST` gera um Trabalho, texto, PDF e goldens; BLOCKED sem o disco |

Os fixtures são gerados pelo projeto, sem software de terceiros, em `tests/fixtures/printer/`:

- `*.raw`: bytes que a Carta entregou; `aceite_applesoft.raw.sha256`: o SHA-256 deles;
- `*.doc.txt`: resumo estável do documento lógico (Páginas, pontos por Página, hash das posições dos pontos, textos), calculado com células de teste no lugar da fonte, para não mudar quando a fonte for retocada.

Se uma mudança legítima alterar um golden, regenere com `UPDATE_GOLDEN=1 cargo test -p apple2-hostio --test printer_aceite_basic --test printer_aceite_graficos` e confira o diff antes de aceitar.

### Transporte Automático

O Applesoft liga o bit 7 de **todo** caractere que manda para `PR#1`, controles inclusos (ESC chega como `0x9B` no Transporte de 8 bits fixo). Por isso o padrão é o Transporte **Automático** (ADR 0008): firmware da Carta (`PR#1`, `PRINT`, `LIST`) tem o bit 7 apagado, e escrita direta na porta (`STA $C090`) chega inteira. Os testes cobrem:

- BASIC (Applesoft e Integer) em Automático: ESC e texto corretos, igual ao 7 bits fixo;
- 8 bits fixo estraga o BASIC (teste que documenta ESC = `0x9B`);
- ESC K por uma rotina em linguagem de máquina que escreve direto em `$C090`: em Automático chega inteira (agulha de cima impressa), sem `PR#1` e sem Ctrl-I;
- a mesma faixa pelo firmware (`JSR $FDED`): perde o bit 7, e é preciso `PRINT CHR$(9);"255N"` para o firmware não inserir espaços.

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
5. Resultado esperado: um Trabalho com 2 ou mais Páginas, texto legível, negrito mais escuro, sublinhado contínuo, quebra de Página no ponto certo. O Transporte **Automático** deve servir; se acentos saírem errados, tente 7 bits fixo.

### Print Shop

Aceite real feito com **The New Print Shop** (lados A e B do disco do programa, `.nib`), Apple //e Enhanced, Disk II no slot 6 (lado A no drive 1, lado B no drive 2) e Generic Printer no slot 1.

**Escolha no programa** (Setup or Convert > Printer Setup):

| Item | Escolha | Observação |
|---|---|---|
| Printer | **Epson LX 80/86/90/800** (página 6 da lista) | a lista tem ainda Epson FX, MX, RX; o LX é o da família da LX-310 |
| Interface Card | **Apple II Parallel** | 2º item da lista; a "Epson APL (8 bit)" não é necessária |
| Slot | **1** | já é o padrão |

Depois de escolher, saia do Setup com **ESC** e **Save and Exit**: o programa só usa a escolha para imprimir depois de gravá-la no lado A. Com o disco protegido contra escrita ele responde "Disk is write-protected" e a impressão trava em "Thinking": use uma **cópia gravável** do lado A (nunca o original).

**Passo a passo** (teclas; Main Menu = Greeting Card, Sign or Poster, Letterhead, Banner, Calendar, Quick Print, Graphic Editor, Setup or Convert, Exit Program):

1. Boot do lado A; na tela de título, **Espaço**. Aguarde o Main Menu.
2. 7x **Seta abaixo** (Setup or Convert), **Return**, **Return** (Printer Setup), **Return** (Printer), 5x **Seta direita** (página 6), 3x **Seta abaixo**, **Return** (Epson LX 80/86/90/800).
3. **Return** (Interface Card), **Seta abaixo**, **Return** (Apple II Parallel). A tela mostra "Printer: Epson LX 80/86/90/800, Interface Card: Apple II Parallel, Slot: 1". *Test Printer* imprime a mensagem de boas-vindas ("Welcome to The New Print Shop", 3018 bytes, ESC L com a pena italiana "New").
4. **ESC**, 2x **Seta abaixo**, **Return** (Save and Exit); de volta ao Main Menu, 6x **Seta abaixo** (Graphic Editor), **Return**, **Seta abaixo** (Create a New Graphic), **Return**.
5. Desenhe (Espaço liga/desliga o ponto sob o cursor; setas movem) e imprima com **P**. O Trabalho fecha por inatividade.

Os discos entregues são só do programa: Greeting Card, Sign e Banner pedem os discos *Graphics* e *Fonts* ("Please insert disk with Graphics/Fonts"), que não fazem parte deles; por isso o aceite usa o **Graphic Editor**, que imprime o desenho pelo mesmo driver de impressão.

**Transporte:** o driver do Print Shop **escreve direto na porta** (`$C090`), sem passar pelo firmware da Carta, e usa as 8 agulhas (ESC L com bytes como `0xFE`/`0x82`). Resultados com o mesmo desenho:

| Transporte | Fluxo | Resultado |
|---|---|---|
| **Automático** | `1B 40 ... 1B 4C 51 00 ... FE 02 ... FE` | correto, caixa e diagonal completas |
| 8 bits | idêntico ao Automático | correto |
| 7 bits | `FE` vira `7E`, `82` vira `02` | agulha de cima perdida: a borda superior e parte da diagonal somem |

Use o **Automático** (padrão). ESC @, ESC J (avanço) e CR passam pelo mesmo caminho e chegam inteiros nos três Transportes.

**Automatizado:** `cargo test --release -p apple2-rust --test printer_aceite_printshop` (em debug ele é pulado: ~10 min; só com os discos em `target/conformance-cache/printer-apps/printshop/`; sem eles termina como BLOCKED). Ele copia os discos para uma pasta temporária, repete os passos acima, e afirma: um Trabalho, ESC @ e ESC L, bit 7 inteiro em Automático, pontos > 0, zero diagnósticos, PDF válido e menos pontos no mesmo fluxo em 7 bits. Para inspecionar, a ferramenta `cargo run --release -p apple2-rust --example printshop_drive -- <roteiro> <pasta>` roda um roteiro de teclas (`key`, `run`, `shot` PNG, `peek`...), útil para ver as telas do programa.

### dBase II (CP/M, Z80 SoftCard)

**Estado: PASS, automatizado** em `crates/apple2-hostio/tests/printer_aceite_dbase2.rs` (roda só com o disco em `target/conformance-cache/printer-apps/dbase2/*.dsk`; sem ele, BLOCKED sem falhar). Disco do dono: `CPM_DBase2.dsk` (SoftCard CP/M 60K 2.23, dBase II 2.3B de 22/02/82; SHA-256 `8b0b2b63...80a3`). O teste lê o disco para a memória, não grava de volta e confere o SHA-256 antes e depois.

Configuração que funcionou (a mesma do teste e do uso manual):

1. Modelo **Apple ][+**, **Z80 SoftCard no slot 4**, **Generic Printer no slot 1**, **Disk II no slot 6** com o disco no drive 1. Boot automático até `A>` em cerca de 600 quadros. O `LST:` do BIOS da SoftCard já escreve na carta paralela do slot 1, sem `CONFIG`; o disco não traz `PIP` nem `STAT`.
2. O disco vem **cheio** (último bloco usado 127): o `CREATE` do dBase grava o cabeçalho e volta ao CP/M sem mensagem, e um `USE` posterior dá `END OF FILE FOUND UNEXPECTEDLY`. Não é defeito do emulador. Libere espaço (na memória, sem tocar o arquivo do disco): `ERA DBASEII.DOC`, `ERA ZIP.COM`, `ERA INSTALL.COM`. Os `.DBF` que vêm no disco (`A`, `CATALOG`, `SCR38:B`) têm 0 registros.
3. **Digite devagar**: o CP/M e o dBase consultam o teclado e perdem teclas coladas em rajada (colar a linha inteira perde letras: `USE A` virou `UEA`). O teste manda um caractere por vez, 4 quadros entre eles, e espera o prompt `.` do dBase antes de digitar o primeiro comando.
4. `DBASE`, RETURN na data, `CREATE TESTE`, campos `NOME,C,12`, `CIDADE,C,10`, `VALOR,N,7,2`, linha vazia, `Y` em *INPUT DATA NOW?*. Cada registro: campo com RETURN, exceto o numérico de largura 7, que avança sozinho quando cheio (digite `0123.45`); campo de largura exata também avança (nome de 12 letras consumiria o RETURN seguinte). Linha vazia termina a entrada.
5. `USE TESTE`, `SET PRINT ON`, `? "RELATORIO DBASE II"`, `LIST`, `SET PRINT OFF`. O Trabalho fecha por inatividade.

Resultado (Transporte **Automático**; o fluxo é de 7 bits, sem ESC): um Trabalho (`Close(Idle)`), 1 Página, 201 bytes, zero diagnósticos, texto invisível com o título e as três linhas do `LIST`, PDF de estrutura válida, golden do documento lógico (`aceite_dbase2.doc.txt`) e do fluxo (`aceite_dbase2.raw`, só o que o teste digitou e o dBase imprimiu). Observações: o dBase com `SET PRINT ON` também manda o eco dos comandos (`. LIST`) e termina linhas com `CR CR LF`/`CR LF` sem sobreposição; a camada de texto do PDF preserva as linhas, mas um extrator genérico (`pdftotext`) separa as colunas numéricas (texto em posições distintas).

### O que registrar

Para cada programa: data, versão do disco (nome do arquivo e SHA-256), Transporte, Papel, número de Páginas, diagnósticos e, se possível, o PDF salvo. Falhas viram issue com o `raw.bin` do Trabalho anexado (ele reproduz o problema sem o disco).

Só o AppleWorks continua com roteiro **manual** (estado: PARTIAL, falta o disco). O dBase II (`printer_aceite_dbase2`) e o Print Shop (`printer_aceite_printshop`) são automatizados e rodam quando os discos do dono estão em `target/conformance-cache/printer-apps/`; sem eles terminam em BLOCKED. Em `printer_aceite_apps` os três só avisam se o disco está presente.
