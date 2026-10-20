# Depurador

O depurador é um porte do depurador do AppleWin (`Debugger/*.cpp`): console de comandos, avaliador de expressões, disassembler, mini-assembler, tabelas de símbolos, breakpoints, observações (watches) e janelas de memória. O núcleo é independente de interface (`apple2-debugger`); o frontend desenha a tela como uma grade de texto de 80x48 caracteres no lugar da tela do Apple.

## Entrando e saindo

- **F7** (botão Debug) entra no depurador e **F7** de novo sai. Se houver breakpoints, o programa sai em modo de "passo a passo" (`stepping`).
- Dentro do depurador, o teclado vai para o console. A emulação fica parada.
- **Ctrl+F11** executa uma instrução.
- Outras formas de sair, que continuam em modo passo a passo:

| Comando | Velocidade | Vídeo | Som |
|---|---|---|---|
| `G` | normal (mais rápido só com o disco ativo) | preciso | preciso |
| `GG` | sempre máxima | periódico | mudo |

- Para voltar ao depurador estando em passo a passo, use F7.
- O comando `VERSION` mostra as versões do emulador e do depurador; inclua-as ao relatar um erro.

## Teclas do console

| Tecla | Efeito |
|---|---|
| Caracteres | Digitam no console |
| Enter | Executa o comando |
| Backspace, Delete | Apagam o último caractere |
| Seta para cima e para baixo | Histórico de comandos |
| Esc | Limpa a linha de entrada |
| Page Up, Page Down | Rolam a saída do console |

Esta versão tem apenas o console: as teclas de atalho do AppleWin que movem o cursor do disassembler (Tab, Espaço para dar um passo, Shift+Espaço para sair da sub-rotina, setas, Ctrl+setas) não existem. Os mesmos efeitos estão nos comandos `T`, `P`, `RTS`, `.`, `=`, `RET` e outros. Veja "Diferenças em relação ao AppleWin".

## Disposição da tela

A grade tem 80 colunas por 48 linhas. À esquerda fica o código desmontado (ou uma janela de dados); na primeira coluna de informações, os registradores, a pilha, os alvos, os ponteiros da página zero e os soft switches; na segunda, os breakpoints, as observações (watches), o scanner de vídeo e dois dumps de memória; embaixo, o console. Os comandos `WIN`, `CODE`, `CODE1`, `CODE2`, `DATA`, `DATA1`, `DATA2`, `CONSOLE`, `SOURCE1` e `SOURCE2` escolhem o que aparece, inclusive em tela cheia.

**Soft switches.** Uma coluna resume o vídeo e a MMU. Os cinco primeiros mostram `$C050` (GR/TEXT), `$C052` (FULL/MIX), `$C054` (PAGE 1/2), `$C056` (LO/HIRES) e `$C05E` (DHGR/HGR), em vídeo inverso para o ativo. Os seguintes mostram `$C000` (80STORE), `$C002-$C005` (leitura e escrita de memória principal ou AUX), `$C00C` (80COL), `$C00E` (ALTCHARSET), `$C080` (Language Card, banco 2), `$C088` (banco 1 e, com `r`NN ou `s`NN, o banco RamWorks ou Saturn ativo).

## Expressões e números

Os valores são hexadecimais. O avaliador aceita registradores (`A`, `X`, `Y`, `PC`, `SP`, `P`), símbolos, `$` e `#` para valor literal, `@` para o conteúdo de uma posição, parênteses, `+ - * / & | ^ ~ %` e outros operadores, sempre avaliados da esquerda para a direita, sem precedência. Faixas usam `,` (endereço e tamanho) e `:` (início e fim). O comando `CALC` mostra o resultado de uma expressão.

## Execução

