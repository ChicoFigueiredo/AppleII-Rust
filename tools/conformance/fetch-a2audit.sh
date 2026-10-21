#!/usr/bin/env bash
# Baixa o a2audit (https://github.com/zellyn/a2audit, licença MIT) num SHA fixo para o cache
# fora do Git. O conteúdo NÃO vai para o Git; só o script, os hashes e os resultados.
# Sem rede o script falha e o resultado do a2audit é BLOCKED.
set -euo pipefail

REPO_URL="https://github.com/zellyn/a2audit"
PINNED_SHA="6fdcedfd909b0d3a41dc7b6511d0932c952ecca0"   # main em 2025-03-21 (último push)
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CACHE_DIR="${A2AUDIT_CACHE:-$ROOT/target/conformance-cache/a2audit}"

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
echo "a2audit em $actual ($CACHE_DIR)"
