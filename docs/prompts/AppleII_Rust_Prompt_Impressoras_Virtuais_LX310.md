# PROMPT MESTRE — AppleII-Rust: Central de Impressoras Virtuais (Epson LX-310 obrigatória)

**Tipo:** Especificação funcional e técnica executável; épico, histórias de usuário, BDD, arquitetura, testes, critérios de aceite e gates de release.  
**Data de elaboração:** 21/10/2026.  
**Projeto:** https://github.com/ChicoFigueiredo/AppleII-Rust  
**Referência upstream:** https://github.com/AppleWin/AppleWin  
**Referência de testes:** https://github.com/AppleWin/AppleWin-Test  
**Baseline inspecionada:** `main` em `b2c4137cad4c4d4b2e99c6b9e9e695ee3990849e` e branch `release/v0.2.0-rc.1` em `9960602eff98c6f3212ecda71cd6b54e4817e7af`; os arquivos de impressora examinados tinham SHAs iguais nas duas referências.  
**Grau de confiança:** evidência de leitura de código e documentos, **não** validação de execução do aplicativo nesta sessão. Antes de implementar, atualizar as referências/SHAs e reconferir mudanças mais recentes.

---

## 0. ORDEM DIRETA AO TIME

Atuem como equipe de engenharia de emulação retro, Rust desktop, protocolos de impressoras matriciais e QA independente. **Implementem uma Central de Impressoras Virtuais integrada ao AppleII-Rust**, cujo comportamento primário é transformar os bytes realmente produzidos pelo Apple II na placa de impressora em páginas visualizadas **em tempo real** numa **janela separada da janela do computador emulado**, e permitir exportá-las a um PDF fiel.

**A Epson LX-310 é OBRIGATÓRIA** e deve ter fidelidade de comandos e saída matricial demonstrada por testes, não apenas mudar o nome de um perfil genérico. Incluir modelos históricos relevantes do Apple II de forma incremental e protocolarmente correta, sem sacrificar a LX-310. O acionamento deve partir da impressão executada pelo sistema Apple II (por exemplo `PR#1` e `PRINT`), sem necessidade de copiar a tela, carregar um arquivo externo ou invocar exportação manual. O usuário pode manter a janela fechada e abrir pelo botão/indicador de impressora sem perder dados.

**Restrições:** preservar a emulação determinística, as opções e os snapshots existentes, o comportamento AppleWin quando o modo virtual estiver desabilitado e a compatibilidade GPL-2.0-or-later. Não esconder incompatibilidades conhecidas sob a expressão “emulação completa”. Entregar código, testes, exemplos reproduzíveis, documentação e evidências; não apenas propostas ou scaffolding.

## 1. PONTO DE PARTIDA VERIFICADO — NÃO REFAZER O QUE EXISTE

1. `crates/apple2-hostio/src/printer.rs`: `ParallelPrinterCard`, trait `PrinterSink` (`open/write/close`), `FileSink`, `MemorySink`, temporização de inatividade, `PrinterOptions`, erros. `io_write` em `$C0s0` envia bytes ao sink, mas executa `value & 0x7F` e opcionalmente filtra valores de controle. `io_read` devolve `0xFF` e pode abrir o trabalho; distinguir ativação por leitura de **primeira escrita imprimível**.
2. `crates/apple2-hostio/tests/printer.rs`: testes reais de `PR#1`, `PRINT "HELLO PRINTER"`, porta I/O, append, timeout, reset e ROM do slot. Aproveitar como regressão.
3. `crates/apple2-config/src/card.rs`: `PrinterSpec` existente (`dump_to_printer`, `convert_encoding`, `filter_unprintable`, `append`, `filename`, `idle_limit`). O padrão de `filter_unprintable` no frontend é **true**, o que elimina `ESC=0x1B` e `FF=0x0C`; incompatível com decodificação ESC/P se essa opção for aplicada ao novo caminho virtual. A opção `convert_encoding` para Pravets também exige tratamento separado.
4. `crates/apple2-rust/src/factory.rs`: `printer_card()` sempre instancia `FileSink`; a fábrica precisará receber/injetar um sink/serviço de impressão, **sem referência direta a egui no núcleo**.
5. `crates/apple2-rust/src/emu.rs`: thread dedicada ao emulador, comandos e estado compartilhado com UI. Não bloquear execução de ciclos por renderização/PDF/I/O; evitar mutex de grande duração e fila ilimitada.
6. `crates/apple2-rust/src/ui.rs`, `app.rs`, `toolbar.rs`, `actions.rs`: frontend `winit` + `wgpu` + `egui 0.31`; há janelas internas egui de opções, mas **não existe Central de Impressoras**. Introduzir botão/indicador de atividade e gerenciar segunda janela nativa.
7. `crates/apple2-rust/src/settings/options.rs`: já existe diálogo `Printer Card` com arquivo de dump, append, conversão, filtro de caracteres e timeout; preservar e ampliar sem perder compatibilidade.
8. `crates/apple2-savestate/src/cards/printer.rs`: salva/restaura opções da `Generic Printer` e o nome personalizado do arquivo. Não mudar silenciosamente o schema AppleWin `.aws.yaml` para armazenar dados da GUI; separar snapshot do hardware e estado dos trabalhos de impressão.
9. `docs/conformance/release-readiness.md`: a equipe registrou a branch `release/v0.2.0-rc.1` como **release candidate Linux**, porém o mesmo documento relata 84/100 em autoavaliação, 77/100 no parecer independente, Windows/macOS ainda sem validação nativa e gates de beta/stable não satisfeitos. **Não alegar versão estável** com base apenas no nome RC. Em consulta às releases públicas, a última release publicada era `v0.1.0`; a tag pública `v0.2.0-rc.1` não foi encontrada naquela consulta.
10. **Escopo de trabalho:** esta feature é uma evolução específica, não uma reabertura indiscriminada de todas as issues de conformidade anteriores. A equipe deve conferir que o histórico e os testes anteriores continuam íntegros.

### 1.1 Distinção crucial — bytes da placa vs. bytes da impressora

A placa paralela do Apple II e a impressora conectada são **dois dispositivos/protocolos distintos**. Algumas ROMs/placas e modos de software trabalham em 7 bits; gráficos ESC/P podem exigir bytes com bit 7 ligado. É proibido simplesmente remover a máscara de 7 bits globalmente e declarar problema resolvido, porque isso pode alterar compatibilidade AppleWin. A solução deve:

- capturar e registrar, quando tecnicamente possível, o byte **antes e depois** da transformação pelo firmware/modelo de interface;
- permitir perfis de transporte explícitos (`legacy_applewin_7bit`, `raw_8bit` quando suportado pelo tipo de interface/driver), com suporte e limitações declarados;
- oferecer ao parser virtual **bytes de controle e dados binários intactos após a transformação de transporte apropriada**, independentemente do filtro destinado ao dump de texto;
- manter a transformação Pravets em um componente identificado, não aplicar transliteração em sequência binária de gráficos;
- demonstrar a rota de **todos os 256 valores de byte** nos testes de captura/decodificação (quando o modo de transporte suportar 8 bits), com testes específicos para 0x00, 0x0A, 0x0C, 0x0D, 0x1B, 0x7F, 0x80, 0xFF.

É aceitável documentar que um **firmware legado específico** só transmite sete bits e que software de gráficos binários precisa de interface/modo apropriado. **Não é aceitável afirmar fidelidade gráfica total no cenário incapaz de transmitir os bytes necessários.**

## 2. CONTRATO DE PRODUTO E EXPERIÊNCIA

### 2.1 Comportamento esperado

- Ao executar `PR#1` / `PRINT` ou programa que escreva na porta da placa, os dados chegam ao spooler virtual e a Central de Impressoras indica atividade instantaneamente. A janela separada pode surgir automaticamente (preferência), ou permanecer oculta e ser aberta pelo botão dedicado.
- A janela possui **seletor de modelo**, **status (online/offline, recebendo, aguardando, concluído, erro)**, **modelo do Apple II e slot**, **papel selecionado**, **qualidade**, **densidade**, **página atual/total**, **prévia visual**, **zoom**, **navegação entre páginas**, **trabalhos de impressão**, **salvar PDF**, **exportar RAW**, **limpar/cancelar com confirmação**, **nova folha/FF** e diagnóstico de sequências não suportadas.
- Impressão conforme o usuário digita no Apple II e conforme programas rodam. O conteúdo deve surgir antes de o trabalho terminar; o PDF definitivo é gerado sob demanda ou ao finalizar, não após cada byte.
- Janela independente no SO: mover, redimensionar, minimizar, usar segundo monitor, fechar e reabrir sem encerrar a emulação. Uma janela `egui::Window` **dentro da mesma janela** não satisfaz o requisito de janela independente; avaliar `egui` multi-viewport com `winit` ou criar uma segunda janela `winit` com `egui_winit`/`egui_wgpu`, documentando custo e compatibilidade.
- O papel deve se comportar como página física, incluindo margem, retorno de carro, avanço de linha, avanço de formulário, posição X/Y, corte lógico/contínuo e paginação. Em modo visual matricial, os pontos precisam ser distinguíveis ao zoom, sem usar uma fonte moderna como falso substituto de toda impressão.

### 2.2 Modelos e fronteiras de compatibilidade

| Perfil | Interface histórica para o Apple II | Prioridade | Contrato mínimo |
|---|---|---|---|
| **Epson LX-310 — 9 agulhas** | Paralela ou serial conforme combinação de interface; usar primeiro a Generic Printer paralela, explicitando limites do adaptador | **P0, obrigatório** | ESC/P 9-pin; 10/12/15/17/20 CPI; Draft/NLQ com amostras; margens, passos de papel, gráficos bit-image; tabela de caracteres; saída PDF |
| Epson FX-80 | Paralela (interface Apple II compatível) | P1 | Subconjunto **verificado** de ESC/P histórico, 9 agulhas, gráficos e fonte com testes |
| Epson MX-80 / MX-80 II | Paralela (interface compatível) | P1 | Comandos históricos efetivamente documentados, especialmente ESC K/L e gráficos de 8 agulhas; não assumir semântica idêntica à LX-310 |
| Apple ImageWriter II | **Serial** via SSC/placa equivalente, não a placa paralela genérica | P2, fase posterior | Perfil e interpretador específicos + conexão serial virtual; cor apenas se comprovada e implementada; testes independentes |
| Apple DMP / ImageWriter I / Silentype | De acordo com hardware original e protocolo | Backlog pós-MVP | Somente após especificação de interface, comandos, fixtures e aprovação de escopo |

**Importante:** ESC/P da LX-310 **não é ESC/POS** de impressoras térmicas Epson TM. Não escolher bibliotecas ESC/POS como parser da LX-310. Da mesma forma, ESC/P 9-pin e ESC/P 2 não são sinônimos; os comandos de cada modelo devem ser declarados em uma matriz de suporte.

### 2.3 Base documental do hardware LX-310

Referências de fonte primária Epson:

- Manual oficial: https://support2.epson.net/manuals/english/sidm/lx_310/index.html
- Especificações Epson: https://download.epson.com.sg/product_brochures/dot_matrix/ESD/Epson_LX310-R1_sg.pdf
- Referência histórica ESC/P 9-pin / ESC/P 2: https://files.support.epson.com/pdf/general/escp2ref.pdf
- FX-80 Epson original: https://files.support.epson.com/pdf/fx80__/fx80__u1.pdf
- MX-80 Epson original: https://files.support.epson.com/pdf/mx802_/mx802_u1.pdf
- ImageWriter II, comandos distintos e interface serial: https://www.virtualii.com/VirtualIIHelp/virtual_II_help.html

Segundo as especificações Epson, a LX-310 é de impacto, 9 agulhas, ESC/P + IBM PPDS, Draft e NLQ, pitch 10/12/15/17/20 CPI, 80/96/120/137/160 CPL correspondentes, espaçamento programável de 1/216”, interfaces paralela, USB e serial, fita padrão preta, buffer físico de 128 KB e papel contínuo e avulso. Esses números são **referências de comportamento/limites**; o spooler do computador host pode usar buffers maiores sem se passar pela memória interna da impressora. Perfis regionais de codificação (incluindo PC850, PC860, BRASCII, ABICOMP) exigem golden tests antes de afirmar fidelidade.

## 3. ARQUITETURA-ALVO

Separar rigorosamente estas responsabilidades:

```text
APPLE II (software / PR#slot / ROM de interface)
  -> Card::io_write e protocolo do adaptador
     -> PrinterPortAdapter (preserva semântica do hardware original)
        -> PrinterCaptureSink / TeeSink [bytes raw + metadados + eventos]
           -> canal ordenado e com backpressure observável
              -> PrintSpoolService / JobManager (fora do frame loop)
                 -> PrinterProtocolEngine (parser ESC/P 9-pin, por perfil)
                    -> PrinterLayout (cursor, margens, fita, papel, dots)
                       -> PageDocument (vetores/raster em resolução definida)
                          -> PreviewSnapshot incremental -> janela nativa independente
                          -> PdfExporter assíncrono -> arquivo .pdf final
                          -> RawArchive / metadados do job / recovery

Opcional, sem quebrar compatibilidade:
  -> Legacy FileSink (Printer.txt)
  -> HostPrintSink (somente sob ação/consentimento explícitos)
```