| Comando | Efeito |
|---|---|
| `G [endereço] [skip,len]`, `GG` | Continua a execução até PC = endereço. Se o PC sair da faixa informada, volta ao passo a passo |
| `T` | Executa uma instrução (entra em sub-rotinas) |
| `P` | Executa uma instrução passando por cima de sub-rotinas |
| `RTS` | Executa até o retorno da sub-rotina atual |
| `TL` | Rastreia com contagem de ciclos |
| `TF arquivo` | Grava o rastreamento em arquivo |
| `R registrador valor` | Altera um registrador |
| `JSR endereço` | Chama uma sub-rotina |
| `NOP` | Troca a instrução atual por NOP |
| `IN $C0xx`, `OUT $C0xx valor` | Lê e escreve portas de I/O |
| `KEY` | Envia uma tecla ao Apple |
| `CYCLES`, `RCC` | Contador de ciclos |
| `LBR`, `PROFILE`, `BENCHMARK` | Último desvio, perfil e benchmark |

Exemplo: `R PC FA62` põe o contador de programa em $FA62; `G C600 D000,3000` roda em passo a passo até PC = $C600 ou até o PC sair da faixa $D000-$FFFF.

## Disassembler e mini-assembler

`U [endereço]` desmonta. `A [endereço]` entra no mini-assembler; as linhas seguintes são instruções. `Z` e `X` marcam bytes como dados ou código (`B` lista os intervalos de dados). `DB`, `DW`, `ASC`, `DF` e `DA` definem dados (bytes, palavras, texto, números de ponto flutuante do Applesoft, endereços). O assembler conhece o conjunto 65C02 quando a CPU é um 65C02.

## Breakpoints

Os breakpoints podem ser desativados e reativados sem perder a definição.

| Comando | Efeito |
|---|---|
| `BPX endereço[,len]` (ou `BP`) | Para quando o PC está na faixa |
| `BPR registrador [op] valor` | Para quando um registrador satisfaz a comparação |
| `BPA registrador [op] valor` | Breakpoint "inteligente" |
| `BPM endereço[,len]` | Para em qualquer acesso (leitura ou escrita) a memória, inclusive DMA do disco rígido |
| `BPMR`, `BPMW` | Apenas leituras, apenas escritas |
| `BPIO $C0xx` | Acesso a um endereço de I/O |
| `BPV linha[,len]` | Posição vertical do scanner de vídeo (desativa-se ao acertar) |
| `BPD #`, `BPE #`, `BPC #` | Desativa, ativa, remove (`BPC *` remove todos) |
| `BPL` | Lista |
| `BPSAVE`, `BPEDIT`, `BPCHANGE` | Salva, edita, altera |
| `BRK [0\|1\|2\|3\|all] [on\|off]` | Para em BRK ou opcodes inválidos |
| `BRKOP opcode` | Para num opcode |
| `BRKINT [on\|off]` | Para logo depois de uma interrupção |

Operadores de comparação: `=` (igual), `!` (diferente), `<`, `>`, `<=` e `>=`. Registradores: `A`, `X`, `Y`, `PC`, `S` e `P`.

Endereços aceitam prefixos separados por `/` (a ordem não importa e as letras podem ser maiúsculas ou minúsculas): `s<n>` para o slot de uma Saturn, um número para o banco RamWorks (00 é o principal) ou o banco de 16K da Saturn, `l1` ou `l2` para o banco de 4K da Language Card e `rom` para a ROM.

```
BPX < FA62
BPR A ! 0
BPV A0
BRK ALL ON
BPM E000,2000
BP 00/L1/D000
BP ROM/FF58
BPMW 01/D000,1000
BPM S3/7/L1/D800
```

## Memória

| Comando | Efeito |
|---|---|
| `MD1`, `MD2` (ou `D`) | Dump hexadecimal nas áreas de memória 1 e 2 |
| `MA1`, `MA2` | Texto ASCII |
| `MT1`, `MT2` | Texto Apple |
| `MEB endereço valor...` | Escreve bytes |
| `MEW endereço valor...` | Escreve palavras |
| `M origem,tamanho destino` | Move memória |
| `F endereço,tamanho valor` | Preenche memória |
| `S`, `SH` | Procura texto ou valores hexadecimais; `@` mostra os resultados |
| `BLOAD`, `BSAVE` | Carrega e grava uma região em arquivo |
| `ME` | Editor de memória: não implementado, como no AppleWin |

