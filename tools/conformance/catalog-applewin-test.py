#!/usr/bin/env python3
"""Cataloga o checkout do AppleWin-Test e gera tests/conformance/inputs/applewin-test-index.csv.

Só a biblioteca padrão. O checkout fica fora do Git (veja fetch-applewin-test.sh); o índice
guarda apenas caminho, tipo, tamanho, SHA-256 e os metadados extraídos dos .aws.yaml.

Uso: catalog-applewin-test.py [--cache DIR] [--out ARQUIVO]
"""
import argparse
import csv
import hashlib
import os
import re
import sys
from pathlib import Path

DEFAULT_CACHE = str(Path(__file__).resolve().parents[2] / "target/conformance-cache/applewin-test")
HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_OUT = os.path.join(HERE, "..", "..", "tests", "conformance", "inputs", "applewin-test-index.csv")
COLUMNS = ["caminho", "tipo", "tamanho", "sha256", "modelo", "cpu", "cartas_slots", "perifericos", "descricao"]

DISK_EXTS = {"dsk", "do", "po", "nib", "woz", "2mg", "hdv"}


def file_type(name):
    low = name.lower()
    if low.endswith(".aws.yaml"):
        return "aws.yaml"
    if low.endswith(".gz"):
        inner = low[:-3].rsplit(".", 1)[-1]
        return "imagem-disco.gz" if inner in DISK_EXTS else "gz"
    ext = low.rsplit(".", 1)[-1] if "." in low else ""
    if ext in DISK_EXTS:
        return ext
    return ext or "sem-extensao"


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def clean(v):
    v = v.split(" #")[0].strip()
    if len(v) >= 2 and v[0] == v[-1] and v[0] in "\"'":
        v = v[1:-1]
    return v


def parse_aws(path):
    """Extrai modelo, CPU, cartas por slot e periféricos de um .aws.yaml (leitura por indentação)."""
    model = cpu = ""
    slots = []
    periph = []
    unit = None
    in_cpu = False
    cur_slot = None
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            lines = f.read().splitlines()
    except OSError:
        return model, cpu, "", ""
    for ln in lines:
        if not ln.strip() or ln.lstrip().startswith("#"):
            continue
        indent = len(ln) - len(ln.lstrip(" "))
        s = ln.strip()
        if indent == 2 and s.startswith("Type:"):
            unit = clean(s[5:])
            in_cpu = False
            cur_slot = None
            continue
        if unit == "Apple2":
            if indent == 4 and s.startswith("Model:"):
                model = clean(s[6:])
            elif indent == 4:
                in_cpu = s.startswith("CPU:")
            elif indent == 6 and in_cpu and s.startswith("Type:") and not cpu:
                cpu = clean(s[5:])
        elif unit == "Auxiliary Slot":
            if indent == 4 and s.startswith("Card:"):
                periph.append("aux=" + clean(s[5:]))
        elif unit == "Slots":
            m = re.fullmatch(r"(\d+):", s)
            if indent == 4 and m:
                cur_slot = m.group(1)
            elif indent == 6 and cur_slot is not None and s.startswith("Card:"):
                slots.append("%s=%s" % (cur_slot, clean(s[5:])))
        elif unit == "Game I/O Connector":
            if indent == 4 and ":" in s:
                periph.append("gameio:" + s.split(":", 1)[0])
        elif unit == "Miscellaneous":
            if indent == 4 and s.endswith(":"):
                periph.append(s[:-1])
        elif unit is not None and indent == 2 and unit not in ("Apple2",):
            pass
    return model, cpu, ";".join(slots), ";".join(periph)


def load_descriptions(cache, names):
    """Associa descrições do 'test index.txt' aos arquivos por nome-base (sem distinguir caixa)."""
    result = {}
    p = os.path.join(cache, "test index.txt")
    if not os.path.isfile(p):
        return result
    by_lower = {}
    for rel in names:
        by_lower.setdefault(os.path.basename(rel).lower(), []).append(rel)
    known = sorted(by_lower, key=len, reverse=True)
    names_group, descs = [], []

    def flush():
        if names_group and descs:
            text = " | ".join(descs)
            for base in names_group:
                for rel in by_lower.get(base, []):
                    result[rel] = (result[rel] + " | " + text) if rel in result else text
        names_group.clear()
        descs.clear()

    with open(p, "r", encoding="utf-8", errors="replace") as f:
        for raw in f:
            line = raw.rstrip("\n").rstrip("\r")
            st = line.strip()
            if not st:
                flush()
                continue
            if st.startswith("."):
                descs.append(st.lstrip(". ").strip())
                continue
            if descs:  # nova entrada depois de descrições
                flush()
            low = st.lower()
            match = None
            for k in known:
                if low == k or low.startswith(k + " "):
                    match = k
                    break
            if match:
                names_group.append(match)
            # linhas de cabeçalho ou sem arquivo correspondente são ignoradas
    flush()
    return result


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cache", default=os.environ.get("APPLEWIN_TEST_CACHE", DEFAULT_CACHE))
    ap.add_argument("--out", default=DEFAULT_OUT)
    a = ap.parse_args()
    if not os.path.isdir(a.cache):
        print("BLOCKED: corpus ausente; rode tools/conformance/fetch-applewin-test.sh", file=sys.stderr)
        return 2
    rels = []
    for root, dirs, files in os.walk(a.cache):
        dirs[:] = [d for d in dirs if d != ".git"]
        for fn in files:
            full = os.path.join(root, fn)
            rels.append(os.path.relpath(full, a.cache).replace(os.sep, "/"))
    rels.sort()
    descs = load_descriptions(a.cache, rels)
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    counts = {}
    with open(a.out, "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(COLUMNS)
        for rel in rels:
            full = os.path.join(a.cache, rel)
            tipo = file_type(rel)
            counts[tipo] = counts.get(tipo, 0) + 1
            model = cpu = slots = periph = ""
            if tipo == "aws.yaml":
                model, cpu, slots, periph = parse_aws(full)
            w.writerow([rel, tipo, os.path.getsize(full), sha256(full), model, cpu, slots, periph, descs.get(rel, "")])
    print("%d arquivos catalogados em %s" % (len(rels), os.path.abspath(a.out)))
    for k in sorted(counts, key=lambda k: (-counts[k], k)):
        print("  %-16s %d" % (k, counts[k]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
