#!/usr/bin/env bash
# Builds the headless AppleWin oracle (audetto fork + trace patch) on Linux.
# Idempotent: clones or updates the fork at a pinned SHA, applies the patch if needed, builds.
#
# Cache (outside Git, shared between worktrees):
#   $ORACLE_CACHE/oracle/audetto-AppleWin   fork clone
#   $ORACLE_CACHE/oracle/build              CMake build tree  (binary: build/audetto-oracle)
# ORACLE_CACHE defaults to <main checkout>/target/conformance-cache.
set -euo pipefail

FORK_URL="https://github.com/audetto/AppleWin"
FORK_SHA="a1e20c0362f392dc25f1a3b218d2f84d4f1a25da"   # audetto/AppleWin master, 2026-10-10

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -z "${ORACLE_CACHE:-}" ]; then
  COMMON="$(git -C "$HERE" rev-parse --path-format=absolute --git-common-dir)"
  ORACLE_CACHE="$(dirname "$COMMON")/target/conformance-cache"
fi
CLONE="$ORACLE_CACHE/oracle/audetto-AppleWin"
BUILD="$ORACLE_CACHE/oracle/build"
PATCH="$HERE/audetto-trace.patch"
JOBS="${JOBS:-$(nproc)}"

mkdir -p "$ORACLE_CACHE/oracle"

if [ ! -d "$CLONE/.git" ]; then
  git clone "$FORK_URL" "$CLONE"
fi
if ! git -C "$CLONE" cat-file -e "$FORK_SHA^{commit}" 2>/dev/null; then
  git -C "$CLONE" fetch origin
fi

# Pin: discard any previous patch/edits in tracked files, then check out the SHA.
git -C "$CLONE" reset --hard -q
git -C "$CLONE" checkout -q --detach "$FORK_SHA"
git -C "$CLONE" submodule update --init --recursive -q

# Apply the trace patch (the tree is pristine after the reset above, so this is a plain apply).
patch -d "$CLONE" -p1 --no-backup-if-mismatch < "$PATCH"

# The oracle frontend lives in Git here and is copied into the fork tree (untracked there).
rm -rf "$CLONE/source/frontends/oracle"
mkdir -p "$CLONE/source/frontends/oracle"
cp "$HERE/audetto-oracle/main.cpp" "$HERE/audetto-oracle/CMakeLists.txt" "$CLONE/source/frontends/oracle/"

# Only the headless executable (BUILD_ORACLE). slirp/pcap/SDL/Qt/evdev are not needed:
# without slirp and pcap the fork compiles a dummy network backend.
cmake -S "$CLONE" -B "$BUILD" -G "${CMAKE_GENERATOR:-Unix Makefiles}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_ORACLE=ON \
  -DENABLE_NETWORKING=OFF
cmake --build "$BUILD" --target audetto-oracle -j "$JOBS"

echo "oracle built: $BUILD/audetto-oracle (fork $FORK_SHA)"
