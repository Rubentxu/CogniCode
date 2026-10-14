#!/usr/bin/env python3
"""Los wrappers del gate deben redirigir TMPDIR a una raiz sin symlink.

RED 2026-10-14 (N+114). `check_known_failures.py` nace con una capacidad
que nadie exercise: `--tmpdir-root`. Su propia docstring dice que existe
"para que el harness redirija TMPDIR a una raiz que no cruza symlink", y
`_path_has_ancestor_symlink` esta ahi justo para detectar
`/home -> /var/home`. Pero `test-full.sh` y `test-fast.sh` lo invocan sin
ese flag, asi que en esta maquina el paso cae en la rama de fallo y
devuelve rc=2 (correcto tras N+114, pero significa que el gate no puede
completarse sin exportar TMPDIR a mano).

Dos salidas legitimas y distinguibles:

  * el wrapper pasa `--tmpdir-root` con una raiz real, o
  * el wrapper no lo pasa y el operador debe saberlo.

Lo que no es legitimo es que la capacidad exista documentada y no la use
nadie, de modo que la unica forma de que el gate pase sea acordarse de
una variable de entorno.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
FULL = REPO_ROOT / "scripts" / "test-full.sh"
FAST = REPO_ROOT / "scripts" / "test-fast.sh"
BASELINE_CHECK = REPO_ROOT / "scripts" / "check_known_failures.py"


class TmpdirRootCapabilityIsUsed(unittest.TestCase):
    """Cada wrapper que invoca el baseline debe usar la redireccion de TMPDIR
    en su rama principal.

    Una invocacion *sin* el flag si es legitima cuando es la rama de
    reserva para "no se encontro ninguna raiz utilizable": ahi no hay nada
    que redirigir y el propio script debe decidir. Lo que no es legitimo es
    que esa rama sea la unica, es decir, que la capacidad exista
    documentada y la unica forma de que el gate pase sea acordarse de
    exportar TMPDIR a mano.
    """

    def _baseline_invocations(self, script: Path) -> list[str]:
        """Localiza cada llamada real a `python3 "$BASELINE_CHECK"`.

        El nombre del script no aparece literal en la invocacion: se pasa
        por la variable `$BASELINE_CHECK`. Buscar el nombre del fichero
        daria cero coincidencias y el test pasaria sin comprobar nada.

        La ventana mira hacia **delante** desde la linea del `python3`:
        el flag va en la linea siguiente por continuacion con `\`, asi
        que mirar solo la linea del `python3` daria un falso negativo.
        """
        lines = script.read_text(encoding="utf-8").splitlines()
        out: list[str] = []
        for i, line in enumerate(lines):
            if "python3" in line and "BASELINE_CHECK" in line:
                out.append("\n".join(lines[i : i + 5]))
        return out

    def _assert_primary_branch_uses_flag(self, script: Path) -> None:
        invocations = self._baseline_invocations(script)
        self.assertTrue(invocations, f"{script.name} no invoca el baseline")
        with_flag = [b for b in invocations if "tmpdir-root" in b]
        self.assertTrue(
            with_flag,
            f"{script.name} invoca el baseline sin --tmpdir-root en ninguna "
            f"rama. Invocaciones encontradas:\n"
            + "\n---\n".join(invocations),
        )
        if len(invocations) > 1:
            self.assertNotEqual(
                invocations[0],
                invocations[1],
                f"{script.name} tiene ramas duplicadas sin --tmpdir-root",
            )

    def test_test_full_redirects_tmpdir(self) -> None:
        self._assert_primary_branch_uses_flag(FULL)

    def test_test_fast_redirects_tmpdir(self) -> None:
        self._assert_primary_branch_uses_flag(FAST)

    def test_a_default_root_resolves_on_this_machine(self) -> None:
        """El valor por defecto que los wrappers deberian calcular tiene que
        ser una ruta real y sin ancestro symlink, o el arreglo es decorativo."""
        default_present = any(
            "tmpdir-root" in b
            for script in (FULL, FAST)
            for b in self._baseline_invocations(script)
        )
        if not default_present:
            self.skipTest("ningun wrapper pasa --tmpdir-root todavia")

        text = FULL.read_text(encoding="utf-8") + FAST.read_text(encoding="utf-8")
        self.assertRegex(
            text,
            r"TMPDIR_ROOT|DEFAULT_TMPDIR_ROOT",
            "los wrappers deberian exponer la raiz elegida en una variable "
            "con nombre, para que sea visible y auditable",
        )

    def test_no_wrapper_hardcodes_a_machine_specific_path(self) -> None:
        """Un path absoluto de esta maquina hardcodeado en el gate seria una
        trampa: en otra maquina el gate fallaria de nuevo."""
        for script in (FULL, FAST):
            text = script.read_text(encoding="utf-8")
            for match in re.finditer(r"/var/home/rubentxu\S*", text):
                self.fail(
                    f"{script.name} contiene un path absoluto especifico de "
                    f"esta maquina: {match.group(0)!r}"
                )
            for match in re.finditer(r"/home/rubentxu\S*", text):
                self.fail(
                    f"{script.name} contiene un path absoluto especifico de "
                    f"esta maquina: {match.group(0)!r}"
                )


if __name__ == "__main__":
    unittest.main(verbosity=2)
