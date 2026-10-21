#!/usr/bin/env python3
"""Framebuffer differential: AppleWin oracle (audetto-oracle --fb-out) x Rust (conformance scene-fb).

Policy (docs/conformance/test-oracles.md, "Vídeo"): every pair must be bit-identical
(max channel difference 0). MAE and the diff bounding box are diagnostics only.

usage: video-differential.py [--oracle BIN] [--conformance BIN] [--out DIR] [--jobs N]
                             [--scene GLOB] [--types 0,1,...] [--hz 60,50] [--cards 0,1,2]
                             [--selfcheck] [--write-cases DIR]

--selfcheck    calibration: runs the oracle twice per job and requires identical SHA-256
--write-cases  after a fully passing run, writes tests/conformance/cases/video-*.toml (framebuffer-hash,
               authority applewin) for the committed selection (see CASE_SELECTION)
"""
import argparse, concurrent.futures as cf, fnmatch, glob, json, os, re, subprocess, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
MODEL = {"apple2ee": "apple2e-enhanced", "apple2e": "apple2e", "apple2p": "apple2plus", "apple2": "apple2"}
TYPE_NAME = {0: "mono-custom", 1: "color-idealized", 2: "color-rgb", 3: "color-monitor", 4: "color-tv",
             5: "mono-tv", 6: "mono-amber", 7: "mono-green", 8: "mono-white"}
CARDS = {0: "video7-sl7", 1: "le-chat-mauve-eve", 2: "le-chat-mauve-feline"}


def scene_info(path):
    info = {"model": "apple2ee"}
    for line in open(path):
        if line.startswith("model "):
            info["model"] = line.split()[1]
        if line.startswith("main") or line.startswith("entry"):
            break
    return info


def frames_for(name):
    return [4, 20] if name.startswith(("text", "flash", "rgb-text")) else [4]


SCENE_STORY = "HU-0201"
VIDHD_SLOT = 2
# Legacy scenes also run with a VidHD installed (centred in the larger framebuffer).
VIDHD_LEGACY = ("text40", "text80", "lores-mixed", "hires-bars", "dhires-noise", "dlores", "midline-hgr-text-24")
# (scene names or globs, types, hz, rgb cards): the cases committed to tests/conformance/cases.
CASE_SELECTION = [
    (("*",), [4], [60], [0], 0),  # every scene and ROM boot on Color TV (the default type)
    (("text40", "text80", "lores-mixed", "hires-bars", "hires-noise", "dhires-noise", "dlores",
      "midline-hgr-text-24"), [0, 1, 2, 3, 5, 6, 7, 8], [60], [0], 0),  # every Tipo de vídeo
    (("text80", "hires-bars", "dhires-noise", "lores", "hires-mixed", "midline-hgr-text-24",
      "midline-1443-hgr-then-text", "midline-1443-text-then-hgr"), [1, 2, 3, 4, 5], [50], [0], 0),  # PAL
    (("rgb-*",), [2], [60], [1, 2], 0),  # the other RGB cards
    # one regression scene per upstream issue (docs/conformance/results/video.csv)
    (("issue-*",), [1, 3], [60], [0], 0),
    (("issue-0636-*", "issue-0643-*", "issue-0828-*", "issue-0850-*", "issue-0851-*", "issue-1018-*", "issue-1435-*"), [2], [60], [0, 1, 2], 0),
    (("issue-0082-*", "issue-0241-*", "issue-0516-*", "issue-1386-*"), [3, 4], [50], [0], 0),
    # VidHD (HU-0202): legacy modes centred in the 640x400 framebuffer, and Super Hi-Res
    (VIDHD_LEGACY, [1, 3, 4, 5, 8], [60], [0], 2),
    (("shr-*",), [0, 1, 3, 4, 5, 8], [60], [0], 2),
]


