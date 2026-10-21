#!/usr/bin/env bash
# Roda o AppleWin 1.32.1.0 (Windows) sob Wine, sem interacao humana, e gera:
#   <out>/<name>.aws.yaml          save-state real (gravado pelo proprio AppleWin)
#   <out>/<name>.bmp (+ .png)      captura de tela do estado salvo
#   <out>/<name>.provenance.toml   proveniencia (versoes, comando, hashes)
#
# Uso:
#   run-applewin-wine.sh --name NOME --model MODELO [--disk ARQ] [--seconds N]
#                        [--out DIR] [--extra "args do AppleWin"]
#   MODELO: apple2 | apple2p | apple2jp | apple2e | apple2ee   (CmdLine.cpp:656-669)
#   Sem --disk: slot 6 fica vazio (-s6 empty) e a maquina cai no prompt do Applesoft.
#   Com --disk: disco no drive 1 do slot 6, com boot (-d1 + -power-on).
#
# Mecanismo (ver README-wine.md): o AppleWin nao tem opcao de CLI para salvar
# estado. Usa-se "Save State On Exit" (.ini via -conf): o estado e gravado no
# WM_DESTROY, e o encerramento e pedido por `wine taskkill` (WM_CLOSE). O
# AppleWin carrega o estado no start se o arquivo existir (senao abre uma
# caixa "Load State"); por isso ha uma passada "semente" que cria o arquivo.
# Nunca faz push, tag, nem aciona CI.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ORACLE_ROOT="${ORACLE_ROOT:-/mnt/d/Chico/AppleRust/target/conformance-cache/oracle}"
VERSION="1.32.1.0"
APP="${ORACLE_ROOT}/applewin-${VERSION}/app"
ZIP="${ORACLE_ROOT}/applewin-${VERSION}/AppleWin${VERSION}.zip"
export WINEPREFIX="${ORACLE_ROOT}/wineprefix"
export WINEDEBUG="-all"
export DISPLAY="${DISPLAY:-:0}"
WINESERVER="${WINESERVER:-$(dirname "$(readlink -f "$(command -v wine)")")/../lib/x86_64-linux-gnu/wine/wineserver}"
[[ -x "${WINESERVER}" ]] || WINESERVER=/usr/lib/x86_64-linux-gnu/wine/wineserver

name="" model="" disk="" seconds=20 out="" extra=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --name) name="$2"; shift 2;;
    --model) model="$2"; shift 2;;
    --disk) disk="$2"; shift 2;;
    --seconds) seconds="$2"; shift 2;;
    --out) out="$2"; shift 2;;
    --extra) extra="$2"; shift 2;;
    *) echo "opcao desconhecida: $1" >&2; exit 2;;
  esac
done
[[ -n "${name}" && -n "${model}" ]] || { sed -n 2,16p "$0" >&2; exit 2; }
out="${out:-${ORACLE_ROOT}/out/${name}}"
mkdir -p "${out}"
out="$(cd "${out}" && pwd)"

[[ -f "${APP}/AppleWin.exe" ]] || "${HERE}/fetch-applewin-windows.sh"
[[ -d "${WINEPREFIX}" ]] || wineboot -i >/dev/null 2>&1

WORK="${ORACLE_ROOT}/work/${name}"
rm -rf "${WORK}"; mkdir -p "${WORK}"
# Unidade Q: do Wine aponta para o diretorio de trabalho: caminhos gravados no
# YAML ficam estaveis ("Q:\disk1.dsk") e nao vazam caminhos da maquina.
mkdir -p "${WINEPREFIX}/dosdevices"
ln -sfn "${WORK}" "${WINEPREFIX}/dosdevices/q:"

disk_args=(); disk_sha=""; disk_src=""
if [[ -n "${disk}" ]]; then
  disk_src="${disk}"
  ext="${disk##*.}"
  cp "${disk}" "${WORK}/disk1.${ext}"
  chmod u+w "${WORK}/disk1.${ext}"
  disk_sha="$(sha256sum "${disk}" | cut -d' ' -f1)"
  disk_args=(-d1 "disk1.${ext}" -power-on)
else
  disk_args=(-s6 empty -power-on)
fi

statefile="${name}.aws.yaml"
cat > "${WORK}/aw.ini" <<EOF
[Configuration]
Version=${VERSION}
Save State On Exit=1
Save State Filename=Q:\\${statefile}
EOF

