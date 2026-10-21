#!/usr/bin/env bash
# Baixa o AppleWin-Test num SHA fixo para o cache fora do Git.
# O repositório AppleWin-Test NÃO tem licença: nunca copiar seu conteúdo para o Git.
# No Git ficam apenas o índice, os hashes e os resultados.
set -euo pipefail

REPO_URL="https://github.com/AppleWin/AppleWin-Test"
PINNED_SHA="4b113ce8cff1426bda5595cac9cb04776adfbb5d"   # master em 2026-10-04
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CACHE_DIR="${APPLEWIN_TEST_CACHE:-$ROOT/target/conformance-cache/applewin-test}"

mkdir -p "$(dirname "$CACHE_DIR")"
if [ ! -d "$CACHE_DIR/.git" ]; then
    git clone --no-checkout "$REPO_URL" "$CACHE_DIR"
fi
if ! git -C "$CACHE_DIR" cat-file -e "${PINNED_SHA}^{commit}" 2>/dev/null; then
    git -C "$CACHE_DIR" fetch origin
fi
git -C "$CACHE_DIR" checkout --force --detach "$PINNED_SHA"
actual="$(git -C "$CACHE_DIR" rev-parse HEAD)"
if [ "$actual" != "$PINNED_SHA" ]; then
    echo "ERRO: SHA obtido $actual difere do fixado $PINNED_SHA" >&2
    exit 1
fi
echo "AppleWin-Test em $actual ($CACHE_DIR)"
