#!/usr/bin/env python3
"""Aplica os resultados por issue (docs/conformance/results/*.csv) à matriz
docs/conformance/applewin-issue-matrix.csv e regenera o issue-audit.md.
Só biblioteca padrão.

Cabeçalho dos resultados: upstream_id,estado_verificacao,teste,evidencia,tipo_oraculo
Atualiza na matriz: estado_verificacao, teste, evidencia, tipo_oraculo e revisado_por
(`resultado:<arquivo-sem-.csv>`; vários arquivos ficam separados por `;`).

tipo_oraculo (obrigatório nas linhas PASS, vazio nas demais) diz em que o PASS se apoia:
  executavel      comparação com o AppleWin rodando (fork do audetto ou Wine)
  cpp-transcrito  teste Rust que reproduz o C++ do a8f0d1d citando arquivo:linha
  especificacao   hardware, a2audit ou vetores públicos
  fixture-real    fixture real (arquivo de terceiros) sem comparação com o AppleWin
  regressao       teste do próprio port, sem fonte externa citada (inclui o comportamento
                  de uma Divergência aprovada, ADR 0006)
Numa issue com vários arquivos, vale o tipo mais forte entre as linhas PASS (ordem acima).
Se a matriz ainda não tem a coluna, ela é acrescentada no fim, sem mexer nas outras.

Uma issue em mais de um arquivo: estado por precedência FAIL > PASS > BLOCKED >
NOT_APPLICABLE; teste e evidência são juntados com " | " na ordem dos arquivos.

Uso:
  python3 tools/conformance/apply_results.py           # grava e regenera o sumário
  python3 tools/conformance/apply_results.py --check   # não grava; sai com 1 se desatualizada
Saídas: 0 ok, 1 matriz desatualizada (só com --check), 2 erro nos dados.
"""
import argparse
import csv
import io
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MATRIX = ROOT / "docs" / "conformance" / "applewin-issue-matrix.csv"
RESULTS_DIR = ROOT / "docs" / "conformance" / "results"
SUMMARIZER = ROOT / "tools" / "conformance" / "summarize_issue_matrix.py"

HEADER_RESULTS = ["upstream_id", "estado_verificacao", "teste", "evidencia", "tipo_oraculo"]
TIPOS = ["executavel", "cpp-transcrito", "especificacao", "fixture-real", "regressao"]
TIPO_FORCA = ["executavel", "especificacao", "fixture-real", "cpp-transcrito", "regressao"]  # do mais forte ao mais fraco
PRECEDENCE = ["FAIL", "PASS", "BLOCKED", "NOT_APPLICABLE"]  # do mais forte ao mais fraco
VALID = set(PRECEDENCE)
UNVERIFIED = "UNVERIFIED"  # estado vazio na matriz, mostrado como UNVERIFIED


def read_matrix_text():
    return MATRIX.read_text(encoding="utf-8")


def parse_matrix(text):
    rows = list(csv.reader(io.StringIO(text, newline="")))
    return rows[0], rows[1:]


def serialize(rows):
    buf = io.StringIO(newline="")
    csv.writer(buf, lineterminator="\n").writerows(rows)
    return buf.getvalue()


def load_results():
    """Devolve {upstream_id: [(arquivo, estado, teste, evidencia, tipo), ...]} e os erros."""
    found = defaultdict(list)
    errors = []
    files = sorted(RESULTS_DIR.glob("*.csv"))
    if not files:
        errors.append(f"nenhum arquivo de resultados em {RESULTS_DIR}")
    for path in files:
        name = path.stem
        with path.open(encoding="utf-8", newline="") as fh:
            reader = csv.reader(fh)
            header = next(reader, None)
            if header != HEADER_RESULTS:
                errors.append(f"{path.name}: cabeçalho {header} diferente de {HEADER_RESULTS}")
                continue
            for lineno, row in enumerate(reader, start=2):
                if not any(c.strip() for c in row):
                    continue
                if len(row) != len(HEADER_RESULTS):
                    errors.append(f"{path.name}:{lineno}: esperadas {len(HEADER_RESULTS)} colunas, vieram {len(row)}")
                    continue
                uid, estado, teste, evidencia, tipo = (c.strip() if i in (0, 1, 4) else c for i, c in enumerate(row))
                if estado not in VALID:
                    errors.append(f"{path.name}:{lineno}: estado inválido {estado!r} (id {uid})")
                    continue
                if estado == "PASS" and tipo not in TIPOS:
                    errors.append(f"{path.name}:{lineno}: PASS sem tipo_oraculo válido ({tipo!r}; id {uid}); "
                                  f"use um de {', '.join(TIPOS)}")
                    continue
                if estado != "PASS" and tipo:
                    errors.append(f"{path.name}:{lineno}: tipo_oraculo {tipo!r} só vale em linhas PASS (id {uid})")
                    continue
                found[uid].append((name, estado, teste, evidencia, tipo))
    return found, errors


def pick_state(entries):
    states = {e[1] for e in entries}
    for s in PRECEDENCE:
        if s in states:
            return s
    raise AssertionError("unreachable")


