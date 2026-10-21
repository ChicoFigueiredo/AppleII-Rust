#!/usr/bin/env bash
# Gera SHA256SUMS dos artefatos de um diretorio (pacotes, SBOM) e confere em seguida.
#   tools/package/checksums.sh <dir>
set -euo pipefail
cd "$1"
rm -f SHA256SUMS
files=$(ls -1 | grep -E '\.(tar\.gz|zip|cdx\.json)$' | LC_ALL=C sort)
[ -n "$files" ] || { echo "nenhum artefato em $1" >&2; exit 1; }
# shellcheck disable=SC2086
sha256sum $files > SHA256SUMS
sha256sum --check --strict SHA256SUMS
