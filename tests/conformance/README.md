# Testes de conformidade

Esta pasta guarda o que a suíte de conformidade lê e produz. A estratégia, a política de comparação e os formatos estão em [docs/conformance/test-oracles.md](../../docs/conformance/test-oracles.md). O Oráculo e a política de Fixtures estão na [ADR 0004](../../docs/adr/0004-oraculo-de-conformidade.md).

O harness é escrito no crate `crates/apple2-conformance`, binário `conformance`. Enquanto ele não existe, esta página descreve a intenção; os comandos abaixo estão marcados "a definir".

## Estrutura

| Pasta | Conteúdo |
|---|---|
| `manifests/` | Manifestos de execução e o esquema `manifests/schema.json` |
| `inputs/` | Entradas pequenas e geradas por nós (sequências de teclas, configurações de Máquina), e o índice das Fixtures externas (caminho, SHA-256, modelo, finalidade) |
| `reference-traces/` | Traces de referência v1 (CSV) gerados pelo AppleWin, com a origem e o SHA nos metadados |
| `reports/` | Relatórios das execuções (JUnit, diffs) |

Em `reports/` ficam só relatórios de referência que valem a pena versionar. O relatório de uma execução comum vai para `target/conformance-report`, fora do controle de versão.

## Como rodar

Tudo abaixo é **a definir**: os nomes dos subcomandos dependem do harness.

```sh
# Rodar a suíte inteira (a definir)
cargo run --release -p apple2-conformance --bin conformance -- run --report target/conformance-report

# Baixar o corpus para o cache (a definir)
cargo run --release -p apple2-conformance --bin conformance -- fetch

# Comparar um Trace de referência (a definir)
cargo run --release -p apple2-conformance --bin conformance -- compare-trace ...
```

O workflow `.github/workflows/conformance.yml` usa o primeiro comando, de forma provisória. Como toda a CI, é manual: ninguém o dispara sem autorização do dono.

## Política de cache e Fixtures

- O repositório `AppleWin/AppleWin-Test` não tem licença. Ele **não** é copiado para o repositório nem redistribuído.
- O harness o baixa com SHA fixo (o de [baseline.md](../../docs/conformance/baseline.md)) para `target/conformance-cache/`, que está fora do Git e é apagada por `cargo clean`.
- No Git ficam só o índice das Fixtures, os hashes SHA-256, e os goldens e Traces gerados por nós.
- Cada arquivo baixado é conferido contra o SHA-256 do índice antes do uso. Hash diferente é FAIL com a causa, não é ignorado.
- Sem rede e sem cache, os testes que dependem do corpus terminam em **BLOCKED**, com a causa registrada no Manifesto. Nunca são pulados em silêncio.
- Fixtures próprias (sequências de teclas, discos em branco gerados na hora) têm proveniência no índice. Software de terceiros só entra com licença e procedimento de obtenção documentados.
- Testes que escrevem em discos ou save-states trabalham numa cópia; a Fixture mestre nunca é alterada.
- As ROMs são as de `roms/` (veja [rom-provenance.md](../../docs/conformance/rom-provenance.md)); o `rom_sha256` entra em todo Manifesto.
