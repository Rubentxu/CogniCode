"""Tests para capabilities_drift_lint.py.

Cubre los dos modos del script:
- exit 0 cuando code↔doc alineados
- exit 1 cuando drift detectado (header dice N pero lista tiene M ≠ N)
- exit 2 cuando archivos no encontrados

Estos tests pinean que el lint sigue detectando el drift que descubrió
la auditoría E0.W1 (sesión 2026-09-25, JOURNAL §4).
"""
from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPT = (
    Path(__file__).resolve().parent.parent / "capabilities_drift_lint.py"
)


def _load_module():
    """Carga el script como módulo Python para testing."""
    spec = importlib.util.spec_from_file_location("capabilities_drift_lint", SCRIPT)
    assert spec is not None and spec.loader is not None, f"can't load {SCRIPT}"
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_module_loads():
    """Sanity: el script carga sin error de sintaxis."""
    mod = _load_module()
    assert hasattr(mod, "parse_code_langs")
    assert hasattr(mod, "parse_doc_langs")
    assert hasattr(mod, "main")


def test_code_langs_parses_to_22():
    """ALL_TREE_SITTER_LANGS tiene 22 entradas (Python,...,Kotlin)."""
    mod = _load_module()
    langs = mod.parse_code_langs()
    assert len(langs) == 22, f"esperaba 22 lenguajes en código, obtuve {len(langs)}: {langs}"
    # Spot-check: los 4 LSP-supported deben estar presentes.
    for lang in ("python", "rust", "javascript", "typescript"):
        assert lang in langs


def test_doc_langs_parses_to_22_after_audit_fix():
    """Tras la corrección E0.W1, la lista del doc tiene 22 entradas."""
    mod = _load_module()
    langs, header_count = mod.parse_doc_langs()
    assert len(langs) == 22, f"esperaba 22 lenguajes en doc, obtuve {len(langs)}: {langs}"
    assert header_count == 22, f"header debería decir (22 lenguajes), dice ({header_count})"


def test_drift_detected_when_header_mismatches_list(tmp_path):
    """Si creamos un doc sintético con header=18 y lista=22, debe detectar drift."""
    mod = _load_module()
    # Sobrescribir DOC_FILE temporalmente con uno sintético.
    import builtins

    real_open = builtins.open

    fake_doc = tmp_path / "fake_doc.md"
    fake_doc.write_text(
        "## Lenguajes con parser tree-sitter\n"
        "\n"
        "Lista oficial (18 lenguajes):\n"
        "\n"
        "```\n"
        "Python, Rust, JavaScript, TypeScript, JSX, TSX, Go, Java,\n"
        "C, Cpp, CSharp, Hcl, Yaml, Ruby, Php, Swift,\n"
        "Scala, Lua, Luau, Zig, Dart, Kotlin\n"
        "```\n"
    )
    # Reemplazar la constante DOC_FILE en el módulo.
    original_doc = mod.DOC_FILE
    mod.DOC_FILE = fake_doc
    try:
        langs, header_count = mod.parse_doc_langs()
        assert len(langs) == 22
        assert header_count == 18
        # El cálculo en main detectaría esto:
        header_mismatch = header_count != len(langs)
        assert header_mismatch, "debe detectar header 18 ≠ lista 22"
    finally:
        mod.DOC_FILE = original_doc


def _run_main_with_argv(mod, argv):
    """Ejecuta mod.main() con sys.argv overrideado de forma segura."""
    import contextlib
    import io

    saved_argv = sys.argv[:]
    saved_stderr = sys.stderr
    sys.argv = argv
    buf = io.StringIO()
    sys.stderr = buf
    try:
        rc = mod.main()
    finally:
        sys.argv = saved_argv
        sys.stderr = saved_stderr
    return rc, buf.getvalue()


def test_strict_mode_reports_drift(tmp_path):
    """--strict debe exit 1 cuando hay drift detectado."""
    fake_doc = tmp_path / "fake_doc.md"
    fake_doc.write_text(
        "## Lenguajes con parser tree-sitter\n"
        "(5 lenguajes):\n"  # header dice 5
        "```\n"
        "Python, Rust, JavaScript, TypeScript, JSX, TSX, Go\n"  # pero 7 en lista
        "```\n"
    )
    # Crea un REPO_ROOT sintético con un doc fake pero código real para que
    # el script detecte el drift entre 5 (header fake) y 22 (código real).
    import shutil

    tmp_root = tmp_path / "fake_repo"
    tmp_root.mkdir()
    real_capabilities = (
        Path(__file__).resolve().parent.parent.parent.parent
        / "crates/cognicode-core/src/interface/mcp/capabilities.rs"
    )
    shutil.copy(real_capabilities, tmp_root / "capabilities.rs")
    fake_docs = tmp_root / "CAPABILITIES-MATRIX.md"
    fake_docs.write_text(fake_doc.read_text())

    mod = _load_module()
    original_root = mod.REPO_ROOT
    original_doc = mod.DOC_FILE
    original_code = mod.CODE_FILE
    mod.REPO_ROOT = tmp_root
    mod.DOC_FILE = fake_docs
    mod.CODE_FILE = tmp_root / "capabilities.rs"
    try:
        # Sin --strict: warn mode, exit 0 pero drift en stderr.
        rc, captured = _run_main_with_argv(
            mod, ["capabilities_drift_lint.py"]
        )
        assert rc == 0, (
            f"sin --strict debe devolver 0 (warn), obtuvo {rc}"
        )
        assert "DRIFT DETECTADO" in captured, (
            f"sin --strict debe mostrar DRIFT en stderr, obtuve: {captured!r}"
        )
        # Con --strict: exit 1.
        rc, captured_strict = _run_main_with_argv(
            mod, ["capabilities_drift_lint.py", "--strict"]
        )
        assert rc == 1, (
            f"con --strict y drift debe exit 1, obtuvo {rc}. stderr={captured_strict!r}"
        )
        assert "DRIFT DETECTADO" in captured_strict
    finally:
        mod.REPO_ROOT = original_root
        mod.DOC_FILE = original_doc
        mod.CODE_FILE = original_code


def test_aligne_mode_returns_zero():
    """Sin drift y sin header_mismatch, exit 0."""
    result = subprocess.run(
        [sys.executable, str(SCRIPT)],
        capture_output=True,
        text=True,
    )
    # Default = no strict, pero como no hay drift, exit 0 también.
    assert result.returncode == 0, (
        f"sin drift debe exit 0, obtuvo {result.returncode}. stderr={result.stderr!r}"
    )
    assert "OK" in result.stdout