- Sugerir um crate `crates/apple2-printer` (domínio puro, sem winit/egui) para **protocolos, layout, spool, exportação e testes**. Se PDF exigir dependências incompatíveis com WASM/headless, separá-lo em camada host.
- Definir `PrinterModel` / `PrinterProtocol` / `TransportMode` / `PrintJob` / `JobEvent` / `PageDocument` / `PrinterState` / `RenderOptions`. Sem enums genéricos que confundam interfaces serial e paralela.
- O job precisa registrar `job_id` estável, origem (modelo da máquina, slot/card), `started_at`, `completed_at` opcional, modelo de impressora, opções congeladas ao iniciar, formato do papel, status, bytes capturados, páginas, warnings, contadores e revisões da prévia. **Não usar apenas `PrinterSnapshot` para armazenar spool.**
- Preferir eventos ordenados `Open`, `Bytes(Vec<u8>)`, `Flush`, `FormFeed`, `Close(reason)`, `Error`, `Cancel` com `sequence_no` por job e offsets. `Close` da UI **não** equivale a encerrar trabalho.
- Implementar captura com **TeeSink** ou `VirtualPrinterSink`, reutilizando a `PrinterSink` e preservando `FileSink` como sink legado opcional. Receber bytes como blocos (batch) para não alocar/redesenhar por byte. Não fazer escrita de PDF nem rasterização pesada dentro de `Card::io_write`.
- Multi-window `winit`: gerir ciclo de vida da janela principal e da impressora de forma explícita; caches GPU e texturas por dispositivo/contexto; tratar foco do teclado para que comandos digitados na Central **não sejam enviados ao Apple II**; se arquitetura de render exigir instância compartilhada, documentar os invariantes.
- O renderer incremental deve aceitar página parcial (`in_progress`) e desenhar apenas regiões modificadas ou atualizar de forma agregada e limitada a 30–60 FPS. O modelo de documento usa unidades físicas (pontos PDF, milímetros ou polegadas) para evitar desvio acumulado.
- O exportador PDF transforma o **mesmo `PageDocument` aprovado na prévia** em PDF, garantindo equivalência. Para pontos de impressão, desenhar dots ou imagem em resolução suficiente; metadados consistentes; texto selecionável apenas se puder coexistir com fidelidade do traçado. PDF deve incluir páginas completas e, se solicitado, a página ainda em construção num snapshot consistente.
- Spool persistente: log raw append-only + manifest JSON/TOML versionado, hashes/offsets, fechamento explícito, recuperação após falha, rotação e limites de retenção; impedir que a limpeza apague ou corrompa trabalhos ativos.
- **Nenhuma impressão física automática:** o legado `dump_to_printer`/`Enable Dump To Real Printer` precisa de comportamento separado, explicitamente revisado. Em nenhuma circunstância enviar sequência ESC/P desconhecida diretamente à impressora real por estar apenas visualizando.

## 4. REGRAS FUNCIONAIS GLOBAIS (INVIOLÁVEIS)

**R01 — Determinismo:** dado `(raw_bytes, printer_profile, transport, paper, settings version)`, o documento lógico gerado deve ser idêntico em todos os SO; timestamps e caminhos externos não influenciam o conteúdo.

**R02 — Sem perda silenciosa:** bytes não reconhecidos geram diagnóstico com offset/hex e tratamento configurado (`warn_ignore_command`, `stop_job` para payload inválido grave) sem confundir dados binários com texto; não ignorar silenciosamente ESC.

**R03 — Protocolos diferentes:** seleção de modelo muda parser/estado, nunca só o cabeçalho de UI. Trocar modelo durante job exige concluir o atual e iniciar outro, ou **reprocessar explicitamente** o raw histórico em uma cópia; escolha do usuário registrada.

**R04 — Separação de fim de trabalho:** `FF` cria nova página mas não encerra obrigatoriamente o job; `CR`, `LF` e `CRLF` seguem semântica de perfil, sem linhas em dobro. Timeout configurável fecha job mas não substitui o FF; `PR#0` só finaliza se houver sinal real documentado de fechamento, ou por timeout; não supor controle implícito que a placa não recebe.

**R05 — Papel completo:** A4, US Letter e papel contínuo 80 colunas, com comprimentos personalizados dentro de intervalos validados. Troca de orientação, tamanho e margens vale para novo job, salvo comando de hardware de mudança de página documentado.

**R06 — Fidelidade de pontos:** modos Draft, NLQ, condensado, negrito, sublinhado e bit-image precisam produzir efeitos perceptíveis e verificáveis na geometria de dots. Reutilizar glifos genéricos apenas no modo “prévia simplificada”, sempre identificado como **não fiel**.

**R07 — Sem travar a emulação:** qualquer falha da Central (fechada, lenta, PDF sem espaço, falta de fonte, parser adversarial) não provoca deadlock do Apple II; entregar status e política de backpressure explícita. Se o modo escolhido não puder garantir perda zero sob carga ilimitada, impedir a configuração ou informar limitação antes do uso.

**R08 — Independência de janela:** abrir/minimizar/fechar a janela de impressora não pausa, reinicia nem mata a janela principal ou o emulador; reabrir mostra os jobs anteriores.

**R09 — Segurança:** nomes de arquivo, PDFs e spool aceitam espaços, acentos, `#` e Unicode; exportação atômica/sem sobrescrita acidental; limites para páginas, sequências e gráficos; escapes não executam comandos externos. Nada de usar `cmd /C copy ... prn` para a função PDF.

**R10 — Compatibilidade:** preservação dos YAML AppleWin anteriores, opções antigas, casos Pravets e testes de `PR#1`, snapshot e FileSink. Modo legado mantém seu resultado até o usuário escolher o virtual.

**R11 — Acessibilidade e ergonomia:** controles legíveis e operáveis por teclado, tooltip das opções, zoom e paginação, nomes de trabalho identificáveis; nenhuma animação sonora simulada obrigatória.

**R12 — Observabilidade:** para cada job medir bytes/s, filas, páginas, comandos aceitos/ignorados, latência do primeiro traço, erros, duração; disponibilizar logs estruturados ou relatório sem dados privados não necessários.

---

## 5. ÉPICOS E HISTÓRIAS DE USUÁRIO (HU)

Formato: **Como [ator], quero [resultado], para [valor]**; critérios de aceite são compromissos verificáveis com exemplos BDD. Prioridade P0 = bloqueia MVP, P1 = primeira evolução, P2 = opcional após aceite do núcleo.

