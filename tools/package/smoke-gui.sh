#!/usr/bin/env bash
# Abre a GUI por alguns segundos, fecha via WM_DELETE_WINDOW e confere codigo de saida 0.
#   tools/package/smoke-gui.sh <rotulo> <cmd...>      (variaveis de ambiente herdadas)
# Ex.: DISPLAY=:0 WINIT_UNIX_BACKEND=x11 tools/package/smoke-gui.sh x11 target/release/apple2-rust -power-on
# Requer sessao X11 (ou XWayland) e python3; sem xdotool/wmctrl.
set -uo pipefail
label=$1; shift
here=$(cd "$(dirname "$0")" && pwd)
log=$(mktemp)
"$@" >"$log" 2>&1 &
pid=$!
sleep "${GUI_SECONDS:-6}"
if ! kill -0 "$pid" 2>/dev/null; then
  wait "$pid"; rc=$?; echo "FAIL  $label: terminou sozinho (rc=$rc)"; tail -5 "$log"; exit 1
fi
python3 -I "$here/x11-close.py" "${GUI_TITLE:-AppleII-Rust}" >/dev/null || { echo "FAIL  $label: janela nao encontrada"; tail -5 "$log"; kill "$pid"; exit 1; }
for _ in $(seq 1 20); do kill -0 "$pid" 2>/dev/null || break; sleep 0.5; done
if kill -0 "$pid" 2>/dev/null; then echo "FAIL  $label: nao fechou"; kill -9 "$pid"; exit 1; fi
wait "$pid"; rc=$?
if [ $rc -eq 0 ]; then echo "PASS  $label: abriu, fechou limpo, rc=0"; else echo "FAIL  $label: rc=$rc"; tail -5 "$log"; exit 1; fi