# Todos os caminhos relativos a Q:\ (diretorio corrente do processo = WORK).
cmd=(wine "${APP}/AppleWin.exe" -noreg -conf aw.ini -model "${model}"
     -no-printscreen-dlg -no-printscreen-key -no-hook-system-key -log
     "${disk_args[@]}")
# shellcheck disable=SC2206
[[ -n "${extra}" ]] && cmd+=(${extra})

stop_app() {
  wine taskkill /im AppleWin.exe >/dev/null 2>&1 || true
  for _ in $(seq 1 40); do
    pgrep -f 'AppleWin(-x64)?\.exe' >/dev/null || return 0
    sleep 0.5
  done
  echo "AVISO: AppleWin nao encerrou; matando o wineserver" >&2
  "${WINESERVER}" -k || true
}
trap '"${WINESERVER}" -k >/dev/null 2>&1 || true' EXIT

cd "${WORK}"

# --- Passada 1 (semente): cria o arquivo de estado para o start nao falhar ----
echo ">> passada semente"
"${cmd[@]}" >"${WORK}/seed.out" 2>&1 &
for _ in $(seq 1 60); do
  xwininfo -root -tree 2>/dev/null | grep -q '"Load State"' && break
  sleep 0.5
done
python3 -I "${HERE}/xsendkey.py" "Load State" Return
sleep 3
stop_app
[[ -s "${WORK}/${statefile}" ]] || { echo "ERRO: passada semente nao gerou ${statefile}" >&2; exit 1; }
cp "${WORK}/AppleWin.log" "${out}/${name}.seed.log" 2>/dev/null || true

# --- Passada 2: roda de verdade por N segundos e salva ao sair ---------------
echo ">> passada real (${seconds}s)"
"${cmd[@]}" >"${WORK}/run.out" 2>&1 &
sleep "${seconds}"
stop_app
[[ -s "${WORK}/${statefile}" ]] || { echo "ERRO: sem ${statefile}" >&2; exit 1; }
cp "${WORK}/AppleWin.log" "${out}/${name}.run.log" 2>/dev/null || true
cp "${WORK}/${statefile}" "${out}/${statefile}"

# --- Captura de tela: recarrega o estado e sai (-screenshot-and-exit) --------
echo ">> captura de tela"
cp "${WORK}/${statefile}" "${WORK}/shot-in.aws.yaml"
printf '[Configuration]\nVersion=%s\n' "${VERSION}" > "${WORK}/aw-shot.ini"
timeout 90 wine "${APP}/AppleWin.exe" -noreg -conf aw-shot.ini \
  -no-printscreen-dlg -no-printscreen-key -no-hook-system-key \
  -load-state shot-in.aws.yaml -screenshot-and-exit "${name}.bmp" >/dev/null 2>&1 || true
if [[ -f "${WORK}/${name}.bmp" ]]; then
  cp "${WORK}/${name}.bmp" "${out}/${name}.bmp"
  python3 -I -c "from PIL import Image; Image.open('${out}/${name}.bmp').save('${out}/${name}.png')" 2>/dev/null || true
else
  echo "AVISO: captura de tela nao gerada" >&2
fi

# --- Proveniencia ------------------------------------------------------------
yaml_sha="$(sha256sum "${out}/${statefile}" | cut -d' ' -f1)"
zip_sha="$(sha256sum "${ZIP}" | cut -d' ' -f1)"
printf -v cmdline '%q ' "${cmd[@]}"
{
  echo "# Proveniencia do save-state gerado pelo AppleWin real (oraculo sob Wine)"
  echo "applewin_version = \"${VERSION}\""
  echo "applewin_zip_sha256 = \"${zip_sha}\""
  echo "wine_version = \"$(wine --version)\""
  echo "host = \"$(uname -srm)\""
  echo "generated_utc = \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\""
  echo "model = \"${model}\""
  echo "run_seconds = ${seconds}"
  echo "disk_source = \"${disk_src##*/}\""
  echo "disk_sha256 = \"${disk_sha}\""
  echo "yaml_file = \"${statefile}\""
  echo "yaml_sha256 = \"${yaml_sha}\""
  echo "yaml_bytes = $(stat -c %s "${out}/${statefile}")"
  echo "deterministic = false  # tempo real sob Wine; ciclos acumulados variam"
  echo "command = '''cd Q:\\\\ && ${cmdline}'''"
  echo "invocation = '''$(printf '%q ' "$0" --name "${name}" --model "${model}" ${disk:+--disk "${disk_src##*/}"} --seconds "${seconds}")'''"
} > "${out}/${name}.provenance.toml"

echo "ok: ${out}/${statefile} (sha256 ${yaml_sha})"
