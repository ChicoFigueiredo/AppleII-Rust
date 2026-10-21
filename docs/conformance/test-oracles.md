# Oráculos e estratégia de testes

Este documento aplica ao projeto a estratégia de oráculos do §17 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). Ele define contra o que cada teste compara, como a comparação é decidida e o que cada execução registra.

## Oráculos

| Oráculo | Quando se usa | Observação |
|---|---|---|
| AppleWin em `a8f0d1d` (padrão) | Qualquer comportamento que o AppleWin tem | Executáveis: AppleWin v1.32.1.0 no Wine 10 e o fork Linux do audetto com SHA fixo. Veja [baseline.md](baseline.md) |
| Especificação de hardware | Onde o AppleWin não cobre o caso | Só prevalece sobre o AppleWin como Divergência intencional, com prova de hardware |
| Fixture validada | Entrada com resultado conhecido, de proveniência documentada | Identificada por SHA-256 |
| Resultado conhecido | Valores publicados (por exemplo, a suíte de CPU de Klaus Dormann) | A fonte e a versão entram no Manifesto |

Quando o AppleWin e o hardware real divergem, o AppleWin vence (ADR 0004).

## Pirâmide de testes

Do mais barato ao mais caro. Cada nível tem seu lugar, e um nível baixo verde não substitui um nível alto.

1. **Unitários.** Registradores, opcodes, memória, Soft switches, relógios, analisadores de WOZ e YAML. Incluem casos de borda e propriedades invariantes (nenhum panic, integridade, ida e volta). Já existem em cada crate; seu resultado, sozinho, é IMPLEMENTED.
2. **Integração.** Boot da ROM, montagem e gravação de disco, expansão de RAM, vídeo, áudio, depurador e salvar e restaurar, sob sequências de entrada fixas. Rodam sem janela.
3. **Diferencial.** O AppleWin e o AppleII-Rust executam sob o mesmo Modelo, a mesma ROM, a mesma entrada e o mesmo Ciclo; comparam-se CPU, memória, barramento e Quadros. Na divergência, o relatório mostra a primeira diferença. Os Traces de referência gerados pelo AppleWin são versionados com a origem e o SHA (formato abaixo).
4. **Corpus real.** Fixtures geradas pelo próprio AppleWin (AppleWin-Test), em várias versões e Modelos. Nunca depende só de fixture escrita pela equipe. O corpus é baixado para `target/conformance-cache/` e não é redistribuído; sem rede o resultado é BLOCKED.
5. **Ponta a ponta entre plataformas.** Instalar o executável distribuído, abrir a janela, digitar, montar um disco, salvar e reabrir, gerar áudio e encerrar. Registra o que foi automatizado e o que foi validação humana. Linux roda local; Windows nativo e macOS ficam BLOCKED até o dono autorizar uma execução de CI.
6. **Robustez.** Entradas WOZ, NIB, DSK, YAML, configuração, serial e rede inválidas; limites de tamanho, tempo e memória. Nenhum panic por arquivo malformado do usuário e nenhuma mutação parcial de dados.
7. **Desempenho.** Benchmarks fixos. Não se sacrifica temporização correta por velocidade.

Testes que exigem hardware físico (gamepad, porta serial, captura de pacotes com pcap, escuta de áudio) têm roteiro manual e ficam BLOCKED até o dono executar o roteiro e registrar o resultado.

## Política de comparação

### Estado digital: igualdade exata

Compara-se valor a valor, sem tolerância: PC, A, X, Y, SP, flags, bancos de memória, a RAM relevante, eventos, a ordem das operações de E/S, o contador de Ciclos e o Quadro lógico antes de efeitos analógicos. Qualquer diferença é FAIL.

### NTSC, PAL e áudio: métricas definidas antes

A simulação analógica de vídeo e a síntese de áudio não têm igualdade bit a bit garantida. Nesses casos:

- Compara-se primeiro o que for digital: eventos e temporização exatos (toques do alto-falante, escritas nos registradores do AY, trocas de modo de vídeo).
- Para o que sobrar, a **métrica, o espaço de cor (ou a janela de áudio), o alinhamento temporal, as regiões críticas e o limiar são escritos no manifesto antes da primeira execução** e calibrados com a referência (por exemplo, o AppleWin contra ele mesmo em duas execuções). Candidatas de métrica: erro absoluto médio e SSIM para imagem; frequências, envelopes e eventos para áudio.
- O limiar não é ajustado depois para fazer um teste passar. Fora do limiar é FAIL, e o julgamento visual ou auditivo não o transforma em PASS. Mudar um limiar exige um novo registro com justificativa e vale só para as execuções seguintes.
- Todo diff gera um artefato (imagem de diferença, histograma, trecho de áudio) guardado em `tests/conformance/reports/`.

### Vídeo (HU-0201, HU-0202): framebuffer contra o AppleWin

Registrada em 2026-10-20, **antes da primeira execução** do diferencial. Vale para as execuções seguintes; qualquer mudança exige novo registro com justificativa.

- **Oráculo**: o fork Linux do AppleWin (`audetto-oracle --fb-out`, ver `tools/conformance/oracle/README.md`), que despeja o framebuffer da camada de vídeo do núcleo (`NTSC.cpp`, `Video.cpp`, `RGBMonitor.cpp`) com a atualização de vídeo por opcode ligada, como nos frontends reais.
- **Entrada**: cenas "scene v1" (`tests/conformance/inputs/video/*.scene`), geradas por `crates/apple2-video/examples/gen_scenes.rs`: bytes de RAM e de código 6502 escritos pela equipe, sem ROM de usuário. O oráculo e o Rust carregam os mesmos bytes logo após o power-on, apontam o PC para o ponto de entrada e rodam do ciclo 0.
- **Alinhamento temporal**: ambos executam até o fim do quadro N (primeira instrução que cruza `N x ciclos por quadro`) e comparam o framebuffer nesse instante; o contador de flash depende desse N (quadros 4 e 20 cobrem flash desligado e ligado).
- **Região**: o framebuffer inteiro de 600x420 (560x384 visíveis mais bordas de 20 e 18 pixels), em RGBA8 de cima para baixo. Cada caso declara se compara `visible` ou `full`; o diferencial amplo usa `full`.
- **Métrica**:
  - modos digitais (`MonoCustom`, `ColorIdealized`, `ColorVideocardRgb`, monocromáticos de fósforo, texto, lo-res, SHR): igualdade bit a bit, isto é, diferença máxima de canal = 0;
  - NTSC e PAL (`ColorMonitorNtsc`, `ColorTv`, `MonoTv`): diferença máxima de canal = 0, porque o `apple2-video` é uma tradução do `NTSC.cpp` e o algoritmo é o mesmo (aritmética inteira); MAE, SSIM e a caixa envolvente das diferenças entram **só como diagnóstico**.
- **Limiar de reprovação**: qualquer pixel diferente é FAIL. Cada diferença é investigada até a causa raiz no C++ e corrigida no Rust com teste de regressão, ou permanece FAIL (ou BLOCKED, se a causa estiver fora do alcance) com as hipóteses testadas; não se relaxa o limiar para fazê-la passar, e o julgamento visual não a transforma em PASS.
- **Calibração**: o oráculo contra ele mesmo em duas execuções deve dar o mesmo SHA-256 (conferido por `tools/conformance/oracle/video-differential.py --selfcheck`).
- **Saída**: `tests/conformance/cases/video-*.toml` (casos `framebuffer-hash` com `authority = "applewin"`, o comando do oráculo e o SHA-256 em `source`) e o registro de divergências em `docs/conformance/audit/video-differential.md`.

### Áudio: Mockingboard, Phasor, SSI263 e alto-falante (HU-0501, HU-0502)

Registrada **antes** da primeira comparação Rust × oráculo (a calibração entra em `audit/audio-differential.md` antes do primeiro resultado). O Oráculo é o fork Linux do AppleWin (`tools/conformance/oracle/`, `audetto-oracle`) com o gancho de áudio `audetto-audio.patch` e um "DirectSound virtual" determinístico (relógio = Ciclos emulados). Os dois lados partem do mesmo estado inicial (RAM + registradores de CPU, ou o mesmo `.aws.yaml`) e rodam o mesmo número de Ciclos.

