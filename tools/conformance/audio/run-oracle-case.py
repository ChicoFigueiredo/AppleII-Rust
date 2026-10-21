#!/usr/bin/env python3
"""Runs an audio conformance case on the AppleWin oracle (audetto-oracle) and records the reference.

  run-oracle-case.py CASE_ID [--bin ORACLE] [--out-dir DIR] [--slice N] [--root tests/conformance]
                     [--no-pcm]

Reads tests/conformance/cases/CASE_ID.toml and writes, into DIR (default tests/conformance/
reference-audio/), CASE_ID.events, CASE_ID.mb.pcm / CASE_ID.spkr.pcm (raw s16le, 44.1 kHz) and
CASE_ID.trace.csv and CASE_ID.ssi.N.pcm (the oracle's CPU trace and SSI263 PCM, only with --keep-all).
The oracle binary: --bin, $ORACLE_BIN, or <repo>/target/conformance-cache/oracle/build/audetto-oracle
(build it with tools/conformance/oracle/build-audetto-oracle.sh). Without it the case is BLOCKED.
"""
import argparse
import os
import subprocess
import sys
import tomllib

MODELS = {
    "apple2": "apple2", "apple2plus": "apple2p", "apple2jplus": "apple2jp",
    "apple2e": "apple2e", "apple2e-enhanced": "apple2ee",
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("case")
    ap.add_argument("--bin")
    ap.add_argument("--out-dir")
    ap.add_argument("--slice", type=int)
    ap.add_argument("--root", default=None)
    ap.add_argument("--workspace", default=None)
    ap.add_argument("--no-pcm", action="store_true")
    ap.add_argument("--program", help="override audio.program (calibration variants)")
    ap.add_argument("--tag", default="", help="suffix for the output file names")
    ap.add_argument("--keep-all", action="store_true", help="keep the oracle CPU trace and the SSI263 PCM (large)")
    ns = ap.parse_args()

    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.abspath(os.path.join(here, "..", "..", ".."))
    root = ns.root or os.path.join(repo, "tests", "conformance")
    workspace = ns.workspace or os.path.abspath(os.path.join(root, "..", ".."))
    out_dir = ns.out_dir or os.path.join(root, "reference-audio")
    os.makedirs(out_dir, exist_ok=True)

    common = os.popen("git -C %s rev-parse --path-format=absolute --git-common-dir" % repo).read().strip()
    default_bin = os.path.join(os.path.dirname(common), "target", "conformance-cache", "oracle", "build", "audetto-oracle")
    binary = ns.bin or os.environ.get("ORACLE_BIN") or default_bin
    if not os.path.exists(binary):
        print(f"BLOCKED: oráculo ausente: {binary} (tools/conformance/oracle/build-audetto-oracle.sh)")
        return 2

    with open(os.path.join(root, "cases", ns.case + ".toml"), "rb") as f:
        case = tomllib.load(f)
    audio = case.get("audio", {})
    run = case["run"]
    name = ns.case + ns.tag
    cmd = [binary]
    fixture = case.get("fixture")
    if fixture:
        state = os.path.join(workspace, fixture["path"])
        if not os.path.exists(state):
            print(f"BLOCKED: fixture ausente: {state}")
            return 2
        cmd += ["--state", state]
    else:
        cmd += ["--model", MODELS[case["model"]]]
    if "cycles" in run:
        cmd += ["--cycles", str(run["cycles"])]
    else:
        cmd += ["--frames", str(run["frames"])]
    cmd += ["--cards", "none", "--out", os.path.join(out_dir, name + ".trace.csv")]
    if audio.get("card"):
        cmd += ["--audio-card", audio["card"], "--audio-slot", str(audio.get("slot", 4))]
    program = ns.program or audio.get("program")
    if program:
        cmd += ["--audio-program", os.path.join(root, program),
                "--audio-load", "%X" % audio.get("load", 0x300),
                "--audio-pc", "%X" % audio.get("pc", audio.get("load", 0x300))]
    for i, t in enumerate(audio.get("ay_sockets", [])):
        cmd += ["--audio-ay-socket", f"{i}={t}"]
    for i, t in enumerate(audio.get("ssi263", [])):
        cmd += ["--audio-ssi263", f"{i}={t}"]
    for i, bad in enumerate(audio.get("bad_6522", [])):
        if bad:
            cmd += ["--audio-bad6522", "ab"[i]]
    if "sc01" in audio:
        cmd += ["--audio-sc01", "on" if audio["sc01"] else "off"]
    if ns.slice:
        cmd += ["--audio-slice", str(ns.slice)]
    cmd += ["--audio-events", os.path.join(out_dir, name + ".events")]
    if not ns.no_pcm:
        cmd += ["--audio-spkr-pcm", os.path.join(out_dir, name + ".spkr.pcm"),
                "--audio-mb-pcm", os.path.join(out_dir, name + ".mb.pcm"),
                "--audio-ssi-pcm", os.path.join(out_dir, name + ".ssi")]
    print(" ".join(cmd))
    r = subprocess.run(cmd)
    if r.returncode == 0 and "cycles" in run:
        # The oracle runs whole slices (--audio-slice, default 1000 Ciclos), so it overshoots the
        # requested count; events at or after the end are not part of the Rust run.
        ev = os.path.join(out_dir, name + ".events")
        with open(ev) as f:
            lines = f.read().splitlines()
        keep = [l for l in lines if l.startswith("#") or int(l.split(",", 1)[0]) < run["cycles"]]
        with open(ev, "w") as f:
            f.write("\n".join(keep) + "\n")
    if r.returncode == 0 and not ns.keep_all:
        # Only the events and the PCM are references; the CPU trace and the SSI263 PCM are large.
        # Keep a PCM file only if some case refers to it (the oracle always records both streams).
        cases_dir = os.path.join(root, "cases")
        used = ""
        for fn in os.listdir(cases_dir):
            with open(os.path.join(cases_dir, fn)) as f:
                used += f.read()
        extra = [n for n in (name + ".mb.pcm", name + ".spkr.pcm") if os.path.join("reference-audio", n) not in used]
        for f in [name + ".trace.csv", name + ".ssi.0.pcm", name + ".ssi.1.pcm"] + extra:
            try:
                os.remove(os.path.join(out_dir, f))
            except FileNotFoundError:
                pass
    return r.returncode


if __name__ == "__main__":
    sys.exit(main())
