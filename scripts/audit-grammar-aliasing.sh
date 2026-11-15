#!/usr/bin/env bash
# Auditoría de las gramáticas nativas: UB por aliasing estricto bajo gcc -O3.
#
# QUÉ ES Y POR QUÉ EXISTE
#
# El scanner de tree-sitter-haskell 0.23.1 tenía UB de aliasing estricto: las
# macros de `array.h` de tree-sitter castean el array tipado a un `Array*`
# genérico, y gcc 16 lo compila mal a -O2/-O3. El resultado es la corrupción
# del heap y un abort (rc=134) que no dice nada del análisis: el producto se
# muere, y el consumidor recibe un proceso muerto en lugar de una respuesta.
#
# Un aviso de gcc no es prueba de un fallo: hasta ahora sólo haskell ha
# abortado. Esta auditoría es la que MIDE si las demás gramáticas del
# workspace abortan o producen un árbol distinto, comparando la misma
# compilación con y sin `-fno-strict-aliasing`.
#
# USO
#
#   scripts/audit-grammar-aliasing.sh --plan            # qué haría, sin compilar
#   scripts/audit-grammar-aliasing.sh                   # todas las gramáticas
#   scripts/audit-grammar-aliasing.sh ruby bash yaml    # un subconjunto
#   scripts/audit-grammar-aliasing.sh --control         # el control positivo
#
# LO QUE NO TOCA
#
# - `~/.cargo/config.toml`: no se lee ni se modifica.
# - La caché de cargo de la máquina: el driver se compila en un directorio de
#   destino propio bajo /tmp (regla 8 del AGENTS.md: una corrida de mutación
#   —y una de compilación nativa— no comparte el directorio de nadie).
# - El repositorio: el driver se genera en un temporal, así que el árbol no
#   cambia ni se ensucia.
#
# EL CONTROL POSITIVO
#
# Un «no aborta» sólo vale si el método sabe abortar. El control es
# tree-sitter-haskell 0.23.1, la gramática que ya se sabe rota: aborta 10/10
# con gcc -O3 y 0/10 con `-fno-strict-aliasing`. `--control` lo compila y lo
# corre; sin él, el veredicto de cada gramática se lee sin saber si el
# instrumento tiene dientes.

set -uo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
LOCK="$ROOT/Cargo.lock"
WORK=${WORK:-$(mktemp -d /tmp/grammar-aliasing-XXXXXX)}
PARSES=${PARSES:-200}
RUNS=${RUNS:-10}
PLAN=0
CONTROL=0
GRAMMARS=()

# La extensión de fichero de cada gramática, para elegir un corpus real.
declare -A EXT=(
  [bash]=sh [c]=c [cpp]=cpp [c-sharp]=cs [dart]=dart [elixir]=ex
  [erlang]=erl [fortran]=f90 [go]=go [groovy]=groovy [haskell]=hs [hcl]=tf
  [java]=java [javascript]=js [json]=json [julia]=jl [kotlin-sg]=kt
  [lua]=lua [php]=php [powershell]=ps1 [python]=py [r]=r [ruby]=rb [rust]=rs
  [scala]=scala [swift]=swift [systemverilog]=sv [typescript]=ts
  [verilog]=v [yaml]=yml [zig]=zig
)

# Cómo expone cada gramática su lenguaje: casi todas con la constante
# `LANGUAGE`; algunas, con la función `language()`. La tabla dice cuál es
# cuál, porque el driver se genera con el nombre exacto y un nombre que no
# existe no compila.
declare -A API=([c]=language)

die() { echo "audit-grammar-aliasing: $*" >&2; exit 1; }

