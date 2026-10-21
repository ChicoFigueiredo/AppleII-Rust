#!/usr/bin/env python3
"""Minimal two-pass 6502 assembler for the audio conformance programs (tests/conformance/audio/).

Only what the programs use: NMOS 6502 official opcodes, labels, `.org`, `.byte`, `.word`, `.fill`,
`NAME = expr`, `<expr` / `>expr`, `+`/`-` in expressions, `*` as the current address.
Usage: asm6502.py prog.s -o prog.bin [-D NAME=VALUE ...]   (the output starts at the first `.org`)
"""
import argparse
import re
import sys

IMPLIED = {
    "BRK": 0x00, "CLC": 0x18, "CLD": 0xD8, "CLI": 0x58, "CLV": 0xB8, "DEX": 0xCA, "DEY": 0x88,
    "INX": 0xE8, "INY": 0xC8, "NOP": 0xEA, "PHA": 0x48, "PHP": 0x08, "PLA": 0x68, "PLP": 0x28,
    "RTI": 0x40, "RTS": 0x60, "SEC": 0x38, "SED": 0xF8, "SEI": 0x78, "TAX": 0xAA, "TAY": 0xA8,
    "TSX": 0xBA, "TXA": 0x8A, "TXS": 0x9A, "TYA": 0x98,
}
ACCUM = {"ASL": 0x0A, "LSR": 0x4A, "ROL": 0x2A, "ROR": 0x6A}
BRANCH = {"BPL": 0x10, "BMI": 0x30, "BVC": 0x50, "BVS": 0x70, "BCC": 0x90, "BCS": 0xB0, "BNE": 0xD0, "BEQ": 0xF0}
# mnemonic -> {mode: opcode}; modes: imm zp zpx zpy abs abx aby izx izy
GROUP = {
    "ORA": dict(imm=0x09, zp=0x05, zpx=0x15, abs=0x0D, abx=0x1D, aby=0x19, izx=0x01, izy=0x11),
    "AND": dict(imm=0x29, zp=0x25, zpx=0x35, abs=0x2D, abx=0x3D, aby=0x39, izx=0x21, izy=0x31),
    "EOR": dict(imm=0x49, zp=0x45, zpx=0x55, abs=0x4D, abx=0x5D, aby=0x59, izx=0x41, izy=0x51),
    "ADC": dict(imm=0x69, zp=0x65, zpx=0x75, abs=0x6D, abx=0x7D, aby=0x79, izx=0x61, izy=0x71),
    "STA": dict(zp=0x85, zpx=0x95, abs=0x8D, abx=0x9D, aby=0x99, izx=0x81, izy=0x91),
    "LDA": dict(imm=0xA9, zp=0xA5, zpx=0xB5, abs=0xAD, abx=0xBD, aby=0xB9, izx=0xA1, izy=0xB1),
    "CMP": dict(imm=0xC9, zp=0xC5, zpx=0xD5, abs=0xCD, abx=0xDD, aby=0xD9, izx=0xC1, izy=0xD1),
    "SBC": dict(imm=0xE9, zp=0xE5, zpx=0xF5, abs=0xED, abx=0xFD, aby=0xF9, izx=0xE1, izy=0xF1),
    "LDX": dict(imm=0xA2, zp=0xA6, zpy=0xB6, abs=0xAE, aby=0xBE),
    "LDY": dict(imm=0xA0, zp=0xA4, zpx=0xB4, abs=0xAC, abx=0xBC),
    "STX": dict(zp=0x86, zpy=0x96, abs=0x8E),
    "STY": dict(zp=0x84, zpx=0x94, abs=0x8C),
    "CPX": dict(imm=0xE0, zp=0xE4, abs=0xEC),
    "CPY": dict(imm=0xC0, zp=0xC4, abs=0xCC),
    "BIT": dict(zp=0x24, abs=0x2C),
    "INC": dict(zp=0xE6, zpx=0xF6, abs=0xEE, abx=0xFE),
    "DEC": dict(zp=0xC6, zpx=0xD6, abs=0xCE, abx=0xDE),
    "ASL": dict(zp=0x06, zpx=0x16, abs=0x0E, abx=0x1E),
    "LSR": dict(zp=0x46, zpx=0x56, abs=0x4E, abx=0x5E),
    "ROL": dict(zp=0x26, zpx=0x36, abs=0x2E, abx=0x3E),
    "ROR": dict(zp=0x66, zpx=0x76, abs=0x6E, abx=0x7E),
    "JMP": dict(abs=0x4C, ind=0x6C),
    "JSR": dict(abs=0x20),
}


def strip(line):
    out, in_str = [], False
    for ch in line:
        if ch == '"':
            in_str = not in_str
        if ch == ";" and not in_str:
            break
        out.append(ch)
    return "".join(out).strip()


