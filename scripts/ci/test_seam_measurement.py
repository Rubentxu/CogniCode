#!/usr/bin/env python3
"""Contrato: la medicion de seams de WorkspaceSession no puede caducar en silencio.

CGH-010 acordo que ninguna extraccion del seam ocurre sin medicion previa, y
`gitignore` solo permite versionar un registro que un gate lee. Este contrato
es ese gate:

  1. El informe versionado existe y esta tracking (la politica de
     `docs/debts`: se versiona si y solo si un gate lo lee).
  2. La medicion actual reproduce el informe (`--check`), o el contrato
     esta en rojo: la regla de extraccion deja de poder consultarse el dia
     que el informe miente, y un informe que miente en silencio es peor
     que no tenerlo.

El instrumento mide en menos de un segundo sobre el arbol caliente
(MEDIDO 2026-11-16: 0.69 s); el coste no es una razon para sacarlo del
batch de contratos.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

TOOL = REPO_ROOT / "scripts" / "measure_workspace_session.py"
DOC = REPO_ROOT / "docs" / "debts" / "SEAM-WORKSPACE-SESSION.md"


def test_measurement_doc_is_tracked():
    """La politica de docs/debts: tracked solo si un gate lo lee — y ese
    gate es este contrato, asi que leerlo sin estar versionado seria la
    excepcion contradictoria."""
    out = subprocess.run(
        ["git", "ls-files", "--error-unmatch",
         str(DOC.relative_to(REPO_ROOT))],
        cwd=REPO_ROOT, capture_output=True, text=True, timeout=60,
    )
    assert out.returncode == 0, (
        f"{DOC.name} no esta versionado: o se force-add (porque este gate "
        f"lo lee) o el informe es local y no puede afirmar la regla"
    )


def test_measurement_reproduces_the_doc():
    """--check compara el informe con una medicion nueva: drift del
    mapa de grupos, metodos publicos sin grupo o caducidad del
    veredicto READY dejan el contrato rojo."""
    proc = subprocess.run(
        [sys.executable, str(TOOL), "--check"],
        cwd=REPO_ROOT, capture_output=True, text=True, timeout=120,
    )
    assert proc.returncode == 0, (
        f"la medicion y el informe han divergido:\n{proc.stderr.strip()}\n"
        f"Regenera con: python3 {TOOL.relative_to(REPO_ROOT)} --write"
    )
