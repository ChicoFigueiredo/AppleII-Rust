#!/usr/bin/env bash
# Baixa o SingleStepTests/65x02 (licença MIT) num SHA fixo para o cache fora do Git.
# Diretórios usados: 6502 (NMOS com ilegais), wdc65c02, rockwell65c02, synertek65c02 (cada um com v1/).
set -euo pipefail

REPO_URL="https://github.com/SingleStepTests/65x02"
PINNED_SHA="2f6980a2d95757486c7bee24355c360e40e2a224"   # main em 2026-10-20
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CACHE_DIR="${C65X02_TESTS_CACHE:-$ROOT/target/conformance-cache/65x02-tests}"

mkdir -p "$CACHE_DIR"
if [ ! -d "$CACHE_DIR/.git" ]; then
    git -C "$CACHE_DIR" init -q
    git -C "$CACHE_DIR" remote add origin "$REPO_URL"
fi
if ! git -C "$CACHE_DIR" cat-file -e "${PINNED_SHA}^{commit}" 2>/dev/null; then
    git -C "$CACHE_DIR" fetch --depth 1 origin "$PINNED_SHA"
fi
git -C "$CACHE_DIR" checkout --force --detach "$PINNED_SHA"
actual="$(git -C "$CACHE_DIR" rev-parse HEAD)"
if [ "$actual" != "$PINNED_SHA" ]; then
    echo "ERRO: SHA obtido $actual difere do fixado $PINNED_SHA" >&2
    exit 1
fi
echo "SingleStepTests/65x02 em $actual ($CACHE_DIR)"
