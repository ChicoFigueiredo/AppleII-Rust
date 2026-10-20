#!/bin/sh
# Installs AppleII-Rust from the extracted tarball into PREFIX (default ~/.local).
set -e
here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}
install -Dm755 "$here/applewin" "$prefix/bin/applewin"
mkdir -p "$prefix/share"
cp -r "$here/share/." "$prefix/share/"
mkdir -p "$prefix/share/applewin"
cp -r "$here/disks" "$prefix/share/applewin/"
command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/share/applications" || true
command -v update-mime-database >/dev/null && update-mime-database "$prefix/share/mime" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$prefix/share/icons/hicolor" || true
echo "Installed to $prefix (make sure $prefix/bin is in PATH)"