# La versión de una gramática en el lock del workspace: la auditoría mide la
# gramática que el producto usa, no la última publicada.
version_of() {
  awk -v crate="tree-sitter-$1" '
    $0 ~ "name = \"" crate "\"" { found = 1; next }
    found && $1 == "version" { gsub(/"/, "", $3); print $3; exit }
  ' "$LOCK"
}

# Un fichero real de esa gramática: el más grande del workspace. El corpus
# tiene que ser código de verdad de ese lenguaje, o el veredicto no dice
# nada del escáner que esa gramática usa.
corpus_of() {
  local ext=$1 candidate
  candidate=$(git -C "$ROOT" ls-files -z 2>/dev/null | tr '\0' '\n' \
    | grep -E "\.${ext}$" | while read -r f; do
        [ -f "$ROOT/$f" ] && echo "$(wc -c < "$ROOT/$f") $ROOT/$f"
      done | sort -rn | head -1 | cut -d' ' -f2-)
  if [ -z "$candidate" ]; then
    candidate=$(find "$ROOT/sandbox" -type f -name "*.$ext" -size +1k 2>/dev/null \
      | head -200 | while read -r f; do echo "$(wc -c < "$f") $f"; done \
      | sort -rn | head -1 | cut -d' ' -f2-)
  fi
  [ -n "$candidate" ] && echo "$candidate"
}

# Todas las gramáticas del lock, menos el runtime y el crate de tipos.
all_grammars() {
  awk '
    /^name = "tree-sitter/ {
      name = $3; gsub(/"/, "", name)
      if (name != "tree-sitter" && name != "tree-sitter-language") print name
    }
  ' "$LOCK" | sed 's/^tree-sitter-//' | sort
}

# --- El driver mínimo -------------------------------------------------------
#
# tree-sitter + la gramática, N parseos de un fichero real, y un digest del
# árbol (clase y rango de cada nodo). El digest es lo que se compara entre las
# dos compilaciones: si los árboles difieren, la UB cambió la salida; si el
# proceso aborta, la UB corrompió la memoria.

write_driver() {
  local dir=$1
  mkdir -p "$dir/src"
  {
    echo "[package]"
    echo "name = \"grammar_audit\""
    echo "version = \"0.1.0\""
    echo "edition = \"2021\""
    echo ""
    echo "[dependencies]"
    echo "tree-sitter = \"=0.27.1\""
    for g in "${GRAMMARS[@]}"; do
      # El control trae su propia versión de haskell (la que se sabe rota), y
      # dos versiones del mismo crate no pueden estar en un driver: la del
      # lock se queda fuera cuando el control está.
      [ "$CONTROL" = 1 ] && [ "$g" = "haskell" ] && continue
      echo "tree-sitter-$g = \"=$(version_of "$g")\""
    done
    [ "$CONTROL" = 1 ] && echo "tree-sitter-haskell = \"=0.23.1\""
  } > "$dir/Cargo.toml"

  {
    cat <<'RS'
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("uso: grammar_audit <gramática> <fichero> <n>");
        return ExitCode::from(2);
    }
    let language = match args[1].as_str() {
RS
    for g in "${GRAMMARS[@]}"; do
      if [ "${API[$g]:-LANGUAGE}" = "language" ]; then
        echo "        \"$g\" => tree_sitter_${g//-/_}::language().into(),"
      else
        echo "        \"$g\" => tree_sitter_${g//-/_}::LANGUAGE.into(),"
      fi
    done
    [ "$CONTROL" = 1 ] && echo "        \"haskell-0.23.1\" => tree_sitter_haskell::LANGUAGE.into(),"
    cat <<'RS'
        other => {
            eprintln!("gramática desconocida: {other}");
            return ExitCode::from(2);
        }
    };
    let source = match std::fs::read(&args[2]) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("el fichero no se lee: {error}");
            return ExitCode::from(2);
        }
    };
    let n: usize = match args[3].parse() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("n no es un número");
            return ExitCode::from(2);
        }
    };
    let mut parser = tree_sitter::Parser::new();
    if let Err(error) = parser.set_language(&language) {
        eprintln!("la gramática no se registra: {error}");
        return ExitCode::from(2);
    }
    let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
    for _ in 0..n {
        let tree = match parser.parse(&source, None) {
            Some(tree) => tree,
            None => {
                eprintln!("el árbol no se parsea");
                return ExitCode::from(3);
            }
        };
        let mut text = String::new();
        walk(tree.root_node(), &mut text);
        for byte in text.as_bytes() {
            digest ^= u64::from(*byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    println!("{digest}");
    ExitCode::SUCCESS
}

fn walk(node: tree_sitter::Node, out: &mut String) {
    out.push_str(node.kind());
    out.push(':');
    out.push_str(&node.start_byte().to_string());
    out.push('-');
    out.push_str(&node.end_byte().to_string());
    out.push(';');
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, out);
    }
}
RS
  } > "$dir/src/main.rs"
}

# --- La corrida -------------------------------------------------------------

# Una gramática, un binario: N parseos por corrida, RUNS corridas. Se cuenta
# cuántas abortan y con qué digest.
audit_one() {
  local bin=$1 grammar=$2 file=$3 label=$4
  local aborts=0 digests="" rc out shown
  for _ in $(seq 1 "$RUNS"); do
    out=$("$bin" "$grammar" "$file" "$PARSES" 2>"$WORK/err")
    rc=$?
    if [ $rc -ne 0 ]; then
      aborts=$((aborts + 1))
    else
      digests="$digests $out"
    fi
  done
  # El digest es el de las corridas que TERMINARON: una corrida abortada no
  # publica árbol, y su ausencia es el dato, no un digest vacío.
  shown=$(echo "$digests" | tr ' ' '\n' | grep -v '^$' | sort -u | tr '\n' ' ')
  [ -z "$shown" ] && shown="(ninguna corrida terminó)"
  printf '    %-28s abortos %d/%d; digests: %s\n' "$label" "$aborts" "$RUNS" "$shown"
  echo "$aborts $digests" > "$WORK/result-$grammar-$(basename "$bin")"
}

