# Impressora virtual Epson LX-310: guia do usuário

A **Impressora virtual** recebe o que o Apple II imprime pela Carta de impressora (Generic Printer) e transforma em **Páginas** de pontos, como faria uma Epson LX-310 de 9 agulhas. Você acompanha a impressão na **Central de Impressoras**, salva em **PDF** e guarda tudo no **Arquivo de impressões**. O vocabulário segue o GLOSSARY.md (seção "Impressão"); a decisão de projeto está em [ADR 0008](../adr/0008-impressora-virtual-lx310.md).

Documentos irmãos:

- [protocols/lx310.md](protocols/lx310.md): matriz de comandos ESC/P (estado de cada comando).
- [fonte-matricial.md](fonte-matricial.md): fonte e tabelas de caracteres.
- [pdf.md](pdf.md): o que o PDF contém.
- [arquivo-de-impressoes.md](arquivo-de-impressoes.md): pasta, manifesto, recuperação.
- [printer-model-capabilities.md](printer-model-capabilities.md): matriz por Perfil de impressora.
- [aceite.md](aceite.md): roteiro de aceite com AppleWorks, Print Shop e dBase II.
- [matriz-de-testes.md](matriz-de-testes.md): estado de T-001 a T-030 e o teste que prova cada um.

## 1. Instalar a Generic Printer no slot 1

1. Abra a configuração (**F8**), aba **Slots**.
2. No **slot 1**, escolha **Printer** (Generic Printer).
3. Aperte **...** ao lado da Carta para abrir as opções da impressora (veja abaixo).
4. Confirme com **OK**. Mudar de Carta reinicia a Máquina (power-cycle), e o programa avisa antes.

Pela linha de comando ou pelo `settings.toml` também funciona; o slot 1 é o costumeiro porque programas da época esperam a impressora ali.

## 2. Opções da Carta

| Opção | Efeito |
|---|---|
| **Printer dump filename** | Arquivo de texto do AppleWin (`Printer.txt`). |
| **Send output to** | **Virtual printer + file** (padrão), **File only** (como o AppleWin) ou **Virtual printer only**. |
| **Transport** | **Automático** (padrão), **8 bits** ou **7 bits (AppleWin)**; veja a seção 4. |
| **Open the printer window at the first byte printed** | A Central abre sozinha no primeiro byte, sem roubar o foco do Apple II. Desligue se preferir abri-la pelo botão. |
| Opções avançadas | Conversão para clones, filtro de não imprimíveis, tempo ocioso: valem **só para o Printer.txt**. |

O Printer.txt continua exatamente como no AppleWin: texto de 7 bits, filtrado. Quem usava só ele não percebe diferença.

## 3. Imprimir: `PR#1`

No Applesoft ou no Integer BASIC:

```
PR#1
LIST
PR#0
```

Ou, dentro de um programa, `PR#1` antes dos `PRINT` e `PR#0` no fim. Programas como AppleWorks, Print Shop e dBase II (CP/M) escolhem a impressora e o slot nos próprios menus: veja [aceite.md](aceite.md).

O firmware da Carta (Apple Parallel Interface) formata a linha: se o programa não for o Applesoft ou o Integer BASIC (por exemplo uma rotina em linguagem de máquina que chama a saída direto), ele **insere espaços** para igualar a coluna do cursor do monitor, o que corrompe comandos e gráficos. Para enviar bytes exatos, mande antes `PRINT CHR$(9);"255N"` (Ctrl-I 255 N): isso desliga a formatação do firmware, e os próprios bytes `Ctrl-I...N` não chegam à impressora. Programas comerciais fazem isso sozinhos.

Um **Trabalho de impressão** começa no primeiro byte e termina quando passam 10 "segundos" da Carta sem imprimir (regra do AppleWin), no reset, ao carregar um estado, ao retirar a Carta ou quando você o conclui na Central. Cada Trabalho vira uma entrada própria no Arquivo. Um **FF** (form feed) só muda de Página; não separa Trabalhos.

## 4. Transporte: Automático, 8 bits ou 7 bits

O Apple II costuma enviar os caracteres com o **bit 7 ligado**. O AppleWin apaga esse bit na Carta (7 bits); os gráficos ESC K/L, porém, precisam do byte inteiro: a **agulha de cima** de cada coluna é o bit 7.

- **Automático (padrão)**: o que passa pelo **firmware da Carta** (`PR#1`, `PRINT`, `LIST`, o monitor) tem o bit 7 apagado; o que um programa escreve **direto na porta** da Carta ($C090 no slot 1) chega com os 8 bits. É o que se quer em quase todos os casos: o BASIC imprime texto e comandos ESC certos, e drivers de impressora de programas como o Print Shop mandam gráficos inteiros.
- **8 bits fixo**: sempre o byte inteiro. Estraga o BASIC: o Applesoft liga o bit 7 de todo caractere, então ESC chega como `0x9B` e o texto como símbolos. Use só se um programa que passa pelo firmware precisar mesmo de bit 7 (raro).
- **7 bits (AppleWin) fixo**: sempre sem o bit 7, como o AppleWin. Use se um driver que escreve direto na porta mandar texto com o bit 7 ligado (os caracteres saem como símbolos) e você não precisa de gráficos.

