#!/usr/bin/env python3
"""Decodes AppleWin resource/CHARSET{82,8M,8C}.bmp into the `csbits` layout used by
NTSC_CharSet.cpp `get_csbits_xy` (:50): per char 8 bytes, bit 0 = leftmost of 7 dots.
Usage: gen_pravets_charsets.py <AppleWin/resource dir> <output data dir>"""
import struct, sys


def load(path):
    d = open(path, "rb").read()
    off, = struct.unpack_from("<I", d, 10)
    w, h = struct.unpack_from("<ii", d, 18)
    stride = ((w + 31) // 32) * 4
    return [d[off + (h - 1 - y) * stride: off + (h - y) * stride] for y in range(h)]  # top-down


def decode(rows, cy0):
    out = bytearray()
    for ch in range(256):
        cy, cx = cy0 + ch // 16, ch % 16
        for y in range(8):
            row = rows[cy * 16 + y * 2]
            n = 0
            for x in range(0, 14, 2):
                px = cx * 16 + x
                bit = (row[px // 8] >> (7 - px % 8)) & 1
                n = (n >> 1) | (0x80 if bit else 0)
            out.append(n)
    return bytes(out)


src, dst = sys.argv[1], sys.argv[2]
for name, out, sets in (("82", "pravets82", [0]), ("8M", "pravets8m", [0]), ("8C", "pravets8c", [0, 16])):
    rows = load(f"{src}/CHARSET{name}.bmp")
    open(f"{dst}/{out}.bin", "wb").write(b"".join(decode(rows, s) for s in sets))