class Asm:
    def __init__(self, defs):
        self.sym = dict(defs)
        self.pc = 0
        self.org = None
        self.out = bytearray()
        self.pass_no = 0

    def num(self, tok):
        tok = tok.strip()
        if tok == "*":
            return self.pc
        if tok.startswith("$"):
            return int(tok[1:], 16)
        if tok.startswith("%"):
            return int(tok[1:], 2)
        if re.fullmatch(r"\d+", tok):
            return int(tok)
        if len(tok) == 3 and tok[0] == "'" and tok[2] == "'":
            return ord(tok[1])
        if tok in self.sym:
            return self.sym[tok]
        if self.pass_no == 0:
            return 0
        raise SystemExit(f"unknown symbol {tok!r}")

    def expr(self, e):
        e = e.strip()
        if e.startswith("<"):
            return self.expr(e[1:]) & 0xFF
        if e.startswith(">"):
            return (self.expr(e[1:]) >> 8) & 0xFF
        parts = re.split(r"([+-])", e)
        total = self.num(parts[0]) if parts[0].strip() else 0
        for i in range(1, len(parts), 2):
            v = self.num(parts[i + 1])
            total = total + v if parts[i] == "+" else total - v
        return total

    def emit(self, *bs):
        for b in bs:
            if self.pass_no == 1:
                self.out.append(b & 0xFF)
            self.pc += 1

    def operand(self, mn, arg):
        """Returns (mode, value)."""
        a = arg.strip()
        if a.startswith("#"):
            return "imm", self.expr(a[1:])
        m = re.fullmatch(r"\((.+)\),\s*[Yy]", a)
        if m:
            return "izy", self.expr(m.group(1))
        m = re.fullmatch(r"\((.+),\s*[Xx]\)", a)
        if m:
            return "izx", self.expr(m.group(1))
        m = re.fullmatch(r"\((.+)\)", a)
        if m:
            return "ind", self.expr(m.group(1))
        m = re.fullmatch(r"(.+),\s*([XxYy])", a)
        if m:
            v, r = self.expr(m.group(1)), m.group(2).lower()
            zp = v < 0x100 and self.known(m.group(1))
            if r == "x":
                return ("zpx" if zp and "zpx" in GROUP[mn] else "abx"), v
            return ("zpy" if zp and "zpy" in GROUP[mn] else "aby"), v
        v = self.expr(a)
        zp = v < 0x100 and self.known(a) and "zp" in GROUP[mn]
        return ("zp" if zp else "abs"), v

    def known(self, e):
        # zero page is only chosen for values known in pass 0 (no forward refs), to keep sizes stable
        e = e.strip().lstrip("<>")
        return bool(re.fullmatch(r"\$[0-9A-Fa-f]{1,2}|\d{1,3}", e)) or (e in self.sym and self.sym[e] < 0x100)

    def line(self, raw):
        s = strip(raw)
        if not s:
            return
        m = re.match(r"^([A-Za-z_][\w]*)\s*=\s*(.+)$", s)
        if m:
            if m.group(1) not in self.defs_fixed:
                self.sym[m.group(1)] = self.expr(m.group(2))
            return
        m = re.match(r"^([A-Za-z_][\w]*):\s*(.*)$", s)
        if m:
            self.sym[m.group(1)] = self.pc
            s = m.group(2).strip()
            if not s:
                return
        parts = s.split(None, 1)
        mn, arg = parts[0].upper(), (parts[1] if len(parts) > 1 else "")
        if mn == ".ORG":
            self.pc = self.expr(arg)
            if self.org is None:
                self.org = self.pc
            return
        if mn == ".BYTE":
            for item in re.split(r",(?=(?:[^\"]*\"[^\"]*\")*[^\"]*$)", arg):
                item = item.strip()
                if item.startswith('"'):
                    for ch in item.strip('"'):
                        self.emit(ord(ch))
                else:
                    self.emit(self.expr(item))
            return
        if mn == ".WORD":
            for item in arg.split(","):
                v = self.expr(item)
                self.emit(v & 0xFF, v >> 8)
            return
        if mn == ".FILL":
            n, v = (arg.split(",") + ["0"])[:2]
            for _ in range(self.expr(n)):
                self.emit(self.expr(v))
            return
        if mn in IMPLIED and not arg:
            self.emit(IMPLIED[mn])
            return
        if mn in ACCUM and (not arg or arg.strip().upper() == "A"):
            self.emit(ACCUM[mn])
            return
        if mn in BRANCH:
            target = self.expr(arg)
            off = target - (self.pc + 2)
            if self.pass_no == 1 and not -128 <= off <= 127:
                raise SystemExit(f"branch out of range at {self.pc:04X}: {raw.strip()}")
            self.emit(BRANCH[mn], off)
            return
        if mn in GROUP:
            mode, v = self.operand(mn, arg)
            if mode not in GROUP[mn]:
                raise SystemExit(f"{mn} does not support {mode}: {raw.strip()}")
            op = GROUP[mn][mode]
            if mode in ("imm", "zp", "zpx", "zpy", "izx", "izy"):
                self.emit(op, v)
            else:
                self.emit(op, v & 0xFF, v >> 8)
            return
        raise SystemExit(f"cannot assemble: {raw.strip()}")

    def run(self, lines, defs):
        self.defs_fixed = set(defs)
        for self.pass_no in (0, 1):
            self.pc, self.out, self.org = 0, bytearray(), None
            for raw in lines:
                self.line(raw)
        return self.org or 0, bytes(self.out)


def expand(path):
    """Source lines with `.include "file"` expanded (relative to the including file)."""
    import os

    out = []
    for raw in open(path).read().splitlines():
        m = re.match(r'^\s*\.include\s+"([^"]+)"', raw, re.I)
        if m:
            out += expand(os.path.join(os.path.dirname(path), m.group(1)))
        else:
            out.append(raw)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("-o", required=True)
    ap.add_argument("-D", action="append", default=[])
    ns = ap.parse_args()
    defs = {}
    for d in ns.D:
        k, v = d.split("=", 1)
        defs[k] = int(v, 0)
    asm = Asm(defs)
    org, data = asm.run(expand(ns.src), defs)
    open(ns.o, "wb").write(data)
    print(f"{ns.src}: org ${org:04X}, {len(data)} bytes", file=sys.stderr)


if __name__ == "__main__":
    main()
