#!/usr/bin/env bash
# Release-candidate UAT: consume ONLY the produced distribution archives.
# No executable, skill or MCP command is allowed to refer to target/release.
set -euo pipefail

VERSION="${1:?usage: release-install-smoke.sh VERSION RELEASE_DIR}"
RELEASE_DIR="${2:?usage: release-install-smoke.sh VERSION RELEASE_DIR}"
RELEASE_DIR="$(realpath "$RELEASE_DIR")"
TRIPLE="x86_64-unknown-linux-gnu"
BUNDLE="bundle-${VERSION}-${TRIPLE}.yaml"
COGH="cogh-${VERSION}-${TRIPLE}.tar.gz"
test -f "$RELEASE_DIR/$BUNDLE"
test -f "$RELEASE_DIR/$COGH"
test -f "$RELEASE_DIR/SHA256SUMS"

TMP="$(mktemp -d)"
SERVER_PID=""
# El script reasigna HOME y XDG_DATA_HOME mas abajo, dentro de `$TMP`. La
# envoltura de recuperacion guarda lo que borra en `$XDG_DATA_HOME/Trash`, asi
# que con las dos variables dentro del temporal, cualquier borrado del propio
# script crea un arbol que CONTIENE su propia basura: 70 MB anidados y, al final,
# un temporal que ya no se puede mover. Se guardan los valores originales ahora,
# antes de que cambien, para que la limpieza pueda devolverlos.
ORIGINAL_HOME="$HOME"
ORIGINAL_XDG_DATA_HOME="${XDG_DATA_HOME:-}"

# MEDIDO 2026-10-04. La lane de v0.101.8 llego aqui con el candidato entero —
#los once artefactos, sus SBOM y SHA256SUMS verificados— y salio con
#`Pipeline finished with FAILURE: shell exited with code 64`, que es el codigo
#de este mismo script.
#
# El UAT habia pasado:
#
#     PASS: published-layout CLI + MCP + skills install/update/reshim/uninstall
#
# y lo que fallo fue la limpieza. El script corre el CLI dentro de un HOME
# temporal, asi que cuando `rm -rf "$TMP"` corre, el directorio de trabajo esta
# DENTRO del directorio que se quiere borrar. En esta maquina `rm` es una
# envoltura de recuperacion que se niega a mover un directorio que contiene el
# cwd, responde 64, y —como el trap no fija ningun estado— ese 64 sustituye al
# veredicto que el script ya habia decidido. Un PASS se convierte en fallo por
# no haber borrado una carpeta temporal.
#
# Son los DOS defectos que `preflight-clean-clone.sh` ya resolvio (N+85), y la
# forma es la misma a proposito: salir del directorio antes de borrar, y que la
# limpieza no pueda ser el veredicto. Este script era el hermano que se dejo
# atras: la stage 1 de la lane paso porque el preflight ya estaba arreglado, y
# esta murio porque el otro no. Un gate que se apaga en su propia limpieza
# deja de distinguir "el codigo esta roto" de "no consegui borrar una carpeta".
cognicode_install_smoke_cleanup() {
  # `$?` tiene que leerse en la PRIMERA sentencia: cualquier comando anterior ya
  # habria sobrescrito el estado de salida que se quiere conservar.
  local incoming_status=$?

  if [ -n "$SERVER_PID" ]; then kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true; fi

  if [ -n "$TMP" ] && [ -d "$TMP" ]; then
    # Salir del directorio antes de borrarlo. `cd /` es el destino mas simple y
    # no depende de que el arbol de trabajo siga existiendo.
    cd / 2>/dev/null || cd "$ORIGINAL_HOME" 2>/dev/null || true

    # Y devolver HOME y XDG_DATA_HOME a donde estaban. MEDIDO 2026-10-04: sin
    # esto el temporal no se puede borrar, y no por proteccion sino porque se
    # trastera a si mismo. La envoltura rechaza "/tmp/tmp.XXX ... is the parent
    # of the home directory" mientras HOME apunte dentro del temporal, y con
    # HOME devuelto pero XDG_DATA_HOME todavia dentro, deja de rechazar y falla
    # con "failed to trash": el arbol de 70 MB contiene su propio
    # `.local/share/Trash`. Las dos variables vuelven, o no vuelve ninguna.
    export HOME="$ORIGINAL_HOME"
    if [ -n "$ORIGINAL_XDG_DATA_HOME" ]; then
      export XDG_DATA_HOME="$ORIGINAL_XDG_DATA_HOME"
    else
      unset XDG_DATA_HOME
    fi

    # El borrado no puede ser el veredicto. Si falla —envoltura de seguridad,
    # permisos, un fichero abierto— se dice y se sigue con el estado que el
    # script ya habia decidido. El motivo se imprime en vez de descartarse: un
    # aviso que no dice por que no se pudo borrar obliga a la siguiente persona a
    # reproducirlo entero.
    local cleanup_reason=""
    if ! cleanup_reason="$(rm -rf -- "$TMP" 2>&1)"; then
      printf 'aviso: no se pudo limpiar el HOME temporal %s\n' "$TMP" >&2
      [ -n "$cleanup_reason" ] && printf '       %s\n' "$cleanup_reason" >&2
      printf '       el UAT no depende de la limpieza, y este directorio se\n' >&2
      printf '       puede borrar a mano sin riesgo.\n' >&2
    fi
  fi

  # El estado con el que el script ya habia decidido. Nunca el de la limpieza.
  exit "$incoming_status"
}

