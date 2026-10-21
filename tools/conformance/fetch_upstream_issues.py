#!/usr/bin/env python3
"""HU-0001: coleta todas as issues e PRs de AppleWin/AppleWin via `gh api` (somente leitura).

Saída: target/conformance-cache/upstream/issues.json
Cache de eventos por issue: target/conformance-cache/upstream/events/N.json
Gera também os lotes (batches/batch-NN.json) e a matriz CSV inicial.
Só biblioteca padrão.
"""
import argparse
import csv
import datetime as dt
import json
import re
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

REPO = "AppleWin/AppleWin"
ROOT = Path(__file__).resolve().parents[2]
CACHE = ROOT / "target" / "conformance-cache" / "upstream"
BODY_MAX = 4000
THREADS = 8
NUM_BATCHES = 10

CSV_COLUMNS = [
    "upstream_id", "tipo", "estado_upstream", "labels", "criada", "fechada", "titulo",
    "commit_correcao", "prs_vinculados", "aplicabilidade", "categoria", "modelo",
    "mapeamento_rust", "estado_verificacao", "teste", "evidencia", "justificativa_na",
    "prioridade", "revisado_por",
]

_rl_lock = threading.Lock()


def gh(args, retries=4):
    last = ""
    for attempt in range(retries):
        p = subprocess.run(["gh", "api", *args], capture_output=True, text=True)
        if p.returncode == 0:
            return p.stdout
        last = p.stderr.strip()
        if "rate limit" in last.lower() or "403" in last:
            wait_for_rate_limit(force=True)
        else:
            time.sleep(2 ** attempt)
    raise RuntimeError(f"gh api {' '.join(args)} falhou: {last}")


def wait_for_rate_limit(force=False, minimum=50):
    """Consulta `gh api rate_limit`; dorme até o reset se o saldo estiver baixo."""
    with _rl_lock:
        out = subprocess.run(["gh", "api", "rate_limit"], capture_output=True, text=True).stdout
        rl = json.loads(out)["resources"]["core"]
        if rl["remaining"] < minimum:
            wait = max(0, rl["reset"] - int(time.time())) + 2
            print(f"rate limit baixo ({rl['remaining']}); aguardando {wait}s", file=sys.stderr)
            time.sleep(wait)
        elif force:
            time.sleep(5)
        return rl


def fetch_all_items():
    out = gh(["--paginate", "--slurp", f"repos/{REPO}/issues?state=all&per_page=100"])
    return [it for page in json.loads(out) for it in page]


def normalize(raw):
    is_pr = "pull_request" in raw
    pr = raw.get("pull_request") or {}
    ms = raw.get("milestone")
    body = raw.get("body") or ""
    return {
        "numero": raw["number"],
        "tipo": "PR" if is_pr else "issue",
        "estado": raw["state"],
        "estado_motivo": raw.get("state_reason"),
        "labels": sorted(l["name"] for l in raw.get("labels", [])),
        "created_at": raw["created_at"],
        "closed_at": raw.get("closed_at"),
        "titulo": raw["title"],
        "corpo": body[:BODY_MAX],
        "corpo_truncado": len(body) > BODY_MAX,
        "milestone": ms["title"] if ms else None,
        "autor": (raw.get("user") or {}).get("login"),
        "pr_mesclado": bool(pr.get("merged_at")) if is_pr else None,
        "url": raw["html_url"],
        "comentarios": raw.get("comments", 0),
    }


def fetch_events(n):
    f = CACHE / "events" / f"{n}.json"
    if f.exists():
        return json.loads(f.read_text())
    pages = json.loads(gh(["--paginate", "--slurp", f"repos/{REPO}/issues/{n}/events?per_page=100"]))
    slim = [
        {"event": e["event"], "commit_id": e.get("commit_id"), "created_at": e.get("created_at")}
        for page in pages for e in page
        if e["event"] in ("closed", "referenced", "merged")
    ]
    f.write_text(json.dumps(slim))
    return slim


def commit_from_events(evs):
    closed = [e for e in evs if e["event"] == "closed" and e.get("commit_id")]
    if closed:
        return closed[-1]["commit_id"], "closed"
    refs = [e for e in evs if e["event"] == "referenced" and e.get("commit_id")]
    if refs:
        return refs[-1]["commit_id"], "referenced"
    return None, None