### ÉPICO IMP-00 — Baseline, proteção de regressão e análise da release (P0)

**HU-IMP-001 — Preservar a funcionalidade existente.** Como mantenedor, quero provar que a nova Central não altera a emulação e a impressão legada, para continuar usando o produto sem regressões.

**Critérios de aceite**
- CA01: registrar SHA exato de `main` e `release/v0.2.0-rc.1`, estado do worktree, toolchain, sistemas testados e resultados da suíte antes da mudança.
- CA02: executar e guardar evidência de `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast`; falhas anteriores e posteriores identificadas separadamente.
- CA03: rodar `crates/apple2-hostio/tests/printer.rs`, testes de cartões/snapshots e smoke test Apple II. Sem regressão em `PR#1`, envio de byte, flush, timeout, append, reset, caminhos personalizados ou codificação Pravets.
- CA04: atualizar matriz `docs/conformance/` sem trocar FAIL/BLOCKED para PASS sem evidência. Revisão do release-readiness por verificador independente.

**BDD:** Dado o modo legado selecionado, quando rodar os fixtures existentes, então os bytes e efeitos do `FileSink` devem ser os mesmos da baseline, sem criação compulsória de janela ou PDF.

### ÉPICO IMP-01 — Captura, transporte e compatibilidade (P0)

**HU-IMP-010 — Receber o fluxo da placa de impressora.** Como usuário, quero que qualquer impressão enviada pela placa instalada no slot chegue ao spool virtual, para não precisar copiar uma tela ou arquivo.

**Critérios de aceite**
- CA01: `PR#1` + `PRINT "OLA, IMPRESSORA"` gera um job com bytes verificáveis, origem `slot=1` e uma página com o texto na prévia, enquanto a máquina continua executando.
- CA02: primeira **escrita** de impressão marca atividade e pode abrir a janela; leitura isolada de status nunca cria conteúdo impresso fantasma.
- CA03: capturar o byte original e o efetivamente entregue ao dispositivo, quando disponíveis; não confundir limitações da ROM/cartão com bug do parser.
- CA04: nos testes com transporte `raw_8bit`, transmitir as 256 combinações sem perda/truncamento; `legacy_applewin_7bit` continua com sua regra explícita.
- CA05: escape `1B 40`, LF `0A`, CR `0D`, FF `0C`, NUL `00` e blocos binários são recebidos no parser virtual mesmo quando a opção legada `filter_unprintable=true` está ligada; o filtro textual legado continua independente.
- CA06: múltiplas escritas são ordenadas deterministamente; reinicialização ou save-state não duplica o bloco anterior. Cada evento tem offset/correlator.

**BDD:** Dado `PR#1` ativo e a LX-310 virtual selecionada, quando o Apple II envia `[0x1B,0x40,0x41,0x0D,0x0A]`, então a engine registra `ESC @`, imprime `A` e movimenta o papel uma vez conforme CR/LF configurados, sem mostrar `@` como texto literal de escape.

**HU-IMP-011 — Preservar modos existentes da Printer Card.** Como usuário legado, quero continuar podendo imprimir em `Printer.txt` e restaurar minhas opções.

**Aceite:** `virtual_only`, `legacy_file_only` e `tee_virtual_plus_file` comportam-se como configurado; dump bruto em arquivo e PDF são destinos diferentes; opções antigas migram com defaults idempotentes; `Print Filename` no YAML continua retrocompatível; `dump_to_real_printer` não dispara por seleção de PDF.

### ÉPICO IMP-02 — Emulador Epson LX-310 / ESC/P 9-pin (P0 — mais importante)

**HU-IMP-020 — Interpretar comandos fundamentais.** Como usuário de software Apple II, quero que a LX-310 entenda comandos de impressora, para imprimir com disposição e formato corretos.

**Aceite — matriz obrigatória para MVP:**

| Comando | Efeito esperado e mínimo demonstrável |
|---|---|
| `ESC @` (`1B 40`) | Inicializa estado de formato conforme manual; **não** apaga dots já impressos nem fecha job sem motivo |
| `CR` (`0D`) | Retorna ao início da linha/margem; sem avanço de papel implícito salvo perfil documentado |
| `LF` (`0A`) | Avança na distância de linha ativa; combinações CR/LF não geram linha dupla |
| `FF` (`0C`) | Vai para topo da próxima folha/segmento; preserva documento acumulado |
| `HT` (`09`) e `ESC D` | Tabulações horizontais reais; NUL terminador tratado como sintaxe, não glifo |
| `ESC 0`, `ESC 2`, `ESC 3 n`, `ESC A n`, `ESC J n` | Espaçamentos/avanços físicos conforme documentação e perfil LX-310; parâmetros e unidades corretos |
| `ESC C n`, `ESC C 0 n`, `ESC l n`, `ESC Q n`, `ESC N`, `ESC O` | Comprimento de formulário, margens e perfuração, com validação de ranges |
| `ESC $ nL nH`, `ESC \\ nL nH` | Posicionamento horizontal absoluto/relativo, observando unidades corretas para o modelo |
| `ESC P`, `ESC M`, `ESC g`, `SI`, `DC2`, `SO`, `DC4`, `ESC W n` | Pitch 10/12/15/17/20 CPI, condensado e expansão, sem heurística baseada só em largura de fonte |
| `ESC E`, `ESC F`, `ESC G`, `ESC H`, `ESC - n`, `ESC ! n` | Ênfase, dupla batida, sublinhado e seleção combinada de modos; prioridade correta |
| `ESC 4`, `ESC 5`, `ESC R n`, `ESC t n` | Itálico/internacional/tabela de caracteres, apenas versões suportadas e comprovadas |
| `ESC K nL nH`, `ESC L`, `ESC Y`, `ESC Z`, `ESC * m nL nH` | Gráficos bitmap: ler **exatamente** a quantidade de bytes definida, respeitando densidade e bit order; dados 0x1B são dados quando em payload |

- CA01: possuir especificação `docs/printer/protocols/lx310.md` com formato/params, fonte primária, classes `SUPPORTED`, `PARTIAL`, `UNSUPPORTED`; comandos não verificados não ganham status SUPPORTED.
- CA02: implementar parser incremental por máquina de estados (`Normal`, `Esc`, `Params`, `Delimited`, `GraphicsPayload` etc.) que aceite sequências partidas em blocos arbitrários, incluindo 1 byte por chamada.
- CA03: nenhuma sequência desconhecida consome o próximo texto arbitrariamente; estratégia documentada de ressincronização e casos de entrada truncada.
- CA04: zero pânico/loop infinito/memória descontrolada com sequência incompleta, comprimento inválido, overflow ou gráfico longo; limites configurados e testes fuzz/property-based.
- CA05: comportamento com `ESC @` e reset físico, modo de pitch, margens e impressão parcial comprovado com goldens por comando; não basta assert de texto.
- CA06: a renderização distingue Draft e NLQ e níveis de intensidade por configuração; para declarar fidelidade de glifos é preciso referência de padrões/golden compatível com a licença. Sem glyph ROM autorizado, rotular a fonte como aproximação e não prometer reprodução bit-a-bit.