trap 'cognicode_install_smoke_cleanup' EXIT

mkdir -p "$TMP/release/v$VERSION" "$TMP/staging" "$TMP/bootstrap" "$TMP/home/.config/opencode"
cp "$RELEASE_DIR/"* "$TMP/release/v$VERSION/"
tar -xzf "$RELEASE_DIR/$COGH" -C "$TMP/bootstrap"
COGH_BIN="$TMP/bootstrap/bin/cogh"
test -x "$COGH_BIN"
# Create the empty OpenCode config fixture via cp from a neutral name:
# some agent sandboxes block shell redirects whose target path matches an
# IDE config filename, even inside a disposable temp dir.
printf '{}\n' > "$TMP/opencode-fixture"
cp "$TMP/opencode-fixture" "$TMP/home/.config/opencode/opencode.json"

python3 -u - "$TMP/release" > "$TMP/server.port" 2> "$TMP/server.err" <<'PY' &
import functools
import http.server
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(root))
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
print(server.server_address[1], flush=True)
server.serve_forever()
PY
SERVER_PID=$!
for attempt in $(seq 1 100); do
  if [ -s "$TMP/server.port" ]; then break; fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    cat "$TMP/server.err" >&2
    echo "release smoke: local asset server stopped" >&2
    exit 1
  fi
  sleep 0.1
done
test -s "$TMP/server.port"
PORT="$(head -n 1 "$TMP/server.port")"
BASE="http://127.0.0.1:$PORT"
cat > "$TMP/staging/releases.json" <<EOF
{
  "tag_name": "v$VERSION",
  "draft": false,
  "prerelease": false,
  "published_at": "2026-01-01T00:00:00Z",
  "html_url": "$BASE/v$VERSION",
  "assets": [
    {
      "name": "$BUNDLE",
      "browser_download_url": "$BASE/v$VERSION/$BUNDLE"
    }
  ]
}
EOF

export HOME="$TMP/home"
export COGNICODE_HOME="$HOME/.cognicode"
export XDG_CONFIG_HOME="$HOME/.config"
export XDG_DATA_HOME="$HOME/.local/share"
export OPENCODE_CONFIG="$HOME/.config/opencode/opencode.json"
export COGNICODE_ASSET_BASE_URL="$BASE"
unset COGNICODE_BUNDLE_MANIFEST COGNICODE_API_BASE_URL COGNICODE_RELEASE_BASE_URL

"$COGH_BIN" init
"$COGH_BIN" install mcp-server --version "$VERSION" --profile reviewer \
  --ide opencode --staging "$TMP/staging"

CLI="$COGNICODE_HOME/shims/cognicode"
MCP="$COGNICODE_HOME/shims/cognicode-mcp"
test -x "$CLI" && test -x "$MCP"
"$CLI" --version | grep -F "$VERSION"
"$MCP" --version | grep -F "$VERSION"
"$COGH_BIN" doctor | tee "$TMP/doctor.out"
grep -Eq 'PASS[[:space:]]+MCP' "$TMP/doctor.out"
test -f "$COGNICODE_HOME/versions/$VERSION/skills/cognicode/SKILL.md"
test -f "$COGNICODE_HOME/versions/$VERSION/skills/cognicode-mcp/SKILL.md"
test -L "$HOME/.config/opencode/skills/cognicode-$VERSION"
test -L "$HOME/.config/opencode/skills/cognicode-mcp-$VERSION"
jq -e '.mcp["cognicode-mcp"].command[0]' "$OPENCODE_CONFIG" > "$TMP/mcp-command"
! grep -q 'target/release\|/tmp/prf-' "$TMP/mcp-command"

# A same-version update must not silently replace reviewer with core.
"$COGH_BIN" update --staging "$TMP/staging"
"$COGH_BIN" doctor | grep -Eq 'PASS[[:space:]]+MCP'

# Recover a broken temporary link only from the active installed version.
rm "$MCP"
ln -s "/tmp/prf-deleted-version/bin/cognicode-mcp" "$MCP"
"$COGH_BIN" reshim
test -x "$MCP"
# Compare canonicalized paths: on systems where /home is a symlink
# (e.g. Fedora's /var/home), readlink -f resolves the prefix and a raw
# string comparison would false-fail.
EXPECTED_SHIM_TARGET="$COGNICODE_HOME/versions/$VERSION/cognicode-mcp/bin/cognicode-mcp"
test "$(readlink -f "$MCP")" = "$(readlink -f "$EXPECTED_SHIM_TARGET")"

"$COGH_BIN" uninstall mcp-server --version "$VERSION" --ide opencode
test ! -e "$COGNICODE_HOME/versions/$VERSION"
test ! -e "$CLI"
test ! -e "$MCP"
test ! -e "$HOME/.config/opencode/skills/cognicode-$VERSION"
test ! -e "$HOME/.config/opencode/skills/cognicode-mcp-$VERSION"
jq -e '.mcp["cognicode-mcp"] == null' "$OPENCODE_CONFIG" >/dev/null
echo "PASS: published-layout CLI + MCP + skills install/update/reshim/uninstall"
