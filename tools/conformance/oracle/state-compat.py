#!/usr/bin/env python3
"""Adapts a save-state written by AppleWin a8f0d1d / v1.32.1.0 so that the older audetto fork can load it.

The fork predates "Unit Game I/O Connector" version 4 (SaveState.cpp: UNIT_GAME_IO_CONNECTOR_VER 3 vs 4,
`Shift Key Mod`). The only change made is to drop a Game I/O Connector unit that carries no copy-protection
dongle (no `Device:` key): in that case the unit holds just `Shift Key Mod`, which does not affect CPU execution.
Everything else is copied byte for byte.

usage: state-compat.py IN.aws.yaml OUT.aws.yaml
"""
import sys


def main(src, dst):
    with open(src, "rb") as f:
        text = f.read().decode("utf-8", errors="surrogateescape")
    lines = text.splitlines(keepends=True)
    out = []
    i = 0
    dropped = 0
    while i < len(lines):
        if lines[i].rstrip("\r\n") == "Unit:":
            j = i + 1
            while j < len(lines) and (lines[j].startswith(" ") or lines[j].strip() == ""):
                j += 1
            block = lines[i:j]
            if any(l.strip() == "Type: Game I/O Connector" for l in block) and not any(
                l.strip().startswith("Device:") for l in block
            ):
                dropped += 1
                i = j
                continue
            out.extend(block)
            i = j
            continue
        out.append(lines[i])
        i += 1
    with open(dst, "wb") as f:
        f.write("".join(out).encode("utf-8", errors="surrogateescape"))
    print(f"state-compat: dropped {dropped} Game I/O Connector unit(s)", file=sys.stderr)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
