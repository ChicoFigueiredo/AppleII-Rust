#!/usr/bin/env python3
"""Rewrites an .aws.yaml saved by the Rust port so the audetto oracle can load it (`--state`).

The oracle is the audetto fork at AppleWin 3e8054b (2026-07-26); the port is based on a8f0d1d
(2026-10-17). Between them AppleWin added "Shift Key Mod" to the Game I/O unit (version 3 -> 4), and
the fork refuses the newer unit ("Version 4 is not supported for game I/O device"; the load fails
and the machine boots instead; relabelling it version 3 does not help: the old unit needs "Device:"). This script:

  * saves with SaveOptions { extension: false } expected (no "AppleII-Rust Extension" block),
    and drops the block if present;
  * removes the Game I/O unit when it only holds "Shift Key Mod" (no dongle).

usage: aws-for-oracle.py IN.aws.yaml OUT.aws.yaml
"""
import re
import sys


def main(src, dst):
    lines = open(src, encoding="utf-8").read().split("\n")
    out = []
    i = 0
    while i < len(lines):
        line = lines[i]
        # the port's extension block (inside the Apple2 unit state), until the next key at its level
        m = re.match(r"^(\s*)AppleII-Rust Extension:", line)
        if m:
            indent = len(m.group(1))
            i += 1
            while i < len(lines) and (not lines[i].strip() or len(lines[i]) - len(lines[i].lstrip()) > indent):
                i += 1
            continue
        out.append(line)
        i += 1
    text = "\n".join(out)
    # Game I/O unit: in 3e8054b it describes a copy-protection dongle ("Device: ..."); in a8f0d1d
    # version 4 carries "Shift Key Mod" when no dongle is plugged. Without a dongle, drop the unit.
    text = re.sub(
        r"\nUnit:\n  Type: Game I/O Connector\n  Version: 4\n  State:\n    Shift Key Mod: (?:true|false)\n",
        "\n",
        text,
    )
    open(dst, "w", encoding="utf-8").write(text)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(*sys.argv[1:])
