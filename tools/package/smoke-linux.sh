#!/usr/bin/env bash
# Smoke Linux nativo: headless, GUI (X11/XWayland e Xvfb), sem audio, HiDPI, tela cheia.
#   tools/package/smoke-linux.sh [binario]
# Wayland nativo: o fechamento por protocolo nao e automatizavel aqui (roteiro manual).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
exe=${1:-target/release/apple2-rust}
gui=$here/smoke-gui.sh
fail=0
run() { "$@" || fail=1; }

# headless em todos os modelos, deterministico (-memclear 0)
for m in apple2 apple2p apple2jp apple2e apple2ee; do
  if "$exe" -model "$m" -memclear 0 --headless-frames 300 --dump-screen "/tmp/smoke-$m.png" >/dev/null 2>&1 \
     && [ -s "/tmp/smoke-$m.png" ]; then echo "PASS  headless $m rc=0"; else echo "FAIL  headless $m"; fail=1; fi
done

# GUI via XWayland da sessao (se houver DISPLAY)
if [ -n "${DISPLAY:-}" ]; then
  run env -u WAYLAND_DISPLAY WINIT_UNIX_BACKEND=x11 "$gui" "x11/xwayland" "$exe" -power-on
  run env -u WAYLAND_DISPLAY WINIT_UNIX_BACKEND=x11 WINIT_X11_SCALE_FACTOR=2 "$gui" "hidpi x2 (WINIT_X11_SCALE_FACTOR=2)" "$exe" -power-on
  run env -u WAYLAND_DISPLAY WINIT_UNIX_BACKEND=x11 "$gui" "tela cheia (-f)" "$exe" -f -power-on
  # sem dispositivo de audio: ALSA aponta para uma configuracao vazia
  echo 'pcm.!default { type null }' >/tmp/smoke-alsa-null.conf
  run env -u WAYLAND_DISPLAY WINIT_UNIX_BACKEND=x11 ALSA_CONFIG_PATH=/dev/null "$gui" "audio ausente (ALSA_CONFIG_PATH=/dev/null)" "$exe" -power-on
fi

# Xvfb isolado (sem GPU real, sem Wayland)
if command -v Xvfb >/dev/null; then
  run env -u WAYLAND_DISPLAY WINIT_UNIX_BACKEND=x11 xvfb-run -a "$gui" "Xvfb" "$exe" -power-on
fi
exit $fail
