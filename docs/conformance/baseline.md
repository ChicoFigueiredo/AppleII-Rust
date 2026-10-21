# Baseline de conformidade

Data de coleta: **2026-10-20**. Todos os repositórios abaixo estão fixados por SHA. Nenhuma comparação deste projeto usa `main` ou `master` móveis. Para mudar um SHA, crie uma nova versão deste arquivo e reexecute os testes afetados (veja a [ADR 0004](../adr/0004-oraculo-de-conformidade.md)).

## SHAs congelados

| Repositório | Referência | SHA completo |
|---|---|---|
| AppleII-Rust | `main` (ponto de partida da auditoria) | `6da60488a2c61b82099d6a7aabece21125eb0963` |
| AppleWin (Oráculo) | commit `a8f0d1d`, de 17/10/2026 | `a8f0d1d40f551ad9e010522c29c2891e33c88275` |
| AppleWin (release de referência) | tag `v1.32.1.0` (commit apontado pela tag) | `daf813dcaced7469bbbc16f23905cfd75abe8496` |
| AppleWin-Test | HEAD de `master` em 2026-10-20 (commit de 2026-10-04) | `4b113ce8cff1426bda5595cac9cb04776adfbb5d` |
| audetto/AppleWin (fork Linux) | HEAD de `master` em 2026-10-20 (commit de 2026-10-10) | `a1e20c0362f392dc25f1a3b218d2f84d4f1a25da` |

O SHA do AppleII-Rust é o de `git rev-parse HEAD` na worktree de trabalho, antes dos commits de documentação desta frente. Ele identifica o código auditado; os commits seguintes só acrescentam documentação, testes e o que for corrigido, e cada Manifesto de execução registra o SHA exato usado.

O AppleWin-Test não tem licença e é usado só como cache local (ADR 0004). O SHA acima é o que o harness baixa.

## Os 5 commits entre a v1.32.1.0 e a8f0d1d

A tag `v1.32.1.0` é ancestral de `a8f0d1d` (comparação do GitHub: `ahead_by = 5`, `behind_by = 0`; base comum `daf813d`, a própria tag). Os commits, do mais antigo para o mais recente, como retornados por:

```sh
gh api repos/AppleWin/AppleWin/compare/v1.32.1.0...a8f0d1d40f551ad9e010522c29c2891e33c88275 \
  --jq '.commits[] | .sha[0:7] + " " + (.commit.message|split("\n")[0])'
```

```text
4eee46a Update README for AppleWin v1.32.0.0 release
6c579da Add cmd line: -no-joystick<1|2>. (#1234)
18303d4 Saved image format - change from bmp to png (#1512 PR #1530)
5b7360e Minor: fix braces for coding style
a8f0d1d Add VS2026 sln/proj files using Platform Toolset v145
```

Consequência para os testes: o executável oficial v1.32.1.0 (Wine) **não contém** esses cinco commits. Duas mudanças podem alterar comportamento observável: a opção de linha de comando `-no-joystick<1|2>` (#1234) e o formato das imagens de captura de tela, que passou de BMP para PNG (#1512). Os demais são documentação, estilo e arquivos de projeto. Onde um teste depender de algo que mudou, o Oráculo é o código de a8f0d1d (e o fork do audetto, se o SHA dele o contiver; isso deve ser conferido no Manifesto), não o executável da v1.32.1.0.

## Toolchain e sistema hospedeiro

Coletados em 2026-10-20 na máquina de desenvolvimento onde a evidência local é gerada.

| Item | Valor |
|---|---|
| `rustc -V` | `rustc 1.98.1 (48a229cea 2026-09-01)` |
| `cargo -V` | `cargo 1.98.1 (797e8a9bc 2026-08-05)` |
| `uname -a` | `Linux cerebro 7.0.0-38-generic #38-Ubuntu SMP PREEMPT_DYNAMIC Fri Sep  4 09:10:14 UTC 2026 x86_64 GNU/Linux` |
| `lsb_release -a` | Ubuntu 26.04.1 LTS (`resolute`), release 26.04 |
| Wine | `wine-10.0 (Ubuntu 10.0~repack-12ubuntu1)` |

## O que este baseline não cobre

- Windows nativo e macOS: não há execução; ficam BLOCKED até o dono autorizar (veja [release-readiness.md](release-readiness.md)).
- Os números de issues do AppleWin (totais, abertas, fechadas) não são fixados aqui; a matriz das issues registra a data e a contagem de cada coleta.
