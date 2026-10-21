# Diferencial de áudio: Rust × AppleWin (HU-0501, HU-0502)

Oráculo: fork Linux do audetto (`a1e20c03`, base AppleWin `3e8054b`) com `tools/conformance/oracle/audetto-audio.patch` e o módulo `audetto-oracle/audio.cpp` (eventos, PCM, DirectSound virtual). Política de comparação: [test-oracles.md](../test-oracles.md), seção "Áudio". Casos: `tests/conformance/cases/audio-*.toml`; referências: `tests/conformance/reference-audio/`; programas 6502: `tests/conformance/audio/programs/`.

Este arquivo é o log da iteração (achado, causa raiz, correção, teste de regressão). A seção "Calibração" foi escrita **antes** da primeira comparação do Rust com o oráculo para o PCM e para a duração dos fonemas.

## Como reproduzir

```sh
tools/conformance/oracle/build-audetto-oracle.sh                      # oráculo (cache do repositório principal)
tools/conformance/audio/build-programs.sh                             # .s -> .bin (os .bin são versionados)
tools/conformance/audio/run-oracle-case.py audio-ay-tone              # grava reference-audio/audio-ay-tone.*
cargo run --release -p apple2-conformance -- run --case audio-ay-tone --report /tmp/rel
cargo run --release -p apple2-conformance -- audio-run --case audio-ay-tone --out-dir /tmp/rust   # eventos e PCM do Rust
```

## Calibração (oráculo contra ele mesmo)

Cada programa de PCM rodou duas vezes no oráculo, com `--audio-slice 1000` e `--audio-slice 250` (o lote de CPU entre atualizações do cartão; muda as fronteiras dos blocos de `MB_Update`). Eventos: **idênticos nas duas execuções em todos os casos** (inclusive o ciclo). PCM, métricas v2 (`conformance pcm-compare`):

| Caso | Eventos | M0 | Pior M3: frequência / RMS / pico | Erro por amostra (M2, informativo) | Limiares (2 × pior, pisos 1/2/2 %) |
|---|---|---|---|---|---|
| audio-ay-tone | 225 idênticos | não | 2,00 % / 0,56 % / 0,00 % | máx 14 336, RMS 1 171 | f 4,00 %, RMS 2,00 %, pico 2,00 % |
| audio-ay-envelope | 148 idênticos | sim | 0 / 0 / 0 | 0 | f 1,00 %, RMS 2,00 %, pico 2,00 % |
| audio-ay-noise | 155 idênticos | não | 20,00 % / 1,54 % / 0,00 % | máx 12 771, RMS 4 014 | f 40,00 %, RMS 3,08 %, pico 2,00 % |
| audio-phasor-two-ay | 76 idênticos | sim | 0 / 0 / 0 | 0 | pisos |
| audio-speaker-clicks | 150 idênticos | sim | 0 / 0 / 0 | 0 | pisos |

Observações da calibração:

- O PCM do AY **não é reprodutível entre fatiamentos do próprio oráculo** (tom e ruído: M0 falha e o erro por amostra chega ao fundo de escala). Causa: `MockingboardCard::MB_Update` gera `(int)(44100 / (clk / intervalo + 0.5))` amostras por chamada (truncado) e a fase da onda depende de quando as chamadas caem. Por isso a v1 da política (M0 + conjunto de valores + erro por amostra) foi descartada antes de qualquer comparação do Rust (registro em test-oracles.md) e as métricas por janela (M3) passaram a decidir.
- O envelope, o alto-falante e o Phasor (silencioso na janela medida) são reprodutíveis bit a bit entre fatiamentos.
- O limiar de frequência do ruído (40 %) é largo porque a taxa de cruzamentos de zero de um LFSR muda com a fase; é o que o oráculo tolera de si mesmo.
- SSI263 (fala): o mesmo programa nas fases 0 e 333 `NOP` (0 e 666 Ciclos). Durações `PHEND − PHON` em Ciclos, fase 0: 28 473, 30 589, 30 589, 30 589, 12 205, 31 594; fase 333: 27 803, 30 589, 30 589, 30 589, 12 205, 31 595. Maior diferença entre as fases: **670** (primeiro fonema, transitório do buffer circular). `J_ssi` = 670 + 1 000 = **1 670 Ciclos**. As durações do oráculo são múltiplos do período de atualização (`ceil(amostras / 22) × 1 019,6` Ciclos no regime): o fim do fonema só é visto em `PeriodicUpdate`.

