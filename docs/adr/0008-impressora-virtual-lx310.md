# Impressora virtual Epson LX-310: 8 bits, Arquivo de impressões permanente e janela que abre sozinha

## Contexto

A especificação [docs/prompts/AppleII_Rust_Prompt_Impressoras_Virtuais_LX310.md](../prompts/AppleII_Rust_Prompt_Impressoras_Virtuais_LX310.md) pede uma Impressora virtual Epson LX-310 ligada à Generic Printer, com janela própria e PDF fiel. O dono quer, além disso, guardar as impressões como lembrança ("carinho guardado"): listagens de BASIC, relatórios de AppleWorks e dBase II, cartões e faixas do Print Shop. Três pontos da especificação ou do AppleWin conflitam com isso e foram decididos pelo dono em 21/10/2026.

## Decisão

1. **Transporte de 8 bits na Impressora virtual.** A Generic Printer do AppleWin apaga o bit 7 na própria carta (`ParallelPrinter.cpp`, `IOWrite`: `value & 0x7F`), o que tira a agulha de cima dos gráficos ESC K/L. O Printer.txt continua com essa regra, igual ao AppleWin. A Impressora virtual recebe o byte inteiro do barramento, e uma opção põe a virtual em 7 bits.
   **Revisão de 22/10/2026 (dono): Transporte Automático como padrão.** O Applesoft, o Integer BASIC e o Monitor mandam todo caractere com o bit 7 ligado (ESC chega como `$9B`), então 8 bits puros estragavam qualquer `PR#1`. No modo Automático, o byte que o firmware da própria carta escreve na porta (a instrução está na ROM do slot, `$Cs00–$CsFF`, ou em `$C800–$CFFF`) perde o bit 7; o byte que um programa escreve direto na porta (drivers gráficos, como o do Print Shop) chega inteiro, como nas cartas de 8 bits da época. 8 bits e 7 bits fixos continuam como opção. Os filtros antigos (`filter_unprintable`, conversão Pravets) valem só para o Printer.txt. É uma Divergência aprovada (ADR 0006): não há Oráculo AppleWin para uma impressora que ele não emula.
2. **Destino padrão: virtual e Printer.txt ao mesmo tempo.** Quem usava o Printer.txt continua recebendo o mesmo arquivo.
3. **A janela da Central de Impressoras abre sozinha no primeiro byte escrito**, sem tirar o foco da janela do Apple II, com opção para desligar. A especificação recomendava o contrário; o dono preferiu ver a impressão ao vivo.
4. **O Arquivo de impressões é permanente.** Cada Trabalho de impressão fica na pasta de dados do usuário (`apple2-rust/impressoes`) com os bytes brutos, um manifesto versionado, uma miniatura, título e nota pessoal. Nada é apagado sem ordem do usuário; há só um aviso de tamanho. Reimprimir com outro Papel, Fita ou tabela gera uma nova versão; os bytes originais nunca mudam.
5. **Fonte matricial própria.** A Epson não licencia a ROM de caracteres da LX-310. O projeto desenha suas matrizes Draft e NLQ (GPL) e as documenta como aproximação; a geometria (pitch, espaçamento, margens, gráficos) segue o manual.
6. **PDF e aparência.** O PDF desenha cada agulha como ponto vetorial e leva uma camada de texto invisível para busca e cópia; os metadados (título, data, Modelo do Apple II, impressora, nota) vão sempre, e a folha de rosto é opcional e vem desligada. O Papel de fábrica é o formulário contínuo zebrado com furos de trator e picote (A4 e Carta como opção); a Fita de fábrica é nova, com fita usada (determinística por Trabalho) e cores como opções, e preto e branco nos Perfis coloridos. Tabelas de caracteres desta entrega: variantes do ESC R, PC437, PC850, PC860, BRASCII e ABICOMP.
7. **O save-state `.aws.yaml` não muda.** Carregar um estado fecha o Trabalho em andamento com esse motivo; o estado da LX-310 (cabeça, modos) não entra no formato do AppleWin.

## Consequências

- Um programa que mande bytes com bit 7 ligado para texto (sem ser gráfico) pode imprimir diferente na virtual e no Printer.txt; a opção de 7 bits resolve.
- O Arquivo de impressões cresce sem limite; o aviso de tamanho é a única proteção.
- Escopo desta entrega: fases A a D, só a LX-310. FX-80, MX-80 e ImageWriter II (serial, pela SSC) ficam para depois, cada um com seu protocolo e seus goldens.

## Aceite (22/10/2026)

O dono aprovou as fases A–D como entrega Linux da v0.3.0 com base no parecer independente final (93/100, [docs/printer/parecer-independente.md](../printer/parecer-independente.md)), com três pendências declaradas: o T-025 (tela cheia do Apple II com a Central no segundo monitor) fica pendente de evidência humana pelo roteiro [docs/conformance/manual-tests/impressora.md](../conformance/manual-tests/impressora.md); Windows e macOS não foram validados nativamente; o aceite com AppleWorks, Print Shop e dBase II (#104) aguarda os discos do dono em `target/conformance-cache/printer-apps/`. As notas da v0.3.0 dizem isso explicitamente.
