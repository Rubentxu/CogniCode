#!/usr/bin/env python3
"""PRF-ANA-01 code↔doc lint: detecta drift entre `ALL_TREE_SITTER_LANGS`
(constantes del crate cognicode-core) y la lista en
`docs/prf/specs/CAPABILITIES-MATRIX.md`.

Uso: python3 sandbox/scripts/capabilities_drift_lint.py [--strict]
  --strict: exit 1 si drift detectado (default: solo warn).
            En CI se invoca con --strict.
Exit codes:
  0  = sin drift detectado
  1  = drift detectado (solo con --strict)
  2  = error de parsing / archivos no encontrados

Este lint cierra el riesgo de drift código↔doc detectado durante la
auditoría E0.W1 (sesión 2026-09-25, JOURNAL §4) y propuesto como
E0.W2: una CI job que ejecuta este script contra todo PR que toque
`crates/.../capabilities.rs` o `CAPABILITIES-MATRIX.md`.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CODE_FILE = REPO_ROOT / "crates/cognicode-core/src/interface/mcp/capabilities.rs"
DOC_FILE = REPO_ROOT / "docs/prf/specs/CAPABILITIES-MATRIX.md"


def parse_code_langs() -> list[str]:
    """Extrae la lista `ALL_TREE_SITTER_LANGS` del código fuente."""
    if not CODE_FILE.is_file():
        print(f"ERROR: no code file at {CODE_FILE}", file=sys.stderr)
        sys.exit(2)
    text = CODE_FILE.read_text(encoding="utf-8")
    m = re.search(
        r"pub const ALL_TREE_SITTER_LANGS:\s*&\[\s*&\s*str\s*\]\s*=\s*&\[\s*(.*?)\s*\];",
        text,
        re.DOTALL,
    )
    if not m:
        print(f"ERROR: ALL_TREE_SITTER_LANGS not found in {CODE_FILE}", file=sys.stderr)
        sys.exit(2)
    block = m.group(1)
    # Cada entrada es "identificador" o "identificador", (con coma al final).
    langs = re.findall(r'"([a-z]+)"', block)
    return sorted(langs)


def parse_doc_langs() -> tuple[list[str], str | None]:
    """Extrae la lista de lenguajes del bloque bajo `## Lenguajes con
    parser tree-sitter` en el doc. Devuelve (lista_normalizada, header_count_o_None).

    El header típico:
        ## Lenguajes con parser tree-sitter

        Lista oficial inferida de (...) (N lenguajes):

        ```
        Python, Rust, JavaScript, ...
        ```

    Devuelve la lista dentro del primer ``` ... ``` posterior al header,
    normalizada a lowercase. Si el header incluye "(N lenguajes)" con un N
    distinto a len(lista), el lint falla (header incoherente).
    """
    if not DOC_FILE.is_file():
        print(f"ERROR: no doc file at {DOC_FILE}", file=sys.stderr)
        sys.exit(2)
    text = DOC_FILE.read_text(encoding="utf-8")
    # El header "(N lenguajes):" puede estar en su propia línea o en la
    # misma línea que el texto introductorio. Probamos ambos.
    m = re.search(
        r"## Lenguajes con parser tree-sitter.*?\((\d+)\s*lenguajes?\)",
        text,
        re.DOTALL,
    )
    header_count = int(m.group(1)) if m else None
    # Primer fenced code block después del header.
    m2 = re.search(
        r"## Lenguajes con parser tree-sitter.*?```\n(.*?)```",
        text,
        re.DOTALL,
    )
    if not m2:
        return [], header_count
    block = m2.group(1)
    # Cada elemento es "Python" o "JSX" o "TSX" o "CSharp" o
    # "Hcl (Terraform)" (nota parentética inline al final). Extraemos
    # solo la primera palabra (lower-cased).
    raw_items = [s.strip().rstrip(",").strip() for s in block.split(",")]
    raw_items = [s for s in raw_items if s]
    # Quitar nota parentética final si existe, y solo quedarnos con el
    # primer token (el lenguaje). Esto preserva "Hcl (Terraform)" -> "hcl".
    first_tokens: list[str] = []
    for item in raw_items:
        first = item.split()[0] if item.split() else ""
        first_tokens.append(first.lower())
    normalized = sorted({s for s in first_tokens if s})
    return normalized, header_count


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--strict",
        action="store_true",
        help="exit 1 si drift (default: solo warn)",
    )
    args = ap.parse_args()

    code_langs = parse_code_langs()
    doc_langs, header_count = parse_doc_langs()

    drift_code_minus_doc = sorted(set(code_langs) - set(doc_langs))
    drift_doc_minus_code = sorted(set(doc_langs) - set(code_langs))

    has_drift = bool(drift_code_minus_doc or drift_doc_minus_code)
    header_mismatch = header_count is not None and header_count != len(doc_langs)

    if not has_drift and not header_mismatch:
        print(
            f"OK: code↔doc aligned, {len(code_langs)} lenguajes "
            f"({code_langs})"
        )
        return 0

    # Reportar drift.
    print("=" * 60, file=sys.stderr)
    print("PRF-ANA-01 code↔doc DRIFT DETECTADO", file=sys.stderr)
    print("=" * 60, file=sys.stderr)
    print(f"Code (ALL_TREE_SITTER_LANGS): {len(code_langs)} entradas", file=sys.stderr)
    for lang in code_langs:
        print(f"  + {lang}", file=sys.stderr)
    print(f"DOC (CAPABILITIES-MATRIX.md lista): {len(doc_langs)} entradas", file=sys.stderr)
    for lang in doc_langs:
        print(f"  - {lang}", file=sys.stderr)
    if header_count is not None and header_count != len(doc_langs):
        print(
            f"  ! Header dice ({header_count} lenguajes) pero la lista "
            f"tiene {len(doc_langs)} → header incoherente",
            file=sys.stderr,
        )
    if drift_code_minus_doc:
        print(
            f"  EN CÓDIGO pero NO en DOC: {drift_code_minus_doc}",
            file=sys.stderr,
        )
    if drift_doc_minus_code:
        print(
            f"  EN DOC pero NO en CÓDIGO: {drift_doc_minus_code}",
            file=sys.stderr,
        )
    print(
        "\nFix: actualiza docs/prf/specs/CAPABILITIES-MATRIX.md para "
        "que la lista de lenguajes (y el header 'N lenguajes') coincida "
        "con ALL_TREE_SITTER_LANGS en crates/cognicode-core/src/"
        "interface/mcp/capabilities.rs.",
        file=sys.stderr,
    )
    return 1 if args.strict else 0


if __name__ == "__main__":
    sys.exit(main())