Se uma impressão sair estranha, mude o Transporte e imprima de novo: o Arquivo guarda os bytes e o Transporte de cada Trabalho. O `Printer.txt` é sempre 7 bits. Os testes de aceite (`crates/apple2-hostio/tests/printer_aceite_*.rs`) provam os três modos.

## 5. A Central de Impressoras

- O botão **Printer** da barra de ferramentas existe enquanto houver uma Generic Printer que envia à impressora virtual e abre (ou traz para a frente) a Central; para fechá-la use o botão de fechar da própria janela. Ela também abre sozinha no primeiro byte (opção da seção 2).
- Fechar a Central **não interrompe nada**: o serviço de impressão continua rodando e as Páginas ficam no Arquivo. Ao reabrir, tudo está lá.
- A prévia mostra a Página como o papel físico: pontos, furos de trator, picote. Há zoom, ajuste à largura e navegação entre Páginas, e a impressão aparece à medida que chega.
- No alto ficam o **estado** (online, recebendo, aguardando, concluído, erro), o Modelo do Apple II e o slot, o Perfil (Epson LX-310) e o Transporte do Trabalho. Os botões **Salvar PDF…**, **Salvar RAW…**, **Concluir trabalho** e **Cancelar trabalho** (pede confirmação; fecha o Trabalho e o marca como cancelado, sem apagar nada) ficam desabilitados, com uma dica, quando não se aplicam. As abas são **Visualização** e **Arquivo de impressões**.
- Mostra também os **diagnósticos** (seção "Diagnósticos" sob a barra da prévia): sequências que a LX-310 não entende ou que o projeto não implementa aparecem com o offset e os bytes, nunca em silêncio.

## 6. Papel e Fita

Mudam só o **visual**: nunca a posição dos pontos.

| | Opções |
|---|---|
| **Papel** | Formulário contínuo zebrado verde (padrão, 9,5" x 11", furos e picote), zebrado azul, contínuo liso, **A4**, **Carta** |
| **Fita** | **Nova** (pontos nítidos e uniformes, padrão) ou **usada** (intensidade irregular, sempre igual para o mesmo Trabalho) |
| **Cor** | Preto (a LX-310 é monocromática); azul e roxo como estilo |
| **Tabela** e **País** | Tabela de caracteres (PC437 padrão, PC850, PC860, BRASCII, ABICOMP, itálico) e conjunto internacional (ESC R) com que cada Trabalho começa; são as chaves DIP da impressora, gravadas nas opções do Trabalho. O programa ainda pode trocá-los, e ESC @ volta a essa tabela e a esse país (não ao padrão de fábrica) |

Papel, Fita, Cor e Tabela se escolhem na Central (linha sob o estado) ou nas opções da Carta, e **valem para o próximo Trabalho**: o Trabalho em andamento continua com o que tinha ao começar. A prévia desenha o Papel e a Cor do próprio Trabalho.

Para ver um Trabalho já impresso com outro Papel, Fita ou Cor use **Reimprimir** na aba **Arquivo de impressões**: gera uma nova **versão** (veja a seção 8). A reimpressão também oferece **Tabela** e **País**; a prévia e a reimpressão de um Trabalho antigo usam as opções gravadas nele, não as atuais da Carta.

## 7. Salvar PDF

**Salvar PDF…** abre um formulário (título, nota, todas as Páginas ou um intervalo, folha de rosto sim/não) e depois o diálogo de arquivo; título e nota também ficam no Arquivo. Exporta o Trabalho (ou só algumas Páginas) com:

- cada agulha como ponto vetorial: o zoom do leitor nunca pixeliza;
- uma **camada de texto invisível**, para buscar e copiar texto no leitor de PDF;
- metadados sempre gravados: título, data, Modelo do Apple II, impressora, nota pessoal e versão do programa;
- **folha de rosto** opcional (desligada por padrão);
- a escolha de Papel e Fita (tamanho da página do PDF = tamanho do Papel).

A gravação é atômica: o arquivo só aparece completo. Exportar um Trabalho em andamento gera uma fotografia coerente e deixa o Trabalho aberto. Detalhes em [pdf.md](pdf.md).

## 8. Arquivo de impressões

Todo Trabalho fica guardado, na pasta de dados do usuário (`apple2-rust/impressoes`):

| Sistema | Caminho típico |
|---|---|
| Linux | `~/.local/share/apple2-rust/impressoes` |
| macOS | `~/Library/Application Support/org.AppleII-Rust.apple2-rust/impressoes` |
| Windows | `%APPDATA%\AppleII-Rust\apple2-rust\data\impressoes` |