**BDD:** Dado o parser com `ESC K 08 00` (oito colunas), quando receber 8 bytes de bit-image em pedaços de 1, 2 e 5 bytes, incluindo `0x1B` no meio, então deve construir exatamente oito colunas de pontos, não executar `ESC` interno e retomar interpretação textual no nono byte.

**HU-IMP-021 — Papel e posicionamento realistas.** Como usuário, quero visualizar páginas com margens e alimentação contínua para conferir formulários e listagens.

**Aceite:** A4, Letter e contínuo parametrizável; X/Y em unidades físicas sem erro acumulativo perceptível após 1.000 linhas; página nova por FF; soft page break por limite físico; preset “formulário 80 colunas”; overscan de margem detectado/relatado; retrato e paisagem com novo job; escala visual e exportação idênticas nas coordenadas.

**HU-IMP-022 — Codificações e português brasileiro.** Como usuário brasileiro, quero imprimir acentos e caracteres de programas antigos com a tabela certa.

**Aceite:** tabela CP437, CP850, CP860, BRASCII e ABICOMP **onde o modelo selecionado as suporta**; seleção de página ESC/P por comando; glifos com golden (incluindo `Ç`, `ç`, `á`, `ã`, `é`, `ó`, `ú`), preservando os bytes raw. `UTF-8` não é implicitamente aplicado a dados de impressora de 8 bits. Em cenário incapaz de representar o caractere, warning visível e fallback configurável; sem substituição silenciosa por `?` no arquivo bruto.

**HU-IMP-023 — Gráficos matriciais e linhas de formulário.** Como usuário de aplicativos de desenho/planilha, quero imprimir gráficos e caracteres personalizados, para ver o que o software realmente mandou para a LX-310.

**Aceite:** ESC K/L/Y/Z e `ESC *` geram padrões de 8/9 pinos e densidades apropriadas; teste de duas linhas contíguas sem transposição de bits, coluna perdida ou dots deslocados; comparação de raster em DPI físico conhecido; faixa da imagem em PDF visualmente correspondente ao preview.

### ÉPICO IMP-03 — Janela nativa independente e fluxo de trabalho (P0)

**HU-IMP-030 — Ter minha impressora ao lado do Apple II.** Como usuário, quero janela própria de impressora para acompanhar listagens em outro monitor.

**Aceite:** botão “Impressora” no toolbar/status; status visível em qualquer momento em que haja impressora virtual; janela **nativa separada** redimensionável e movimentável independentemente; `Autoabrir no primeiro byte útil` configurável, **default desativado** para não interromper o jogo/trabalho; fechar/reabrir mantém páginas; fechar principal encerra recursos ordenadamente; foco de teclado isolado.

**BDD:** Dada a janela secundária em um segundo monitor, quando o usuário a fecha durante um job e a reabre após mais 100 linhas impressas, então as 100 linhas estão disponíveis e o Apple II nunca parou.

**HU-IMP-031 — Controlar minha impressão.** Como usuário, quero selecionar LX-310 e papel, visualizar páginas, exportar e limpar trabalhos.

**Aceite:** controles para seleção de modelo, pitch (default herdado do modelo), qualidade, tamanho/formulário, modo de transporte, filtros explicitados, visualização fiel/simplificada, página atual, zoom 25–400%, ajustar à largura, primeira/anterior/próxima/última; botões `Salvar PDF`, `Salvar RAW`, `Nova página/FF`, `Concluir trabalho`, `Cancelar`, `Limpar histórico` com confirmação quando destrutivos. Botões incompatíveis com estado atual desabilitados com explicação. Nada de exportar página vazia inadvertidamente.

**HU-IMP-032 — Prévia instantânea.** Como usuário, quero ver o papel sendo preenchido à medida que chega a impressão.

**Aceite:** para trabalho pequeno, a primeira alteração visível ocorre no máximo **250 ms após o evento chegar ao spool**, em equipamento de referência definido no plano de testes; não esperar timeout de 10 “segundos” emulados, `PR#0` ou FF. A UI mantém 30 FPS ou mais em cenário sustentado realista; sob carga máxima deve limitar atualizações e conservar dados, com backlog visível. Repaint não é chamado para cada byte.

### ÉPICO IMP-04 — Gerenciador de trabalhos e recuperação (P0)

**HU-IMP-040 — Histórico sem perda.** Como usuário, quero consultar impressões anteriores depois de fechar o emulador.

**Aceite:** raw spool persistente por job e metadados, incluindo opção do perfil e manifesto versionado; fechamento normal ou crash deixa trabalho íntegro até último checkpoint confirmado; leitura/recuperação marca job `interrupted` sem inventar bytes; carregamento de job de 100+ páginas sem manter raster inteiro em RAM; histórico mantém ordem e identifica incompletos; políticas de quota por tamanho/quantidade/idade configuráveis, **sem descarte automático de trabalho ativo**.

**HU-IMP-041 — Múltiplos trabalhos e reset.** Como usuário, quero distinguir uma listagem da outra.

**Aceite:** separação por eventos `open/close`, timeout de inatividade, reset e ação explícita; reiniciar máquina não concatena automaticamente jobs; FF é quebra de página, não separação obrigatória; mudanças de modelo/papel aplicam-se ao job seguinte, com decisão explícita para o atual; quando a placa é retirada do slot, job corrente é fechado de forma rastreável sem perder histórico.

**HU-IMP-042 — Reprocessar raw com outro perfil.** Como usuário, quero comparar LX-310 com FX-80 sem perder o documento original.

**Aceite:** reprocessamento cria nova revisão/visualização com hash dos bytes original, modelos registrados, sem reexecutar o Apple II; o job original fica imutável; alerta quando o modelo/interface física seriam incompatíveis; saída divergente nunca é substituída sem confirmação.

### ÉPICO IMP-05 — Exportação PDF e integração com o SO (P0)

**HU-IMP-050 — Salvar PDF com layout fiel.** Como usuário, quero guardar a listagem como PDF para pesquisar, imprimir ou compartilhar.

