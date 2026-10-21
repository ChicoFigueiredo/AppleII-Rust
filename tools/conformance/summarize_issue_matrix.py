#!/usr/bin/env python3
"""Lê docs/conformance/applewin-issue-matrix.csv e gera docs/conformance/issue-audit.md
e docs/conformance/upstream-open-issues-triage.md. Só biblioteca padrão.

Taxa = PASS / total aplicável (aplicável = estado_verificacao != NOT_APPLICABLE).
Issues sem aplicabilidade ficam em "(nao classificada)" e contam como aplicáveis.
"""
import csv
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CSV_PATH = ROOT / "docs" / "conformance" / "applewin-issue-matrix.csv"
OUT_AUDIT = ROOT / "docs" / "conformance" / "issue-audit.md"
OUT_TRIAGE = ROOT / "docs" / "conformance" / "upstream-open-issues-triage.md"
JSON_PATH = ROOT / "target" / "conformance-cache" / "upstream" / "issues.json"
NC = "(nao classificada)"
STATES = ["PASS", "FAIL", "IMPLEMENTED", "UNVERIFIED", "BLOCKED", "NOT_APPLICABLE"]


def table(title, counter, total):
    rows = [f"### {title}", "", "| Valor | Quantidade | % |", "|---|---:|---:|"]
    for k, v in sorted(counter.items(), key=lambda kv: (-kv[1], kv[0])):
        rows.append(f"| {k} | {v} | {100 * v / total:.1f} |")
    return rows + [""]


def rates(title, dimension, rows_by_key):
    out = [f"### {title}", "",
           "| Valor | Total | Aplicável | PASS | FAIL | UNVERIFIED | Taxa PASS/aplicável |",
           "|---|---:|---:|---:|---:|---:|---:|"]
    for k, rs in sorted(rows_by_key.items(), key=lambda kv: (-len(kv[1]), kv[0])):
        st = Counter(r["estado_verificacao"] or "UNVERIFIED" for r in rs)
        aplic = len(rs) - st["NOT_APPLICABLE"]
        taxa = f"{100 * st['PASS'] / aplic:.1f}%" if aplic else "n/a"
        out.append(f"| {k} | {len(rs)} | {aplic} | {st['PASS']} | {st['FAIL']} | {st['UNVERIFIED']} | {taxa} |")
    return out + [""]


def main():
    with CSV_PATH.open(newline="", encoding="utf-8") as fh:
        rows = list(csv.DictReader(fh))
    total = len(rows)
    meta = {}
    if JSON_PATH.exists():
        meta = json.loads(JSON_PATH.read_text())["metadados"]

    by_model = {}
    for r in rows:
        models = [m for m in r["modelo"].split(";") if m] or [NC]
        for m in models:
            by_model.setdefault(m, []).append(r)
    by_cat, by_app = {}, {}
    for r in rows:
        by_cat.setdefault(r["categoria"] or NC, []).append(r)
        by_app.setdefault(r["aplicabilidade"] or NC, []).append(r)

    L = ["# Auditoria de issues upstream do AppleWin", "",
         "Gerado por `tools/conformance/summarize_issue_matrix.py` a partir de "
         "`docs/conformance/applewin-issue-matrix.csv`. Não editar à mão.", ""]
    if meta:
        c = meta["contagens"]
        L += ["## Coleta", "",
              f"- Data da coleta (UTC): {meta['coletado_em_utc']}",
              f"- SHA do upstream (HEAD de {meta['repositorio']}): `{meta['upstream_head_sha']}`",
              f"- Itens coletados: {c['itens_total']} ({c['issues_total']} issues e {c['prs_total']} PRs)",
              f"- Issues: {c['issues_abertas']} abertas e {c['issues_fechadas']} fechadas",
              f"- PRs: {c['prs_abertos']} abertos, {c['prs_mesclados']} mesclados, "
              f"{c['prs_fechados_sem_merge']} fechados sem merge",
              f"- Issues fechadas com commit de correção identificado: "
              f"{c['issues_fechadas_com_commit']} ({c['issues_fechadas_com_commit_closed']} por evento `closed`, "
              f"{c['issues_fechadas_com_commit_referenced']} por `referenced`)",
              f"- Issues com PR vinculado (heurístico): {c['issues_com_pr_vinculado']}", ""]
    else:
        L += ["## Coleta", "", "Metadados indisponíveis (`issues.json` ausente); rode "
              "`tools/conformance/fetch_upstream_issues.py`.", ""]

    L += [f"## Contagens (total de issues na matriz: {total})", ""]
    L += table("Estado upstream", Counter(r["estado_upstream"] for r in rows), total)
    L += table("Aplicabilidade", Counter(r["aplicabilidade"] or NC for r in rows), total)
    L += table("Categoria", Counter(r["categoria"] or NC for r in rows), total)
    mc = Counter()
    for m, rs in by_model.items():
        mc[m] = len(rs)
    L += ["Observação: uma issue com vários modelos conta em cada um, então a soma pode exceder o total.", ""]
    L += table("Modelo", mc, total)
    L += table("Estado de verificação", Counter(r["estado_verificacao"] or "UNVERIFIED" for r in rows), total)

    st = Counter(r["estado_verificacao"] or "UNVERIFIED" for r in rows)
    aplic = total - st["NOT_APPLICABLE"]
    L += ["## Taxas", "",
          f"Taxa geral: PASS {st['PASS']} / aplicáveis {aplic} = "
          f"{(100 * st['PASS'] / aplic if aplic else 0):.1f}%. "
          "Aplicável = estado diferente de NOT_APPLICABLE. Issues ainda não classificadas contam como aplicáveis.", ""]
    L += rates("Por modelo", "modelo", by_model)
    L += rates("Por categoria", "categoria", by_cat)
    L += rates("Por aplicabilidade", "aplicabilidade", by_app)
    OUT_AUDIT.write_text("\n".join(L), encoding="utf-8")

    # Lista de issues abertas para o §15/EP-11 (tudo vem da matriz, sem inventar dados)
    openr = [r for r in rows if r["estado_upstream"] == "open"]
    T = ["# Issues abertas do AppleWin para triagem (HU-1101 / EP-11)", "",
         "Gerado por `tools/conformance/summarize_issue_matrix.py`. Títulos, datas e labels vêm da coleta"
         + (f" de {meta['coletado_em_utc']} UTC (upstream `{meta['upstream_head_sha'][:12]}`)." if meta else "."),
         f"Total de issues abertas: {len(openr)}. As colunas valor/esforço/risco/selecionada ficam vazias até a triagem.", "",
         "| Nº | Título | Criada | Labels | Link | Valor | Esforço | Risco | Selecionada |",
         "|---:|---|---|---|---|---|---|---|---|"]
    for r in sorted(openr, key=lambda r: int(r["upstream_id"])):
        n = r["upstream_id"]
        titulo = r["titulo"].replace("|", "\\|").replace("\n", " ")
        T.append(f"| {n} | {titulo} | {r['criada']} | {r['labels'].replace(';', ', ')} | "
                 f"[#{n}](https://github.com/AppleWin/AppleWin/issues/{n}) | | | | |")
    OUT_TRIAGE.write_text("\n".join(T) + "\n", encoding="utf-8")
    print(f"{OUT_AUDIT}\n{OUT_TRIAGE}\nissues={total} abertas={len(openr)}")


if __name__ == "__main__":
    main()
