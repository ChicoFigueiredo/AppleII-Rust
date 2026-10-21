#!/usr/bin/env bash
# SBOM CycloneDX 1.5 (JSON) do executavel `apple2-rust`. Requer: cargo install cargo-cyclonedx
#   tools/package/sbom.sh <saida.cdx.json>
set -euo pipefail
mkdir -p "$(dirname "$1")"; out=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
mkdir -p "$(dirname "$out")"
# SOURCE_DATE_EPOCH = data do commit, para SBOM reproduzivel
SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct 2>/dev/null || date +%s)} \
  cargo cyclonedx --manifest-path crates/apple2-rust/Cargo.toml --format json --spec-version 1.5 \
  --target all --no-build-deps --override-filename apple2-rust.cdx
mv crates/apple2-rust/apple2-rust.cdx.json "$out"
# arquivos por-crate que o cargo-cyclonedx gera no workspace nao devem sobrar na arvore
find crates -maxdepth 2 -name '*.cdx.json' -delete
python3 -I - "$out" <<'EOF'
import json, sys
d = json.load(open(sys.argv[1]))
assert d["bomFormat"] == "CycloneDX", d.get("bomFormat")
print(f"SBOM {d['specVersion']}: {len(d.get('components', []))} componentes")
EOF
