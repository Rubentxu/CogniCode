#!/usr/bin/env bash
# Se ha construido ESTE binario desde ESTE arbol?
#
# MEDIDO 2026-10-03. La lane de v0.101.5, ya con aarch64 entero en verde, cayo
# en `skill-bundles` con
#
#     error: unrecognized subcommand 'skills'
#
# La stage lo habia pedido correctamente y el subcomando existe en el arbol
# (7d074fef es ancestro del tag). El problema es de donde salio el binario:
#
#     $ ls --time-style=full-iso "$TARGET_DIR/release/cognicode-release"
#     -rwxr-xr-x 2 rubentxu rubentxu 1494424 2026-10-03 20:13:00
#
# La lane corrio a las 21:30. El binario es de las 20:13, de un checkout
# distinto, y `cargo build --release --bin cognicode-release` no lo toco.
#
# Por que no: `~/.cargo/config.toml` de esta maquina fija
# `build.target-dir = /var/home/rubentxu/cargo-targets`, compartido por varios
# checkouts y agentes concurrentes. Cargo decide si hay que reconstruir
# comparando mtimes, y los ficheros de ESTE checkout son mas antiguos —los
# escribio git al crearlo— que un binario que otro arbol construyo despues.
# Freshness comparada entre dos arboles que no comparten historial: no dice nada.
#
# Esto es la misma trampa que `check-cross-toolchain.sh` en el otro extremo, con
# la misma leccion: una precondicion que el build da por cumplida y nadie
# comprueba. Alli era "¿compila este target?". Aqui es "¿este binario es de este
# arbol?", que es la misma pregunta con un sujeto distinto: una lane que
# certifica con una herramienta construida por otra no esta certificando el
# codigo que dice certificar.
#
# Lo que mas caro sale es el retardo: el fallo aparece veinte minutos despues,
# en una stage distinta, y blamed en un subcomando que el arbol si tiene.
#
# Uso: check-built-binary.sh <binario> [raiz-del-arbol]
# Exit: 0 si el binario no es anterior a ningun fuente; 1 si lo es.
set -uo pipefail

binary="${1:?usage: check-built-binary.sh <binary> [repo-root]}"
root="${2:-$(cd "$(dirname "$0")/../.." && pwd)}"

if [ ! -f "$binary" ]; then
    echo "FAIL: no existe el binario $binary." >&2
    echo "  La stage que lo construye corre antes de esta; fallo o no se ejecuto." >&2
    exit 1
fi

# Las fuentes que pueden producir ESTE binario. Un `.rs` bajo `crates/` es lo
# que decide su contenido; los ficheros de texto de la raiz no entran en el
# binario. Se comparan mtimes, que es la misma medida que usa cargo, y por eso
# solo puede decir "el binario es mas viejo que algo del arbol" — que es
# exactamente la condicion que hay que rechazar.
newer="$(find "$root/crates" -name '*.rs' -newer "$binary" -print 2>/dev/null | head -1 || true)"

if [ -n "$newer" ]; then
    echo "FAIL: $binary es mas antiguo que fuentes de este arbol." >&2
    echo "  fuente mas nueva: ${newer#"$root"/}" >&2
    echo "  binario        : $(date -r "$binary" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo '?')" >&2
    echo >&2
    echo "  Cargo decide reconstruir comparando mtimes, y el directorio de" >&2
    echo "  artefactos de esta maquina lo comparten varios checkouts. Un binario" >&2
    echo "  construido por otro checkout puede ser mas nuevo que las fuentes de" >&2
    echo "  este, y entonces cargo lo da por fresco y esta lane ejecuta una" >&2
    echo "  herramienta que no salio de este arbol." >&2
    echo >&2
    echo "  Que hacer: darle a la lane su propio CARGO_TARGET_DIR, para que las" >&2
    echo "  huellas de cargo sean de un arbol solo." >&2
    exit 1
fi

echo "  ok: $binary no es anterior a ninguna fuente de ${root#"$root"/}crates"
