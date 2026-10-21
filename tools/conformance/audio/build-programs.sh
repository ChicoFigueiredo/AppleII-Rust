#!/usr/bin/env bash
# Assembles the audio conformance programs (tests/conformance/audio/programs/*.s) into .bin files
# with tools/conformance/audio/asm6502.py. The .bin files are committed: both emulators load them.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROG="$HERE/../../../tests/conformance/audio/programs"
ASM="$HERE/asm6502.py"
for s in "$PROG"/*.s; do
  name="$(basename "$s" .s)"
  if [ "$name" = "ssi263-phonemes" ]; then
    # PHASE = NOP bytes before the first SSI263 access: the same program at different phases of
    # AppleWin's 1000-cycle SSI263 update (calibration of the phoneme-duration tolerance).
    for phase in 0 333; do
      python3 -I "$ASM" "$s" -D "PHASE=$phase" -o "$PROG/$name-p$phase.bin"
    done
  else
    python3 -I "$ASM" "$s" -o "$PROG/$name.bin"
  fi
done