CLOSING = re.compile(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\b[:\s]+(?:AppleWin/AppleWin)?#(\d+)", re.I)
ANYREF = re.compile(r"(?<![\w/])#(\d+)\b")


def link_prs(items):
    """issue -> PRs que a citam (palavra de fechamento no corpo, ou #N no título)."""
    links = {}
    for it in items:
        if it["tipo"] != "PR":
            continue
        refs = {int(x) for x in CLOSING.findall(it["corpo"])}
        if not it["titulo"].startswith("Merge "):  # "Merge pull request #1 ..." cita outro PR
            refs |= {int(x) for x in ANYREF.findall(it["titulo"])}
        for r in refs:
            if r != it["numero"]:
                links.setdefault(r, set()).add(it["numero"])
    return {k: sorted(v) for k, v in links.items()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--no-events", action="store_true")
    ap.add_argument("--limit-events", type=int, default=0, help="só para testes")
    args = ap.parse_args()

    (CACHE / "events").mkdir(parents=True, exist_ok=True)
    (CACHE / "batches").mkdir(parents=True, exist_ok=True)

    rl = wait_for_rate_limit()
    print(f"rate limit restante: {rl['remaining']}", file=sys.stderr)
    collected_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    items = sorted((normalize(r) for r in fetch_all_items()), key=lambda x: x["numero"])
    print(f"{len(items)} itens (issues+PRs)", file=sys.stderr)

    prlinks = link_prs(items)
    closed_issues = [i for i in items if i["tipo"] == "issue" and i["estado"] == "closed"]
    todo = closed_issues[: args.limit_events] if args.limit_events else closed_issues
    events = {}
    if not args.no_events:
        counter = [0]

        def work(it):
            counter[0] += 1
            if counter[0] % 50 == 0:
                wait_for_rate_limit()
            return it["numero"], fetch_events(it["numero"])

        with ThreadPoolExecutor(max_workers=THREADS) as ex:
            events = dict(ex.map(work, todo))

    for it in items:
        it["commit_correcao"] = None
        it["commit_origem"] = None
        it["prs_vinculados"] = prlinks.get(it["numero"], []) if it["tipo"] == "issue" else []
        if it["numero"] in events:
            it["commit_correcao"], it["commit_origem"] = commit_from_events(events[it["numero"]])

    issues = [i for i in items if i["tipo"] == "issue"]
    prs = [i for i in items if i["tipo"] == "PR"]
    head = gh([f"repos/{REPO}/commits/HEAD", "--jq", ".sha"]).strip()
    counts = {
        "itens_total": len(items),
        "issues_total": len(issues),
        "issues_abertas": sum(i["estado"] == "open" for i in issues),
        "issues_fechadas": sum(i["estado"] == "closed" for i in issues),
        "prs_total": len(prs),
        "prs_abertos": sum(i["estado"] == "open" for i in prs),
        "prs_mesclados": sum(bool(i["pr_mesclado"]) for i in prs),
        "prs_fechados_sem_merge": sum(i["estado"] == "closed" and not i["pr_mesclado"] for i in prs),
        "issues_fechadas_com_commit": sum(1 for i in issues if i["estado"] == "closed" and i["commit_correcao"]),
        "issues_fechadas_com_commit_closed": sum(1 for i in issues if i["commit_origem"] == "closed"),
        "issues_fechadas_com_commit_referenced": sum(1 for i in issues if i["commit_origem"] == "referenced"),
        "issues_com_pr_vinculado": sum(1 for i in issues if i["prs_vinculados"]),
    }
    data = {
        "metadados": {
            "repositorio": REPO,
            "coletado_em_utc": collected_at,
            "upstream_head_sha": head,
            "eventos_coletados": not args.no_events,
            "contagens": counts,
        },
        "itens": items,
    }
    (CACHE / "issues.json").write_text(json.dumps(data, ensure_ascii=False, indent=1))
    print(json.dumps(counts, indent=1))

    # lotes de issues (corpo completo), contíguos por número, tamanho parecido
    n = len(issues)
    for b in range(NUM_BATCHES):
        chunk = issues[b * n // NUM_BATCHES:(b + 1) * n // NUM_BATCHES]
        payload = {"lote": b + 1, "total_lotes": NUM_BATCHES, "coletado_em_utc": collected_at,
                   "upstream_head_sha": head, "issues": chunk}
        (CACHE / "batches" / f"batch-{b + 1:02d}.json").write_text(
            json.dumps(payload, ensure_ascii=False, indent=1))

    # matriz CSV inicial (nunca sobrescreve uma matriz já classificada)
    csv_path = ROOT / "docs" / "conformance" / "applewin-issue-matrix.csv"
    csv_path.parent.mkdir(parents=True, exist_ok=True)
    if csv_path.exists():
        print(f"{csv_path} já existe; não sobrescrevo (apague para regenerar).", file=sys.stderr)
        return
    with csv_path.open("w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=CSV_COLUMNS, lineterminator="\n")
        w.writeheader()
        for i in issues:
            w.writerow({
                "upstream_id": i["numero"], "tipo": i["tipo"], "estado_upstream": i["estado"],
                "labels": ";".join(i["labels"]), "criada": i["created_at"][:10],
                "fechada": (i["closed_at"] or "")[:10], "titulo": i["titulo"],
                "commit_correcao": i["commit_correcao"] or "",
                "prs_vinculados": ";".join(str(x) for x in i["prs_vinculados"]),
                "estado_verificacao": "UNVERIFIED",
            })


if __name__ == "__main__":
    main()
