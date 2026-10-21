#!/usr/bin/env bash
# Licencas de terceiros (HTML) para o pacote. Requer: cargo install cargo-about --features cli
#   tools/package/licenses.sh <saida.html>
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$(dirname "$1")"; out=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
mkdir -p "$(dirname "$out")"
cargo about generate -c "$here/about.toml" --manifest-path crates/apple2-rust/Cargo.toml "$here/about.hbs" -o "$out"
echo "licencas de terceiros: $(grep -c '<li' "$out") entradas -> $out"
