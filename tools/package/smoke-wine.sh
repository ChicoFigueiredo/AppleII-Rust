#!/usr/bin/env bash
# Smoke do executavel Windows (cross-compilado com cargo-xwin) no Wine, comparado ao binario Linux.
# Wine NAO e Windows nativo: ver docs/conformance/platform/windows-wine.md.
#   tools/package/smoke-wine.sh [apple2-rust.exe] [apple2-rust-linux]
set -uo pipefail

cache=${CONFORMANCE_CACHE:-/mnt/d/Chico/AppleRust/target/conformance-cache}
exe=${1:-$cache/target-win/x86_64-pc-windows-msvc/release/apple2-rust.exe}
lin=${2:-target/release/apple2-rust}
export WINEPREFIX=$cache/wineprefix WINEDEBUG=-all
out=$cache/smoke
mkdir -p "$out"
fail=0
ok()  { echo "PASS  $*"; }
bad() { echo "FAIL  $*"; fail=1; }

[ -d "$WINEPREFIX" ] || wineboot -i >/dev/null 2>&1

for m in apple2 apple2p apple2jp apple2e apple2ee; do
  "$lin" -model "$m" -memclear 0 --headless-frames 300 --dump-screen "$out/lin-$m.png"
  wine "$exe" -model "$m" -memclear 0 --headless-frames 300 --dump-screen "$out/win-$m.png"; rc=$?
  if [ $rc -eq 0 ] && cmp -s "$out/lin-$m.png" "$out/win-$m.png"; then
    ok "headless $m: PNG identico bit a bit ($(sha256sum "$out/win-$m.png" | cut -c1-12))"
  else bad "headless $m (rc=$rc)"; fi
done

# Obs.: o Apple ][ liga com RAM aleatoria (como o AppleWin); -memclear 0 torna a tela deterministica.
# boot do DOS 3.3 headless
dos="assets/disks/DOS 3.3 System Master - 680-0210-A.dsk"
"$lin" -d1 "$dos" -power-on --headless-frames 900 --dump-screen "$out/lin-dos.png"
wine "$exe" -d1 "$dos" -power-on --headless-frames 900 --dump-screen "$out/win-dos.png"; rc=$?
if [ $rc -eq 0 ] && cmp -s "$out/lin-dos.png" "$out/win-dos.png"; then ok "boot DOS 3.3 headless: PNG identico"
else bad "boot DOS 3.3 (rc=$rc)"; fi

# --help (o exe e subsistema GUI: a saida so aparece se houver console; no Wine, via stdout redirecionado)
if wine "$exe" --help 2>&1 | grep -q "Usage: apple2-rust"; then ok "--help"; else bad "--help"; fi

# janela real (Vulkan do host via winevulkan) por ~15 s, fechada com WM_DELETE_WINDOW; Wine demora a abrir
here=$(cd "$(dirname "$0")" && pwd)
if env -u WAYLAND_DISPLAY DISPLAY=${DISPLAY:-:0} GUI_SECONDS=15 "$here/smoke-gui.sh" wine-gui wine "$exe" -power-on; then :; else fail=1; fi

exit $fail