**Aceite:** exportação do `PageDocument` vigente, formato e orientação corretos (tamanho de página medido no PDF); dots e gráficos preservados sem alterar aspecto; saída legível em leitor de PDF comum; múltiplas páginas na ordem; sem artefatos por zoom; arquivo só fica definitivo após finalização atômica; exportar trabalho ainda ativo gera snapshot coerente e mantém job aberto; PDF gerado por job de 20+ páginas passa em verificador estrutural e rasterização de páginas para comparação golden.

**HU-IMP-051 — Exportar bruto para diagnóstico.** Como mantenedor, quero salvar o fluxo binário original e log de comandos.

**Aceite:** `job.raw` preserva bytes e ordem, SHA-256 reportado, log opcional mostra offset/comando/parâmetros/resultado, sem modificar conteúdo por UTF-8 ou substituir binários; arquivos podem ser lidos sem UI por ferramentas de teste.

### ÉPICO IMP-06 — Suporte a Epson FX-80 e MX-80 (P1)

**HU-IMP-060 — Compatibilidade com softwares históricos.** Como entusiasta Apple II, quero usar perfis de impressoras populares da época.

**Aceite:** dois perfis com matriz de comandos **própria** e testes isolados; FX-80 inclui pitch, fontes e bitmap suportados; MX-80 implementa seu conjunto histórico verificado de ESC K/L; testes com cabeçalhos e comandos conhecidos dos manuais; nenhuma opção prometida só porque a LX-310 a implementa. Registrar limites do adaptador paralelo, por exemplo problemas de 7 bits/CHR$(9) conhecidos na Apple II.

### ÉPICO IMP-07 — Apple ImageWriter II via serial (P2; projeto separado após MVP)

**HU-IMP-070 — Imprimir com ImageWriter II.** Como usuário de softwares AppleWriter/MousePaint, quero conectar uma ImageWriter II virtual à porta serial compatível.

**Aceite:** **não** roteá-la fingindo ser impressora Epson na `GenericPrinter` paralela; criar sink de impressão a partir de SSC/serial virtual, com handshake/status se aplicável; interpretar comandos próprios e realizar testes de regressão; se suportar fita colorida, especificar separadamente cores e prova de fidelidade; sem afirmar suporte até que caso real, comando e porta sejam demonstrados.

### ÉPICO IMP-08 — Segurança, limites, desempenho e testes (P0)

**HU-IMP-080 — Não travar o Apple II por imprimir.** Como usuário, quero jogos/programas e áudio funcionando durante spool intenso.

**Aceite:** render e PDF não rodam na thread emulada; producer/consumer com fila limitada e política de spill-to-disk, sem drop silencioso de bytes; P95 da operação de `io_write` no cenário de referência não sofre regressão material (fixar baseline, orçamento inicial <=2x overhead da saída legada e revisar com medidas); CPU/memória do spool com quotas; ao faltar disco, UI/report diagnosticável e escolha de parar somente spool/alterar destino, sem deadlock; teste de 1 MiB de bytes gráficos, 100 páginas e simulação de UI bloqueada.

**HU-IMP-081 — Ser seguro com arquivos e dados.** Como usuário, quero exportar sem sobrescrever arquivos importantes ou executar comandos recebidos do Apple II.

**Aceite:** escapes e payloads são interpretados como dados; export em arquivo temporário e rename atômico quando suportado; o nome da impressão não se torna comando shell; caminhos com Unicode, `#`, espaços, barras, traversal, symlink e diretório sem escrita testados; importação de manifest/spool corrompido não permite exaustão de memória nem escrita fora da pasta de spool; cancelamento de PDF mantém spool íntegro.

**HU-IMP-082 — Ser reproduzível em 3 sistemas.** Como QA, quero testes automatizados e manuais para Linux, Windows e macOS.

**Aceite:** CI de `cargo fmt`, `clippy`, testes unitários/integração, smoke da segunda janela, export PDF e comparações de saída nos três SO; testes headless do parser/renderer sem GPU; evidência nativa de múltiplas janelas, DPI/HiDPI, foco, múltiplos monitores; cada caso com resultado PASS/FAIL/BLOCKED, logs e versões.

---

## 6. SUÍTE DE TESTES — DEVE EXISTIR, NÃO BASTA DESCREVER

### 6.1 Vetores de entrada obrigatórios

Gerar fixtures sintéticos livres de direitos, versionar apenas bytes e goldens próprios, e incluir runner Rust que reconstrua os jobs. Além do teste real `PR#1`, testar o parser direto para isolar a semântica de transporte.

| ID | Vetor de entrada / condição | Verificação objetiva |
|---|---|---|
| T-001 | `PR#1` e `PRINT "HELLO PRINTER"` | Bytes alcançam virtual, primeira página mostra texto, indicador acende |
| T-002 | `1B 40 41 0D 0A 42` | RESET lógico, `A`, avanço correto, `B`, sem ESC literal |
| T-003 | `[65, 13, 66, 10, 67, 13, 10]` | Distingue `CR`, `LF` e `CRLF`, sem linha dupla não autorizada |
| T-004 | `A FF B` | Duas páginas; `A` na primeira, `B` na segunda; **mesmo job** |
| T-005 | `ESC 0`, `ESC 2`, `ESC 3 36` em segmentos | Avanço de papel medido com precisão compatível com o manual |
| T-006 | ESC pitch 10, 12, 15, condensado, duplo | Colunas por largura física, sem recorte indevido e sem mudança de payload |
| T-007 | `ESC E`, `ESC F`, `ESC - 1/0`, `ESC !` | Realce e sublinhado afetam dots; reset correto |
| T-008 | `ESC K 08 00 + 8 bytes`, inclusive `0x1B` | Exatamente 8 colunas bitmap, nenhum comando fictício; termina no byte correto |
| T-009 | `ESC * m nL nH` com bytes `00 FF 1B 0A` | Respeita contagem de dados e densidade; não interpreta ESC/LF dentro de payload |
| T-010 | Sequência partida em todos os offsets | Mesmo output/hash do input em bloco único |
| T-011 | `ESC` isolado e gráficos declarados maiores que o stream | Parser aguarda/diagnostica; não dá panic ou consome novos jobs |
| T-012 | Bytes `0x00..0xFF` em `raw_8bit` | Arquivo raw 256 bytes idênticos, sem perda; preview apenas o que o protocolo define |
| T-013 | Mesma sequência em legado 7bit | Rota legada preservada; incompatibilidades explícitas |
| T-014 | `filter_unprintable=true` do modo legado + ESC/P virtual | ESC/FF continuam no virtual; saída antiga preservada conforme filtro |
| T-015 | CP437, CP850, CP860, BRASCII e ABICOMP selecionados | Golden de acentos correta por tabela/protocolo; sem UTF-8 indevido |
| T-016 | 1.000 linhas de avanço e FF | Erro de deslocamento físico dentro de tolerância definida, sem drift acumulativo |
| T-017 | 100 páginas, 1 MiB de bitmap, render na UI | Quotas e desempenho; todo dado preservado; latência medida |
| T-018 | Fechar janela no meio, continuar imprimindo, reabrir | Nenhuma perda; no panic, sem duplicações |
| T-019 | Reset/reboot/retirar card/hot-swap | Jobs fechados com reason; novo job não mistura bytes antigos |
| T-020 | Salvar `.aws.yaml`, carregar, imprimir novamente | Compatibilidade antiga; sem corrupção/dobro em caminho de impressora |
| T-021 | Crash após blocos persistidos | Recuperação truncada apenas até checkpoint verificável; job `interrupted` |
| T-022 | Exportar PDF com job ativo e após conclusão | Páginas e dots coerentes; PDF estruturalmente válido, sem dano ao raw |
| T-023 | Destino read-only/disk full/path `#` e Unicode | Erro visível; sem deadlock, sem sobrescrita acidental |
| T-024 | Alternar LX-310 -> FX-80 no meio do job | Escolha explícita; corrente imutável ou reprocessada com revisão separada |
| T-025 | Fullscreen Apple II + janela impressora no segundo monitor | Foco, entrada do teclado, DPI e ciclo de vida corretos |
| T-026 | Headless sem desktop/GPU | Parser/spool/PDF funcionam sem frontend |
| T-027 | 10.000 comandos ESC aleatórios com limites definidos | Fuzz/sanitizer/proptests sem panic, timeout ou OOM |
| T-028 | `ESC @` após 2 linhas e antes de FF | Formatação volta ao default sem apagar o documento anterior |
| T-029 | `ESC D` tabs + comando sem NUL final | Estado incremental correto, aviso de truncamento |
| T-030 | Perfil MX-80 recebe comando não suportado LX-310 | Warning do modelo histórico, sem assumir funcionalidade de outro perfil |

