#!/usr/bin/env bash
# Extrai o pacote Linux num diretorio temporario e roda o executavel DE DENTRO dele.
#   tools/package/test-package.sh <applewin-X-linux-x64.tar.gz>
set -euo pipefail
pkg=$(realpath "$1")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
tar -C "$tmp" -xzf "$pkg"
root=$(echo "$tmp"/applewin-*)
cd "$root"          # nada do repositorio fica no caminho de busca: so o conteudo do pacote
fail=0
ok()  { echo "PASS  $*"; }
bad() { echo "FAIL  $*"; fail=1; }

for f in applewin LICENSE README.md THIRD-PARTY-LICENSES.html install.sh \
         share/applications/io.github.chicofigueiredo.appleii-rust.desktop \
         share/mime/packages/io.github.chicofigueiredo.appleii-rust.xml \
         share/icons/hicolor/256x256/apps/io.github.chicofigueiredo.appleii-rust.png; do
  [ -s "$f" ] && ok "contem $f" || bad "falta $f"
done
[ -x applewin ] && ok "applewin executavel" || bad "applewin sem bit x"

# headless a partir do pacote (ROMs/simbolos embutidos)
./applewin --headless-frames 300 --dump-screen "$tmp/boot.png" >/dev/null 2>&1 && [ -s "$tmp/boot.png" ] \
  && ok "headless a partir do pacote" || bad "headless"

# discos do pacote: cada um e aceito e monta (carregamento + boot headless)
n=0
for d in disks/*; do
  n=$((n+1))
  ./applewin -d1 "$d" -power-on --headless-frames 600 --dump-screen "$tmp/d$n.png" >/dev/null 2>&1 && [ -s "$tmp/d$n.png" ] \
    && ok "disco '$d' montado e bootado headless" || bad "disco '$d'"
done
# o boot do DOS 3.3 e do ProDOS precisa diferir da tela sem disco (prova que o disco foi lido)
./applewin -power-on --headless-frames 600 --dump-screen "$tmp/nodisk.png" >/dev/null 2>&1
for d in disks/DOS*.dsk disks/ProDOS*.po; do
  ./applewin -d1 "$d" -power-on --headless-frames 600 --dump-screen "$tmp/x.png" >/dev/null 2>&1
  cmp -s "$tmp/x.png" "$tmp/nodisk.png" && bad "'$d' igual a tela sem disco (nao leu)" || ok "'$d' altera a tela (disco lido)"
done

# manual (F1): embutido no binario via include_str!, nao depende de arquivos externos
if grep -aq "Linha de comando" applewin && grep -aq "primeiros-passos\|Primeiros passos" applewin; then
  ok "manual embutido no executavel (F1 nao depende de arquivos externos)"
else bad "manual nao encontrado no binario"; fi

# integridade da lista de licencas
grep -q "MIT" THIRD-PARTY-LICENSES.html && ok "THIRD-PARTY-LICENSES.html com licencas" || bad "licencas vazias"
exit $fail
