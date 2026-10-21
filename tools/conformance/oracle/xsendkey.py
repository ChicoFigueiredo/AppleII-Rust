#!/usr/bin/env python3
"""Envia teclas a uma janela X11 via XTest (ctypes), sem depender de xdotool.

Uso: xsendkey.py "<trecho do titulo da janela>" Return [F11 ...]

Usa libX11 + libXtst (presentes em Ubuntu padrao). Localiza a janela por
`xwininfo -root -tree`, foca com XSetInputFocus e injeta KeyPress/KeyRelease.
"""
import ctypes
import re
import subprocess
import sys
import time

X11 = ctypes.CDLL("libX11.so.6")
XTST = ctypes.CDLL("libXtst.so.6")
X11.XOpenDisplay.restype = ctypes.c_void_p
X11.XOpenDisplay.argtypes = [ctypes.c_char_p]
X11.XStringToKeysym.restype = ctypes.c_ulong
X11.XStringToKeysym.argtypes = [ctypes.c_char_p]
X11.XKeysymToKeycode.restype = ctypes.c_ubyte
X11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
X11.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
X11.XRaiseWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
X11.XFlush.argtypes = [ctypes.c_void_p]
XTST.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]


def find_window(fragment):
    out = subprocess.run(["xwininfo", "-root", "-tree"], capture_output=True, text=True).stdout
    best = None
    for line in out.splitlines():
        m = re.match(r'\s*(0x[0-9a-f]+) "(.*?)":', line)
        if m and fragment in m.group(2):
            best = int(m.group(1), 16)
    return best


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    win = find_window(sys.argv[1])
    if win is None:
        sys.exit("janela nao encontrada: " + sys.argv[1])
    dpy = X11.XOpenDisplay(None)
    if not dpy:
        sys.exit("sem DISPLAY")
    X11.XRaiseWindow(dpy, win)
    X11.XSetInputFocus(dpy, win, 1, 0)  # RevertToPointerRoot, CurrentTime
    X11.XFlush(dpy)
    time.sleep(0.3)
    for name in sys.argv[2:]:
        ks = X11.XStringToKeysym(name.encode())
        kc = X11.XKeysymToKeycode(dpy, ks)
        if not kc:
            sys.exit("keysym invalido: " + name)
        XTST.XTestFakeKeyEvent(dpy, kc, 1, 0)
        X11.XFlush(dpy)
        time.sleep(0.08)
        XTST.XTestFakeKeyEvent(dpy, kc, 0, 0)
        X11.XFlush(dpy)
        time.sleep(0.15)


main()