### 6.2 Oráculos e tolerâncias

- Usar **oráculo de especificação/manual** para ESC/P e hardware; AppleWin upstream serve como oráculo para **interface da placa e comportamento legado**, **não** para emulação nova de LX-310 (ele não é engine gráfica de LX-310).
- Criar pelo menos **20 golden tests por protocolo P0**, cobrindo texto/controle, gráficos e páginas; golden de documento lógico deve incluir largura/altura, comandos reconhecidos, posições físicas, sequência de dots (matriz/bitmap) e limites de página.
- Para PDFs, validar com ferramenta de leitura/rasterização de PDF (por exemplo `pdfinfo`/`pdftoppm`/`mutool`, conforme disponibilidade) e checar geometria das páginas, contagem, integridade e raster; não comparar bytes do PDF quando timestamps/metadados legítimos diferirem.
- Para precisão de posições, escolher unidade canônica física e exigir diferença <= 1 dot do grid documentado de cada modo, sem drift acumulativo. Se a documentação model-specific exigir arredondamento diferente, registrar golden com justificativa.
- Para comparação visual, usar diferença pixel-a-pixel exata para raster determinístico e tolerância perceptual explicitamente medida somente para antialias de *display*; **o raster lógico não usa tolerância cosmética**.
- Para UI, definir ambiente de referência, medir P50/P95 de latência e anexar captura/vídeo do trabalho sendo construído, não apenas screenshot final.
- Todos os casos materiais P0 devem possuir teste automatizado; casos de hardware, foco e segundo monitor exigem roteiro manual com evidência datada por SO.

### 6.3 Exemplos BDD end-to-end

```gherkin
Funcionalidade: Impressora Epson LX-310 virtual
  Cenário: Listagem do Apple II aparece enquanto a máquina continua rodando
    Dado um Apple IIe com Generic Printer no slot 1
    E o destino Virtual Printer com modelo Epson LX-310
    E a Central está aberta no segundo monitor
    Quando o Apple II executa PR#1 e imprime 10 linhas
    Então a primeira linha aparece antes do encerramento do job
    E a CPU não é pausada
    E existem 10 linhas nas posições esperadas
    E o job guarda os bytes brutos

  Cenário: ESC/P e gráfico em blocos
    Dado que a LX-310 aceita gráficos ESC K no perfil ativo
    Quando envio o início da sequência e os dados em blocos arbitrários
    Então os bytes de gráfico inclusive 1B não são comandos
    E a imagem na prévia coincide com o golden matricial

  Cenário: PDF de uma impressão ainda em andamento
    Dado que a página 2 está sendo construída
    Quando clico em Salvar PDF
    Então obtenho um PDF válido com o estado consistente atual
    E as novas linhas continuam chegando à página virtual
    E o arquivo bruto permanece íntegro

  Cenário: Persistência após fechar janela
    Dado um job ativo e parcialmente renderizado
    Quando fecho e reabro somente a janela da impressora
    Então o histórico e o job permanecem íntegros
    E nenhuma tecla da Central foi encaminhada ao Apple II
```

---

## 7. DIVISÃO DE FASES E PRs — SEM BIG BANG

**Fase A — Contratos e captura (P0):** documentar baseline/SHAs; adicionar tipos da engine, capture sink, separação filtro legado vs comandos virtuais e testes binários/`PR#1`. **Gate A:** T-001, T-002, T-012, T-013, T-014 PASS; testes legados verdes.

**Fase B — ESC/P LX-310 e documento físico (P0):** parser incremental, comandos de formatação, páginas, dots de caracteres e gráficos, codificações prioritárias, goldens. **Gate B:** T-003 a T-011, T-015, T-016, T-028/29 PASS e matriz de comandos auditada; se glifo bit-a-bit não for comprovável, comunicar status `PARTIAL` e não aprovar alegação de fidelidade completa.

**Fase C — Segunda janela e tempo real (P0):** multi-window nativa, toolbar, estado, preview incremental, ciclo de vida, foco e controles. **Gate C:** T-018, T-025 e latência P95 dentro da meta observada (definir bancada), sem travar emulador.

**Fase D — PDF, spool e recuperação (P0):** exportação, histórico e manifests, persistência atômica, segurança, falhas simuladas. **Gate D:** T-017, T-019 a T-024, T-026, T-027 PASS.

**Fase E — Epson FX-80 e MX-80 (P1):** protocolos próprios com documentação e goldens independentes. **Gate E:** funcionalidade comprovada para cada modelo suportado.

**Fase F — ImageWriter II serial (P2):** abrir épico separado; conectar SSC virtual à engine própria. **Gate F:** demonstração por bytes reais da serial, papel e render corretos; nunca anexar a Generic Printer sem adaptador/protocolo real justificáveis.

