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

## Resultados

(preenchidos depois da calibração, na seção "Cenários × resultado" abaixo)
