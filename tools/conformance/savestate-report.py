#!/usr/bin/env python3
"""Builds tests/conformance/reports/savestate-import.md and the savestate-* cases (HU-0401).

Input: the CSV files `savestate-compare.py` wrote for the real Wine states and for the
AppleWin-Test corpus. For every file it also runs `conformance check-import` (the imported machine
against its own YAML). A case TOML is written for each file whose trace matched the oracle.

usage: savestate-report.py --wine wine.csv --corpus corpus.csv [--cycles N]
"""
import csv
import collections
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CACHE = os.environ.get("APPLEWIN_TEST_CACHE", "/mnt/d/Chico/AppleRust/target/conformance-cache/applewin-test")
BIN = os.path.join(ROOT, "target/release/conformance")

MODELS = {
    "Apple][": "apple2", "Apple][+": "apple2plus", "Apple][ J-Plus": "apple2jplus",
    "Apple//e": "apple2e", "Enhanced Apple//e": "apple2e-enhanced", "Base 64A": "base64a",
    "Pravets82": "pravets82", "Pravets8M": "pravets8m", "Pravets8A": "pravets8a",
    "TK3000//e": "tk3000e",
}

# Reasons that turn a divergence (or a failed run) into BLOCKED: the comparison cannot be made.
BLOCKERS = [
    ("NoSlotClockTest", "dependência de tempo real: o No-Slot Clock devolve a hora do hospedeiro (fixa no Rust, real no oráculo)"),
    ("skm-", "o fork do oráculo não tem Shift Key Mod (unidade Game I/O v4); state-compat.py a remove, então o botão lido difere"),
    ("CPM2.2(56k)-3", "Z80 SoftCard ativo: o trace cobre só a CPU 6502 e o ciclo inicial do oráculo difere do arquivo"),
    ("SSC-", "dependência de TCP/serial do hospedeiro; o fork não emula o 6551 do SSC (audetto-ssc-rom.patch só restaura a ROM) e o oráculo lê o barramento flutuante em $C0A8-$C0AF"),
]
# Known causes of FAIL, from the first divergence (all earlier ones are fixed: Saturn, SmartPort,
# SNES MAX, mouse VBL and 6522/SSI263 now match the oracle).
CAUSES = []


def sh(*a):
    return subprocess.run(a, capture_output=True, text=True).stdout


def rel(path):
    if path.startswith(CACHE):
        return path[len(CACHE) + 1:]
    return os.path.relpath(path, ROOT)


def classify(name, res, detail):
    if res == "FAIL":
        for k, why in BLOCKERS:
            if k in name:
                return "BLOCKED", why
        for k, why in CAUSES:
            if k in name:
                return "FAIL", why + " | " + detail
        return "FAIL", detail
    return res, detail