def apply(header, rows, results):
    """Devolve (novas_linhas, erros, conflitos, ids_aplicados)."""
    idx = {name: i for i, name in enumerate(header)}
    for col in ("upstream_id", "estado_verificacao", "teste", "evidencia", "revisado_por"):
        if col not in idx:
            raise SystemExit(f"matriz sem a coluna {col!r}")
    if "tipo_oraculo" not in idx:  # coluna nova, no fim: as outras não mudam de posição
        header.append("tipo_oraculo")
        idx["tipo_oraculo"] = len(header) - 1
        rows = [r + [""] for r in rows]
    by_id = {}
    for i, r in enumerate(rows):
        by_id.setdefault(r[idx["upstream_id"]], i)

    unknown = sorted((u for u in results if u not in by_id), key=lambda u: (len(u), u))
    if unknown:
        return None, None, [f"upstream_id inexistente na matriz: {u} (arquivos: "
                      f"{', '.join(sorted({e[0] for e in results[u]}))})" for u in unknown], [], []

    conflicts, applied = [], []
    new_rows = [list(r) for r in rows]
    for uid, entries in results.items():
        entries = sorted(entries, key=lambda e: e[0])
        files = [e[0] for e in entries]
        estado = pick_state(entries)
        if len(entries) > 1:
            distintos = sorted({e[1] for e in entries})
            if len(distintos) > 1:
                conflicts.append(f"{uid}: " + "; ".join(f"{e[0]}={e[1]}" for e in entries)
                                 + f" -> {estado}")
            else:
                conflicts.append(f"{uid}: em {', '.join(files)} com o mesmo estado ({estado})")
        teste = " | ".join(e[2] for e in entries if e[2])
        evidencia = " | ".join(e[3] for e in entries if e[3])
        row = new_rows[by_id[uid]]
        row[idx["estado_verificacao"]] = estado
        row[idx["teste"]] = teste
        row[idx["evidencia"]] = evidencia
        row[idx["revisado_por"]] = ";".join(f"resultado:{f}" for f in files)
        tipos = {e[4] for e in entries if e[1] == "PASS" and e[4]} if estado == "PASS" else set()
        row[idx["tipo_oraculo"]] = next((t for t in TIPO_FORCA if t in tipos), "")
        applied.append(uid)
    return header, new_rows, [], conflicts, applied


def state_counts(rows, idx):
    return Counter((r[idx["estado_verificacao"]] or UNVERIFIED) for r in rows)


def fmt_counts(counter):
    order = PRECEDENCE + [UNVERIFIED]
    keys = [k for k in order if k in counter] + sorted(k for k in counter if k not in order)
    return ", ".join(f"{k}={counter[k]}" for k in keys) or "(vazio)"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="não grava; sai com 1 se a matriz estiver desatualizada")
    args = ap.parse_args()

    text = read_matrix_text()
    header, rows = parse_matrix(text)
    idx = {name: i for i, name in enumerate(header)}
    results, errors = load_results()
    if errors:
        print("ERRO nos resultados (nada foi gravado):", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 2

    per_file = defaultdict(Counter)
    for uid, entries in results.items():
        for name, estado, _, _, _ in entries:
            per_file[name][estado] += 1

    before = state_counts(rows, idx)
    header, new_rows, errors, conflicts, applied = apply(header, rows, results)
    if errors:
        print("ERRO: ids inexistentes na matriz (nada foi gravado):", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 2

    idx = {name: i for i, name in enumerate(header)}
    sem_tipo = [r[idx["upstream_id"]] for r in new_rows
                if r[idx["estado_verificacao"]] == "PASS" and not r[idx["tipo_oraculo"]]]
    if sem_tipo:
        print(f"ERRO: {len(sem_tipo)} PASS sem tipo_oraculo na matriz (ids: {', '.join(sem_tipo[:10])}...)",
              file=sys.stderr)
        return 2
    new_text = serialize([header] + new_rows)
    after = state_counts(new_rows, idx)
    changed = new_text != text

    print(f"Estados antes:  {fmt_counts(before)}")
    print(f"Estados depois: {fmt_counts(after)}")
    print("PASS por tipo de oráculo: " + ", ".join(
        f"{t}={c}" for t, c in sorted(Counter(r[idx['tipo_oraculo']] for r in new_rows
                                              if r[idx['estado_verificacao']] == 'PASS').items(),
                                      key=lambda kv: TIPOS.index(kv[0]))))
    print("Por arquivo de resultados (linhas lidas por estado):")
    for name in sorted(per_file):
        print(f"  {name}: {fmt_counts(per_file[name])} (total {sum(per_file[name].values())})")
    print(f"Issues aplicadas: {len(applied)}; linhas da matriz alteradas: "
          f"{sum(1 for a, b in zip(rows, new_rows) if a != b)}")
    if conflicts:
        print("Issues em mais de um arquivo de resultados:")
        for c in sorted(conflicts, key=lambda c: int(c.split(":")[0])):
            print(f"  - {c}")

    if args.check:
        if changed:
            print(f"{MATRIX.relative_to(ROOT)} está desatualizada; rode sem --check.")
            return 1
        print("Matriz em dia.")
        return 0

    if changed:
        MATRIX.write_text(new_text, encoding="utf-8", newline="")
        print(f"Gravado: {MATRIX.relative_to(ROOT)}")
    else:
        print("Nenhuma alteração na matriz.")
    subprocess.run([sys.executable, str(SUMMARIZER)], check=True, cwd=ROOT)
    return 0


if __name__ == "__main__":
    sys.exit(main())
