#!/usr/bin/env python3
"""Envia WM_DELETE_WINDOW (o clique no X da janela) a janelas X11 cujo titulo contem um texto.
Usado nos smokes com GUI, sem xdotool/wmctrl. Uso: x11-close.py <texto-do-titulo> [--list]
Codigo de saida: 0 = enviou a pelo menos uma janela; 1 = nenhuma encontrada."""
import ctypes, ctypes.util, sys

x = ctypes.CDLL(ctypes.util.find_library("X11"))
x.XOpenDisplay.restype = ctypes.c_void_p
x.XDefaultRootWindow.restype = ctypes.c_ulong
x.XInternAtom.restype = ctypes.c_ulong
x.XFetchName.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_char_p)]
x.XQueryTree.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong),
                         ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)),
                         ctypes.POINTER(ctypes.c_uint)]
x.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.c_void_p]

class ClientMessage(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int),
                ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong),
                ("format", ctypes.c_int), ("data", ctypes.c_long * 5)]

class XEvent(ctypes.Union):
    _fields_ = [("cm", ClientMessage), ("pad", ctypes.c_long * 24)]

needle = sys.argv[1]
dpy = x.XOpenDisplay(None)
if not dpy:
    sys.exit("sem DISPLAY X11")
root = x.XDefaultRootWindow(dpy)
proto = x.XInternAtom(dpy, b"WM_PROTOCOLS", 0)
delete = x.XInternAtom(dpy, b"WM_DELETE_WINDOW", 0)

def walk(w):
    r, p = ctypes.c_ulong(), ctypes.c_ulong()
    kids, n = ctypes.POINTER(ctypes.c_ulong)(), ctypes.c_uint()
    if not x.XQueryTree(dpy, w, ctypes.byref(r), ctypes.byref(p), ctypes.byref(kids), ctypes.byref(n)):
        return
    for i in range(n.value):
        yield kids[i]
        yield from walk(kids[i])

class ClassHint(ctypes.Structure):
    _fields_ = [("res_name", ctypes.c_char_p), ("res_class", ctypes.c_char_p)]

def big(w):  # ignora janelas de 1x1 (ex.: "Default IME" do Wine)
    buf = (ctypes.c_int * 64)()
    x.XGetWindowAttributes(dpy, w, buf)
    return buf[2] > 50 and buf[3] > 50

sent = 0
for w in walk(root):
    name = ctypes.c_char_p()
    hit = x.XFetchName(dpy, w, ctypes.byref(name)) and name.value and needle.encode() in name.value
    if not hit:  # Wine nao poe WM_NAME: casa pela classe (WM_CLASS) do processo
        ch = ClassHint()
        if x.XGetClassHint(dpy, w, ctypes.byref(ch)) and ch.res_class and needle.encode().lower() in ch.res_class.lower():
            hit = big(w)
            name = ctypes.c_char_p(ch.res_class)
    if hit:
        print(f"janela 0x{w:x}: {name.value.decode(errors='replace')}")
        if "--list" in sys.argv:
            continue
        ev = XEvent()
        ev.cm.type, ev.cm.window, ev.cm.message_type, ev.cm.format = 33, w, proto, 32
        ev.cm.data[0] = delete
        x.XSendEvent(dpy, w, 0, 0, ctypes.byref(ev))
        sent += 1
x.XFlush(dpy)
sys.exit(0 if sent or "--list" in sys.argv else 1)
