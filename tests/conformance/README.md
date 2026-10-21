# Testes de conformidade

Esta pasta guarda o que a suíte de conformidade lê e produz. A estratégia, a política de comparação e os formatos estão em [docs/conformance/test-oracles.md](../../docs/conformance/test-oracles.md). O Oráculo e a política de Fixtures estão na [ADR 0004](../../docs/adr/0004-oraculo-de-conformidade.md).

O harness é escrito no crate `crates/apple2-conformance`, binário `conformance`.

## Estrutura

| Pasta | Conteúdo |
|---|---|
| `manifests/` | Manifestos de execução e o esquema `manifests/schema.json` |
| `inputs/` | Entradas pequenas e geradas por nós (sequências de teclas, configurações de Máquina), e o índice das Fixtures externas (caminho, SHA-256, modelo, finalidade). `inputs/video/*.scene`: cenas determinísticas do diferencial de vídeo, geradas por `crates/apple2-video/examples/gen_scenes.rs` |
| `reference-traces/` | Traces de referência v1 (CSV) gerados pelo AppleWin, com a origem e o SHA nos metadados |
| `audio/` | Programas 6502 de áudio (`programs/*.s` e os `.bin` montados) e os scripts do núcleo do AY (`ay-core/*.script`); ver [audit/audio-differential.md](../../docs/conformance/audit/audio-differential.md) |
| `reference-audio/` | Eventos de áudio (`*.events`) e PCM (`*.pcm`, s16le) gravados pelo oráculo (fork do audetto com `audetto-audio.patch`) para os casos `audio-*` |
| `reports/` | Relatórios das execuções (JUnit, diffs) |

Em `reports/` ficam só relatórios de referência que valem a pena versionar. O relatório de uma execução comum vai para `target/conformance-report`, fora do controle de versão.

## Casos de vídeo

Os `cases/video-*.toml` são casos `framebuffer-hash` com `authority = "applewin"`: o SHA-256 do framebuffer completo (600x420, ou 680x436 com VidHD) que o oráculo (`audetto-oracle --fb-out`) produziu para a mesma cena, Tipo de vídeo, frequência e carta. O `source` de cada um traz o comando do oráculo. A tabela `[video]` aceita `type`, `hz`, `rgb_card`, `region` (`visible` ou `full`), `scene`, `scene_sha256` e `vidhd_slot`. Regerar: `tools/conformance/oracle/video-differential.py --cards 0,1,2 --write-cases tests/conformance/cases`. O registro do diferencial está em [video-differential.md](../../docs/conformance/audit/video-differential.md).

## Como rodar

```sh
cargo build --release -p apple2-conformance

# Rodar todos os casos (cases/*.toml) e gerar o relatório (JUnit e manifestos)
./target/release/conformance run --all --report target/conformance-report

# Um caso só
./target/release/conformance run --case audio-ay-tone --report target/conformance-report

# Baixar o corpus AppleWin-Test (SHA fixo) para o cache; sem ele, os casos que o usam ficam BLOCKED
tools/conformance/fetch-applewin-test.sh
```

Saída: `PASS`, `FAIL` ou `BLOCKED` por caso; os casos com `known_divergence` (PCM do AY e do alto-falante, SSI263 na fase +666, #1176) terminam em FAIL por desenho e fazem o comando sair com código diferente de zero. As contagens e os relatórios por área estão em [docs/conformance/relatorio-final.md](../../docs/conformance/relatorio-final.md).

O workflow `conformance.yml` roda `conformance run --all`. Como toda a CI, é manual: ninguém o dispara sem autorização do dono.

## Desempenho (HU-1001)

O crate `crates/apple2-bench` (binário `apple2-bench`) mede nove cenários determinísticos (CPU pura, boot e texto, HGR, DHGR, NTSC monitor, NTSC TV, PAL, Disk II e Mockingboard), a latência de entrada e a decomposição do custo de um quadro.

```sh
# Medir (10 repetições por cenário, cada uma num processo novo) e gerar relatório e JSON
cargo run --release -p apple2-bench -- --breakdown \
  --json tests/conformance/reports/performance.json --report tests/conformance/reports/performance.md

# Regravar o baseline (só ao mudar de propósito timing/vídeo/áudio, ou a máquina de referência)
cargo run --release -p apple2-bench -- --no-latency --write-baseline tests/conformance/perf-baseline.json

# Conferir contra o baseline (falha com código 1 em regressão; usado no conformance.yml)
cargo run --release -p apple2-bench -- --check tests/conformance/perf-baseline.json --no-latency
```

- `perf-baseline.json` guarda, por cenário, o MHz mediano e o melhor (sobre o tempo de CPU da thread), o CV, a margem e a impressão digital (ciclos emulados, hash do último quadro, hash do áudio).
- A margem é `max(15 %, 3 x CV)`; a explicação, o critério (mediana e melhor caso) e o modo entre máquinas estão em `crates/apple2-bench/src/report.rs`.
- A impressão digital precisa ser idêntica: uma otimização que mude timing, vídeo ou áudio falha o `--check` antes de qualquer discussão de velocidade.
- Meça em máquina ociosa. Um vizinho no mesmo núcleo físico custou ~30 % nos cenários Color TV; veja a nota em `reports/performance.md`.

## Política de cache e Fixtures

- O repositório `AppleWin/AppleWin-Test` não tem licença. Ele **não** é copiado para o repositório nem redistribuído.
- O harness o baixa com SHA fixo (o de [baseline.md](../../docs/conformance/baseline.md)) para `target/conformance-cache/`, que está fora do Git e é apagada por `cargo clean`.
- No Git ficam só o índice das Fixtures, os hashes SHA-256, e os goldens e Traces gerados por nós.
- Cada arquivo baixado é conferido contra o SHA-256 do índice antes do uso. Hash diferente é FAIL com a causa, não é ignorado.
- Sem rede e sem cache, os testes que dependem do corpus terminam em **BLOCKED**, com a causa registrada no Manifesto. Nunca são pulados em silêncio.
- Fixtures próprias (sequências de teclas, discos em branco gerados na hora) têm proveniência no índice. Software de terceiros só entra com licença e procedimento de obtenção documentados.
- Testes que escrevem em discos ou save-states trabalham numa cópia; a Fixture mestre nunca é alterada.
- As ROMs são as de `roms/` (veja [rom-provenance.md](../../docs/conformance/rom-provenance.md)); o `rom_sha256` entra em todo Manifesto.
