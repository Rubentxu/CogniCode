#!/usr/bin/env bash
# scripts/ci/check-bin-tracking.sh
#
# QW-03 guard: pinea que TODOS los `[[bin]]` declarados en el workspace
# tienen su source file tracked por git.
#
# Historia: el bin `cognicode-control-plane` se creó en entry 9 pero
# `src/bin/control_plane.rs` quedó untracked porque `.gitignore`
# silenciaba `bin/` a nivel de crate. CI falló al primer release.
# Este guard pinea que el bug no pueda repetirse silenciosamente.
#
# Exit codes:
#   0  — todos los bin sources están tracked
#   1  — al menos un bin source está untracked o falta
#   2  — fallo interno (no se pudo parsear un Cargo.toml)
#
# Uso:
#   bash scripts/ci/check-bin-tracking.sh
#
# Test contractual:
#   Para validar que el guard pinea, hacer `git rm --cached <path>` o
#   añadir un `[[bin]]` con `path` apuntando a un archivo no creado.
#   El guard debe fallar con exit 1.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

# Crates a inspeccionar (los que tienen [[bin]] en este workspace).
# Si añades un crate nuevo con bins declarados, añádelo aquí.
CRATES=(
  "crates/cognicode-cli"
  "crates/cognicode-explorer"
  "crates/cognicode-mcp"
  "crates/cognicode-runtime"
  "crates/cognicode-sandbox"
)

errors=0
checked=0
bin_names=()

echo "→ Guard de bin source tracking (QW-03, 2026-09-26)"
echo "  Crates inspeccionados: ${#CRATES[@]}"
echo

for crate in "${CRATES[@]}"; do
  cargo_toml="$REPO_ROOT/$crate/Cargo.toml"
  if [ ! -f "$cargo_toml" ]; then
    echo "WARN: $cargo_toml no existe (omitiendo)"
    continue
  fi

  # Extraer cada bloque [[bin]] con sus campos name y path.
  # awk en modo record+RS vacío (GNU awk) o perl (más portable).
  bin_blocks=$(perl -0777 -ne '
    while (/\[\[bin\]\](.*?)(?=\n\[\[|\n\[|\Z)/gs) {
      my $block = $1;
      my $name  = ($block =~ /name\s*=\s*"([^"]+)"/) ? $1 : "";
      my $path  = ($block =~ /path\s*=\s*"([^"]+)"/) ? $1 : "";
      if ($name) {
        print "$name|$path\n";
      }
    }
  ' "$cargo_toml")

  if [ -z "$bin_blocks" ]; then
    continue
  fi

  while IFS='|' read -r bin_name bin_path; do
    [ -z "$bin_name" ] && continue
    checked=$((checked + 1))
    bin_names+=("$bin_name")

    # Si path está vacío, Cargo usa src/main.rs o src/bin/<name>.rs por defecto.
    if [ -z "$bin_path" ]; then
      if [ "$bin_name" == "${crate##*/}" ] || [ "$crate" == "crates/cognicode-cli" ]; then
        bin_path="src/main.rs"
      else
        bin_path="src/bin/${bin_name}.rs"
      fi
    fi

    full_path="$crate/$bin_path"

    if [ ! -e "$full_path" ]; then
      echo "FAIL: bin '$bin_name' (crate $crate) declara path '$bin_path' pero el archivo NO EXISTE"
      errors=$((errors + 1))
      continue
    fi

    if ! git ls-files --error-unmatch -- "$full_path" >/dev/null 2>&1; then
      echo "FAIL: bin '$bin_name' (crate $crate) — '$full_path' NO está tracked por git"
      errors=$((errors + 1))
      continue
    fi

    # Verificar también que el path NO está silenciado por .gitignore.
    # Si `git check-ignore` retorna 0, está siendo ignorado (peligro).
    if git check-ignore -- "$full_path" >/dev/null 2>&1; then
      echo "FAIL: bin '$bin_name' (crate $crate) — '$full_path' está en .gitignore (tracking frágil)"
      errors=$((errors + 1))
      continue
    fi

    echo "OK:   $bin_name ($full_path)"
  done <<< "$bin_blocks"
done

echo
echo "→ Resumen: $checked bin(s) verificado(s), $errors error(es)"

if [ "$errors" -gt 0 ]; then
  echo
  echo "Acción de recuperación:"
  echo "  1. Si el archivo existe pero está untracked:"
  echo "       git add -f <path>"
  echo "  2. Si está siendo ignorado por .gitignore:"
  echo "       añade una negación '!<path>/' en .gitignore"
  echo "  3. Si el path es incorrecto, corrige el Cargo.toml del crate"
  exit 1
fi

echo "OK: todos los bin sources están tracked."
