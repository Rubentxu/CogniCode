#!/usr/bin/env python3
"""Contrato de `check_known_failures.prepare_tmpdir` y su consumidor.

RED 2026-10-14 (N+114). `prepare_tmpdir` devuelve `(None, reason)` cuando el
root no vale, pero `main` lo desempaqueta como `(root_used, scratch)` y
comprueba `scratch is None`. Como en el fallo `scratch` es *el texto del
error*, esa comprobacion da False y el script sigue adelante con:

    child_env = {**os.environ, "TMPDIR": scratch}

Es decir,TMPDIR recibe un mensaje en prosa partido por espacios. De ahi:

  1. los 16 falsos "NEW REGRESSIONS" (los tests corren contra un TMPDIR
     relativo/invalido y fallan por eso, no por regresion), y
  2. un directorio basura creado dentro del repo, nombrado con el mensaje.

El baseline declara entradas ausentes porque el ruido las tapa, y el gate
acaba rechazando una suite que en realidad esta verde. Un contrato que
devuelve el motivo del fallo por el mismo canal que el valor es un contrato
que no se puede distinguir del exito.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest import mock

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "check_known_failures.py"


def _load_module():
    spec = importlib.util.spec_from_file_location("ckf_under_test", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PrepareTmpdirContract(unittest.TestCase):
    """`prepare_tmpdir` no debe poder devolver un motivo por el canal del valor."""

    def setUp(self) -> None:
        self.m = _load_module()

    def test_success_returns_scratch_dir_that_exists(self) -> None:
        root_used, scratch = self.m.prepare_tmpdir(str(REPO_ROOT))
        self.assertIsNotNone(scratch, "un root valido debe devolver scratch")
        self.assertTrue(
            Path(scratch).is_dir(),
            f"el scratch debe existir como directorio: {scratch!r}",
        )
        self.assertIsNotNone(root_used)

    def test_missing_root_is_reported_not_used_as_a_path(self) -> None:
        """Un root inexistente no puede devolver el motivo como si fuera el valor.

        Contrato documentado: exito es `(root, scratch)`, fallo es
        `(None, motivo)`. El centinela va SIEMPRE en el primer campo, y el
        motivo se devuelve en el segundo como texto, nunca como ruta.
        """
        missing = REPO_ROOT / "no-existe-este-directorio"
        root_used, reason = self.m.prepare_tmpdir(str(missing))
        self.assertIsNone(
            root_used,
            f"un root inexistente debe dar root_used=None; devolvio {root_used!r}",
        )
        self.assertIsInstance(reason, str, "el motivo debe ser texto")
        self.assertIn("no-existe-este-directorio", reason)
        self.assertFalse(
            Path(reason).is_dir(),
            f"el motivo no debe ser un directorio utilizable: {reason!r}",
        )


class MainRejectsBadTmpdir(unittest.TestCase):
    """`main` debe abortar (rc=2) en vez de exportar el motivo como TMPDIR."""

    def setUp(self) -> None:
        self.m = _load_module()

    def _run_with(self, tmpdir_root: str) -> int:
        argv = ["check_known_failures.py", "--package", "cognicode-core", "--target", "lib"]
        if tmpdir_root is not None:
            argv += ["--tmpdir-root", tmpdir_root]
        with mock.patch.object(sys, "argv", argv), mock.patch.dict(
            "os.environ", {}, clear=False
        ):
            return int(self.m.main())

    def test_missing_tmpdir_root_exits_2_without_running_cargo(self) -> None:
        missing = REPO_ROOT / "no-existe-este-directorio"
        with mock.patch.object(self.m, "run_tests") as run_tests:
            rc = self._run_with(str(missing))
        self.assertEqual(rc, 2, "un TMPDIR invalido debe dar rc=2")
        run_tests.assert_not_called()

    def test_symlinked_tmpdir_root_exits_2_without_running_cargo(self) -> None:
        """El caso real: tempfile.gettempdir() puede cruzar un symlink."""
        with mock.patch.object(self.m, "_path_has_ancestor_symlink", return_value=True):
            with mock.patch.object(self.m, "run_tests") as run_tests:
                rc = self._run_with(str(REPO_ROOT))
        self.assertEqual(rc, 2, "un root con ancestro symlink debe dar rc=2")
        run_tests.assert_not_called()


if __name__ == "__main__":
    unittest.main(verbosity=2)
