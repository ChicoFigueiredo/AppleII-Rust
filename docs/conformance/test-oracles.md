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

**PCM, mesma taxa interna.** O PCM do Mockingboard é comparado na taxa interna (44 100 Hz, estéreo, 16 bits, antes de qualquer reamostragem do host). O AY8910 do port traduz `AY8910.cpp`, então a expectativa é de amostras **idênticas**. Duas diferenças de estrutura são conhecidas e declaradas aqui, antes de medir:

1. O AppleWin gera o PCM em blocos (`MB_Update`, a cada interrupção do Timer 1 ou 1000 Ciclos) com `nNumSamples` truncado e corrigido pela posição do buffer circular; o Rust gera cada amostra no seu Ciclo exato. O DirectSound virtual do oráculo começa com o buffer circular pré-carregado em 3/8 (dentro da faixa 1/4-1/2 em que a correção é zero) e consome em tempo emulado; o oráculo antecede o fluxo com essa latência, que **não** entra no PCM gravado. O alinhamento temporal é o índice de amostra, com um desvio por janela (abaixo).
2. Escritas no AY dentro de um bloco são aplicadas na amostra do deslocamento em Ciclos (`sound_ay_write(reg, val, offset)`); o Rust renderiza até o Ciclo da escrita e aplica. A mesma amostra de arredondamento pode cair numa amostra vizinha.

Métricas (janelas de 4 410 amostras, 100 ms, por canal; só janelas em que o oráculo não é silêncio exato):

| Métrica | Definição |
|---|---|
| M0 | **Igualdade exata** das amostras, com um único desvio global `L` (0 ≤ \|L\| ≤ 735, o de menor erro nos primeiros 4 410 amostras não nulas). Todas iguais: PASS exato |
| M1 | Conjunto de valores distintos de amostra de cada canal igual nos dois lados (vêm da mesma tabela de níveis do AY) |
| M2 | Por janela, com desvio local `l_w` (\|l_w\| ≤ 2 205): erro absoluto máximo `E_w` e erro quadrático médio `R_w` |
| M3 | Por janela, a razão de cruzamentos de zero (frequência) e o RMS da janela, diferença relativa |

Limiares: `E_max`, `R_max`, `f_tol` e `rms_tol` são **2 vezes o máximo observado** de M2/M3 entre duas execuções do próprio oráculo no mesmo programa com fatiamento diferente (`--audio-slice 1000` contra `--audio-slice 250`: muda onde caem as fronteiras dos blocos de `MB_Update`), com pisos de `E_max ≥ 256`, `R_max ≥ 64`, `f_tol ≥ 1 %` e `rms_tol ≥ 2 %`. Decisão: M0 satisfeita = PASS (exato); senão M1 igual **e** M2 e M3 dentro dos limiares = PASS (tolerância); qualquer outro caso = FAIL. Nenhum limiar é alterado depois da primeira comparação; uma mudança é um novo registro, só para as execuções seguintes.

**SSI263 (fala).** `J_ssi` = o maior `|(PHEND − PHON)| ` observado entre duas fases de partida do oráculo (o mesmo programa precedido de 0 e de 333 `NOP` (0 e 666 Ciclos), que mudam o alinhamento com o `PeriodicUpdate` de 1000 Ciclos), somado a 1000 Ciclos (uma granularidade do `PeriodicUpdate`). O PCM do SSI263 não é comparado amostra a amostra: o port renderiza a mesma forma de onda de fonema na taxa do host, e o AppleWin a grava em blocos; compara-se a **duração** (e o silêncio em power-down, #1412), nunca o timbre. O timbre fica para o roteiro manual (`manual-tests/audio.md`).

**Alto-falante.** Os toques (`SPK`) são exatos. O PCM do alto-falante (44 100 Hz mono, `QueueOneFrame` com o filtro DC) usa as mesmas métricas M0-M3 com a mesma calibração.

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