def sha256_file(path):
    import hashlib
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def write_cases(a, results):
    """One TOML per selected (scene, type, hz, card, frames) whose run passed. A selected run that did
    not pass (a known divergence, see docs/conformance/audit/video-differential.md) gets no case."""
    os.makedirs(a.write_cases, exist_ok=True)
    n = 0
    for r in results:
        if r["result"] != "PASS":
            continue
        sel = False
        for names, types, hzs, cards, vh in CASE_SELECTION:
            if any(fnmatch.fnmatch(r["scene"], g) for g in names) and r["type"] in types and r["hz"] in hzs \
                    and r["rgb_card"] in cards and r["vidhd"] == vh:
                sel = True
        if not sel:
            continue
        boot = r["scene"].startswith(("boot-", "boot0-"))
        scene_rel = None if boot else f"inputs/video/{r['scene']}.scene"
        scene_sha = None if boot else sha256_file(os.path.join(ROOT, "tests", "conformance", scene_rel))
        scene_arg = "" if boot else "--scene tests/conformance/inputs/video/" + r["scene"] + ".scene "
        cid = f"video-{r['scene']}-{TYPE_NAME[r['type']]}"
        if r["hz"] == 50:
            cid += "-pal"
        if r["vidhd"]:
            cid += "-vidhd"
        if r["type"] == 2 and r["rgb_card"]:
            cid += "-" + CARDS[r["rgb_card"]]
        if r["frames"] != 4:
            cid += f"-f{r['frames']}"
        card_txt = f" com a carta {CARDS[r['rgb_card']]}" if r["type"] == 2 else ""
        cmd = (f"audetto-oracle --model {r['model']} --frames {r['frames']} --out /dev/null --cards none "
               f"--fb-out OUT {'--memclear 0 ' if r['scene'].startswith('boot0-') else ''}{scene_arg}--video-type {r['type']} "
               f"--video-hz {r['hz']} --rgb-card {r['rgb_card']}{' --vidhd ' + str(r['vidhd']) if r['vidhd'] else ''} => fb_sha256_full={r['oracle_full']} "
               f"(fork audetto a1e20c0, AppleWin 3e8054b, compilado com -fno-strict-aliasing; "
               f"regenerar: tools/conformance/oracle/video-differential.py --write-cases)")
        what = (f"do boot da ROM ({r['model']}, sem disco{', RAM zerada' if r['scene'].startswith('boot0-') else ''})"
                if boot else f"da cena {r['scene']}")
        crit = (f"O framebuffer (600x420, bordas incluídas) {what} em {TYPE_NAME[r['type']]} "
                f"a {r['hz']} Hz{card_txt}{' com um VidHD (framebuffer 640x400)' if r['vidhd'] else ''}, após {r['frames']} quadros, é idêntico bit a bit ao do AppleWin")
        toml = (f'id = "{cid}"\n'
                f'story = "{SCENE_STORY}"\n'
                f'criterion = "{crit}"\n'
                f'model = "{MODEL[r["model"]]}"\n' + ("memclear = 0\n" if r["scene"].startswith("boot0-") else "") + "\n"
                f'[run]\nframes = {r["frames"]}\n\n'
                f'[video]\ntype = "{TYPE_NAME[r["type"]]}"\nhz = {r["hz"]}\nrgb_card = "{CARDS[r["rgb_card"]]}"\n'
                f'region = "full"\n' + (f"vidhd_slot = {r['vidhd']}\n" if r["vidhd"] else "") + ("" if boot else f'scene = "{scene_rel}"\nscene_sha256 = "{scene_sha}"\n') + "\n"
                f'[oracle]\nkind = "framebuffer-hash"\nauthority = "applewin"\nsha256 = "{r["oracle_full"]}"\n'
                f'source = "{cmd}"\n')
        open(os.path.join(a.write_cases, cid + ".toml"), "w").write(toml)
        n += 1
    print(f"wrote {n} cases to {a.write_cases}", file=sys.stderr)


def parse_kv(out):
    return dict(l.split("=", 1) for l in out.splitlines() if l.startswith("fb_"))