# --- Los argumentos ---------------------------------------------------------

while [ $# -gt 0 ]; do
  case "$1" in
    --plan) PLAN=1; shift;;
    --control) CONTROL=1; shift;;
    --parses) PARSES=$2; shift 2;;
    --runs) RUNS=$2; shift 2;;
    -h|--help) sed -n '2,40p' "$0"; exit 0;;
    *) GRAMMARS+=("$1"); shift;;
  esac
done

[ -f "$LOCK" ] || die "no se encuentra Cargo.lock en $ROOT"
if [ "$CONTROL" = 1 ]; then
  # El control ya trae la gramática haskell 0.23.1: la del lock no cabe en el
  # mismo driver y su veredicto es el del control.
  FILTERED=()
  for g in "${GRAMMARS[@]}"; do
    [ "$g" = "haskell" ] || FILTERED+=("$g")
  done
  GRAMMARS=("${FILTERED[@]}")
fi
if [ ${#GRAMMARS[@]} -eq 0 ]; then
  mapfile -t GRAMMARS < <(all_grammars)
fi

# El plan: lo que se mediría, sin compilar nada.
if [ "$PLAN" = 1 ]; then
  echo "Auditoría de aliasing estricto — plan (no compila nada)"
  echo "  workspace:   $ROOT"
  echo "  compilador:  $(gcc --version | head -1)"
  echo "  director:    $WORK"
  echo "  parseos:     $PARSES por corrida, $RUNS corridas por gramática"
  for g in "${GRAMMARS[@]}"; do
    v=$(version_of "$g")
    ext=${EXT[$g]:-}
    if [ -z "$v" ]; then
      echo "  $g: no está en el lock del workspace — se salta"
    elif [ -z "$ext" ]; then
      echo "  $g $v: sin extensión conocida — se salta"
    else
      f=$(corpus_of "$ext")
      if [ -z "$f" ]; then
        echo "  $g $v: sin corpus real (.*$ext) — se salta"
      else
        echo "  $g $v: $f ($(wc -l < "$f") líneas)"
      fi
    fi
  done
  [ "$CONTROL" = 1 ] && echo "  control: tree-sitter-haskell 0.23.1 (se sabe rota)"
  exit 0
fi

# El driver y sus dos compilaciones: la misma, con y sin el flag.
DRIVER="$WORK/driver"
write_driver "$DRIVER"
echo "Compilando el driver (gcc -O3 y gcc -O3 -fno-strict-aliasing)…"
CARGO_TARGET_DIR="$WORK/target-o3" cargo build --release --manifest-path "$DRIVER/Cargo.toml" >/dev/null 2>&1 \
  || die "no se pudo compilar el driver"
CFLAGS="-O3 -fno-strict-aliasing" CARGO_TARGET_DIR="$WORK/target-noalias" \
  cargo build --release --manifest-path "$DRIVER/Cargo.toml" >/dev/null 2>&1 \
  || die "no se pudo compilar el driver con -fno-strict-aliasing"

A="$WORK/target-o3/release/grammar_audit"
B="$WORK/target-noalias/release/grammar_audit"

if [ "$CONTROL" = 1 ]; then
  HS=$(mktemp "$WORK/control-XXXX.hs")
  cat > "$HS" <<'HS'
module Main where

data Shape = Circle Double | Rectangle Double Double

area :: Shape -> Double
area (Circle r) = pi * r * r
area (Rectangle w h) = w * h

main :: IO ()
main = do
  let shapes = [Circle 1.0, Rectangle 2.0 3.0]
  print (map area shapes)
HS
  echo "=== control positivo: tree-sitter-haskell 0.23.1 (se sabe rota) ==="
  audit_one "$A" "haskell-0.23.1" "$HS" "gcc -O3"
  audit_one "$B" "haskell-0.23.1" "$HS" "gcc -O3 -fno-strict-aliasing"
fi

for g in "${GRAMMARS[@]}"; do
  v=$(version_of "$g")
  ext=${EXT[$g]:-}
  if [ -z "$v" ]; then
    echo "=== $g: no está en el lock del workspace — se salta ==="
    continue
  fi
  if [ -z "$ext" ]; then
    echo "=== $g $v: sin extensión conocida — se salta ==="
    continue
  fi
  f=$(corpus_of "$ext")
  if [ -z "$f" ]; then
    echo "=== $g $v: sin corpus real — se salta ==="
    continue
  fi
  echo "=== $g $v :: $f ($(wc -l < "$f") líneas) ==="
  audit_one "$A" "$g" "$f" "gcc -O3"
  audit_one "$B" "$g" "$f" "gcc -O3 -fno-strict-aliasing"
done

echo
echo "Directorio de trabajo: $WORK"
