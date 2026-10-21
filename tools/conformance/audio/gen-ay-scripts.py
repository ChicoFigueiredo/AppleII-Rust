#!/usr/bin/env python3
"""Writes the AY core self-test scripts (tests/conformance/audio/ay-core/*.script).

The scripts drive the fork's AY8913 class (AY8910.cpp) in the oracle (`--audio-ay-script`) and the
Rust `Ay` with the same register writes and frame lengths. Deterministic: a fixed LCG, no clock.
Format: see tools/conformance/oracle/audetto-oracle/audio.cpp (ayCoreSelfTest).
"""
import os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "tests", "conformance", "audio", "ay-core")


class Lcg:
    def __init__(self, seed):
        self.s = seed

    def next(self, n):
        self.s = (self.s * 1103515245 + 12345) & 0x7FFFFFFF
        return (self.s >> 8) % n


def script(name, clk, ay_type, frame_sizes, cycles_per_sample, seed, writes_per_frame):
    r = Lcg(seed)
    lines = [f"# {name}: clk {clk}, {ay_type}, seed {seed}", f"clk {clk}", f"type {ay_type}", "reset"]
    # a base patch so the voices are audible from the start
    for reg, val in ((7, 0x38), (8, 15), (9, 12), (10, 9), (0, 0xFE), (1, 1), (2, 0x7F), (3, 0), (4, 0x40), (5, 0)):
        lines.append(f"w {reg} {val} 0")
    for i, n in enumerate(frame_sizes):
        span = int(n * cycles_per_sample)
        ts = sorted(r.next(max(span, 1)) for _ in range(r.next(writes_per_frame + 1)))
        for t in ts:
            kind = r.next(10)
            if kind < 4:
                reg, val = r.next(6), r.next(256)
            elif kind == 4:
                reg, val = 6, r.next(32)
            elif kind == 5:
                reg, val = 7, r.next(64)
            elif kind < 8:
                reg, val = 8 + r.next(3), r.next(32)
            elif kind == 8:
                reg, val = 11 + r.next(2), r.next(256) if r.next(2) else r.next(4)
            else:
                reg, val = 13, r.next(16)
            lines.append(f"w {reg} {val} {t}")
        lines.append(f"frame {n}")
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, name + ".script"), "w") as f:
        f.write("\n".join(lines) + "\n")


# NTSC Mockingboard: 735-sample frames (a 60 Hz period) and the 43-sample chunks MB_Update makes
script("ntsc-ay8913-frames", 1020484, "ay8913", [735] * 90, 23.14, 1, 8)
script("ntsc-ay8913-chunks", 1020484, "ay8913", [43, 44, 43, 43, 44] * 60, 23.14, 2, 3)
# Phasor native mode: the AY runs at twice the base clock
script("phasor-ay8912-2x", 2040968, "ay8912", [367, 368, 367] * 50, 23.14, 3, 4)
# PAL clock, AY-3-8910, slow envelopes and noise
script("pal-ay8910-mixed", 1015657, "ay8910", [441, 735, 22, 1000, 300] * 30, 23.03, 4, 6)
# YM2149F
script("ntsc-ym2149", 1020484, "ym2149", [512] * 80, 23.14, 5, 5)
