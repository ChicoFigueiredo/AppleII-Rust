#!/usr/bin/env python3
"""Runs the AY core self-test scripts on the oracle and records tests/conformance/reference-audio/ay-core/*.pcm.

  run-ay-core.py [--bin ORACLE]

The oracle binary is --bin, $ORACLE_BIN or <repo>/target/conformance-cache/oracle/build/audetto-oracle.
Output per script: raw s16le, 3 voices interleaved per sample (what AY8913::sound_frame wrote).
"""
import argparse
import os
import subprocess
import sys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin")
    ns = ap.parse_args()
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.abspath(os.path.join(here, "..", "..", ".."))
    common = os.popen("git -C %s rev-parse --path-format=absolute --git-common-dir" % repo).read().strip()
    default_bin = os.path.join(os.path.dirname(common), "target", "conformance-cache", "oracle", "build", "audetto-oracle")
    binary = ns.bin or os.environ.get("ORACLE_BIN") or default_bin
    if not os.path.exists(binary):
        print(f"BLOCKED: oráculo ausente: {binary}")
        return 2
    src = os.path.join(repo, "tests", "conformance", "audio", "ay-core")
    out = os.path.join(repo, "tests", "conformance", "reference-audio", "ay-core")
    os.makedirs(out, exist_ok=True)
    for name in sorted(os.listdir(src)):
        if not name.endswith(".script"):
            continue
        base = name[: -len(".script")]
        cmd = [binary, "--model", "apple2p", "--cycles", "100", "--cards", "none", "--out", os.devnull,
               "--audio-ay-script", os.path.join(src, name), "--audio-ay-out", os.path.join(out, base + ".pcm")]
        r = subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print(base, "ok" if r.returncode == 0 else f"exit {r.returncode}")
        if r.returncode:
            return r.returncode
    return 0


if __name__ == "__main__":
    sys.exit(main())
