#!/usr/bin/env python3
"""Atualiza as histórias HU (#15 a #44) no GitHub com o estado real de verificação.

Para cada HU: troca a label `estado:*` (hoje `estado:UNVERIFIED`) pela do estado
medido e deixa UM comentário curto de evidência, marcado com um comentário HTML
para que novas execuções o editem em vez de duplicá-lo. Não fecha issues: o
fechamento é decisão do dono, depois da revisão independente. Não altera o corpo
(o campo "Estado atual" do corpo continua sendo o do `backlog.py`).

Regra: `estado:PASS` só se todos os Critérios de aceite (CA) da história estão em
PASS; senão `estado:FAIL` (algum CA medido discorda do Oráculo) ou
`estado:BLOCKED` (algum CA depende de fixture, hardware, CI ou decisão que ainda
não existe). Os estados são o resultado da autoavaliação de 2026-10-21 e valem
até a revisão independente; os números vêm de docs/conformance/relatorio-final.md.

Uso:
  python3 tools/conformance/update_hu_issues.py --dry-run   # só imprime o que faria
  python3 tools/conformance/update_hu_issues.py             # aplica (idempotente)

Idempotência: label já correta e comentário já igual => nada é escrito. Só
biblioteca padrão; chama o `gh` já autenticado.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys

REPO = "ChicoFigueiredo/AppleII-Rust"
MARCA = "<!-- update_hu_issues:v1 -->"
SHA = "bb01e3415c45fcb3d7e1103e788cc6cec2279e5f"
BASE = f"https://github.com/{REPO}/blob/main/"
ESTADOS = ["PASS", "FAIL", "BLOCKED", "UNVERIFIED", "IMPLEMENTED", "NOT_APPLICABLE"]

C = "docs/conformance/"
RF = C + "relatorio-final.md"
RR = C + "release-readiness.md"
CM = C + "capability-matrix.md"
GAP = C + "gaps-and-known-limitations.md"
SSI = "tests/conformance/reports/savestate-import.md"

# issue -> (HU, estado, resumo, arquivos)
HU = {
    15: ("HU-0001", "BLOCKED",
         "1.115 issues com estado e evidência/justificativa (479 PASS, 14 FAIL, 133 BLOCKED, 489 N/A, 0 UNVERIFIED), matriz e sumário gerados. CA-07 (auditor independente) cobriu só a triagem original (465 issues); os resultados da onda final são autoavaliação e aguardam revisão independente.",
         [C + "applewin-issue-matrix.csv", C + "issue-audit.md", C + "issue-audit-review.md", C + "results/"]),
    16: ("HU-0002", "BLOCKED",
         "Corpus fixado em `4b113ce`, 115 estados reais: 88 PASS, 0 FAIL, 27 BLOCKED com causa (imagem ausente, rede, hora real, Z80). Falta o CA de CI headless com JUnit: a CI nunca foi executada (cota esgotada, só o dono autoriza).",
         [SSI, C + "test-oracles.md", RR]),
    17: ("HU-0101", "PASS",
         "7 traces idênticos ao Oráculo (até 5 milhões de ciclos), 10.220.000 vetores SingleStepTests/65x02 sem inesperados, determinismo 10x, regressões de issues de CPU (#1360 e outras). Diferenças AppleWin x hardware catalogadas e copiadas do AppleWin (ADR 0004).",
         [CM, "tests/conformance/reports/65x02-singlestep.md", C + "audit/cpu-trace-differential.md"]),
    18: ("HU-0102", "PASS",
         "Contagem de bancos do Saturn corrigida e testada com 4 e 8 bancos; RamWorks, Language Card, #1486/#1414/#1520 reproduzidas; `Saturn64`/`Saturn128` reais agora PASS no trace de 100.000 ciclos. Fixtures de terceiros (CA condicional) seguem BLOCKED: #400, #422, #1126.",
         [CM, SSI, GAP]),
    19: ("HU-0103", "PASS",
         "Floating bus e sincronismo CPU/vídeo/E-S iguais ao Oráculo nos 4 Modelos (traces e cenas). Exceção rastreada em HU-0201 e na issue #46: a cena #241 em Color Monitor PAL difere em 1 pixel.",
         [CM, C + "audit/video-differential.md"]),
    20: ("HU-0201", "FAIL",
         "1.573 telas idênticas bit a bit (411 casos `video-*`), mas a cena #241 em Color Monitor PAL (floating bus, TEXT/GR por linha) difere em 1 pixel de borda, causa não localizada. Vídeo dos clones (Pravets, TK3000, Base64A, J-Plus) sem comparação de framebuffer.",
         [C + "audit/video-differential.md", GAP, CM]),
    21: ("HU-0202", "PASS",
         "Modos legados centrados e Super Hi-Res da VidHD idênticos ao Oráculo; estado de vídeo sobrevive ao save/load (`vidhd_roundtrip.rs`). 80x45, cor de borda e VidHD no ][/][+ não existem no AppleWin e estão documentados como diferença.",
         [CM, C + "audit/video-differential.md"]),
    22: ("HU-0301", "PASS",
         "56 casos `disk-*` com 103.146.754 instruções idênticas e SHA-256 final das imagens; gravação atômica, queda simulada, flush no motor, fuzz de WOZ/NIB/DSK/2MG/gz/zip e limites de tamanho. Sem NIB real nem WOZ1 real (documentado).",
         [C + "audit/disk-differential.md", RF]),
    23: ("HU-0302", "BLOCKED",
         "Troca de disco, ejetar/remontar e somente leitura passam nos testes Rust (Linux), mas o CA exige Linux, Windows e macOS: Windows nativo e macOS não foram executados. #640 (P0) BLOCKED: o Oráculo não troca disco com a máquina ligada.",
         [CM, GAP, C + "manual-tests/plataformas.md"]),
    24: ("HU-0303", "PASS",
         "8 casos `disk-hdd-*` (boot, E/S de bloco, DMA, imagem de 32 MB) idênticos ao Oráculo; firmware do DIB 132.1 igual ao `a8f0d1d`; `SmartPort-firmware` real agora PASS. Ressalva: HDD acima de 2 MiB é gravado no lugar, não por renomeação atômica (risco médio, no relatório).",
         [CM, RF, SSI]),
    25: ("HU-0401", "BLOCKED",
         "88 de 115 estados reais PASS (trace de 100.000 ciclos), 0 FAIL, 27 BLOCKED com causa (18 por imagem ausente do corpus, 3 SSC por TCP/serial, 2 relógio real, 2 Shift Key Mod, 1 Z80, 1 Uthernet). O CA de comparar todos os estados não fecha enquanto houver BLOCKED.",
         [SSI, C + "savestate-schema.md"]),
    26: ("HU-0402", "PASS",
         "Round-trip determinístico, Unicode/espaços/contrabarras, estado de RGB e impressora, #1499/#1510/#1372, gravação atômica e queda simulada que não destrói o último save, migração de versões antigas pelo corpus. #564 (P0) segue BLOCKED sem Oráculo de janela; o teste Rust passa.",
         [CM, RF]),
    27: ("HU-0501", "FAIL",
         "Eventos do AY/6522/Phasor idênticos (27 casos PASS) e IRQ do 6522 (#1176) corrigido, mas o PCM do AY (tom, envelope, ruído) e do alto-falante sai dos limiares registrados antes (casos `audio-*-pcm` FAIL), por diferença de fatiamento do AppleWin. Audição humana BLOCKED.",
         [C + "audit/audio-differential.md", GAP, CM]),
    28: ("HU-0502", "FAIL",
         "Fonemas encadeados, power-down (#1412) e fixtures SSI263 x4/SC-01 x2 passam, mas `audio-ssi263-phonemes-p333` FAIL: 1.683 ciclos contra o limiar de 1.670 (13 ciclos). Retomada no save-state (#1372) coberta.",
         [C + "audit/audio-differential.md", GAP]),
    29: ("HU-0601", "BLOCKED",
         "1.604.000 vetores Z80 e ZEXDOC/ZEXALL 65/65 passam; IRQ/NMI/HALT no núcleo. O CA da sessão CP/M legalmente obtida e do trace comparável não foi executado: sem fixture com SoftCard e sem Oráculo para o Z80 em sistema.",
         ["tests/conformance/reports/z80-singlestep.md", "tests/conformance/reports/z80-zex.md", GAP]),
    30: ("HU-0602", "PASS",
         "Conflitos de slot como o CardManager do AppleWin, Breakpoint Card ligada aos breakpoints, 4Play e SNES MAX recebem os controles; `SNES_MAX-slot4` real agora PASS. ThunderClock existe na biblioteca sem tipo escolhível, como no AppleWin (diferença documentada).",
         [CM, SSI]),
    31: ("HU-0701", "BLOCKED",
         "Uthernet I e II com testes locais (loopback, UDP, TCP, malformados) e modos sem suporte recusados. Faltam rede real e múltiplas sessões contra um Oráculo; estados com TCP BLOCKED.",
         [CM, C + "manual-tests/rede-serial-impressora.md"]),
    32: ("HU-0702", "FAIL",
         "Teclado, mouse, joystick, #1522, #1234 e a tecla presa (`hu0702_input.rs`) passam. FAIL: #1396, o AppleWin apaga o Open Apple no F2 (Alt+F2) e o port mantém (teste `#[ignore]` em `p23_entrada.rs`); decisão do dono. Gamepads físicos por SO e captura de mouse: roteiros manuais não executados.",
         [C + "results/p23-entrada.csv", CM, C + "manual-tests/"]),
    33: ("HU-0703", "BLOCKED",
         "Erros de serial, impressora e pcap aparecem na barra de status e a configuração é preservada (testes Rust). Detecção de dispositivos e permissões por SO exigem hardware e Windows/macOS: BLOCKED.",
         [C + "manual-tests/rede-serial-impressora.md", GAP]),
    34: ("HU-0801", "FAIL",
         "73 issues de depurador PASS (62 + 11 da onda final) e 251 comandos, mas 10 FAIL (#114, #245, #466, #468, #713, #1145, #1377, #1381, #1426, #1431) e a #458 reproduzem bugs do próprio AppleWin `a8f0d1d`. Manter a paridade ou corrigir como divergência é decisão do dono.",
         [C + "results/depurador.csv", C + "results/p23-depurador.csv", GAP, RR]),
    35: ("HU-0802", "BLOCKED",
         "49 issues de interface verificadas por testes Rust (tela cheia, título, status, escala, captura de tela, soltar disco, somente leitura, importar configuração). Sem Oráculo de janela e sem execução fora do Linux (atalhos, Alt+Enter e tela cheia em Windows e macOS). #458 (Ctrl+PrintScreen no depurador) segue o AppleWin e aguarda decisão do dono.",
         [C + "results/p23-ui.csv", GAP, RR]),
    36: ("HU-0901", "BLOCKED",
         "Linux nativo PASS (smoke, pacote extraído) e Windows sob Wine PASS. Windows nativo, macOS e a CI nos 3 SO nunca foram executados (cota esgotada; só o dono autoriza o `workflow_dispatch`).",
         [C + "platform/relatorio-plataformas.md", C + "platform/windows-wine.md", RR]),
    37: ("HU-0902", "BLOCKED",
         "SHA256SUMS, SBOM e licenças de terceiros preparados e pacote Linux reproduzido; `release.yml` nunca executou em CI e os pacotes não são assinados nem notarizados (decisão e custo do dono).",
         [C + "platform/assinatura.md", RR]),
    38: ("HU-1001", "BLOCKED",
         "Benchmarks determinísticos por cenário publicados (mediana, máquina compartilhada). Faltam hardware fixo, p95 e latência de entrada, e a detecção de regressão de desempenho em CI.",
         ["tests/conformance/reports/performance.md", CM]),
    39: ("HU-1101", "PASS",
         "358 issues abertas do AppleWin coletadas em 2026-10-21 (00:47 UTC) e todas com estado; 36 histórias locais (#45 a #80) derivadas das selecionadas; nenhum número inventado.",
         [C + "upstream-open-issues-triage.md", C + "upstream-open-selected.md"]),
    40: ("HU-1201", "PASS",
         "Crate `apple2-rewind` com testes de ida e volta determinísticos; extensão sem equivalente no AppleWin.",
         [CM]),
    41: ("HU-1202", "PASS",
         "Crates `apple2-dap` e `applewin-dap` com cliente DAP de teste automatizado; extensão sem equivalente no AppleWin.",
         [CM]),
    42: ("HU-1203", "PASS",
         "Crate `apple2-lab` com exercícios reproduzíveis e sem ROM proprietária; manual cap. 14.",
         [CM]),
    43: ("HU-1204", "PASS",
         "Investigação entregue (ADR 0005 e estudo): Apple I fica fora do escopo até decisão do dono; nenhum suporte prometido.",
         [C + "apple1-investigation.md", "docs/adr/0005-apple-i-fora-do-escopo-ate-decisao.md"]),
    44: ("HU-1205", "PASS",
         "Estudo e protótipo opt-in: o núcleo compila para `wasm32-unknown-unknown` sem mudanças; crate `applewin-web`.",
         [C + "wasm-estudo.md", CM]),
}


def gh(*args, entrada=None):
    r = subprocess.run(["gh", *args], capture_output=True, text=True, input=entrada)
    if r.returncode != 0:
        sys.exit(f"gh {' '.join(args)}: {r.stderr.strip()}")
    return r.stdout


def corpo_do_comentario(hu, estado, resumo, arquivos):
    links = "\n".join(f"- [{a}]({BASE}{a})" for a in arquivos)
    return (
        f"{MARCA}\n**{hu}: estado `{estado}`** (autoavaliação de 2026-10-21 sobre a `main` `{SHA[:7]}`; "
        f"a revisão independente ainda não aconteceu e esta issue **não deve ser fechada** antes dela).\n\n"
        f"{resumo}\n\nEvidência:\n{links}\n\n"
        f"Regra: PASS só com todos os CA em PASS; senão FAIL (CA medido discorda do Oráculo) ou BLOCKED "
        f"(CA depende de fixture, hardware, CI ou decisão do dono). Gerado por "
        f"`tools/conformance/update_hu_issues.py`."
    )


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dry-run", action="store_true", help="só imprime o que faria")
    args = ap.parse_args()

    contagem = {}
    for numero, (hu, estado, resumo, arquivos) in sorted(HU.items()):
        dados = json.loads(gh("issue", "view", str(numero), "-R", REPO, "--json",
                              "title,labels,state,comments"))
        if not dados["title"].startswith(f"[{hu}]"):
            sys.exit(f"#{numero}: título inesperado {dados['title']!r} (esperava [{hu}])")
        atuais = [l["name"] for l in dados["labels"] if l["name"].startswith("estado:")]
        novo = f"estado:{estado}"
        contagem[estado] = contagem.get(estado, 0) + 1
        acoes = []
        if atuais != [novo]:
            remover = [l for l in atuais if l != novo]
            acoes.append(f"labels {atuais} -> [{novo}]")
            if not args.dry_run:
                cmd = ["issue", "edit", str(numero), "-R", REPO, "--add-label", novo]
                for r in remover:
                    cmd += ["--remove-label", r]
                gh(*cmd)
        texto = corpo_do_comentario(hu, estado, resumo, arquivos)
        existente = next((c for c in dados["comments"] if MARCA in c["body"]), None)
        if existente is None:
            acoes.append("comentário novo")
            if not args.dry_run:
                gh("issue", "comment", str(numero), "-R", REPO, "--body", texto)
        elif existente["body"].strip() != texto.strip():
            acoes.append("comentário atualizado")
            if not args.dry_run:
                comentario_id = existente["url"].rsplit("-", 1)[-1].replace("issuecomment", "")
                gh("api", "-X", "PATCH", f"repos/{REPO}/issues/comments/{comentario_id}",
                   "-f", f"body={texto}")
        if dados["state"] != "OPEN":
            acoes.append(f"AVISO: issue {dados['state']}")
        print(f"#{numero} {hu} {estado}: {'; '.join(acoes) or 'já em dia'}")
    print("Totais:", ", ".join(f"{k}={v}" for k, v in sorted(contagem.items())),
          "(dry-run)" if args.dry_run else "")


if __name__ == "__main__":
    main()
