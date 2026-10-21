#!/usr/bin/env bash
# Generates a Trace v1 (CSV) with the AppleWin oracle (audetto fork), headless and deterministic.
#
# usage: run-audetto-oracle.sh --model <apple2|apple2p|apple2jp|apple2e|apple2ee>
#                              (--cycles N | --frames N) [--out FILE]
#                              [--disk IMAGE] [--disk2 IMAGE] [--hdd IMAGE] [--hdd2 IMAGE] [--authentic]
#                              [--state FILE.aws.yaml]
#                              [--memclear 0..7] [--cards disk2|none]
# Without --out the CSV goes to stdout. Run build-audetto-oracle.sh first.
set -euo pipefail

# Determinism: AppleWin itself (a8f0d1d too, LanguageCard.cpp:345,533) allocates the Saturn banks
# with `new BYTE[kMemBankSize]` and never clears them; a partial `Memory BankNN` dump in a
# save-state (e.g. "0000: 00") leaves the rest undefined (heap garbage on Linux, different between
# states). This is undefined behaviour of AppleWin, not a fork defect. glibc fills every malloc with
# the complement of this byte, i.e. zeros, which fixes the undefined bytes at the value the Rust port
# (and a fresh Windows heap) holds.
export MALLOC_PERTURB_=255

FORK_SHA="a1e20c0362f392dc25f1a3b218d2f84d4f1a25da"
PORT_BASE_SHA="a8f0d1d40f551ad9e010522c29c2891e33c88275"   # upstream commit the Rust port started from
FORK_UPSTREAM_SHA="3e8054b4627624398e4589f7f27b3d40a6b9718e"   # newest upstream AppleWin commit contained in the fork (merge-base)

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -z "${ORACLE_CACHE:-}" ]; then
  COMMON="$(git -C "$HERE" rev-parse --path-format=absolute --git-common-dir)"
  ORACLE_CACHE="$(dirname "$COMMON")/target/conformance-cache"
fi
CLONE="$ORACLE_CACHE/oracle/audetto-AppleWin"
BIN="${ORACLE_BIN:-$ORACLE_CACHE/oracle/build/audetto-oracle}"
[ -x "$BIN" ] || { echo "run-audetto-oracle: $BIN not found; run build-audetto-oracle.sh" >&2; exit 1; }

MODEL="" OUT="" ARGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --model) MODEL="$2"; ARGS+=("$1" "$2"); shift 2 ;;
    --out)   OUT="$(realpath -m "$2")"; shift 2 ;;   # absolute: the oracle may change directory
    --disk|--disk2|--hdd|--hdd2|--state) ARGS+=("$1" "$(realpath "$2")"); shift 2 ;;
    --authentic) ARGS+=("$1"); shift ;;
    --cycles|--frames|--memclear|--cards) ARGS+=("$1" "$2"); shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

# model -> ROM image(s) embedded in the fork (resource/*.rom -> apple2roms_data)
case "$MODEL" in
  apple2)           ROMS=(Apple2.rom) ;;
  apple2p|apple2plus)   ROMS=(Apple2_Plus.rom) ;;
  apple2jp|apple2jplus) ROMS=(Apple2_JPlus.rom) ;;
  apple2e)          ROMS=(Apple2e.rom) ;;
  apple2ee)         ROMS=(Apple2e_Enhanced.rom) ;;
  "")               ROMS=() ;;   # model comes from a save-state
  *) echo "unsupported model: $MODEL" >&2; exit 2 ;;
esac
ROM_META=()
for rom in "${ROMS[@]}"; do
  sum="$(sha256sum "$CLONE/resource/$rom" | cut -d' ' -f1)"
  ROM_META+=(--meta "rom_sha256=$sum  $rom")
done
for ((i = 0; i < ${#ARGS[@]}; i++)); do
  case "${ARGS[i]}" in
    --disk|--disk2|--hdd|--hdd2|--state) ROM_META+=(--meta "fixture_sha256=$(sha256sum "${ARGS[i+1]}" | cut -d' ' -f1)  ${ARGS[i]#--}") ;;
  esac
done

META=("${ROM_META[@]}"
  --meta "applewin_sha=$FORK_UPSTREAM_SHA"
  --meta "port_base_applewin_sha=$PORT_BASE_SHA"
  --meta "fork_sha=$FORK_SHA"
  --meta "fork_url=https://github.com/audetto/AppleWin")

case "$OUT" in ""|/*) ;; *) OUT="$PWD/$OUT" ;; esac
TARGET="${OUT:-/dev/stdout}"
if [ -n "$OUT" ]; then
  mkdir -p "$(dirname "$OUT")"
  TMP="$OUT.tmp.$$"
else
  TMP="$(mktemp)"
fi
# The oracle never touches ~/.config/applewin (in-memory registry); HOME is pinned anyway so
# that stray writes (screenshots, logs) cannot leak into the user's profile.
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"; [ -n "$OUT" ] && rm -f "$TMP"' EXIT
# A --state file written by a newer AppleWin than the fork is adapted first (see state-compat.py);
# the fixture hash in the metadata stays that of the original file.
for ((i = 0; i < ${#ARGS[@]}; i++)); do
  if [ "${ARGS[i]}" = "--state" ]; then
    # Disk images are looked up next to the save-state: link the neighbours into the scratch dir.
    STATE_DIR="$(cd "$(dirname "${ARGS[i+1]}")" && pwd)"
    # ORACLE_IMAGE_DIRS (colon-separated) adds more directories to look in.
    IFS=: read -r -a SEARCH_DIRS <<< "$STATE_DIR:${ORACLE_IMAGE_DIRS:-}"
    for d in "${SEARCH_DIRS[@]}"; do
      [ -n "$d" ] && [ -d "$d" ] || continue
      for f in "$(cd "$d" && pwd)"/*; do
        case "$f" in *.aws.yaml) ;; *) [ -f "$f" ] && ln -sf "$f" "$SCRATCH/$(basename "$f")" ;; esac
      done
    done
    python3 -I "$HERE/state-compat.py" "${ARGS[i+1]}" "$SCRATCH/state.aws.yaml"
    ARGS[i+1]="$SCRATCH/state.aws.yaml"
  fi
done
# cwd = the scratch dir: AppleWin opens the images named in a save-state relative to the cwd.
(cd "$SCRATCH" && HOME="$SCRATCH" "$BIN" "${ARGS[@]}" "${META[@]}" --out "$TMP" 1>&2)
if [ -n "$OUT" ]; then mv "$TMP" "$OUT"; else cat "$TMP" > "$TARGET"; rm -f "$TMP"; fi
