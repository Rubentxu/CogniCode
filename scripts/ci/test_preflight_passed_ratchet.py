#!/usr/bin/env python3
"""The preflight's passing-count ratchet must fail when tests disappear, not grow.

MEDIDO 2026-10-03, certificando el tag v0.101.0. La batería en el clon limpio
dio `passed=5934 failed=0 ignored=30` contra un baseline de
`passed=5579 failed=0 ignored=37`, y el preflight lo rechazó:

    diff:     passed=355 failed=0 ignored=-7 (tolerancia ±2)
    FAIL: regresión de tests passed: delta=355 > tolerancia=2

Cero tests fallando, y el gate lo llamó regresión. La condición era

    if [ "${PASSED_DIFF#-}" -gt "$TOLERANCE" ]; then

`${PASSED_DIFF#-}` quita el signo, así que comparaba el **valor absoluto**: +355 y
-355 se trataban igual. El mensaje y la aritmética decían cosas distintas, y las
dos lecturas son defendibles por separado, que es exactamente el problema:

- Si la intención era "el número de tests no debería moverse mucho", el mensaje
  dice otra cosa — y en un repositorio cuya agenda declarada es añadir tests, un
  guard que falla cuando la suite crece está rojo por el trabajo que se le pidió
  hacer. El `ignored` cayendo de 37 a 30 encaja: siete tests que estaban
  `#[ignore]` se ejecutan ahora.
- Si la intención era "no perdemos tests", el fallo depende solo de tests que
  desaparecen, y `OBSERVED_FAILED > 0` cubre el otro caso dos líneas más abajo.

Se conserva la lectura que el mensaje describe. Una regresión es que el número de
tests **baje**.

## Lo que este contrato NO hace

No re-baselinea. El baseline es del operador, y el propio script lo dice
(`PREFLIGHT_BASELINE_*`, "requiere re-baseline explícito"). Con un baseline por
debajo de la realidad este ratchet queda débil: una caída grande de la suite real
seguiría dentro de tolerancia mientras el total no baje del baseline. Arreglar el
signo no arregla eso, y este contrato no finge que sí — sólo comprueba la
dirección del guard.

Run:
    python3 scripts/ci/test_preflight_passed_ratchet.py
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "ci" / "preflight-clean-clone.sh"
SEAM = "--print-passed-verdict"
BASELINE_SEAM = "--print-clone-ref"

# El baseline que encontró el fallo, y el observado real de aquel certifying run.
MEASURED_BASELINE = 5579
MEASURED_OBSERVED = 5934

PROBE_TIMEOUT_SECONDS = 20
SEAM_BUDGET_SECONDS = 30


def script_code() -> str:
    """The script's executable lines, comments removed.

    A comment that quotes a command is indistinguishable from code that runs it,
    in both directions — a comment can satisfy an assertion about behaviour, and
    can break one about text. Same reason `pipeline_authority.strip_line_comments`
    exists.
    """
    text = SCRIPT.read_text(encoding="utf-8")
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )


def verdict(observed: int, baseline: int = MEASURED_BASELINE) -> str:
    """'OK', 'REGRESSION', or 'SEAM-ABSENT'.

    The seam is checked by inspection first. Without that, a script that lacks
    it reads the flag as a SHA and runs the whole preflight — a real clone of
    this repository, twenty minutes of build — before the contract could say
    anything. A contract whose red state costs more than the gate it watches is a
    contract nobody runs on a PR.
    """
    code = script_code()
    if SEAM not in code:
        return "SEAM-ABSENT"
    if f"passed_count_regressed()" not in code:
        return "SEAM-ABSENT"

    started = time.monotonic()
    try:
        done = subprocess.run(
            ["bash", str(SCRIPT), SEAM, str(observed)],
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            timeout=PROBE_TIMEOUT_SECONDS,
            env={
                **dict(__import__("os").environ),
                "PREFLIGHT_BASELINE_PASSED": str(baseline),
                "PREFLIGHT_TOLERANCE": "2",
            },
        )
    except subprocess.TimeoutExpired:
        raise AssertionError(
            f"{SEAM} did not answer within {PROBE_TIMEOUT_SECONDS}s. The seam "
            "exists so this contract does not have to run a clean clone; if it "
            "runs one, this test silently becomes a 20-minute test"
        ) from None
    elapsed = time.monotonic() - started
    if elapsed > SEAM_BUDGET_SECONDS:
        raise AssertionError(f"{SEAM} took {elapsed:.1f}s, over the seam budget")

    first = done.stdout.strip().splitlines()[-1] if done.stdout.strip() else ""
    if first not in ("OK", "REGRESSION"):
        raise AssertionError(
            f"{SEAM} printed {first!r} instead of OK|REGRESSION.\n"
            f"stdout: {done.stdout!r}\nstderr: {done.stderr!r}"
        )
    return first


def test_more_passing_tests_is_not_a_regression() -> None:
    """The regression, on the numbers that caused it.

    355 more tests passing, zero failing, is the direction this repository has
    been moving on purpose.
    """
    got = verdict(MEASURED_OBSERVED)
    assert got == "OK", (
        f"growing the suite by {MEASURED_OBSERVED - MEASURED_BASELINE} passing "
        f"tests is reported as {got}. The guard compares the absolute value, so "
        "a project that adds tests is gated by the work it was asked to do. A "
        "regression is tests disappearing, not tests arriving"
    )


def test_fewer_passing_tests_is_a_regression() -> None:
    """The other direction still gates, or the fix is just 'never fail'."""
    lost = 355
    got = verdict(MEASURED_BASELINE - lost)
    assert got == "REGRESSION", (
        f"losing {lost} passing tests is reported as {got}. Fixing the sign must "
        "not have removed the ratchet; a guard that cannot fail is a file, not a "
        "gate"
    )


def test_movement_within_tolerance_passes_in_both_directions() -> None:
    """Churn is allowed, which is what the tolerance was for."""
    for observed, why in (
        (MEASURED_BASELINE + 2, "two more"),
        (MEASURED_BASELINE + 1, "one more"),
        (MEASURED_BASELINE - 1, "one fewer"),
        (MEASURED_BASELINE - 2, "two fewer"),
    ):
        got = verdict(observed)
        assert got == "OK", f"{why} passing test(s) is {got}, not OK"


def test_just_past_tolerance_fails_in_the_losing_direction_only() -> None:
    """The boundary is where the tolerance says it is."""
    assert verdict(MEASURED_BASELINE - 3) == "REGRESSION", (
        "three fewer passing tests is inside the stated tolerance of 2"
    )
    assert verdict(MEASURED_BASELINE + 3) == "OK", (
        "three more passing tests is still not a regression"
    )


def test_the_failure_message_describes_the_direction_it_checks() -> None:
    """The message and the arithmetic must agree.

    This is the defect itself: `regresión de tests passed` was printed for a
    growth. Whatever the guard checks, the sentence a human reads at 3am has to
    name the same direction.
    """
    code = script_code()
    assert "regresión de tests passed" not in code, (
        "the old two-sided guard's message is still in the code. It called a "
        "growth a regression, and that contradiction is the bug this contract "
        "exists for. (Checked against code, not the whole file: the comment "
        "above the function quotes that sentence on purpose, and a check that "
        "fails on its own documentation is a check that gets deleted)"
    )
    assert "por debajo del baseline" in code, (
        "the guard must name the direction it actually checks, so the message "
        "and the comparison describe the same property"
    )
    assert "re-baseline" in code, (
        "the failure must say what to do when the drop was intentional, "
        "including the variables that re-baseline it. A guard that fails "
        "without saying how to proceed is a gate people route around"
    )


def test_the_rule_is_defined_once_and_called_by_the_comparison() -> None:
    """One definition, one caller.

    The rule was inline in stage 6; making it a function is what lets a contract
    reach it, and a function nobody calls is a function that no longer describes
    the gate.
    """
    code = script_code()
    assert code.count("passed_count_regressed()") == 1, (
        "the function is defined more than once, so the copy the contract reads "
        "and the copy stage 6 calls can drift"
    )
    assert 'if passed_count_regressed "$BASELINE_PASSED" "$OBSERVED_PASSED"' in code, (
        "stage 6 no longer calls the shared function, so the rule under test and "
        "the rule the preflight runs could disagree"
    )
    # The absolute-value idiom is the bug itself and must not come back.
    assert "PASSED_DIFF#-" not in code, (
        "'${PASSED_DIFF#-}' strips the sign and makes the check two-sided again"
    )


def test_the_seam_answers_in_bounded_time() -> None:
    """Cheap in green, so it can be run on a PR.

    Asserted here as its own test rather than only inside `verdict` so that a
    future change to the seam's cost is a named failure rather than a slow suite
    somebody starts skipping.
    """
    started = time.monotonic()
    got = verdict(MEASURED_OBSERVED)
    elapsed = time.monotonic() - started
    assert got != "SEAM-ABSENT", (
        f"{SEAM} is not implemented in the script's code. The comparison is "
        "otherwise only reachable after a full clean-clone preflight, which "
        f"means it is untested; {BASELINE_SEAM} is the pattern to copy"
    )
    assert elapsed < SEAM_BUDGET_SECONDS, f"the seam took {elapsed:.1f}s"


def main() -> int:
    failures: list[str] = []
    for func in (
        test_more_passing_tests_is_not_a_regression,
        test_fewer_passing_tests_is_a_regression,
        test_movement_within_tolerance_passes_in_both_directions,
        test_just_past_tolerance_fails_in_the_losing_direction_only,
        test_the_failure_message_describes_the_direction_it_checks,
        test_the_rule_is_defined_once_and_called_by_the_comparison,
        test_the_seam_answers_in_bounded_time,
    ):
        try:
            func()
        except AssertionError as exc:
            failures.append(str(exc))
        except Exception as exc:  # noqa: BLE001 - a check failing is data
            failures.append(f"{func.__name__} raised {type(exc).__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(failure)
        return 1
    print(
        "PASS — the ratchet fails when passing tests disappear and passes when "
        f"they grow; +{MEASURED_OBSERVED - MEASURED_BASELINE} is OK and "
        f"-355 is a regression."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