## Cenários × resultado

Comando: `conformance run --all` (manifestos em `tests/conformance/reports/`). O PCM de cada programa é um caso separado (`-pcm`) para que a igualdade exata dos eventos não fique escondida atrás de um FAIL de PCM.

| Cenário | Caso | Resultado | Evidência |
|---|---|---|---|
| Notas no AY (oito notas, acorde, AY direito): escritas 6522/AY | `audio-ay-tone` | PASS | 225 eventos idênticos, ciclo incluído |
| Envelopes do AY (oito formas, zumbido) | `audio-ay-envelope` | PASS | 148 eventos idênticos |
| Ruído do AY | `audio-ay-noise` | PASS | 155 eventos idênticos |
| Timers do 6522, IRQ, IFR/IER/ACR | `audio-via-timers-irq` | PASS | 110 eventos idênticos (inclui `IRQ,6522` e leituras de IFR) |
| Cliques do alto-falante (leitura e escrita em `$C030`) | `audio-speaker-clicks` | PASS | 150 eventos idênticos |
| Phasor nativo, 2 AY por 6522, leitura dos registradores 14, Echo+ | `audio-phasor-two-ay` | PASS | 76 eventos idênticos; PCM dentro da tolerância (RMS 1,03 %) |
| Soquetes de AY (AUD-P-006) e `6522a-bad` (AUD-P-007) | `audio-ay-sockets-bad6522` | PASS | 524 eventos idênticos (16 registradores dos dois AY e o DDRB lidos de volta) |
| SSI263, fonemas encadeados por IRQ, power-down (#1412) | `audio-ssi263-phonemes-p0` | PASS | 49 eventos; 17 exatos e 32 por chip; pior duração 1 013 ciclos (J_ssi 1 670); PCM exatamente zero depois de CTL=1 |
| SSI263, mesmo programa, fase +666 Ciclos | `audio-ssi263-phonemes-p333` | FAIL (D-4) | pior duração 1 683 ciclos > 1 670 |
| SSI263 x4, fixture real, 90 quadros (#1372) | `audio-ssi263-4x-fixture` | PASS | 104 eventos, pior duração 2 193 ciclos (J_ssi 3 977) |
| SC-01 x2, fixture real, 90 quadros | `audio-sc01-2x-fixture` | PASS | 52 eventos, pior duração 2 012 ciclos (J_ssi 3 987) |
| PCM do AY, tom | `audio-ay-tone-pcm` | FAIL (D-2) | frequência 8,77 % > 4,00 %; RMS 4,45 % > 2,00 % |
| PCM do AY, envelope | `audio-ay-envelope-pcm` | FAIL (D-2) | frequência 5,88 % > 1,00 % (16 contra 17 cruzamentos numa janela quase muda) |
| PCM do AY, ruído | `audio-ay-noise-pcm` | FAIL (D-2) | frequência 58,33 % > 40,00 % |
| PCM do alto-falante | `audio-speaker-clicks-pcm` | FAIL (D-3) | RMS 2,46 % > 2,00 % |
| Núcleo do AY contra `AY8910.cpp` (5 scripts, tipos 8910/8912/8913/YM2149F, relógios NTSC, PAL e Phasor 2x, ~330 mil amostras) | `crates/apple2-audio/tests/ay_core_oracle.rs` | PASS, amostras idênticas | cada amostra das 3 vozes igual à do `AY8913::sound_frame` |
| Audição | `manual-tests/audio.md` | BLOCKED | até o dono executar o roteiro |

Sem o cache do AppleWin-Test os dois casos de fixture ficam BLOCKED (nunca PASS); sem o executável do oráculo os casos continuam rodando contra as referências versionadas em `tests/conformance/reference-audio/`.

## Divergências

Cada uma com causa raiz, correção (ou o motivo de não haver) e teste de regressão. O limite de esforço do trabalho (duas hipóteses testadas) vale para as que continuam abertas.

### D-1 (corrigida): o IFR do T2 era visto um opcode atrasado

- Achado: `audio-via-timers-irq` divergiu na linha 102. Um laço `LDA $C48D / AND #$20 / BEQ` viu o bit do T2 no AppleWin em +517 Ciclos e no Rust só em +526.
- Hipótese 1 (confirmada): o Rust atualizava o 6522 até o ciclo do acesso ao barramento (início do opcode + 3) e o AppleWin até o primeiro ciclo do opcode (`g_nCumulativeCycles` do tratador de E/S) e depois olha `GetOpcodeCyclesForRead` ciclos à frente. Com o contador em -1 no primeiro ciclo o AppleWin marca `m_timer2IrqDelay`, e a leitura de IFR vê o estouro; no Rust o contador já tinha passado de -2, sem a marca, e o estouro ficava invisível para o IFR até o evento do escalonador (a instrução seguinte).
- Correção: `Via::read_at/write_at(reg, início do opcode, ciclos do opcode)`; a carta passa `CardCtx::access_cycle` e `acesso - início + 1`. `read`/`write` antigos são o caso de 1 ciclo (testes unitários inalterados).
- Regressão: `via6522::tests::ifr_poll_sees_t2_underflow_in_the_same_opcode_as_applewin` e o caso `audio-via-timers-irq` (110 eventos, ciclo incluído).

### D-2 (aberta, política v2): PCM do AY não é idêntico ao do AppleWin; a diferença vem da linha do tempo do oráculo

- Medidas: ver a tabela. O núcleo do AY é idêntico amostra a amostra quando a linha do tempo é a mesma (`ay_core_oracle.rs`), então a diferença não está na síntese.
- Causa raiz: o AppleWin gera o PCM em blocos de `(int)(44100 / (clk / intervalo + 0.5))` amostras por `MB_Update` (43 amostras onde seriam 43,2: -0,4 % de tempo de AY por bloco, compensado só quando o buffer circular cai abaixo de 25 %), e o primeiro bloco só sai cerca de 1000 Ciclos depois do Ciclo 0. O Rust gera cada amostra no seu Ciclo exato. A calibração mostra que o próprio oráculo muda de fase com o fatiamento (M0 falha entre `--audio-slice 1000` e `250`).
- Medida direta do tamanho: o fluxo do oráculo tem 13 772 quadros onde o Rust tem 13 828 (-0,4 %) no envelope; nas janelas, a diferença são 1 ou 2 cruzamentos de zero em janelas de 16 a 100 cruzamentos.
- Hipótese 2 (testada): aproximar o fluxo do Rust do do oráculo copiando o fatiamento. Descartada: o fatiamento depende do lote de 1 ms do frontend e do relógio do hospedeiro (`CpuExecute`), então copiá-lo tornaria o Rust dependente do hospedeiro; a escolha de projeto do port (cada amostra no Ciclo exato, `clock.rs`) fica.
- Estado: FAIL pelo limiar registrado antes (casos `-pcm` com `known_divergence`). Não é uma regressão do Rust. Recomendação (novo registro, valeria só para execuções futuras): decidir por janelas maiores que o desvio acumulado do oráculo (o desvio é de ~0,4 % do tempo) ou por contagem absoluta de cruzamentos, em vez de razões em janelas de 100 ms com poucos cruzamentos. Não aplicada aqui para não ajustar o limiar depois de ver o resultado.

### D-3 (aberta): amostragem do alto-falante a clk/23 no AppleWin

- Medida: eventos idênticos (150 toques), PCM com RMS 2,46 % > 2,00 %, 39 140 quadros no oráculo contra 38 893 no Rust em 900 mil Ciclos.
- Causa raiz: `SetClksPerSpkrSample` (Speaker.cpp:221-229) usa o número inteiro de Ciclos por amostra (`(UINT)(clk / 44100)` = 23, contra 23,14), então o fluxo do AppleWin tem 0,6 % mais quadros por segundo de Apple (o AppleWin compensa regulando a velocidade da CPU pelo buffer, `g_nCpuCyclesFeedback`). O Rust amostra o alto-falante em clk/44100 exato.
- Por que não se corrige: copiar o quantum de 23 Ciclos mudaria a escala de tempo do alto-falante em 0,6 % e o desalinharia dos outros fluxos do mixer (Mockingboard), que seguem clk/taxa. O hardware real não tem esse quantum. Candidata a Divergência intencional (precisa de decisão do dono; ADR 0004 dá a vitória ao AppleWin enquanto não houver prova de hardware, então fica FAIL).

### D-4 (aberta, margem de 13 Ciclos): primeiro fonema da fase +666

- Medida: `audio-ssi263-phonemes-p333`, primeiro fonema: oráculo 27 803 Ciclos, Rust 29 486, diferença 1 683 > J_ssi 1 670. Na fase 0 a diferença é 1 013 (PASS).
- Causa raiz: o fim do fonema só é visto pelo AppleWin em `PeriodicUpdate` (a cada ~1000 Ciclos) e o primeiro `Update` pré-carrega 25 % do buffer, o que encurta/alonga o primeiro fonema conforme a fase; o Rust agenda o fim em `início + comprimento` (decisão de projeto em `ssi263.rs`). A fase 333 é a segunda execução de calibração, e o Rust fica a 13 Ciclos (0,8 %) do limiar nela.
- Estado: FAIL pelo limiar registrado; nos 12 fonemas seguintes e nas fixtures o Rust fica dentro da tolerância.

### D-5 (corrigida): tipo de AY padrão

- Achado ao implementar AUD-P-006: o padrão de `SoundSockets` era `[AY-3-8910, AY-3-8910, vazio, vazio]` (e o Phasor `8910 x4`); no AppleWin todo AY nasce AY-3-8913 (`AY8913::AY8913`, AY8910.cpp:127) e o registro nunca é lido na partida (Mockingboard.cpp:101-102, TODO). Com o soquete 3 vazio, o segundo AY do Phasor ficava mudo por padrão.
- Correção: padrão `[AY-3-8913; 4]`. Regressão: `sockets::default_sockets_are_all_ay_3_8913_like_applewin` e o caso `audio-ay-sockets-bad6522` (os registradores 14/15 dependem do tipo).

### D-6 (do oráculo, corrigida): rótulo de ciclo dos acessos `$C0(8+slot)x`

- `PhasorIOInternal` não sincroniza `g_nCumulativeCycles`; o gancho do oráculo chama `CpuCalcCycles(nExecutedCycles)` antes de registrar `PIO` (o primeiro evento saía com ciclo 0). Sem efeito no Rust.

### D-7 (nota, frontend): relógio do barramento de áudio ao carregar um estado PAL

- A fixture SSI263-4x é PAL (20 280 Ciclos por quadro). O relógio do `AudioBus` só é ajustado depois do carregamento; o fonema reiniciado no carregamento (`rearm_events`) usa o relógio antigo (NTSC) e dura ~0,5 % a mais que os seguintes (122 923 contra 122 338 Ciclos). Dentro da tolerância; a correção é do frontend (`crates/applewin`, fora desta fronteira): criar o barramento com o padrão de vídeo do estado antes de reconstruir as cartas.

### D-8 (nota): rótulo de ciclo dos eventos

- O AppleWin entrega ao tratador de E/S o ciclo do início do opcode (`g_nCumulativeCycles`); o log de eventos do Rust usa `CardCtx::access_cycle`, o mesmo valor. As sincronizações internas do 6522 usam o ciclo do acesso (`cycles`) com a compensação do D-1.

### D-9 (nota): ordem de eventos simultâneos de dois chips SSI263

- O AppleWin visita o chip 0 e depois o 1 em `PeriodicUpdate`; o escalonador do Rust reinsere eventos simultâneos em ordem inversa (como o `SynchronousEventManager`), então dois `PHON` no mesmo ciclo saem em ordem trocada. Não é observável pelo programa; o comparador trata cada chip como sua própria sequência.

## Lacunas

- SC-01 e SSI263 só têm duração de fonema comparada; o timbre (PCM do SSI263) fica para o roteiro manual.
- SSI263 em modo Phasor e Echo+ não têm caso de fonema (o Phasor tem caso de AY).
- MEGA Audio e SD Music (um 6522) não têm caso diferencial.
- O PCM do Mockingboard com SSI263 somado (`Mixer`) não é comparado.