def main():
    a = sys.argv[1:]
    wine = corpus = None
    cycles = 100000
    while a:
        x = a.pop(0)
        if x == "--wine": wine = a.pop(0)
        elif x == "--corpus": corpus = a.pop(0)
        elif x == "--cycles": cycles = int(a.pop(0))
    index = {}
    for r in csv.DictReader(open(os.path.join(ROOT, "tests/conformance/inputs/applewin-test-index.csv"))):
        if r["tipo"] == "aws.yaml":
            index[r["caminho"]] = r
    rows = []  # (group, relname, abs, result, cycles, detail, diffs)
    for group, f in (("Wine/AppleWin 1.32.1.0", wine), ("AppleWin-Test", corpus)):
        if not f:
            continue
        for r in csv.DictReader(open(f)):
            p = r["arquivo"]
            absp = p if os.path.isabs(p) else os.path.join(ROOT, p)
            res, det = classify(os.path.basename(p), r["resultado"], r["detalhe"])
            out = subprocess.run([BIN, "check-import", absp, "--image-dir", os.path.join(ROOT, "tests/conformance/inputs/applewin-real")],
                                 capture_output=True, text=True).stdout.splitlines()
            nd = sum(1 for l in out if l.startswith("DIFF"))
            first = next((l[5:] for l in out if l.startswith("DIFF")), "")
            fmt = "BLOCKED" if (out and out[0].startswith("BLOCKED")) else ("PASS" if nd == 0 else f"FAIL ({nd})")
            rows.append((group, rel(absp), absp, res, int(r["ciclos_ate_divergir"]), det, fmt, first))

    cnt = collections.Counter(r[3] for r in rows)
    md = ["# Importação de save-states reais do AppleWin (HU-0401)", "",
          "Gerado por `tools/conformance/savestate-report.py` a partir de `savestate-compare.py`. "
          "Oráculo: AppleWin (fork audetto no SHA fixo, ver `docs/adr/0004-oraculo-de-conformidade.md`) "
          f"carregando o mesmo YAML (`run-audetto-oracle.sh --state ... --cycles {cycles}`). "
          "Corpus AppleWin-Test: SHA `4b113ce8cff1426bda5595cac9cb04776adfbb5d` (não redistribuído).", "",
          "Duas verificações por arquivo:", "",
          "1. **Formato** (`conformance check-import`): logo após importar, registradores, flags, memória principal e AUX "
          "byte a byte, bancos LC/Saturn, chaves (leituras de `$C011-$C01F`) e o estado das cartas (reescrita da máquina "
          "no formato AppleWin comparada campo a campo com o YAML). Campos que o porte não modela ou normaliza de propósito "
          "ficam em `NOT_MODELLED` em `crates/apple2-conformance/src/import_check.rs`.",
          f"2. **Trace** após importar: `conformance trace --state` contra o oráculo, {cycles} ciclos, comparação linha a linha "
          "(ciclo, PC, opcode, A, X, Y, SP, P). PASS só quando o trace inteiro bate; FAIL na primeira divergência; "
          "BLOCKED quando falta imagem, há dependência de hardware ou de tempo real, ou o oráculo não consegue carregar.", "",
          "## Totais (trace)", "", "| resultado | arquivos |", "|---|---|"]
    for k in ("PASS", "FAIL", "BLOCKED"):
        md.append(f"| {k} | {cnt.get(k, 0)} |")
    md.append(f"| total | {len(rows)} |")
    md += ["", "## Por arquivo", "",
           "| arquivo | trace | bate até (ciclos) | formato | detalhe |", "|---|---|---|---|---|"]
    for g, name, absp, res, cyc, det, fmt, first in rows:
        det = det.replace("|", "\\|")[:260]
        md.append(f"| {name} | {res} | {cyc if res != 'BLOCKED' else '-'} | {fmt} | {det} |")
    md += ["", "## Causas raiz corrigidas durante a importação", "",
           "- `P` carregado sem o bit B (`CPU.cpp:963` faz `| AF_RESERVED | AF_BREAK`); PLP/RTI também limpavam B (`cpu_instructions.inl`).",
           "- `Cycles This Frame` acima do quadro travava o contador (AUD-C-060): normalizado no load e `>=` no `tick()`.",
           "- RAM não listada em dumps parciais ficava zerada: `MemReset` coloca o padrão de power-on antes do load.",
           "- Controlador de HDD de estados antigos: Hddrvr.bin (v1-3) e Hddrvr-v2.bin (v4) em vez do firmware SmartPort; `FixupUnitNum`.",
           "- Barramento flutuante lia a RAM principal; o AppleWin lê a imagem de leitura da CPU (aux com RAMRD), AUD-C-050.",
           "- Entrada de IRQ/NMI não gera linha de trace (o oráculo registra depois do fetch do opcode); opcode em `$C100-$CFFF` vem do fetch real.",
           "- Saturn64/Saturn128: o fork aloca os bancos com `new BYTE[16K]` sem zerar e o YAML só lista `0000: 00`; o oráculo lia lixo de heap (`$E000` = `$46`/`$32`). `run-audetto-oracle.sh` exporta `MALLOC_PERTURB_=255` (zera as alocações), como o porte e um heap novo do Windows.",
           "- SmartPort-firmware: o DIB devolvia `130.0` como versão do firmware; o `version.h` do oráculo é 1.32.0.0, logo `132.0` (`FW_VERSION` em `hdd.rs`).",
           "- SNES MAX-slot4: o fork do oráculo deixa o cartão sem tratador de E/S (o `SNESMAX.cpp` de `source/linux/duplicates` é um esqueleto); `audetto-snesmax.patch` o restaura sem joystick. O porte já seguia o AppleWin.",
           "- dazzledraw1 (3 arquivos): o primeiro evento de VBL do mouse vem 12480 ciclos (192 linhas, scanner em 0,0) depois do load, não do `Cycles This Frame`; e o evento recalculava a posição do quadro somando de novo os ciclos do opcode (7 ciclos antes).",
           "- 6522-one-shot e SSI263-test: não era o reset do 6522 (GH#1176). O programa varre os slots lendo `$Cn04`; o fork não instalava a ROM do SSC em `$C2xx` e lia 0 onde o porte lê `$0C`. `audetto-ssc-rom.patch` instala a ROM.",
           "- Ferramental: o oráculo precisava de o `CpuExecute(slice, true)` do oráculo (sem ele o scanner de vídeo ficava parado e o barramento flutuante era constante), "
           "da cwd com as imagens e de `state-compat.py` (remove a unidade Game I/O v4 que o fork não conhece).", ""]
    out = os.path.join(ROOT, "tests/conformance/reports/savestate-import.md")
    open(out, "w").write("\n".join(md) + "\n")

    # Cases for the files that matched the oracle.
    cdir = os.path.join(ROOT, "tests/conformance/cases")
    for f in os.listdir(cdir):
        if f.startswith(("savestate-wine-", "savestate-corpus-")):
            os.remove(os.path.join(cdir, f))
    n = 0
    for g, name, absp, res, cyc, det, fmt, first in rows:
        if res != "PASS":
            continue
        m = re.search(r"rows_sha256=([0-9a-f]{64})", det)
        if not m:
            continue
        sha = sh("sha256sum", absp).split()[0]
        wine = g.startswith("Wine")
        idx = index.get(name)
        model = MODELS.get(idx["modelo"], None) if idx else ("apple2p" if "apple2p" in name else "apple2e-enhanced")
        if wine:
            model = "apple2plus" if "apple2p" in name else "apple2e-enhanced"
        if not model:
            continue
        slug = re.sub(r"[^A-Za-z0-9._-]+", "-", os.path.splitext(os.path.splitext(os.path.basename(name))[0])[0]).strip("-")
        cid = ("savestate-wine-" if wine else "savestate-corpus-") + slug
        fpath = name if wine else "target/conformance-cache/applewin-test/" + name
        dirs = '["tests/conformance/inputs/applewin-real"]' if wine else "[]"
        toml = f'''id = "{cid}"
story = "HU-0401"
criterion = "Importar o save-state real {os.path.basename(name)} e rodar {cycles} ciclos gera o mesmo trace do AppleWin (Trace v1)"
model = "{model}"
notes = "Oráculo: AppleWin (fork audetto) carregando o mesmo YAML. Reproduzir: tools/conformance/oracle/run-audetto-oracle.sh --state <arquivo> --cycles {cycles}. Hash das linhas do trace do oráculo."

[fixture]
path = "{fpath}"
sha256 = "{sha}"
save_state = true
image_dirs = {dirs}

[run]
cycles = {cycles}

[oracle]
kind = "trace"
authority = "applewin"
source = "AppleWin (fork audetto a1e20c0) --state {os.path.basename(name)} --cycles {cycles}; ver tools/conformance/savestate-compare.py"
trace_rows_sha256 = "{m.group(1)}"
'''
        open(os.path.join(cdir, cid + ".toml"), "w").write(toml)
        n += 1
    print(f"relatório: {out}; {n} casos; {dict(cnt)}")


if __name__ == "__main__":
    main()
