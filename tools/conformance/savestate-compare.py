#!/usr/bin/env python3
"""Compares Rust and oracle traces for every real AppleWin save-state (HU-0401).

For each .aws.yaml: the oracle (run-audetto-oracle.sh --state) and `conformance trace --state` run N
cycles from the state; the traces are compared row by row. Result per file:
  PASS     the traces are identical for N cycles
  FAIL     first divergence (cycle offset from the start is reported)
  BLOCKED  an image the state names is missing, the oracle could not load the state, or it crashed

usage: savestate-compare.py [--cycles N] [--out results.csv] [--jobs J] FILE_OR_DIR...
"""
import csv
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ORACLE = os.path.join(ROOT, "tools/conformance/oracle/run-audetto-oracle.sh")


def conformance_bin():
    common = subprocess.run(
        ["git", "-C", ROOT, "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True, text=True).stdout.strip()
    for cand in (os.path.join(ROOT, "target/release/conformance"),
                 os.path.join(os.path.dirname(common), "target/release/conformance")):
        if os.path.exists(cand):
            return cand
    sys.exit("build first: cargo build --release -p apple2-conformance")


def rows(path):
    out = []
    with open(path) as f:
        for line in f:
            if line.startswith("#") or line.startswith("cycle,"):
                continue
            out.append(line.rstrip("\n"))
    return out


def run_one(args):
    state, cycles, image_dirs, binary = args
    with tempfile.TemporaryDirectory() as tmp:
        r_out, o_out = os.path.join(tmp, "r.csv"), os.path.join(tmp, "o.csv")
        cmd = [binary, "trace", "--state", state, "--cycles", str(cycles), "-o", r_out]
        for d in image_dirs:
            cmd += ["--image-dir", d]
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            msg = (r.stderr or r.stdout).strip().splitlines()
            msg = [m for m in msg if m.startswith("erro")] or msg
            first = msg[0] if msg else "falha"
            return state, "BLOCKED" if "BLOCKED" in first else "FAIL", 0, "rust: " + first[:200]
        env = dict(os.environ)
        parents, d = [], os.path.dirname(os.path.abspath(state))
        for _ in range(3):
            d = os.path.dirname(d)
            parents.append(d)
        env["ORACLE_IMAGE_DIRS"] = ":".join(list(image_dirs) + parents)
        o = subprocess.run([ORACLE, "--state", state, "--cycles", str(cycles), "--video-update", "--out", o_out],
                           capture_output=True, text=True, timeout=600, env=env)
        if o.returncode != 0 or not os.path.exists(o_out):
            err = (o.stderr.strip().splitlines() or ["?"])[-1]
            return state, "BLOCKED", 0, "oráculo falhou: " + err[:200]
        if "MessageBox" in o.stderr:
            box = [l for l in o.stderr.splitlines() if "MessageBox" in l][0]
            return state, "BLOCKED", 0, "oráculo não carregou o estado: " + box[:200]
        er, rr = rows(o_out), rows(r_out)
        start = int(er[0].split(",")[0]) if er else 0
        for i in range(min(len(er), len(rr))):
            if er[i] != rr[i]:
                cyc = int(er[i].split(",")[0]) - start
                return state, "FAIL", cyc, f"linha {i}: oráculo {er[i]} / rust {rr[i]}"
        if len(er) != len(rr):
            return state, "FAIL", cycles, f"número de instruções: oráculo {len(er)}, rust {len(rr)}"
        import hashlib
        digest = hashlib.sha256(("\n".join(er) + "\n").encode()).hexdigest()
        return state, "PASS", cycles, f"{len(er)} instruções idênticas; rows_sha256={digest}"


def main():
    argv = sys.argv[1:]
    cycles, out, jobs, files, image_dirs = 100000, None, 4, [], []
    while argv:
        a = argv.pop(0)
        if a == "--cycles": cycles = int(argv.pop(0))
        elif a == "--out": out = argv.pop(0)
        elif a == "--jobs": jobs = int(argv.pop(0))
        elif a == "--image-dir": image_dirs.append(argv.pop(0))
        elif os.path.isdir(a):
            for dp, _, fs in os.walk(a):
                files += sorted(os.path.join(dp, f) for f in fs if f.endswith(".aws.yaml"))
        else:
            files.append(a)
    binary = conformance_bin()
    work = [(f, cycles, image_dirs, binary) for f in files]
    with ThreadPoolExecutor(jobs) as ex:
        results = list(ex.map(run_one, work))
    w = csv.writer(open(out, "w", newline="")) if out else csv.writer(sys.stdout)
    w.writerow(["arquivo", "resultado", "ciclos_ate_divergir", "detalhe"])
    for s, res, cyc, det in results:
        w.writerow([s, res, cyc, det])


if __name__ == "__main__":
    main()