**Eventos, igualdade exata.** O log de eventos de áudio (formato `audio-events v1`, uma linha `cycle,KIND,campos`) é comparado linha a linha, sem tolerância. Qualquer linha diferente (ciclo, tipo ou valor) é FAIL, e o relatório mostra a primeira diferença.

| `KIND` | Campos | O que é |
|---|---|---|
| `IOW` / `IOR` | `slot,addr_lo,valor` | Escrita/leitura na página `$Cs00-$CsFF` da carta (registradores do 6522 e do SSI263 por endereço). Leituras com o valor devolvido; a falsa leitura do 6502 (GH#52) não gera linha |
| `AYW` / `AYR` | `slot,unidade,ay,reg,valor` | Escrita/leitura num registrador do AY (`_AYWriteReg` / `AYReadReg`) |
| `IRQ` | `fonte,nivel` | Transição da linha de IRQ de uma fonte (`6522`, `SPEECH`), somente quando o nível muda |
| `SPK` | (nenhum) | Toque do alto-falante (`$C030-$C03F`) |
| `PHON` | `slot,unidade,fonema,amostras` | Início de um fonema (`SSI263::Play`); `amostras` é o comprimento da forma de onda em amostras de 22,05 kHz |
| `PHEND` | `slot,unidade` | Fim do fonema (IRQ de "fonema completo", `SSI263::UpdateIRQ`) |
| `PIO` | `slot,addr_lo,escrita` | Acesso de seleção de modo do Phasor em `$C0(8+slot)x` (`PhasorIO`); `escrita` é 1 para escrita |

`cycle` é o contador acumulado de Ciclos do 6502 no momento do acesso (o mesmo de `UpdateCycles`/`CpuCalcCycles` no AppleWin e de `CardCtx::cycles` no Rust). Exceção declarada para SSI263: o AppleWin só detecta o fim do fonema em `PeriodicUpdate` (a cada 1000 Ciclos, com o buffer circular do DirectSound), e o port Rust agenda o fim exatamente em `início + comprimento` (decisão de projeto em `ssi263.rs`, sem laço de realimentação). Por isso, para `PHEND` (e para o `IRQ SPEECH` e o `IOR` de IFR que dependem dele) a comparação é: mesma **sequência** de fonemas e IRQs; a diferença `(PHEND − PHON)` do Rust contra a do oráculo deve ficar dentro de `J_ssi`, definido abaixo. As linhas que **não** dependem do fim do fonema continuam exatas.

**PCM, mesma taxa interna.** O PCM do Mockingboard é comparado na taxa interna (44 100 Hz, estéreo, 16 bits, antes de qualquer reamostragem do host). O AY8910 do port traduz `AY8910.cpp`, então a expectativa inicial era de amostras **idênticas**. Duas diferenças de estrutura são conhecidas e declaradas aqui:

1. O AppleWin gera o PCM em blocos (`MB_Update`, a cada interrupção do Timer 1 ou 1000 Ciclos) com `nNumSamples` truncado e corrigido pela posição do buffer circular; o Rust gera cada amostra no seu Ciclo exato. O DirectSound virtual do oráculo começa com o buffer circular do Mockingboard pré-carregado em 3/8 (dentro da faixa 1/4-1/2 em que a correção é zero), o do SSI263 vazio (o `SSI263::Update` pré-carrega 25 % sozinho), e consome em tempo emulado; o oráculo antecede o fluxo com essa latência, que **não** entra no PCM gravado. O alinhamento temporal é o índice de amostra depois de um desvio global (abaixo).
2. Escritas no AY dentro de um bloco são aplicadas na amostra do deslocamento em Ciclos (`sound_ay_write(reg, val, offset)`); o Rust renderiza até o Ciclo da escrita e aplica. A mesma amostra de arredondamento pode cair numa amostra vizinha.

**Registro v1 (descartado antes de qualquer comparação Rust × oráculo).** A primeira versão desta política decidia por M0, conjunto de valores (M1) e erro por amostra com desvio local (M2), com limiares de 2 vezes o máximo observado entre duas execuções do oráculo. A calibração (`--audio-slice 1000` contra `--audio-slice 250`, `audit/audio-differential.md`, seção "Calibração") mostrou que **o próprio oráculo não concorda consigo mesmo** nessas métricas: com outro fatiamento do tempo o PCM do AY muda de fase (`MB_Update` gera `nNumSamples` truncado por chamada; as bordas da onda quadrada andam algumas amostras), e o erro por amostra chega ao fundo de escala (14 336) e o conjunto de valores intermediários nas bordas (2 389 etc.) muda. Limiares de 2 vezes isso aceitariam qualquer sinal. Uma política que o oráculo reprova contra si mesmo não separa nada, então ela foi substituída pela v2 abaixo. A v2 foi escrita antes de qualquer execução do Rust contra o oráculo para o PCM; a execução exploratória do Rust com limiares padrão (sem veredito) que a precedeu só serviu para depurar o código de medição.

**Métricas v2** (janelas inteiras de 4 410 amostras, 100 ms, por canal; o alinhamento é o índice de amostra depois de um único desvio global `L`, 0 ≤ \|L\| ≤ 735, o de menor erro nas primeiras 4 410 amostras não nulas do oráculo, que absorve o início do fluxo do AppleWin cerca de 1000 Ciclos depois do Ciclo 0):

| Métrica | Definição | Papel |
|---|---|---|
| M0 | Todas as amostras iguais com o desvio global `L` | Decide (PASS exato) |
| M1 | Conjunto de valores distintos de amostra igual nos dois lados | Informativa |
| M2 | Erro absoluto máximo e RMS, amostra a amostra | Informativa |
| M3 | Por janela: diferença relativa da taxa de cruzamentos de zero (frequência), do RMS e do pico (`\|a−b\|/max(a,b)`) | Decide (PASS por tolerância) |

Janelas silenciosas nos dois lados são ignoradas; silêncio de um lado só dá diferença relativa de 100 %.

Limiares: para cada caso, `f_tol`, `rms_tol` e `peak_tol` são **2 vezes o pior valor de M3** observado entre duas execuções do próprio oráculo no mesmo programa com fatiamento diferente (`--audio-slice 1000` contra `--audio-slice 250`: muda onde caem as fronteiras dos blocos de `MB_Update`), com pisos de 1 %, 2 % e 2 %. Decisão: M0 satisfeita = PASS (exato); senão M3 dentro dos três limiares = PASS (tolerância); qualquer outro caso = FAIL. Os limiares de cada caso ficam no `[oracle] pcm_limits` do caso, com a calibração registrada antes. Nenhum limiar é alterado depois da primeira comparação; uma mudança é um novo registro, só para as execuções seguintes.

**SSI263 (fala).** `J_ssi` = a maior diferença, entre duas fases de partida do oráculo, da duração `PHEND − PHON` de um mesmo fonema (o mesmo programa precedido de 0 e de 333 `NOP`, isto é, 0 e 666 Ciclos, que mudam o alinhamento com o `PeriodicUpdate` de 1000 Ciclos), somada a 1000 Ciclos (uma granularidade do `PeriodicUpdate`). O caso compara o Rust com o oráculo da fase 0; a execução da fase 333 é a segunda execução de calibração, e a comparação do Rust com ela também é registrada no log. Para um save-state (fixture), onde não há como inserir `NOP`s, as duas execuções de calibração são `--audio-slice 1000` e `--audio-slice 250` do oráculo sobre a própria fixture, com a mesma fórmula. Depois do primeiro fim de fonema o programa reage a IRQs que os dois lados levantam em ciclos ligeiramente diferentes e os chips derivam entre si, então a ordem entre chips diferentes não é invariante: cada chip (e os eventos que não são de fonema) é comparado como sequência própria, sem ciclos, e a duração `PHEND − PHON` de cada fonema entra na tolerância; um `PHEND` sem `PHON` antes (fonema reiniciado pelo carregamento do estado, #1372) conta a partir do ciclo 0, o carregamento. Um evento a mais perto do fim da execução (a menos de 2 `J_ssi`) é aceito de um lado só. O PCM do SSI263 não é comparado amostra a amostra: o port renderiza a mesma forma de onda de fonema na taxa do host, e o AppleWin a grava em blocos; compara-se a **duração** (e o silêncio em power-down, #1412), nunca o timbre. O timbre fica para o roteiro manual (`manual-tests/audio.md`).

**Núcleo do AY, amostras idênticas.** A expectativa de amostras idênticas só pode valer onde as duas linhas do tempo são a mesma. Por isso o gerador do AY também é comparado isolado: `tools/conformance/audio/gen-ay-scripts.py` escreve scripts de escritas de registrador e tamanhos de quadro, o oráculo os executa na classe `AY8913` do próprio AppleWin (`--audio-ay-script`, sem máquina emulada) e `crates/apple2-audio/tests/ay_core_oracle.rs` roda os mesmos scripts no `Ay` do Rust; cada amostra das três vozes tem de ser igual (sem tolerância). A escrita a `TSTATES` Ciclos do início do quadro cai na amostra `TSTATES * 882 / (clk/50)` (`sound_ay_overlay`).

**Alto-falante.** Os toques (`SPK`) são exatos. O PCM do alto-falante (44 100 Hz mono, `QueueOneFrame` com o filtro DC) usa as mesmas métricas e a mesma calibração.

**Sem cache ou sem o executável do oráculo, o resultado é BLOCKED**, nunca PASS.

### Software de terceiros e testes instáveis

Jogos e software de terceiros só entram com licença adequada e procedimento reproduzível para obtê-los. Resultados que dependem de firmware registram o SHA-256 da ROM. Teste instável não é removido por conveniência: investiga-se a causa, isola-se o ambiente e registra-se a evidência.

## Trace de referência v1

É um arquivo CSV em UTF-8 com cabeçalho `cycle,pc,op,a,x,y,sp,p`. Há uma linha por instrução, gravada **antes** de executá-la.

| Coluna | Formato |
|---|---|
| `cycle` | Decimal. Ciclos acumulados desde o power-on |
| `pc` | 4 dígitos hexadecimais em maiúsculas |
| `op` | 2 dígitos hexadecimais em maiúsculas (o opcode a executar) |
| `a`, `x`, `y`, `sp`, `p` | 2 dígitos hexadecimais em maiúsculas |

Linhas começadas por `#` são metadados, por exemplo `# model=apple2e-enhanced` e `# rom_sha256=...`. Sugere-se também registrar `applewin_sha`, a origem do Trace (executável e opções) e a versão do formato nos metadados.

### Convenções do Trace v1

O gerador Rust (`conformance trace`) e o oráculo (`audetto-oracle`) seguem as mesmas convenções; sem elas os dois traces não coincidem mesmo com a CPU correta.

1. **`p` com os bits 4 e 5 ligados.** O AppleWin mantém B (bit 4) e o bit reservado (bit 5) sempre em 1 em `regs.ps`; o P após o reset é `34`. O registrador do núcleo Rust não muda (`24` após o reset); é só a representação: `TraceRow::from_machine` grava `p | 0x30` (`trace::P_FIXED_BITS`).
2. **Interrupção não gera linha.** No oráculo o gancho roda depois do Fetch do opcode; IRQ e NMI tomadas não fazem Fetch, logo não geram linha, e a linha seguinte é a primeira instrução do tratador. No Rust a entrada na interrupção é um `Machine::step`; o `TraceWriter` consulta `Machine::next_step_is_interrupt_entry()` (consulta pura, sem custo quando não usada) e não grava linha para esse passo.
3. **`op` é o opcode buscado.** Em `$C100-$CFFF` (ROM de cartão) o `peek` não lê a ROM sem efeito colateral; o `TraceWriter` usa o opcode que a CPU de fato buscou.
4. **RAM de power-on.** O padrão do AppleWin (`g_nMemoryClearType` = `MIP_FF_FF_00_00`, opção `-memclear N`, 0 a 7, mais -1 para sorteado) vale nos dois lados, com o sorteio de `getRandomTime()` fixo em 0 (`xx28/xx29/xx68/xx69` = `00`, `$4E`=`$4F`=`20`). No Rust: `MachineConfig::ram_init` (`RamInit`, `RandomSource::Zero` ou `Seed(n)`; o núcleo nunca lê o relógio, ADR 0001). `conformance trace --memclear N` e `--deterministic-ram` (padrão); `--ram-seed N` sorteia. A RAM auxiliar do //e fica zerada nos dois (o AppleWin só preenche `memmain`).
5. **Relógio de vídeo.** O oráculo avança o relógio de vídeo NTSC a cada opcode (`CpuExecute(..., bVideoUpdate=true)`), como o AppleWin do Windows. Com `false` (padrão dos frontends headless do fork) o relógio fica em (0,0) para sempre e todo floating bus lê o mesmo endereço do scanner, o que o AppleWin real nunca faz (HU-0103).
6. **Lotes de 1 ms.** O oráculo roda `CpuExecute(1020)` e chama `Card::Update(ciclos)` ao fim de cada lote; o `TraceWriter` chama `Machine::update_cards` a cada 1020 ciclos (`trace::BATCH_CYCLES`). O Disk II conta nesse `Update` o atraso de 1 s do motor desligado.
7. **Disk II.** `--cards disk2` (slot 6) e `--disk IMAGEM` (drive 1, protegido contra escrita) no Rust; o oráculo usa o mesmo cartão (`--cards disk2`, padrão). Os bytes "aleatórios" do Disk II vêm do `rand()` da glibc após `srand(1)`, igual nos dois.

Casos `authority = "applewin"`: `tests/conformance/cases/trace-applewin-*.toml`. Os de 500 mil ciclos têm o trace versionado; os maiores (2 e 5 milhões) ficam em `target/conformance-cache/reference-traces/` e o caso guarda o SHA-256 (`trace_sha256`) e o comando para regenerá-los em `source`; sem o arquivo, o resultado é BLOCKED com esse comando. O `applewin_sha` do manifesto vem do cabeçalho `#applewin_sha` do trace de referência. Registro das divergências encontradas: `docs/conformance/audit/cpu-trace-differential.md`.

Exemplo (valores ilustrativos do formato, não de uma execução real):

```csv
# model=apple2e-enhanced
# rom_sha256=aab38a03ca8deabbb2f868733148c2efd6f655a59cd9c5d058ef3e0b7aa86a1a
cycle,pc,op,a,x,y,sp,p
0,FA62,D8,00,00,00,FF,34
2,FA63,A9,00,00,00,FF,34
```

A comparação para na **primeira linha diferente** e relata essa linha e as N anteriores (N configurável), para mostrar a sequência que levou à divergência. Os metadados (`#`) não entram na comparação linha a linha, mas diferenças entre eles (modelo, ROM) são avisadas, porque tornam o resultado inválido.

## Manifesto de execução

Cada execução de teste produz um Manifesto de execução: o registro de tudo que a reproduz. Os campos são os do §17 do plano, todos obrigatórios:

| Campo | Conteúdo |
|---|---|
| `test_id` | Identificador estável do teste |
| `user_story` | História (HU) a que o teste pertence |
| `acceptance_criterion` | Critério de aceite (CA) avaliado |
| `applewin_sha` | SHA do AppleWin usado como Oráculo |
| `rust_sha` | SHA do AppleII-Rust testado |
| `fixture_sha256` | SHA-256 das Fixtures de entrada |
| `rom_sha256` | SHA-256 das ROMs usadas |
| `machine_model` | Modelo da Máquina |
| `cpu_model` | 6502 ou 65C02 |
| `cards_slots` | Cartas e Slots configurados |
| `host_os` | Sistema operacional hospedeiro |
| `host_arch` | Arquitetura do hospedeiro |
| `toolchain` | Versões de rustc, cargo e do Wine, quando usado |
| `input_trace` | Sequência de entradas aplicada |
| `command` | Comando exato executado |
| `expected` | Resultado esperado, com a origem |
| `actual` | Resultado obtido |
| `diff_artifact` | Caminho ou hash do artefato de diferença |
| `result` | `PASS`, `FAIL`, `BLOCKED`, `UNVERIFIED` ou `NOT_APPLICABLE` |
| `timestamp_utc` | Data e hora da execução em UTC |
| `reviewer` | Quem revisou o resultado (não pode ser quem implementou a alteração) |

O formato é validado pelo esquema `tests/conformance/manifests/schema.json`, escrito por outra frente de trabalho. Este documento só o referencia: em caso de divergência entre a lista acima e o esquema, vale o esquema, e esta página deve ser corrigida. Os manifestos ficam em `tests/conformance/manifests/` e os relatórios em `tests/conformance/reports/`.

Um resultado `BLOCKED` também gera Manifesto, com a causa em `actual`.