- **Nada é apagado sozinho.** Só você apaga (com confirmação). Se o Arquivo ficar grande, o programa apenas avisa.
- Cada Trabalho tem os **bytes brutos exatos** (`raw.bin`, com SHA-256 no manifesto), título (editável), nota pessoal, data e Modelo do Apple II.
- Se o programa fechar de repente, o Trabalho volta como "interrompido", até o último ponto de controle gravado (a cada 32 KiB ou 2 s), sem bytes inventados.
- A lista mostra o **título** (primeira linha impressa + data, editável), não o id; o id do Arquivo aparece em dica e no detalhe.
- **Concluir trabalho** fecha o Trabalho na própria Carta (motivo `User`): o próximo byte abre um Trabalho novo. Se a emulação estiver pausada e a Carta não responder em 1,5 s, o serviço fecha o Trabalho sozinho. **Cancelar trabalho** faz o mesmo e marca o Trabalho como *cancelado*; os bytes, o título e as versões continuam no Arquivo (só **Apagar trabalho…** remove).
- Cada PDF ou cópia RAW exportada é registrada no manifesto na hora.
- Na aba **Arquivo de impressões**: lista (título, data, máquina, Transporte, Páginas, estado), edição de título e nota, versões (**Ver**, **Exportar PDF…**), **Reimprimir** e **Apagar trabalho…** (com confirmação; o Trabalho em andamento não pode ser apagado).
- **Reimprimir** com outro Papel, Fita, Cor, Tabela ou País gera uma **nova versão**; os bytes originais nunca mudam.

Estrutura e manifesto em [arquivo-de-impressoes.md](arquivo-de-impressoes.md).

## 9. O que é fiel e o que é aproximação

**Fiel** (segue o manual da LX-310 e os testes de posição exata): posições de impressão em 1/2160", passos de linha e de caractere (10, 12, 15 cpi, condensado, largura dupla), margens, tabulações, comprimento de página, FF, gráficos de bit-image (ESC K, L, Y, Z, `*`, `^`) e o tratamento dos comandos marcados **SUPPORTED** em [protocols/lx310.md](protocols/lx310.md).

**Aproximação** (documentada, não copiada da Epson):

- a **fonte matricial** (Draft e NLQ) é desenho próprio; os caracteres podem diferir dos da ROM da LX-310;
- o **tamanho do ponto** (0,35 mm) e o desenho do **papel** (cor das faixas, furos) são estimados;
- **ênfase**, **dupla batida**, **itálico** e **largura dupla** têm geometria aproximada onde o manual não dá números (itens **PARTIAL** do protocolo);
- os **caracteres definidos pelo usuário** (`ESC &`, `ESC %`) não são impressos: a carga é consumida pelo comprimento exato, para não sair como lixo, e há um diagnóstico "caractere do usuário não suportado";
- com **duas Cartas** de impressora (por exemplo nos slots 1 e 2), cada Trabalho carrega o slot de origem, que a Central mostra na lista; os bytes de uma Carta nunca entram no Trabalho da outra;
- a **Fita usada** é um efeito visual determinístico, não uma medida de desgaste.

## 10. Limites

- Só o Perfil **Epson LX-310** (9 agulhas, monocromática). FX-80, MX-80 e ImageWriter II são trabalho futuro ([printer-model-capabilities.md](printer-model-capabilities.md)).
- Comandos reconhecidos mas não implementados (por exemplo código de barras e alguns `ESC (`) geram diagnóstico; nada é impresso por eles.
- Por Trabalho: até 5.000 Páginas, 64.000.000 pontos (2.000.000 por Página) e 10.000 diagnósticos (acima disso o motor avisa uma vez e para de acumular aquilo, mas os bytes brutos continuam sendo gravados e as outras Páginas imprimem). Uma listagem de centenas de Páginas imprime inteira.
- **Memória e fila**: a Página guarda pontos vetoriais, não raster; o pior caso adversarial por Trabalho é ~768 MiB de pontos no motor, e o pico de RSS medido é 769 MiB porque a prévia e o fechamento compartilham as Páginas (`Arc<Page>`, ver [desempenho.md](desempenho.md)); uma listagem de 100 Páginas usa ~60 MB. A fila entre a Carta e o Spool aceita 256 MiB não consumidos; acima disso os bytes excedentes são descartados, contados e avisados (nunca em silêncio) e a CPU do Apple II não espera. "Salvar bytes brutos" recusa Trabalhos acima de 512 MiB.
- **Arquivos hostis**: um Trabalho com manifesto ilegível, de schema futuro ou com `raw.bin` inconsistente aparece como **danificado**, com o motivo, e não pode ser exportado; os bytes ficam no disco.
- Medições de carga e de custo na Carta: [desempenho.md](desempenho.md).
- A impressão é um dispositivo de **saída**: não há leitura de status real da impressora (sem papel, falha), e o save-state `.aws.yaml` não guarda o estado da impressora. Carregar um estado fecha o Trabalho em andamento com o motivo "estado carregado".
