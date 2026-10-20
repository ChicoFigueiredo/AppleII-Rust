#!/usr/bin/env bash
# Builds the release archive for one platform. Run from the repo root.
#   packaging/package.sh <linux|windows|macos> <version> <dir-with-applewin[.exe]>
# Output: dist/applewin-<version>-{linux-x64.tar.gz | windows-x64.zip | macos-universal.zip}
# Nothing is needed at runtime besides the executable (ROMs and symbols are embedded); disks/
# is a convenience, like AppleWin's Master.dsk.
set -euo pipefail

os=$1; ver=${2#v}; bindir=$3
icons=packaging/icons
id=io.github.chicofigueiredo.appleii-rust
mkdir -p dist
dist=$(pwd)/dist

zip_dir() { # zip_dir <parent> <name> <out.zip>
  if command -v zip >/dev/null; then (cd "$1" && zip -qry "$3" "$2")
  else (cd "$1" && 7z a -tzip "$3" "$2" >/dev/null); fi
}

copy_common() { # copy_common <dir>
  cp LICENSE README.md "$1/"
  mkdir -p "$1/disks"
  cp assets/disks/* "$1/disks/"
}

case "$os" in
  linux)
    name=applewin-$ver-linux-x64
    root=$(mktemp -d)/$name
    mkdir -p "$root"
    cp "$bindir/applewin" "$root/"
    copy_common "$root"
    for s in 16 24 32 48 64 128 256 512; do
      d=$root/share/icons/hicolor/${s}x${s}/apps
      mkdir -p "$d"
      cp "$icons/applewin-$s.png" "$d/$id.png"
      cp "$icons/disk-$s.png" "$d/$id-disk.png"
    done
    install -Dm644 packaging/linux/$id.desktop "$root/share/applications/$id.desktop"
    install -Dm644 packaging/linux/$id.xml "$root/share/mime/packages/$id.xml"
    install -m755 packaging/linux/install.sh "$root/install.sh"
    chmod +x "$root/applewin"
    tar -C "$(dirname "$root")" -czf "$dist/$name.tar.gz" "$name"
    ;;
  windows)
    name=applewin-$ver-windows-x64
    root=$(mktemp -d)/$name
    mkdir -p "$root"
    cp "$bindir/applewin.exe" "$root/"
    copy_common "$root"
    zip_dir "$(dirname "$root")" "$name" "$dist/$name.zip"
    ;;
  macos)
    name=applewin-$ver-macos-universal
    root=$(mktemp -d)/$name
    app=$root/AppleII-Rust.app
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$bindir/applewin" "$app/Contents/MacOS/applewin"
    chmod +x "$app/Contents/MacOS/applewin"
    sed "s/@VERSION@/$ver/g" packaging/macos/Info.plist.in > "$app/Contents/Info.plist"
    cp "$icons/applewin.icns" "$app/Contents/Resources/applewin.icns"
    cp "$icons/disk.icns" "$app/Contents/Resources/disk.icns"
    copy_common "$root"
    if command -v codesign >/dev/null; then
      # ad-hoc signature: no Developer ID, so Gatekeeper still needs right-click > Open once
      codesign --force --deep -s - "$app"
      codesign --verify --deep --strict "$app"
    fi
    # ditto keeps the bundle's permissions/xattrs; fall back to zip elsewhere
    if command -v ditto >/dev/null; then
      (cd "$(dirname "$root")" && ditto -c -k --sequesterRsrc --keepParent "$name" "$dist/$name.zip")
    else
      zip_dir "$(dirname "$root")" "$name" "$dist/$name.zip"
    fi
    ;;
  *) echo "unknown os $os" >&2; exit 2 ;;
esac
ls -l "$dist"
