#!/usr/bin/env bash
# Baixa o SingleStepTests/z80 (licença MIT) num SHA fixo para o cache fora do Git.
set -euo pipefail

REPO_URL="https://github.com/SingleStepTests/z80"
PINNED_SHA="ebe1875d48f374bcfd4b505d8eb8ee751568b5f7"   # main em 2026-10-20
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CACHE_DIR="${Z80_TESTS_CACHE:-$ROOT/target/conformance-cache/z80-tests}"

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
echo "SingleStepTests/z80 em $actual ($CACHE_DIR/v1)"

# ZEXDOC/ZEXALL (Frank Cringle, via superzazu/z80, MIT) para tests/z80_zex.rs.
ZEX_SHA="d64fe10a2274e5e40019b1086bf7d8990cbc5f23"
ZEX_DIR="$CACHE_DIR/zex"
mkdir -p "$ZEX_DIR"
fetch_zex() {
    local name="$1" sum="$2"
    if [ ! -f "$ZEX_DIR/$name" ]; then
        curl -fsSL -o "$ZEX_DIR/$name" \
            "https://raw.githubusercontent.com/superzazu/z80/$ZEX_SHA/roms/$name"
    fi
    echo "$sum  $ZEX_DIR/$name" | sha256sum -c --quiet - || { echo "ERRO: sha256 de $name" >&2; exit 1; }
}
fetch_zex zexdoc.cim 10b7c3972ff6765712ed160e5bd8750e4a13642f62b75711e062ef06a7f2f7b5
fetch_zex zexall.cim af7e5d86146d390a68440fb85668648f14a648602da29a1816d2ef11459411ae
echo "ZEXDOC/ZEXALL em $ZEX_DIR"