def run_oracle(a, scene, model, frames, vt, hz, card, vidhd, prefix, memclear=None):
    cmd = [a.oracle, "--model", model, "--frames", str(frames), "--out", "/dev/null", "--cards", "none",
           "--fb-out", prefix] + (["--scene", scene] if scene else []) + [
           "--video-type", str(vt), "--video-hz", str(hz), "--rgb-card", str(card)] + (
           ["--vidhd", str(vidhd)] if vidhd else []) + (["--memclear", str(memclear)] if memclear is not None else [])
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        return None, " ".join(cmd) + "\n" + r.stderr
    return parse_kv(r.stdout), " ".join(cmd)


def run_rust(a, scene, model, frames, vt, hz, card, vidhd, prefix, memclear=None):
    cmd = [a.conformance, "scene-fb"] + (["--scene", scene] if scene else []) + [
           "--model", MODEL[model], "--frames", str(frames),
           "--type", str(vt), "--hz", str(hz), "--rgb-card", str(card)] + (
           ["--vidhd", str(vidhd)] if vidhd else []) + (
           ["--memclear", str(memclear)] if memclear is not None else []) + ["--out", prefix]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        return None, r.stderr
    return parse_kv(r.stdout), " ".join(cmd)


def diff_pixels(pa, pb, size, bw=20, bh=18):
    w, h = size
    A, B = open(pa, "rb").read(), open(pb, "rb").read()
    if len(A) != len(B):
        return {"size_mismatch": [len(A), len(B)]}
    n = 0
    maxd = 0
    sum_abs = [0, 0, 0]
    minx = miny = 10**9
    maxx = maxy = -1
    first = None
    regions = {"border": 0, "visible": 0}
    for i in range(0, len(A), 4):
        if A[i:i + 4] != B[i:i + 4]:
            n += 1
            x, y = (i // 4) % w, (i // 4) // w
            for c in range(3):
                d = abs(A[i + c] - B[i + c])
                sum_abs[c] += d
                maxd = max(maxd, d)
            minx, maxx, miny, maxy = min(minx, x), max(maxx, x), min(miny, y), max(maxy, y)
            if first is None:
                first = (x, y, list(A[i:i + 4]), list(B[i:i + 4]))
            inside = bw <= x < w - bw and bh <= y < h - bh
            regions["visible" if inside else "border"] += 1
    return {"differing_pixels": n, "max_channel_diff": maxd,
            "mae_rgb": [round(s / (w * h), 5) for s in sum_abs],
            "bbox": [minx, miny, maxx, maxy] if n else None, "first": first, "regions": regions}


def one(a, job):
    scene, name, model, frames, vt, hz, card, vidhd = job
    tag = f"{name}.f{frames}.t{vt}.h{hz}.c{card}.v{vidhd}"
    po, pr = os.path.join(a.out, "ora", tag), os.path.join(a.out, "rust", tag)
    memclear = 0 if name.startswith("boot0-") else None
    o, ocmd = run_oracle(a, scene, model, frames, vt, hz, card, vidhd, po, memclear)
    r, rcmd = run_rust(a, scene, model, frames, vt, hz, card, vidhd, pr, memclear)
    if a.selfcheck and o is not None:
        o2, _ = run_oracle(a, scene, model, frames, vt, hz, card, vidhd, po + ".again", memclear)
        if os.path.exists(po + ".again.rgba"):
            os.remove(po + ".again.rgba")
        if o2 is None or o2["fb_sha256_full"] != o["fb_sha256_full"]:
            return {"scene": name, "model": model, "frames": frames, "type": vt, "hz": hz, "rgb_card": card,
                    "vidhd": vidhd, "oracle_cmd": ocmd, "rust_cmd": rcmd, "result": "FAIL",
                    "error": "oracle not deterministic"}
    res = {"scene": name, "model": model, "frames": frames, "type": vt, "hz": hz, "rgb_card": card,
           "vidhd": vidhd, "oracle_cmd": ocmd, "rust_cmd": rcmd}
    if o is None or r is None:
        res.update(result="BLOCKED", error=(ocmd if o is None else rcmd))
        return res
    res.update(oracle_visible=o["fb_sha256_visible"], oracle_full=o["fb_sha256_full"],
               rust_visible=r["fb_sha256_visible"], rust_full=r["fb_sha256_full"],
               oracle_cycles=o.get("fb_cycles"), rust_cycles=r.get("fb_cycles"))
    same = o["fb_sha256_full"] == r["fb_sha256_full"]
    res["result"] = "PASS" if same else "FAIL"
    if same:
        for p in (po, pr):
            os.remove(p + ".rgba")
    else:
        size = tuple(int(v) for v in o["fb_size"].split("x"))
        res["diff"] = diff_pixels(po + ".rgba", pr + ".rgba", size)
    return res


def main():
    ap = argparse.ArgumentParser()
    cache = os.environ.get("ORACLE_CACHE", os.path.join(ROOT, "target", "conformance-cache"))
    ap.add_argument("--oracle", default=os.path.join(cache, "oracle", "build", "audetto-oracle"))
    ap.add_argument("--conformance", default=os.path.join(ROOT, "target", "release", "conformance"))
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "video-differential"))
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--scene", default="*")
    ap.add_argument("--types", default="0,1,2,3,4,5,6,7,8")
    ap.add_argument("--hz", default="60,50")
    ap.add_argument("--cards", default="0", help="RGB card numbers (only tried with type 2)")
    ap.add_argument("--frames", default="", help="override frame counts, comma separated")
    ap.add_argument("--selfcheck", action="store_true")
    ap.add_argument("--write-cases", default="")
    a = ap.parse_args()
    os.makedirs(os.path.join(a.out, "ora"), exist_ok=True)
    os.makedirs(os.path.join(a.out, "rust"), exist_ok=True)
    jobs = []
    for path in sorted(glob.glob(os.path.join(ROOT, "tests", "conformance", "inputs", "video", "*.scene"))):
        name = os.path.basename(path)[:-6]
        if not fnmatch.fnmatch(name, a.scene):
            continue
        model = scene_info(path)["model"]
        for vidhd in ([VIDHD_SLOT] if name.startswith("shr-") else [0] + ([VIDHD_SLOT] if name in VIDHD_LEGACY else [])):
            for hz in map(int, a.hz.split(",")):
                for vt in map(int, a.types.split(",")):
                    if hz == 50 and (vt not in (1, 2, 3, 4, 5) or vidhd):
                        continue
                    cards = [int(c) for c in a.cards.split(",")] if vt == 2 else [0]
                    for card in cards:
                        fr = [int(f) for f in a.frames.split(",")] if a.frames else frames_for(name)
                        for f in fr:
                            jobs.append((path, name, model, f, vt, hz, card, vidhd))
    # ROM boot (no scene): the power-on screen with the cursor flashing, per model.
    # "boot-" uses the AppleWin default RAM pattern (FF FF 00 00); "boot0-" runs both sides with
    # --memclear 0 (all RAM zero).
    if a.scene in ("*", "boot*"):
        for model, prefix in [(m, p) for p in ("boot", "boot0") for m in ("apple2ee", "apple2e", "apple2p", "apple2")]:
            for hz in map(int, a.hz.split(",")):
                for vt in map(int, a.types.split(",")):
                    if hz == 50 and vt not in (1, 2, 3, 4, 5):
                        continue
                    for f in (30, 120):
                        jobs.append((None, f"{prefix}-{model}", model, f, vt, hz, 0, 0))
    print(f"{len(jobs)} runs", file=sys.stderr)
    results = []
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        for i, r in enumerate(ex.map(lambda j: one(a, j), jobs)):
            results.append(r)
            if r["result"] != "PASS":
                d = r.get("diff", {})
                print(f"{r['result']} {r['scene']} frames={r['frames']} type={r['type']} hz={r['hz']} "
                      f"card={r['rgb_card']} vidhd={r['vidhd']} {d.get('differing_pixels', '')} px max={d.get('max_channel_diff', '')} "
                      f"bbox={d.get('bbox', '')}", flush=True)
    json.dump(results, open(os.path.join(a.out, "results.json"), "w"), indent=1)
    if a.write_cases:
        write_cases(a, results)
    from collections import Counter
    print(dict(Counter(r["result"] for r in results)))
    return 0 if all(r["result"] == "PASS" for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
