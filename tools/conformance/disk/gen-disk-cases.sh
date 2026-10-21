#!/usr/bin/env bash
# Gera os casos de disco (tests/conformance/cases/disk-*.toml) e os arquivos de checkpoints
# (tests/conformance/reference-traces/disk/*.ckpt) a partir de disk-cases.txt, rodando o oráculo
# AppleWin (fork audetto) sobre cópias das imagens e comparando o Rust passo a passo.
#
# Pré-requisitos: tools/conformance/oracle/build-audetto-oracle.sh já rodou; as imagens do
# corpus (@C@) vêm de tools/conformance/fetch-applewin-test.sh (as ausentes são puladas);
# cargo build --release -p apple2-storage --example disk_follow --example make_micro.
# As imagens dos micro-programas: target/release/examples/make_micro tests/conformance/inputs/disk-micro
#
# usage: gen-disk-cases.sh [filtro-de-id]
# Variáveis: DISK_SCRATCH (traces completos do oráculo; padrão target/disk-differential),
#            ORACLE_CACHE (cache do oráculo; use um privado se outros agentes reconstroem o binário).
set -eu
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
W="$(git -C "$HERE" rev-parse --show-toplevel)"
COMMON="$(git -C "$HERE" rev-parse --path-format=absolute --git-common-dir)"
C="$(dirname "$COMMON")/target/conformance-cache/applewin-test/v2"
S="${DISK_SCRATCH:-$W/target/disk-differential}"
cd "$W"
mkdir -p tests/conformance/reference-traces/disk "$S"
DOS="assets/disks/DOS 3.3 System Master - 680-0210-A.dsk"
PRO="assets/disks/ProDOS_2_4_3.po"
M=tests/conformance/inputs/disk-micro
filter=${1:-}
sub() { local v=$1; v=${v//@DOS@/$DOS}; v=${v//@PRO@/$PRO}; v=${v//@C@/$C}; v=${v//@M@/$M}; echo "$v"; }

while IFS='|' read -r id rmodel omodel cyc mc fx fx2 hdd hdd2 flags label story issues; do
  [ -z "$id" ] && continue
  case "$id" in \#*) continue;; esac
  [ -n "$filter" ] && [[ "$id" != *$filter* ]] && continue
  fx=$(sub "$fx"); fx2=$(sub "$fx2"); hdd=$(sub "$hdd"); hdd2=$(sub "$hdd2")
  missing=0
  for f in "$fx" "$fx2" "$hdd" "$hdd2"; do
    if [ -n "$f" ] && [ ! -f "$f" ]; then echo "pulando $id: falta $f"; missing=1; fi
  done
  [ $missing = 1 ] && continue
  authentic=0; ro=()
  IFS=',' read -ra fl <<<"$flags"
  for f in "${fl[@]:-}"; do case "$f" in authentic) authentic=1;; ro:*) ro+=("${f#ro:}");; esac; done

  # --- oráculo sobre cópias (ele grava nas imagens) ---
  rd="$S/run/$id"; rm -rf "$rd"; mkdir -p "$rd"
  args=(); finals=()
  put() { # papel caminho opção
    local role=$1 path=$2 flag=$3
    [ -z "$path" ] && return
    local dst="$rd/$role-$(basename "$path")"
    cp "$path" "$dst"; chmod 644 "$dst"
    for r in "${ro[@]:-}"; do [ "$r" = "$role" ] && chmod 444 "$dst"; done
    args+=("$flag" "$dst"); eval "FILE_$role=\$dst"
  }
  put fixture "$fx" --disk; put fixture2 "$fx2" --disk2; put hdd "$hdd" --hdd; put hdd2 "$hdd2" --hdd2
  [ $authentic = 1 ] && args+=(--authentic)
  tools/conformance/oracle/run-audetto-oracle.sh --model "$omodel" --cycles "$cyc" --memclear "$mc" "${args[@]}" --out "$S/$id.o.csv" 2>&1 | tail -1
  for role in fixture fixture2 hdd hdd2; do
    var="FILE_$role"
    if [ -n "${!var:-}" ]; then
      # .gz: o conteúdo descomprimido (os bytes comprimidos dependem do compressor)
      finals+=(--oracle-final "$role=$(case "${!var}" in *.gz) gunzip -c "${!var}" ;; *) cat "${!var}" ;; esac | sha256sum | cut -d' ' -f1)")
    fi
    unset "$var" 2>/dev/null || true
  done

  # --- arquivo do caso ---
  case "$rmodel" in
    apple2plus) modeltxt="Apple ][+"; cards='["0:language-card-16k", "6:disk2"]';;
    apple2e) modeltxt="Apple //e"; cards='["6:disk2"]';;
    *) modeltxt="Apple //e Enhanced"; cards='["6:disk2"]';;
  esac
  {
    echo "id = \"$id\""
    echo "story = \"${story:-HU-0301}\""
    echo "criterion = \"$label\""
    echo "model = \"$rmodel\""
    echo "cards = $cards"
    echo "memclear = $mc"
    [ $authentic = 1 ] && echo "authentic = true"
    if [ ${#ro[@]} -gt 0 ]; then
      printf 'readonly = ['; first=1
      for r in "${ro[@]}"; do [ $first = 0 ] && printf ', '; printf '"%s"' "$r"; first=0; done
      echo ']'
    fi
    echo "notes = \"Issues do AppleWin: ${issues:-n/a}. Boot de imagem ou micro-programa de boot (tests/conformance/inputs/disk-micro, gerado por cargo run --release -p apple2-storage --example make_micro) comparado com o oráculo AppleWin (fork audetto, $modeltxt, memclear $mc$([ $authentic = 1 ] && echo ', Enhance Disk Speed desligado')): sequência de instruções ciclo a ciclo e SHA-256 das imagens ao final. As imagens do corpus AppleWin-Test não são versionadas (sem elas o caso fica BLOCKED). Regerar: tools/conformance/disk/gen-disk-cases.sh $id (ver docs/conformance/audit/disk-differential.md).\""
    emit() { # tabela caminho
      [ -z "$2" ] && return
      local rel=$2; case "$rel" in "$C"/*) rel="target/conformance-cache/applewin-test/v2/${rel#$C/}";; esac
      echo; echo "[$1]"; echo "path = \"$rel\""; echo "sha256 = \"$(sha256sum "$2" | cut -d' ' -f1)\""
    }
    echo; echo "[run]"; echo "cycles = $cyc"
    emit fixture "$fx"; emit fixture2 "$fx2"; emit hdd "$hdd"; emit hdd2 "$hdd2"
    echo; echo "[oracle]"; echo 'kind = "trace-checkpoints"'; echo 'authority = "applewin"'
    echo "source = \"AppleWin (fork audetto a1e20c03, upstream 3e8054b) via tools/conformance/oracle/run-audetto-oracle.sh --model $omodel --cycles $cyc --memclear $mc; checkpoints gerados por disk_follow\""
    echo "trace = \"reference-traces/disk/$id.ckpt\""
  } > "tests/conformance/cases/$id.toml"
  echo "== $id"
  target/release/examples/disk_follow --case "tests/conformance/cases/$id.toml" --oracle "$S/$id.o.csv" \
    --emit-ckpt "tests/conformance/reference-traces/disk/$id.ckpt" "${finals[@]}" | tail -"${TL:-6}"
done < "$HERE/disk-cases.txt"
