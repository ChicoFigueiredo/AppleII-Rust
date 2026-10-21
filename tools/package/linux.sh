#!/usr/bin/env bash
# Reproduz localmente o empacotamento Linux do release.yml (mesmos passos, mesma ordem):
#   build release -> smoke headless -> licencas -> package.sh linux -> SBOM -> SHA256SUMS
# e testa o pacote extraido DE FORA da arvore de desenvolvimento.
#   tools/package/linux.sh [versao]        (padrao: versao do [workspace.package])
# Saida: dist/ (pacote, SBOM, SHA256SUMS). Nao aciona CI.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
here=tools/package

ver=${1:-$(sed -n '/^\[workspace\.package\]/,/^\[/{s/^version *= *"\(.*\)".*/\1/p}' Cargo.toml | head -n1)}
rm -rf dist
mkdir -p dist

cargo build -p apple2-rust --release
BINDIR=target/release

# smoke headless (igual ao do release.yml)
"$BINDIR/apple2-rust" --headless-frames 300 --dump-screen dist/smoke.png >/dev/null 2>&1
test -s dist/smoke.png && rm dist/smoke.png

"$here/licenses.sh" dist/THIRD-PARTY-LICENSES.html
THIRD_PARTY_LICENSES=$(pwd)/dist/THIRD-PARTY-LICENSES.html packaging/package.sh linux "$ver" "$BINDIR"
rm dist/THIRD-PARTY-LICENSES.html
"$here/sbom.sh" "dist/apple2-rust-$ver.cdx.json"
"$here/checksums.sh" dist

"$here/test-package.sh" "dist/apple2-rust-$ver-linux-x64.tar.gz"