Propor PRs atômicos por fase ou submódulo; cada PR com mudança de código, teste diretamente associado, link da HU/CA e evidência. Não fazer merge de branch de protótipo que só desenha texto em egui como se fosse implementada a LX-310.

## 8. CONTRATO DE RELEASE E CRITÉRIOS DE PARADA

**Release funcional mínima:** Fases A–D aprovadas e LX-310 **obrigatória**; FX-80/MX-80 podem ser P1, ImageWriter II explicitamente P2. Os manuais e UI devem distinguir `supported`, `partial` e `unsupported` para cada comando. Caso requisito P0 esteja parcialmente implementado, NÃO rotular a funcionalidade como completa; abrir defeito/gap reproduzível.

**Gates:**

1. **G0 Spec:** cada HU com CA, comando/fixture de teste, owner de código e oráculo; dependências licenciadas; sem ambiguidade entre placa, transporte, protocolo e PDF.
2. **G1 Engine:** P0 de raw capture/ESC/P/dots/FF/CR/LF/apresentação física verdes, testes headless, fuzz sem falhas.
3. **G2 Desktop:** janela nativa separada em Linux/Windows/macOS; teste de foco/segundo monitor, eventos de UI sem perda, autoabrir opcional; erro não encerra emulador.
4. **G3 Output:** raw persistente, recuperação, export PDF validado com ferramenta externa, segurança e testes de pressão.
5. **G4 Release:** sem defeitos P0/P1 não aceitos formalmente, checks verdes, relatórios reproduzíveis, README/manual, aprovação independente; a candidata Linux não vira automaticamente candidata Windows/macOS.

**Nota Verônica:** avaliar cada história e milestone pela rubrica: 25 especificação, 25 corretude/evidência, 15 testes, 10 arquitetura, 10 segurança/resiliência, 5 entrega, 10 eficiência. **96–100 + zero blocker crítico** para `APPROVE_AND_STOP`; 90–95 significa apenas correção focada. Não inflar nota porque a equipe investiu esforço. No máximo 3 workers independentes, um único responsável por arquivo, verificador separado. Ao atingir os critérios, encerrar agentes e parar expansão cosmética. 2 rodadas sem progresso: STOP_AND_REPLAN.

**Evidência mínima por HU:** ID, branch/SHA, caminhos alterados, comandos de teste, resumo PASS/FAIL/BLOCKED, artefato golden, gif/screenshot ou PDF de exemplo quando aplicável, limites conhecidos e parecer revisor. `BLOCKED` e `UNVERIFIED` nunca são convertidos em `PASS` para cumprir prazo.

## 9. ENTREGÁVEIS OBRIGATÓRIOS

- Código modular no Rust, com engine/headless sem GUI, sink da card, spooler e frontend da segunda janela.
- Configuração versionada, migração dos parâmetros legados, comandos de UI e documentação do estado de jobs/snapshots.
- Catálogo de modelos com matriz `printer-model-capabilities.md` e documentação ESC/P LX-310 com links de referência.
- Testes unitários, integração real Apple II (`PR#1`), BDD, stress, fuzz/property tests, comparação golden e testes de PDF; pelo menos um arquivo de exemplo RAW + PDF para cada perfil realmente entregue.
- Vídeo/captura mostrando Apple II rodando e **outra janela independente** preenchendo uma página, em tempo real; screenshot multimonitor se possível.
- Evidência `cargo fmt`, `cargo clippy`, `cargo test` e CI; matriz nativa Linux/Windows/macOS, com bloqueios justificados.
- `docs/printer/README.md`: como instalar card no slot, executar `PR#1`, mudar LX-310, configurar papel e usar `Salvar PDF`.
- Issues/PRs no **repositório do AppleII-Rust** com histórias, critérios, dependências e relatório de status; não criar issues upstream AppleWin sem autorização.
- Resultado auditável com declaração explícita **do que é fiel** e **do que é apenas aproximação**, e sem afirmar que a release anterior tinha gates que a própria documentação mantém bloqueados.

## 10. TAREFA INICIAL OBRIGATÓRIA

**Não comece desenhando janela.** Primeiro faça uma prova automatizada curta que demonstrará o ponto mais arriscado desta feature: capturar o comando ESC/P `1B 40` e um `FF=0C` vindos do caminho de impressão virtual do Apple II, com os filtros legados ativados, preservando o `FileSink` antigo. Depois entregue o pequeno diff, logs de teste, estratégia de modo 7/8 bits e um diagrama mínimo de como a segunda janela observará eventos sem bloquear o emulador. Só então avance para as fases B–F.

---

## 11. LEITURAS E ALERTAS PARA A EQUIPE

1. Não distribuir fontes de impressora, firmware proprietário, material de manual ou ROM não licenciados no repositório: documentar origens/licenças; criar glifos/fixtures próprios quando preciso. Repositório declara `GPL-2.0-or-later`.
2. Não misturar emulação da impressora com o firmware da card; o parser ESC/P não deve residir dentro do `Card::io_write`.
3. `PrinterSnapshot` é hardware lógico legado; job spool é dado de usuário e precisa de persistência própria. A restauração do YAML AppleWin não deve gerar PDF falso/duplicado.
4. `ESC/POS` é outro protocolo, usado em impressoras POS térmicas; não usar parser ESC/POS para LX-310.
5. Epson LX-310 é posterior ao auge do Apple II: **compatibilidade de comando e de interface** precisa ser definida explicitamente. “Existiu em 1984” não é critério para esta escolha solicitada.
6. Apple ImageWriter II é serial, com comandos próprios e possibilidade de fita colorida. Sua implementação correta deve ser etapa separada, não filtro cosmético sobre o renderer da LX-310.
7. `Frame` de vídeo Apple II não é documento para impressão. O objetivo é consumir bytes produzidos pelo software, não screenshot/copy-screen.
8. PDF não é o spool primário nem precisa estar com bytes finais válidos durante a impressão; a **prévia do documento** deve evoluir em tempo real, e a exportação PDF ocorre a partir de um snapshot estável.
9. Refazer a verificação de release: o `main` pode ser posterior à branch `release/v0.2.0-rc.1`; usar SHA do commit efetivamente compilado e nunca tomar documentação de branch local como tag publicada.

**Critério supremo:** A história só está concluída quando um programa Apple II envia caracteres e sequências ESC/P para a Printer Card, uma **janela nativa independente** mostra a página virtual se formando com o comportamento correto da **Epson LX-310**, e o usuário gera um **PDF fiel daquelas páginas**, com reprodução automatizada, recuperação e sem regressões da emulação.
