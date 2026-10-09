#!/usr/bin/env python3
"""Que los wrappers de tests fast/full cumplan su contrato minimo.

MEDIDO 2026-10-10. AGENTS.md § 'Estrategia de tests (fast / full)' declara
una disciplina contractual: durante evolutivos, `bash scripts/test-fast.sh`;
antes de release, `bash scripts/test-full.sh`. El selector CR-08
(`scripts/ci/select-suites.sh`) ya pineaba que suite correr; el wrapper
lo expone al flujo del desarrollador.

Tres verificaciones que el contrato no admite que se rompan:

  (1) los dos wrappers existen y son ejecutables;
  (2) `test-fast.sh` delega en `select-suites.sh` y en
      `known_failures.yaml` para distinguir regresiones reales;
  (3) `test-full.sh` corre fmt + clippy `-D warnings` + tests --workspace
      + tests --workspace --doc (la bateria completa que el gate exige).

Sin estas guardas, una refactorizacion inocente podria borrar el wrapper
o cambiar su cableado y nadie se enteraria hasta que el gate fallase en
silencio. Pine los caminos, no los contenidos.
"""
from __future__ import annotations

import os
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
FAST = REPO_ROOT / "scripts" / "test-fast.sh"
FULL = REPO_ROOT / "scripts" / "test-full.sh"
SELECTOR = REPO_ROOT / "scripts" / "ci" / "select-suites.sh"
BASELINE = REPO_ROOT / "scripts" / "known_failures.yaml"
BASELINE_CHECK = REPO_ROOT / "scripts" / "check_known_failures.py"


def _read(path: Path) -> str:
    if not path.is_file():
        return ""
    return path.read_text(encoding="utf-8")


# ---------- assertions (each pins one mutation that the contract refuses) ----------


def test_fast_wrapper_exists_and_is_executable() -> None:
    """M1: scripts/test-fast.sh debe existir y ser ejecutable.

    Sin el binario, el desarrollador no puede invocar la disciplina fast.
    AGENTS.md § 'Estrategia de tests' lo referencia como comando canonico.
    """
    assert FAST.is_file(), f"FAIL: {FAST} no existe"
    assert os.access(FAST, os.X_OK), f"FAIL: {FAST} no es ejecutable"


def test_full_wrapper_exists_and_is_executable() -> None:
    """M2: scripts/test-full.sh debe existir y ser ejecutable.

    El gate de release exige bateria completa; el wrapper la expone.
    """
    assert FULL.is_file(), f"FAIL: {FULL} no existe"
    assert os.access(FULL, os.X_OK), f"FAIL: {FULL} no es ejecutable"


def test_fast_delegates_to_selector() -> None:
    """M3: test-fast.sh debe invocar select-suites.sh.

    El wrapper no recalcula la seleccion; consume la salida JSON del
    selector CR-08. Si lo cambia, se pierde la trazabilidad de la
    decision de que correr (cambio de paths -> cambio de suites).
    """
    text = _read(FAST)
    assert "select-suites.sh" in text, (
        f"FAIL: {FAST.name} no referencia scripts/ci/select-suites.sh. "
        f"El wrapper DEBE delegar en el selector CR-08; no recalcular."
    )


def test_fast_uses_known_failures_baseline() -> None:
    """M4: test-fast.sh debe aplicar known_failures.yaml sobre core/lib.

    La red minima que evita que una regresion nueva se pierda en el
    ruido conocido. Si la quita, el feedback de fast pierde precision.
    """
    text = _read(FAST)
    assert "check_known_failures.py" in text or "known_failures" in text, (
        f"FAIL: {FAST.name} no aplica scripts/check_known_failures.py. "
        f"El baseline check es la red minima contra regresiones nuevas."
    )


def test_full_runs_the_release_battery() -> None:
    """M5: test-full.sh debe correr fmt + clippy + tests + doc-tests.

    El gate de release exige la bateria completa. Si el wrapper omite
    alguno, una falla silente pasara hasta el merge.
    """
    text = _read(FULL)
    missing = []
    for needle in ("cargo fmt", "cargo clippy", "cargo test", "--doc"):
        if needle not in text:
            missing.append(needle)
    assert not missing, (
        f"FAIL: {FULL.name} no corre {missing}. "
        f"El gate de release exige fmt + clippy + test + doc-test."
    )


def test_full_invokes_known_failures_baseline() -> None:
    """M6: test-full.sh debe invocar check_known_failures al final.

    La bateria completa + baseline = exit 0 si no hay regresiones nuevas.
    Si omite el baseline, regresiones nuevas pasan en verde.
    """
    text = _read(FULL)
    assert "check_known_failures.py" in text or "known_failures" in text, (
        f"FAIL: {FULL.name} no invoca scripts/check_known_failures.py. "
        f"El baseline es la red contra regresiones nuevas."
    )


def test_both_wrappers_have_set_euo_pipefail() -> None:
    """M7: ambos wrappers deben usar `set -uo pipefail` (o equivalente).

    Sin errexit + nounset + pipefail, los wrappers silencian errores
    parciales (cargo falla con un test, el siguiente pasa y el rc final
    es 0). Disciplina minima de shell.
    """
    for path in (FAST, FULL):
        text = _read(path)
        assert "pipefail" in text, (
            f"FAIL: {path.name} no declara 'set -.*pipefail'. "
            f"Sin pipefail los rc parciales se silencian."
        )


def test_selector_dependency_exists() -> None:
    """M8: el selector CR-08 (dependencia de fast) debe existir.

    El wrapper lo invoca via bash $SELECTOR; si el selector desaparece,
    fast se rompe en runtime. Pine la dependencia.
    """
    assert SELECTOR.is_file(), (
        f"FAIL: {SELECTOR} no existe pero {FAST.name} lo invoca. "
        f"Discrepancia entre wrapper y dependencia declarada."
    )


def test_baseline_dependencies_exist() -> None:
    """M9: known_failures.yaml y check_known_failures.py deben existir.

    El baseline es parte de la red minima; si los ficheros desaparecen,
    el check se rompe en runtime.
    """
    assert BASELINE.is_file(), f"FAIL: {BASELINE} no existe"
    assert BASELINE_CHECK.is_file(), f"FAIL: {BASELINE_CHECK} no existe"


def main() -> int:
    failures: list[str] = []
    for fn in (
        test_fast_wrapper_exists_and_is_executable,
        test_full_wrapper_exists_and_is_executable,
        test_fast_delegates_to_selector,
        test_fast_uses_known_failures_baseline,
        test_full_runs_the_release_battery,
        test_full_invokes_known_failures_baseline,
        test_both_wrappers_have_set_euo_pipefail,
        test_selector_dependency_exists,
        test_baseline_dependencies_exist,
    ):
        try:
            fn()
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("test-fast/full wrappers contract: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    print(
        f"test-fast/full wrappers contract: OK "
        f"(fast={FAST.stat().st_size}B, full={FULL.stat().st_size}B)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())