Os arquivos são lidos e gravados no diretório do depurador (`PWD` mostra, `CD` muda).

## Observações e página zero

`WA` adiciona uma observação (watch), `WC` remove, `WD` e `WE` desativam e ativam, `WL` lista, `WSAVE` salva. `ZPA`, `ZPC`, `ZPD`, `ZPE`, `ZPL`, `ZPSAVE` e `ZP0` a `ZP7` fazem o mesmo para ponteiros de página zero.

## Símbolos

Um símbolo é um nome ligado a um endereço. O depurador lê tabelas de símbolos em arquivos de texto, uma linha por símbolo no formato `ENDEREÇO NOME`, com `;` para comentários (formatos de saída dos assemblers ACME e Merlin). São nove tabelas independentes, cada uma ativável à parte:

| Tabela | Arquivo |
|---|---|
| MAIN | `APPLE2E.SYM` |
| BASIC | `A2_BASIC.SYM` |
| ASM | `A2_ASM.SYM` |
| User1, User2 | `A2_USER1.SYM`, `A2_USER2.SYM` |
| Src1, Src2 | `A2_SRC1.SYM`, `A2_SRC2.SYM` |
| DOS33 | `A2_DOS33.SYM2` |
| PRODOS | `A2_PRODOS.SYM` |

As tabelas MAIN, BASIC e DOS33 vêm embutidas no programa.

| Comando | Efeito |
|---|---|
| `SYM` | Quantidade de símbolos nas tabelas |
| `SYM símbolo` ou `SYM endereço` | Procura nas tabelas |
| `SYM nome = endereço` | Define ou altera um símbolo da tabela do usuário |
| `SYMMAIN`, `SYMBASIC`, `SYMASM`, `SYMUSER`, `SYMUSER2`, `SYMSRC`, `SYMSRC2`, `SYMDOS33`, `SYMPRODOS` | Procura numa tabela; `LOAD` recarrega e `CLEAR` esvazia |
| `SYMINFO`, `SYMLIST` | Resumo e procura em várias tabelas |

## Bookmarks

`BMA` adiciona ou atualiza um bookmark (endereço marcado), `BMC` remove, `BML` lista, `BMG` leva o cursor a um bookmark, `BMSAVE` salva.

## Janelas de vídeo

`TEXT`, `TEXT1`, `TEXT2`, `TEXT80`, `TEXT81`, `TEXT82`, `GR`, `GR1`, `GR2`, `DGR`, `DGR1`, `DGR2`, `HGR`, `HGR0` a `HGR8`, `DHGR`, `DHGR1`, `DHGR2` e `SHR` mostram a memória como cada modo de vídeo (a barra invertida `\` mostra a saída do Apple até pressionar uma tecla). `VIDEOINFO` configura a informação do scanner de vídeo.

## Cores e configuração

`COLOR`, `MONO`, `BW`, `HCOLOR`, `FONT`, `DISASM`, `NTSC`, `SAVE` e `LOAD` mostram, alteram, salvam e carregam as cores e as opções de exibição do depurador. `ECHO`, `PRINT`, `PRINTF`, `LOG`, `RUN arquivo` (executa um script de comandos), `STARTUP`, `SOURCE` e `SYNC` completam o conjunto. A opção de linha de comando `-debugger-auto-run arquivo` executa um script ao entrar no depurador.

## Ajuda interna

`?` lista todos os comandos. `HELP comando`, ou simplesmente digitar o comando sem argumentos, mostra o modo de uso. Os nomes e a ajuda de cada comando vêm da tabela do AppleWin (`Debugger_Commands.cpp`).
