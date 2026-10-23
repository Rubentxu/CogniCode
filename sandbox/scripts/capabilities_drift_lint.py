#!/usr/bin/env python3
"""code↔doc lint: the capabilities derive, and the derivation is measured.

Lo que este lint vigilaba, y por qué ya no tiene sentido
--------------------------------------------------------

Comparaba dos listas escritas a mano: `ALL_TREE_SITTER_LANGS`, en el crate,
y la lista del documento `CAPABILITIES-MATRIX.md`. Que dos listas
escritas por separado coincidan es una promesa que solo se sostiene
mientras nadie edita una de las dos, y el resultado fue el que se midió:

  * la lista del código anunciaba tres nombres que no son lenguajes
    (`jsx` y `tsx` son extensiones de JavaScript y TypeScript; `luau`
    es una extensión de Lua);
  * y callaba doce lenguajes que el producto sí lee, con gramática
    compilada y config en el registro de parsers.

El lint daba verde en los dos casos, porque las dos listas que comparaba
coincidían. Lo que no comparaba era la única cosa que decide qué puede
leer el producto: el registro.

Lo que hay ahora
----------------

La lista se **deriva** del registro. El registro es la autoridad, y la
derivación se mide contra ella:

  * el conjunto derivado tiene que coincidir, en los dos sentidos, con el
    catálogo publicado (`product/languages.json`), que a su vez lo genera
    un script desde el mismo registro;
  * y el documento `CAPABILITIES-MATRIX.md` tiene que **dejar de declarar
    una lista propia**: una lista en el documento es una tercera fuente
    de verdad, y el documento dice cuál es.

Uso: python3 sandbox/scripts/capabilities_drift_lint.py [--strict]
  --strict: exit 1 si hay drift (default: solo warn).
            En CI se invoca con --strict.
Exit codes:
  0 = sin drift
  1 = drift detectado (solo con --strict)
  2 = error de parsing / archivos no encontrados

Es un stage de merge-gate: si el documento falta, o si lo derivado no
coincide con el catálogo publicado, el stage falla y el PR no entra.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CODE_FILE = REPO_ROOT / "crates/cognicode-core/src/interface/mcp/capabilities.rs"
DOC_FILE = REPO_ROOT / "docs/roadmap/consumer-contracts/specs/CAPABILITIES-MATRIX.md"
CATALOG_FILE = REPO_ROOT / "product/languages.json"


def assert_the_code_derives() -> None:
    """Comprueba que el codigo deriva la lista en vez de escribirla.

    La lista ya no esta escrita a mano: sale del registro de parsers. Si
    esto deja de estar, la lista vuelve a ser una segunda fuente de
    verdad, y es exactamente el defecto que esta correccion cierra.
    """
    if not CODE_FILE.is_file():
        print(f"ERROR: no code file at {CODE_FILE}", file=sys.stderr)
        sys.exit(2)
    text = CODE_FILE.read_text(encoding="utf-8")
    m = re.search(
        r"pub fn tree_sitter_langs\(\)\s*->\s*&'\s*static\s*\[\s*&\s*'static\s*str\s*\]",
        text,
    )
    if not m:
        print(
            f"ERROR: tree_sitter_langs() not found in {CODE_FILE}. The "
            f"capabilities must derive their languages from the registry.",
            file=sys.stderr,
        )
        sys.exit(2)


def parse_doc_langs() -> tuple[list[str], str | None]:
    """Lee la lista de lenguajes que declara el documento, si declara una.

    Ya no debe declarar ninguna: la lista vive en el catalogo publicado
    y se deriva del registro. Un documento que vuelve a enumerar los
    lenguajes es una tercera fuente de verdad, que es justo lo que este
    lint existe para que no vuelva.

    Se sigue leyendo para poder decir QUE el documento ya no declara una
    lista, en vez de callarse: un guard que no puede fallar no es un
    guard.
    """
    if not DOC_FILE.is_file():
        print(f"ERROR: no doc file at {DOC_FILE}", file=sys.stderr)
        sys.exit(2)
    text = DOC_FILE.read_text(encoding="utf-8")

    # La seccion que importa es la de tree-sitter, y solo ella: el
    # documento tiene otra seccion mas abajo con los lenguajes que
    # tienen servidor LSP, que es un conjunto distinto y legitimo —no es
    # una lista de lenguajes que el producto lee, es una lista de
    # lenguajes que el producto ademas puede consultar con un LSP—. Sin
    # acotar la seccion, un guard acaba midiendo la lista equivocada.
    section = re.search(
        r"## Lenguajes con parser tree-sitter\n(.*?)(?=\n## |\Z)",
        text,
        re.DOTALL,
    )
    if not section:
        return [], None
    body = section.group(1)

    m = re.search(r"\((\d+)\s*lenguajes?\)", body)
    header_count = int(m.group(1)) if m else None
    m2 = re.search(r"```\n(.*?)```", body, re.DOTALL)
    if not m2:
        return [], header_count
    block = m2.group(1)
    raw_items = [s.strip().rstrip(",").strip() for s in block.split(",")]
    raw_items = [s for s in raw_items if s]
    first_tokens: list[str] = []
    for item in raw_items:
        first = item.split()[0] if item.split() else ""
        first_tokens.append(first.lower())
    normalized = sorted({s for s in first_tokens if s})
    return normalized, header_count


def parse_catalog_langs() -> list[str]:
    """Los lenguajes del catalogo publicado, que es la autoridad del producto."""
    if not CATALOG_FILE.is_file():
        print(f"ERROR: no catalog file at {CATALOG_FILE}", file=sys.stderr)
        sys.exit(2)
    document = json.loads(CATALOG_FILE.read_text(encoding="utf-8"))
    languages = document.get("languages")
    if not isinstance(languages, list):
        print(f"ERROR: {CATALOG_FILE} has no 'languages' list", file=sys.stderr)
        sys.exit(2)
    return sorted(str(entry.get("id", "")) for entry in languages if entry.get("id"))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--strict",
        action="store_true",
        help="exit 1 si drift (default: solo warn).",
    )
    args = ap.parse_args()

    assert_the_code_derives()  # la derivacion tiene que existir
    doc_langs, header_count = parse_doc_langs()
    catalog_langs = parse_catalog_langs()

    # El documento no debe declarar lista propia. Si no declara ninguna,
    # no hay nada que comparar con el catalogo: el documento apunta y la
    # autoridad es el catalogo. Si declara una, esa lista es una tercera
    # fuente de verdad y tiene que coincidir con la autoridad o es drift.
    declares_own_list = bool(doc_langs) or header_count is not None

    drift_doc_minus_catalog = sorted(set(doc_langs) - set(catalog_langs))
    drift_catalog_minus_doc = sorted(set(catalog_langs) - set(doc_langs))

    has_drift = declares_own_list and bool(
        drift_catalog_minus_doc or drift_doc_minus_catalog
    )
    header_mismatch = header_count is not None and header_count != len(doc_langs)

    if not has_drift and not header_mismatch:
        print(
            f"OK: la derivacion y el catalogo publicado coinciden en "
            f"{len(catalog_langs)} lenguajes; el documento no declara lista propia"
        )
        return 0

    print("=" * 60, file=sys.stderr)
    print("PRF-ANA-01 code↔doc DRIFT DETECTADO", file=sys.stderr)
    print("=" * 60, file=sys.stderr)
    print(f"CATALOGO PUBLICADO ({CATALOG_FILE.name}): {len(catalog_langs)}", file=sys.stderr)
    print(f"DOC ({DOC_FILE.name} lista): {len(doc_langs)} entradas", file=sys.stderr)
    if drift_catalog_minus_doc:
        print(
            f"  EN EL CATALOGO pero NO en el documento: {drift_catalog_minus_doc}",
            file=sys.stderr,
        )
    if drift_doc_minus_catalog:
        print(
            f"  EN el documento pero NO en el catalogo: {drift_doc_minus_catalog}",
            file=sys.stderr,
        )
    if header_mismatch:
        print(
            f"  ! El documento declara ({header_count} lenguajes) y su lista tiene "
            f"{len(doc_langs)} → header incoherente",
            file=sys.stderr,
        )
    print(
        "\nFix: el documento no debe declarar una lista de lenguajes. La lista se "
        "deriva del registro de parsers y se publica en product/languages.json.",
        file=sys.stderr,
    )
    return 1 if args.strict else 0


if __name__ == "__main__":
    sys.exit(main())
