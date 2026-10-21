# Fixtures obrigatórias da HU-0002 no AppleWin-Test

SHA fixado: `4b113ce8cff1426bda5595cac9cb04776adfbb5d` (master em 2026-10-04).
Cache fora do Git: `target/conformance-cache/applewin-test/` (o repositório não tem licença; nada dele é copiado para o Git).
Verificação feita em 2026-10-20 sobre o checkout desse SHA; hashes SHA-256 em `applewin-test-index.csv`.

| Nome citado no plano | Caminho real no SHA | Situação |
|---|---|---|
| `v2/Memory/Saturn64.aws.yaml` | `v2/Memory/Saturn64.aws.yaml` | encontrado |
| `Saturn128.aws.yaml` | `v2/Memory/Saturn128.aws.yaml` | encontrado (o plano não dava o diretório) |
| `v2/Memory/LC-RAMWrite-Test.aws.yaml` | `v2/Memory/LC-RamWrite-Test.aws.yaml` | encontrado com **grafia diferente** (`RamWrite`, não `RAMWrite`); em sistema de arquivos que distingue caixa o nome do plano não existe |
| `v2/Mockingboard/SSI263-4x_phonemes_playing_continuously.aws.yaml` | `v2/Mockingboard/SSI263-4x_phonemes_playing_continuously.aws.yaml` | encontrado (há também a variante `-alt`) |
| `v2/HDD-32MB.aws.yaml` | `v2/HDD-32MB.aws.yaml` | encontrado |
| `v2/AS-S1_Turn the disk over(PAL).aws.yaml` | `v2/AS-S1_Turn the disk over(PAL).aws.yaml` | encontrado |

Observação: o relatório de carga está em `tests/conformance/reports/applewin-test-load.md`. "Encontrado" aqui significa só que o arquivo existe; não é PASS de fidelidade.
