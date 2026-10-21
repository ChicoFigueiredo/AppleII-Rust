#!/usr/bin/env bash
# Baixa e verifica o AppleWin v1.32.1.0 oficial (Windows) para uso como oraculo sob Wine.
#
# Destino (fora do Git, compartilhado entre worktrees):
#   /mnt/d/Chico/AppleRust/target/conformance-cache/oracle/applewin-1.32.1.0/
#     AppleWin1.32.1.0.zip   release oficial
#     app/                   zip descompactado (AppleWin.exe etc.)
#
# Uso: fetch-applewin-windows.sh [--force]
# Requer: gh (autenticado ou release publica), unzip, sha256sum.
# Nao aciona CI/Actions: apenas `gh release download`.
set -euo pipefail

VERSION="1.32.1.0"
TAG="v${VERSION}"
ASSET="AppleWin${VERSION}.zip"
# SHA-256 fixado apos o primeiro download (2026-10-20).
EXPECTED_SHA256="94783120695ff1749823c933c02798a7df6155d16cc9d75363f73b0dccc88ff0"
ORACLE_ROOT="${ORACLE_ROOT:-/mnt/d/Chico/AppleRust/target/conformance-cache/oracle}"
DEST="${ORACLE_ROOT}/applewin-${VERSION}"
ZIP="${DEST}/${ASSET}"
APP="${DEST}/app"

force=0
[[ "${1:-}" == "--force" ]] && force=1

mkdir -p "${DEST}"

if [[ ! -f "${ZIP}" || ${force} -eq 1 ]]; then
  rm -f "${ZIP}"
  gh release download "${TAG}" -R AppleWin/AppleWin -p "${ASSET}" -D "${DEST}"
fi

actual="$(sha256sum "${ZIP}" | cut -d' ' -f1)"
if [[ "${actual}" != "${EXPECTED_SHA256}" ]]; then
  echo "ERRO: SHA-256 de ${ZIP} nao confere." >&2
  echo "  esperado: ${EXPECTED_SHA256}" >&2
  echo "  obtido:   ${actual}" >&2
  exit 1
fi
echo "SHA-256 ok: ${actual}"

if [[ ! -f "${APP}/AppleWin.exe" || ${force} -eq 1 ]]; then
  rm -rf "${APP}"
  mkdir -p "${APP}"
  unzip -oq "${ZIP}" -d "${APP}"
fi
echo "AppleWin.exe: ${APP}/AppleWin.exe"
