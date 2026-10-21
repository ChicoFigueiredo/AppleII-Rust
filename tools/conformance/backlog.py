#!/usr/bin/env python3
"""Cria e atualiza no GitHub o backlog de conformidade do AppleII-Rust.

A fonte da verdade é o plano em docs/prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md:
títulos, prioridades, critérios de aceite (CA) e links do AppleWin são lidos dele.

Uso:
    python3 tools/conformance/backlog.py --dry-run   # só imprime o que faria (sem escrita)
    python3 tools/conformance/backlog.py             # aplica no GitHub e grava docs/conformance/

Idempotência: as issues são localizadas pelo prefixo do título ("[EP-01]", "[HU-0102]");
labels são criadas com --force; milestones são verificadas antes de criadas; corpos e
metadados só são reescritos quando diferem do desejado. Rodar duas vezes não duplica nada.

Somente biblioteca padrão. Chama o `gh` (já autenticado) via subprocess.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

REPO = "ChicoFigueiredo/AppleII-Rust"
RAIZ = Path(__file__).resolve().parents[2]
PLANO_REL = "docs/prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md"
SAIDA_MD = RAIZ / "docs/conformance/backlog.md"
SAIDA_JSON = RAIZ / "docs/conformance/backlog.json"

# §19 do plano: ondas de execução por épico.
ONDA_DO_EPICO = {
    "EP-00": 0,
    "EP-01": 1, "EP-03": 1, "EP-04": 1,
    "EP-02": 2, "EP-05": 2, "EP-06": 2, "EP-07": 2,
    "EP-08": 3, "EP-09": 3, "EP-10": 3,
    "EP-11": 4, "EP-12": 4,
}

# Prioridades que não podem ser herdadas do épico (o épico traz "P0/P1" ou similar).
PRIORIDADE_FORCADA = {"HU-0902": "P1"}

GATES_ORDEM = ["G0", "G1", "G2", "G3"]

# Histórias sem frase "Como ..." no plano: texto complementar coerente com a HU.
COMO_COMPLEMENTAR = {
    "HU-0103": "Como desenvolvedor de software retro, quero que leituras de soft switches e o floating bus respeitem o ciclo e o raster da máquina, para que software que depende de temporização exata funcione como no hardware.",
    "HU-0202": "Como usuário de VidHD e de software de 80 colunas, quero que os modos de vídeo exigidos pelo AppleWin estejam suportados ou documentados como não suportados, para saber o que esperar e manter capturas e switches após save/load.",
    "HU-0301": "Como jogador e colecionador, quero montar, usar, salvar e ejetar discos sem perda ou corrupção, para não perder progresso nem imagens de disco.",
    "HU-0302": "Como usuário, quero trocar, ejetar e reconfigurar imagens de disco com efeito previsível, para não ter estado fantasma nem perder RAM por reset inesperado.",
    "HU-0303": "Como usuário de discos rígidos e SmartPort, quero boot, leitura e gravação confiáveis em imagens grandes, para manter meus dados íntegros após desligar e reiniciar.",
    "HU-0402": "Como usuário, quero salvar e carregar estado sem risco de corrupção, para não perder minha sessão mesmo que o computador trave durante a gravação.",
    "HU-0502": "Como usuário de síntese de fala, quero que o SSI263 e o SAM funcionem após power-down e após save/load, para ouvir a fala correta e sem trechos cortados.",
    "HU-0602": "Como usuário de cartões de expansão, quero que cada cartão listado esteja disponível, seja configurável por slot e esteja efetivamente conectado, para que a configuração escolhida tenha efeito real.",
    "HU-0701": "Como usuário de Uthernet, quero comunicação de rede confiável e com erros explícitos, para usar aplicações de rede sem travamentos.",
    "HU-0702": "Como usuário de jogos e aplicativos, quero que teclado, mouse e joysticks funcionem de forma consistente em cada sistema operacional, para não ter entradas presas ou fora de sincronia.",
    "HU-0703": "Como usuário de impressora e porta serial, quero que os dispositivos sejam detectados e configurados com mensagens claras, para usar periféricos sem crash quando algo não estiver disponível.",
    "HU-0802": "Como usuário migrando do AppleWin, quero que as ações de UI listadas existam ou estejam documentadas como diferenças, para encontrar as mesmas operações sem surpresas.",
    "HU-0902": "Como usuário final, quero baixar um pacote versionado, verificável e com licenças documentadas, para instalar com segurança e confiança na origem.",
    "HU-1001": "Como usuário e mantenedor, quero que o emulador fique mais rápido sem perder precisão, para jogar com desempenho adequado sem alterar temporização nem áudio.",
    "HU-1202": "Como programador que usa VS Code, quero depurar o emulador por DAP com as mesmas operações do debugger nativo, para trabalhar no meu editor preferido.",
    "HU-1203": "Como estudante de Assembly 6502, quero ver cada instrução com bytes, ciclos e efeitos nos registradores, para aprender com exemplos reproduzíveis.",
    "HU-1204": "Como mantenedor do projeto, quero uma decisão documentada sobre suporte a Apple I, para que o escopo seja explícito e não implícito.",
    "HU-1205": "Como usuário sem instalação local, quero executar o emulador em um navegador, para experimentar o Apple II sem instalar nada.",
}

# Dependências específicas entre histórias (além do oráculo da onda 0).
DEPENDENCIA_ESPECIFICA = {
    "HU-0002": "HU-0001 (inventário que define as issues e o corpus)",
    "HU-0202": "HU-0201 (framebuffer e métricas de vídeo)",
    "HU-0302": "HU-0301 (integridade de discos)",
    "HU-0303": "HU-0301 (integridade de discos)",
    "HU-0402": "HU-0401 (saves reais importados)",
    "HU-0502": "HU-0501 (Mockingboard/AY-3-8910)",
    "HU-0602": "HU-0601 (Z80 e interrupções)",
    "HU-0802": "HU-0801 (tabela de comandos do debugger)",
    "HU-0902": "HU-0901 (build e testes nos três SO)",
    "HU-1201": "HU-0402 (save-state atômico) e HU-0801 (debugger)",
    "HU-1202": "HU-0801 (debugger nativo como referência)",
    "HU-1205": "HU-0901 (build multiplataforma)",
}

# Texto de contexto por épico (modelos, descrição e rollback), derivado do plano.
META_EPICO = {
    "EP-00": {
        "desc": "Inventário histórico das issues do AppleWin, fixtures de referência (AppleWin-Test) e oráculo. Pré-requisito de todos os demais épicos.",
        "modelos": "Todos os modelos suportados: registrar, para cada issue ou fixture, o modelo e a configuração afetados. Não há configuração única.",
        "rollback": "Somente documentação, metadados e fixtures versionadas com SHA e licença; sem mudança de código. Reverter removendo os artefatos gerados.",
    },
    "EP-01": {
        "desc": "CPU 6502/65C02, temporização, barramento e memória (Saturn, Language Card, RamWorks, floating bus). Risco central de estado errado.",
        "modelos": "Apple II, II+, IIe, IIe Enhanced e clones; CPU 6502 e 65C02 separadamente; cartões de memória e slots conforme o teste.",
        "rollback": "Alterações no núcleo de CPU/memória em PRs pequenos, com teste de regressão e dump de bancos antes/depois; reverter o PR se o diferencial divergir.",
    },
    "EP-02": {
        "desc": "Vídeo, NTSC/PAL e modos de exibição: texto, low-res, hi-res, double hi-res, 40/80 colunas, Color TV e VidHD.",
        "modelos": "Por modelo de vídeo (II, II+, IIe, IIe Enhanced) e modo de cor (NTSC, PAL, Color TV), sempre configurado explicitamente.",
        "rollback": "Mudanças de framebuffer podem mascarar regressões; manter métricas e limiares versionados e reverter por PR.",
    },
    "EP-03": {
        "desc": "Disk II, WOZ/DSK/NIB, HDD/SmartPort e integridade das gravações. Risco de perda de dados do usuário.",
        "modelos": "Disk II em Apple II/II+/IIe; controladora SmartPort/HDD conforme a fixture; formatos WOZ, DSK, NIB e PO.",
        "rollback": "Mutações de disco exigem cópia de segurança e teste de integridade antes; nunca escrever sobre fixture mestre; reverter restaurando a cópia.",
    },
    "EP-04": {
        "desc": "Save-state (YAML): importação de saves reais do AppleWin, round-trip e gravação atômica.",
        "modelos": "Save-states de versões distintas do .aws.yaml; máquina, cartões e mídias registrados no próprio estado.",
        "rollback": "Gravação atômica com arquivo temporário e renomeação; migração de schema explícita; nunca sobrescrever o último save válido.",
    },
    "EP-05": {
        "desc": "Mockingboard, Phasor, AY-3-8910 e SSI263/SAM: registradores, IRQs, fonemas e áudio.",
        "modelos": "Mockingboard/Phasor com configuração de chips por slot; SSI263 e SAM; sockets Phasor conforme o cartão.",
        "rollback": "Diferenças de áudio exigem eventos e tolerância definidos antes do teste; reverter mudanças de mixagem em PR isolado.",
    },
    "EP-06": {
        "desc": "Z80 (interrupções, MEMPTR, CP/M), ThunderClock, Breakpoint Card e SNES MAX.",
        "modelos": "Cartão Z80 em slot específico; ThunderClock, Breakpoint Card e SNES MAX por slot; CP/M com imagem legalmente obtida.",
        "rollback": "Opcodes não suportados são relatados, não mascarados; reverter o PR de Z80 se o trace divergir da referência.",
    },
    "EP-07": {
        "desc": "Uthernet I/II, entrada (teclado, mouse, joystick, paddles), serial e impressora.",
        "modelos": "Uthernet I/II em slot; joystick e paddles por sistema operacional; impressora e serial conforme o host.",
        "rollback": "Rede testada localmente (loopback), sem Internet; reverter por PR isolado ou desativar o recurso opt-in em caso de falha.",
    },
    "EP-08": {
        "desc": "Debugger (comandos, atalhos, breakpoints, watchpoints) e ações de UI equivalentes às do AppleWin.",
        "modelos": "Independe do modelo, exceto quando o comando depende da CPU (6502/65C02) ou de configuração de vídeo.",
        "rollback": "Renderizar a UI do debugger não pode alterar o estado do emulador; reverter por PR com teste de regressão.",
    },
    "EP-09": {
        "desc": "Qualidade multiplataforma (Windows, macOS e Linux): CI bloqueante, empacotamento reproduzível e cadeia de suprimentos.",
        "modelos": "Três sistemas operacionais e suas arquiteturas registrados em cada execução; DPI, áudio e teclado regional conforme o ambiente.",
        "rollback": "Nenhum build vermelho gera release stable; reverter a mudança de CI ou de empacotamento em caso de falha.",
    },
    "EP-10": {
        "desc": "Performance preservando correção: benchmarks por cenário e otimização apenas após suíte diferencial verde.",
        "modelos": "Benchmarks por cenário (CPU, texto, HGR, DHGR, NTSC/PAL, Disk II, áudio) em hardware e compilação fixos.",
        "rollback": "Otimização só entra com traces e temporização inalterados; reverter se o diferencial falhar.",
    },
    "EP-11": {
        "desc": "Triagem de issues upstream abertas e histórias de compatibilidade com problemas históricos.",
        "modelos": "Conforme a issue upstream triada; registrar modelo e configuração de cada caso.",
        "rollback": "A triagem não altera código; cada história nova tem reversão própria.",
    },
    "EP-12": {
        "desc": "Evoluções próprias sem comprometer a compatibilidade: rewind, DAP, laboratório educacional, investigação de Apple I e Web/WASM (opt-in).",
        "modelos": "Apple II e II+/IIe conforme o escopo. Apple I somente como investigação (NÃO IMPLÍCITO).",
        "rollback": "Recursos opt-in e isolados; sem regressão no desktop; a investigação de Apple I não altera o núcleo Apple II.",
    },
}

ORACULO_PADRAO = "especificação de hardware, fixture validado, AppleWin no SHA fixo ou resultado conhecido."
TESTE_PADRAO = "unitário; integração; diferencial; fixture real; cross-platform; manual quando necessário."
DOD_PADRAO = "CA completos, testes verdes, revisão independente, documentação atualizada e sem regressões."
DOC_REFERENCIA = "docs/manual/12-diferencas-em-relacao-ao-applewin.md"

RE_EPICO = re.compile(r"^## \d+\. ÉPICO (EP-\d{2}) — (.+?) \(P([0-3])(?:/P[0-3])?\)\s*$")
RE_HU = re.compile(r"^\*\*(HU-\d{4}) — (.+?)\*\*\s*$")
RE_GATE = re.compile(r"^\*\*Gate (G\d) — ([^:*]+):\*\*\s*(.+)$")
RE_PRIO_TITULO = re.compile(r"\s*\(P([0-3])[^)]*\)\s*$")
RE_CA = re.compile(r"^\*\*CA:\*\*\s*(.*)$")
RE_LINK = re.compile(r"https://github\.com/AppleWin/[^\s)\]>,;]+")
RE_ISSUE_LINK = re.compile(r"/issues/(\d+)$")


# ---------------------------------------------------------------- plano

def parse_plano(texto: str):
    """Lê o plano e devolve (epicos, hus, gates) na ordem do documento."""
    epicos: dict[str, dict] = {}
    hus: list[dict] = []
    gates: dict[str, tuple[str, str]] = {}
    epico_atual = None
    hu_atual = None

    for linha in texto.splitlines():
        s = linha.rstrip()

        m = RE_EPICO.match(s)
        if m:
            epico_atual = m.group(1)
            hu_atual = None
            epicos[epico_atual] = {
                "id": epico_atual,
                "nome": m.group(2).strip(),
                "prioridade": f"P{m.group(3)}",
                "onda": ONDA_DO_EPICO[epico_atual],
            }
            continue

        m = RE_GATE.match(s)
        if m:
            gates[m.group(1)] = (m.group(2).strip(), m.group(3).strip())
            continue

        if s.startswith("## ") or s.startswith("### "):
            hu_atual = None
            continue

        m = RE_HU.match(s)
        if m:
            hu_atual = {"id": m.group(1), "titulo_bruto": m.group(2).strip(),
                        "epico": epico_atual, "linhas": []}
            hus.append(hu_atual)
            continue

        if hu_atual is not None:
            hu_atual["linhas"].append(s)

    return epicos, [montar_hu(h, epicos) for h in hus], gates


def extrair_ca(linhas: list[str]) -> list[str]:
    for i, s in enumerate(linhas):
        m = RE_CA.match(s)
        if not m:
            continue
        resto = m.group(1).strip()
        if resto:
            # CA escritos em linha única, separados por "; ".
            return [p.strip().rstrip(".") for p in resto.split("; ") if p.strip()]
        itens: list[str] = []
        for t in linhas[i + 1:]:
            if t.startswith("- "):
                itens.append(t[2:].strip().rstrip("."))
            elif t.strip() == "" and not itens:
                continue
            else:
                break
        return itens
    return []


def montar_hu(h: dict, epicos: dict) -> dict:
    hid = h["id"]
    titulo = h["titulo_bruto"]
    m = RE_PRIO_TITULO.search(titulo)
    prio_titulo = f"P{m.group(1)}" if m else None
    if m:
        titulo = titulo[:m.start()].rstrip()

    epico = h["epico"]
    prioridade = PRIORIDADE_FORCADA.get(hid) or prio_titulo or epicos[epico]["prioridade"]

    como = next((l for l in h["linhas"] if l.startswith("Como ")), None)
    if como is None:
        como = COMO_COMPLEMENTAR[hid]

    cas = extrair_ca(h["linhas"])
    if not cas:
        raise SystemExit(f"{hid}: critérios de aceite não encontrados no plano")

    texto = "\n".join(h["linhas"])
    refs: list[str] = []
    for url in RE_LINK.findall(texto):
        url = url.rstrip(".")
        if url not in refs:
            refs.append(url)

    if epico == "EP-00":
        gate = "G0"
    elif prioridade == "P0":
        gate = "G2"
    else:
        gate = "G3"

    dep = DEPENDENCIA_ESPECIFICA.get(hid)
    base = "HU-0001 e HU-0002 (oráculo, onda 0)" if epico != "EP-00" else "nenhuma dentro do backlog"
    dependencias = f"{dep}; {base}" if dep else base
    if hid == "HU-0001":
        dependencias = "nenhuma dentro do backlog (primeira história da onda 0)"

    return {
        "id": hid,
        "titulo": titulo,
        "epico": epico,
        "prioridade": prioridade,
        "onda": epicos[epico]["onda"],
        "gate": gate,
        "como": como,
        "cas": cas,
        "refs": refs,
        "dependencias": dependencias,
        "nao_implicito": hid == "HU-1204",
    }


def parse_gates_titulo(gates: dict) -> dict[str, str]:
    return {g: f"{g} — {nome}" for g, (nome, _) in gates.items()}


# ---------------------------------------------------------------- corpos

def corpo_hu(hu: dict, meta: dict, numero_epico: int | None) -> str:
    refs = [f"- [#{u.rsplit('/', 1)[1]}]({u})" if RE_ISSUE_LINK.search(u) else f"- {u}" for u in hu["refs"]]
    refs.append(f"- Documentação AppleII-Rust: {DOC_REFERENCIA}")
    epico_ref = f"#{numero_epico}" if numero_epico else "(a definir)"
    linhas = []
    if hu["nao_implicito"]:
        linhas += ["> **NÃO IMPLÍCITO:** Apple I é arquitetura diferente de Apple II. Esta história é investigação separada "
                   "(ADR), não requisito obrigatório; não prometer suporte sem decisão explícita de escopo e suíte própria.", ""]
    linhas += [
        "**Tipo:** história",
        "",
        hu["como"],
        "",
        "**Referência:**",
        *refs,
        "",
        "**Estado atual:** UNVERIFIED",
        "",
        f"**Modelos/configurações:** {meta['modelos']}",
        "",
        f"**Dependências:** {hu['dependencias']}",
        "",
        "**Critérios de aceite (CA):**",
        *[f"- CA-{i:02d}: {c}" for i, c in enumerate(hu["cas"], 1)],
        "",
        f"**Plano de teste:** {TESTE_PADRAO}",
        "",
        f"**Oráculo:** {ORACULO_PADRAO}",
        "",
        "**Evidências:** a preencher (comandos, logs, SHA, captura/hash/dump, SO, versão, artefatos).",
        "",
        f"**Definition of Done:** {DOD_PADRAO}",
        "",
        f"**Rollback/risco:** {meta['rollback']}",
        "",
        "---",
        f"Épico: {epico_ref}",
        f"Origem: {PLANO_REL}",
    ]
    return "\n".join(linhas) + "\n"


def corpo_epico(ep: dict, meta: dict, hus_do_epico: list[dict], numeros: dict[str, int]) -> str:
    linhas = [
        meta["desc"],
        "",
        f"**Prioridade:** {ep['prioridade']} · **Onda:** {ep['onda']} · **Tipo:** épico",
        "",
        "## Histórias",
        "",
    ]
    for hu in hus_do_epico:
        n = numeros.get(hu["id"])
        ref = f"#{n} " if n else ""
        linhas.append(f"- [ ] {ref}{hu['id']} {hu['titulo']} — {hu['prioridade']}")
    linhas += ["", "---", f"Origem: {PLANO_REL}"]
    return "\n".join(linhas) + "\n"


# ---------------------------------------------------------------- gh

def gh(args: list[str], corpo: str | None = None) -> str:
    r = subprocess.run(["gh", *args], input=corpo, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError(f"gh {' '.join(args[:3])} falhou: {r.stderr.strip()}")
    return r.stdout


def rotulos_padrao() -> list[tuple[str, str, str]]:
    r = []
    for p, cor, d in [("P0", "B60205", "Prioridade P0: bloqueante"),
                      ("P1", "D93F0B", "Prioridade P1: alta"),
                      ("P2", "FBCA04", "Prioridade P2: média"),
                      ("P3", "C5DEF5", "Prioridade P3: baixa ou investigação")]:
        r.append((f"prioridade:{p}", cor, d))
    r += [
        ("tipo:epico", "5319E7", "Épico do backlog"),
        ("tipo:historia", "1D76DB", "História de usuário"),
        ("tipo:bug", "D73A4A", "Bug"),
        ("tipo:divida", "BFD4F2", "Dívida técnica"),
        ("tipo:teste", "006B75", "Teste ou infraestrutura de teste"),
    ]
    for i in range(13):
        r.append((f"epico:EP-{i:02d}", "7057FF", f"Épico EP-{i:02d}"))
    r += [
        ("estado:IMPLEMENTED", "FBCA04", "Implementado, sem prova de PASS"),
        ("estado:PASS", "0E8A16", "Verificado com evidência"),
        ("estado:FAIL", "B60205", "Falhou na verificação"),
        ("estado:UNVERIFIED", "BFBFBF", "Ainda não verificado"),
        ("estado:NOT_APPLICABLE", "D4C5F9", "Não aplicável, com justificativa"),
        ("estado:BLOCKED", "E99695", "Bloqueado por causa externa"),
    ]
    for i, cor in enumerate(["C2E0C6", "BFDADC", "A2EEEF", "FEF2C0", "F9D0C4"]):
        r.append((f"onda:{i}", cor, f"Onda {i} do plano (§19)"))
    return r


def sincronizar_rotulos(dry: bool) -> None:
    for nome, cor, desc in rotulos_padrao():
        if dry:
            print(f"  [dry-run] rótulo {nome} ({cor})")
        else:
            gh(["label", "create", nome, "--repo", REPO, "--color", cor, "--description", desc, "--force"])


def sincronizar_milestones(titulos: dict[str, str], gates: dict, dry: bool) -> None:
    existentes = {m["title"] for m in json.loads(gh(["api", f"repos/{REPO}/milestones?state=all&per_page=100"]))}
    for g, titulo in titulos.items():
        if titulo in existentes:
            print(f"  milestone já existe: {titulo}")
            continue
        desc = gates[g][1]
        if dry:
            print(f"  [dry-run] milestone {titulo}")
        else:
            gh(["api", f"repos/{REPO}/milestones", "-f", f"title={titulo}", "-f", f"description={desc}"])
            print(f"  milestone criada: {titulo}")


def carregar_existentes() -> dict[str, dict]:
    dados = json.loads(gh(["issue", "list", "--repo", REPO, "--state", "all", "--limit", "1000",
                           "--json", "number,title,body,labels,milestone"]))
    mapa: dict[str, dict] = {}
    for it in dados:
        m = re.match(r"\[(EP-\d{2}|HU-\d{4})\]", it["title"])
        if not m:
            continue
        if m.group(1) in mapa:
            raise SystemExit(f"duplicata no GitHub para {m.group(1)}: #{mapa[m.group(1)]['number']} e #{it['number']}")
        mapa[m.group(1)] = it
    return mapa


def sincronizar_issue(rec: dict, existentes: dict, dry: bool, corpo_fn=None, so_criar=False) -> int | None:
    """Cria ou atualiza uma issue. Devolve o número (None no dry-run ao criar)."""
    chave = rec["chave"]
    ex = existentes.get(chave)
    if ex is not None:
        numero = ex["number"]
        if so_criar:
            return numero
        corpo = corpo_fn() if corpo_fn else None
        labels_atuais = {l["name"] for l in ex.get("labels", [])}
        milestone_atual = (ex.get("milestone") or {}).get("title")
        precisa = (
            (corpo is not None and corpo.replace("\r\n", "\n") != (ex.get("body") or "").replace("\r\n", "\n"))
            or milestone_atual != rec["milestone"]
            or not set(rec["labels"]) <= labels_atuais
        )
        if not precisa:
            print(f"  OK          #{numero} {rec['titulo']}")
            return numero
        print(f"  ATUALIZAR   #{numero} {rec['titulo']}")
        if not dry:
            args = ["issue", "edit", str(numero), "--repo", REPO, "--milestone", rec["milestone"]]
            for l in rec["labels"]:
                args += ["--add-label", l]
            gh([*args, "--body-file", "-"], corpo)
        return numero

    print(f"  CRIAR       {rec['titulo']}  [{', '.join(rec['labels'])}]  milestone={rec['milestone']}")
    if dry:
        return None
    corpo = corpo_fn() if corpo_fn else (rec.get("corpo_inicial") or "")
    args = ["issue", "create", "--repo", REPO, "--title", rec["titulo"], "--milestone", rec["milestone"]]
    for l in rec["labels"]:
        args += ["--label", l]
    url = gh([*args, "--body-file", "-"], corpo).strip().splitlines()[-1]
    numero = int(url.rsplit("/", 1)[1])
    existentes[chave] = {"number": numero, "title": rec["titulo"], "body": corpo,
                         "labels": [{"name": l} for l in rec["labels"]],
                         "milestone": {"title": rec["milestone"]}}
    return numero


# ---------------------------------------------------------------- saídas

def gravar_saidas(linhas_tabela: list[dict]) -> None:
    SAIDA_MD.parent.mkdir(parents=True, exist_ok=True)
    md = [
        "# Backlog de conformidade — AppleII-Rust",
        "",
        f"Gerado por `tools/conformance/backlog.py` a partir de `{PLANO_REL}`. Não editar à mão; rodar o script de novo.",
        "",
        "| ID | Título | Prioridade | Onda | Gate | Issue GitHub (#N) |",
        "|---|---|---|---|---|---|",
    ]
    for r in linhas_tabela:
        md.append(f"| {r['id']} | {r['titulo']} | {r['prioridade']} | {r['onda']} | {r['gate']} | #{r['issue']} |")
    SAIDA_MD.write_text("\n".join(md) + "\n", encoding="utf-8")
    SAIDA_JSON.write_text(json.dumps(linhas_tabela, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


# ---------------------------------------------------------------- main

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dry-run", action="store_true", help="só imprime o que faria; não escreve no GitHub nem em docs/")
    args = ap.parse_args()
    dry = args.dry_run

    texto = (RAIZ / PLANO_REL).read_text(encoding="utf-8")
    epicos, hus, gates = parse_plano(texto)
    if len(epicos) != 13:
        raise SystemExit(f"esperados 13 épicos, encontrados {len(epicos)}")
    titulos_gate = parse_gates_titulo(gates)
    print(f"Plano: {len(epicos)} épicos, {len(hus)} histórias, {len(gates)} gates.")

    print("\nRótulos:")
    sincronizar_rotulos(dry)
    print("\nMilestones:")
    sincronizar_milestones(titulos_gate, gates, dry)

    existentes = carregar_existentes()

    # Registros de épicos
    recs_ep: dict[str, dict] = {}
    for eid, ep in epicos.items():
        filhas = [h for h in hus if h["epico"] == eid]
        gate_ep = min((h["gate"] for h in filhas), key=GATES_ORDEM.index)
        recs_ep[eid] = {
            "chave": eid,
            "titulo": f"[{eid}] {ep['nome']}",
            "labels": [f"prioridade:{ep['prioridade']}", "tipo:epico", f"epico:{eid}", f"onda:{ep['onda']}"],
            "milestone": titulos_gate[gate_ep],
            "gate": gate_ep,
            "hus": filhas,
        }

    print("\nÉpicos (fase 1: criar os que faltam):")
    numeros: dict[str, int] = {}
    for eid, rec in recs_ep.items():
        placeholder = corpo_epico(epicos[eid], META_EPICO[eid], rec["hus"], {})
        rec["corpo_inicial"] = placeholder
        n = sincronizar_issue(rec, existentes, dry, so_criar=True)
        if n is not None:
            numeros[eid] = n

    print("\nHistórias:")
    for hu in hus:
        eid = hu["epico"]
        meta = META_EPICO[eid]
        rec = {
            "chave": hu["id"],
            "titulo": f"[{hu['id']}] {hu['titulo']} — {hu['prioridade']}",
            "labels": [f"prioridade:{hu['prioridade']}", "tipo:historia", f"epico:{eid}",
                       "estado:UNVERIFIED", f"onda:{hu['onda']}"],
            "milestone": titulos_gate[hu["gate"]],
        }
        n = sincronizar_issue(
            rec, existentes, dry,
            corpo_fn=(lambda h=hu, m=meta, e=eid: corpo_hu(h, m, numeros.get(e))) if not dry else None,
        )
        if n is not None:
            numeros[hu["id"]] = n

    print("\nÉpicos (fase 2: corpo final com checklist de histórias):")
    for eid, rec in recs_ep.items():
        n = sincronizar_issue(
            rec, existentes, dry,
            corpo_fn=(lambda e=eid, r=rec: corpo_epico(epicos[e], META_EPICO[e], r["hus"], numeros)) if not dry else None,
        )
        if n is not None:
            numeros[eid] = n

    if dry:
        print("\n[dry-run] nada foi escrito no GitHub nem em docs/conformance/.")
        return 0

    # Tabela final
    linhas = []
    for eid, ep in epicos.items():
        r = recs_ep[eid]
        linhas.append({"id": eid, "tipo": "épico", "titulo": ep["nome"], "prioridade": ep["prioridade"],
                       "onda": ep["onda"], "gate": r["milestone"], "epico": eid, "issue": numeros[eid]})
    for hu in hus:
        linhas.append({"id": hu["id"], "tipo": "história", "titulo": hu["titulo"], "prioridade": hu["prioridade"],
                       "onda": hu["onda"], "gate": titulos_gate[hu["gate"]], "epico": hu["epico"],
                       "issue": numeros[hu["id"]]})
    gravar_saidas(linhas)
    print(f"\nGravados: {SAIDA_MD} e {SAIDA_JSON}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
