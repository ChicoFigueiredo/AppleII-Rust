#!/usr/bin/env python3
"""Diferencial de VideoStyle (50% scan lines desligado, vertical blend): oráculo x Rust.

Mesmo critério do video-differential.py (bit a bit, framebuffer inteiro). Estilo = bits de
VideoStyle_e: 1 = VS_HALF_SCANLINES (padrão), 2 = VS_COLOR_VERTICAL_BLEND (só no tipo 1).
Os estilos 0 e 3 cobrem "scan lines 100%" e "blend + 50%"; 2 cobre blend sem scan lines.
usage: video-style-differential.py [--oracle BIN] [--write-cases DIR]
"""
import argparse, concurrent.futures as cf, importlib.util, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("vd", os.path.join(HERE, "video-differential.py"))
vd = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vd)
ROOT = vd.ROOT

SCENES = ["text40", "text80", "hires-mixed", "lores-mixed", "hires-bars", "hires-noise", "dhires-noise", "dlores",
          "midline-hgr-text-24", "midline-1443-hgr-then-text"]
JOBS = []  # (scene, type, hz, style)
for sc in SCENES:
    for t in range(9):
        JOBS.append((sc, t, 60, 0))
    JOBS.append((sc, 1, 60, 3))
    for t in (1, 2, 3, 4, 5):  # PAL: o #1157 (sujeira na borda direita) aparece em NTSC e PAL
        JOBS.append((sc, t, 50, 0))
for sc in ("hires-bars", "hires-noise", "lores-mixed", "text40"):
    JOBS.append((sc, 1, 60, 2))
# GH#1258: Apple IIe Card for Macintosh LC double lo-res quirk (RGB_SetMacLCCardDLGR), RGB card path
for sc in ("dlores", "dlores-mixed", "issue-0516-auxmove-dlores"):
    for t_ in (2, 3):
        JOBS.append((sc, t_, 60, 1, 1))


def sh(cmd):
    r = subprocess.run(cmd, capture_output=True, text=True)
    return None if r.returncode else vd.parse_kv(r.stdout)


def one(a, job):
    sc, t, hz, st, mac = (list(job) + [0])[:5]
    path = os.path.join(ROOT, "tests", "conformance", "inputs", "video", sc + ".scene")
    model = vd.scene_info(path)["model"]
    fr = vd.frames_for(sc)[0]
    tag = f"{sc}.t{t}.h{hz}.s{st}.m{mac}"
    o = sh([a.oracle, "--model", model, "--frames", str(fr), "--out", "/dev/null", "--cards", "none", "--fb-out",
            os.path.join(a.out, "o" + tag), "--scene", path, "--video-type", str(t), "--video-hz", str(hz),
            "--rgb-card", "0", "--video-style", str(st)] + (["--mac-lc-dlgr", "1"] if mac else []))
    r = sh([a.conformance, "scene-fb", "--scene", path, "--model", vd.MODEL[model], "--frames", str(fr), "--type", str(t),
            "--hz", str(hz), "--rgb-card", "0", "--style", str(st), "--out", os.path.join(a.out, "r" + tag)] + (["--mac-lc-dlgr", "1"] if mac else []))
    ok = bool(o and r and o["fb_sha256_full"] == r["fb_sha256_full"])
    return dict(job=tuple((list(job) + [0])[:5]), model=model, frames=fr, ok=ok, sha=o and o["fb_sha256_full"], rust=r and r["fb_sha256_full"])


def main():
    ap = argparse.ArgumentParser()
    cache = os.environ.get("ORACLE_CACHE", os.path.join(ROOT, "target", "conformance-cache"))
    ap.add_argument("--oracle", default=os.path.join(cache, "oracle", "build", "audetto-oracle"))
    ap.add_argument("--conformance", default=os.path.join(ROOT, "target", "release", "conformance"))
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "video-style-differential"))
    ap.add_argument("--write-cases", default="")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    with cf.ThreadPoolExecutor(8) as ex:
        res = list(ex.map(lambda j: one(a, j), JOBS))
    bad = [r for r in res if not r["ok"]]
    for r in bad:
        print("FAIL", r["job"], r["sha"], r["rust"])
    print(f"{len(res) - len(bad)}/{len(res)} PASS")
    if a.write_cases:
        for r in res:
            if not r["ok"]:
                continue
            sc, t, hz, st, mac = r["job"]
            cid = (f"video-maclc-{sc}-{vd.TYPE_NAME[t]}" if mac else f"video-style{st}-{sc}-{vd.TYPE_NAME[t]}") + ("-pal" if hz == 50 else "")
            rel = f"inputs/video/{sc}.scene"
            sha = vd.sha256_file(os.path.join(ROOT, "tests", "conformance", rel))
            cmd = (f"audetto-oracle --model {r['model']} --frames {r['frames']} --out /dev/null --cards none --fb-out OUT "
                   f"--scene tests/conformance/{rel} --video-type {t} --video-hz {hz} --rgb-card 0 --video-style {st}{' --mac-lc-dlgr 1' if mac else ''} => "
                   f"fb_sha256_full={r['sha']} (fork audetto a1e20c0, AppleWin 3e8054b, -fno-strict-aliasing; regenerar: "
                   f"tools/conformance/oracle/video-style-differential.py --write-cases)")
            toml = (f'id = "{cid}"\nstory = "HU-0201"\n'
                    f'criterion = "O framebuffer (600x420) da cena {sc} em {vd.TYPE_NAME[t]} a {hz} Hz com VideoStyle={st} '
                    f'({"50% scan lines" if st & 1 else "sem 50% scan lines"}{", vertical blend" if st & 2 else ""}{", quirk DLGR do Mac LC (GH#1258)" if mac else ""}), após {r["frames"]} quadros, é idêntico bit a bit ao do AppleWin"\n'
                    f'model = "{vd.MODEL[r["model"]]}"\n\n[run]\nframes = {r["frames"]}\n\n'
                    f'[video]\ntype = "{vd.TYPE_NAME[t]}"\nhz = {hz}\nrgb_card = "video7-sl7"\nstyle = {st}\n{'mac_lc_dlgr = true' + chr(10) if mac else ''}region = "full"\n'
                    f'scene = "{rel}"\nscene_sha256 = "{sha}"\n\n'
                    f'[oracle]\nkind = "framebuffer-hash"\nauthority = "applewin"\nsha256 = "{r["sha"]}"\nsource = "{cmd}"\n')
            open(os.path.join(a.write_cases, cid + ".toml"), "w").write(toml)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
