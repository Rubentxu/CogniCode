"""Ejecutor de procesos: lanza el argv de una obligación y cuenta casos.

No interpreta "verde" a partir del exit code. TH-S03 avisa de que un 101 de Cargo
puede ser error de compilación, fallo de aserción o error de harness, así que aquí
se clasifica con marcadores de la salida estructurada, y ante la duda el resultado
es `INFRA_ERROR`, que no es PASS.

Un proceso sin evento terminal no es un PASS. Por eso el orquestador escribe el
recibo `RUNNING` **antes** de lanzar nada: si este proceso muere, queda constancia
de que había algo corriendo, y `resume` lo trata como interrumpido.
"""

from __future__ import annotations

import os
import re
import signal
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

RUNNING_TEST_RE = re.compile(r"^running (\d+) tests?", re.MULTILINE)
RESULT_RE = re.compile(
    r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;"
    r"(?: (\d+) measured;)?(?: (\d+) filtered out;)?",
    re.MULTILINE,
)
COMPILE_ERROR_MARKERS = (
    "error: could not compile",
    "error[E",
    "error: failed to",
    "error: no such command",
    "error: linking with",
    "wrapper script does not exist",
)
ASSERTION_MARKERS = ("test result: FAILED", "panicked at", "assertion", "failures:")
INFRA_MARKERS = (
    "No such file or directory",
    "cannot connect to the Docker daemon",
    "connection refused",
    "Could not find",
    "permission denied",
    "Killed",
)


@dataclass(frozen=True)
class CaseCounts:
    discovered: int = 0
    executed: int = 0
    passed: int = 0
    failed: int = 0
    skipped: int = 0

    def to_json(self) -> dict[str, int]:
        return {
            "discovered_cases": self.discovered,
            "executed_cases": self.executed,
            "passed_cases": self.passed,
            "failed_cases": self.failed,
            "skipped_cases": self.skipped,
        }


@dataclass(frozen=True)
class ProcessOutcome:
    status: str  # PASSED | FAILED | TIMED_OUT | CANCELLED | INFRA_ERROR
    exit_code: int
    counts: CaseCounts
    duration_s: float
    classification: str  # ok | assertion | build-error | infra | timeout | signal | no-evidence
    detail: str


def parse_counts(output: str) -> CaseCounts:
    """Suma `test result:` de todos los targets que cargo haya ejecutado.

    Una invocación con `--lib --bins` produce varios bloques, y quedarse con el
    primero contaría de menos: exactamente el error que convierte una suite a
    medias en un verde.
    """
    discovered = 0
    for match in RUNNING_TEST_RE.finditer(output):
        discovered += int(match.group(1))
    passed = failed = ignored = 0
    seen_result = False
    for match in RESULT_RE.finditer(output):
        seen_result = True
        passed += int(match.group(2))
        failed += int(match.group(3))
        ignored += int(match.group(4))
    executed = passed + failed + ignored if seen_result else 0
    return CaseCounts(
        discovered=discovered,
        executed=executed,
        passed=passed,
        failed=failed,
        skipped=ignored,
    )


def classify(exit_code: int, output: str, timed_out: bool) -> tuple[str, str]:
    """(estado, clasificación). Nunca devuelve PASSED si hubo un fallo real."""
    if timed_out:
        return "TIMED_OUT", "timeout"
    if exit_code < 0:
        return "CANCELLED", "signal"
    counts = parse_counts(output)
    if exit_code == 0:
        if counts.executed > 0 or counts.discovered > 0:
            return "PASSED", "ok"
        # Exit 0 sin ningún caso: puede ser un script legítimo (no hay `test
        # result:`) o una suite filtrada a cero. `no-evidence` deja el juicio al
        # consumidor, que exigirá casos a quien deba tenerlos.
        return "PASSED", "no-evidence"
    if counts.failed > 0:
        return "FAILED", "assertion"
    lowered = output
    if any(marker in lowered for marker in COMPILE_ERROR_MARKERS):
        return "INFRA_ERROR", "build-error"
    if any(marker in lowered for marker in ASSERTION_MARKERS):
        return "FAILED", "assertion"
    if any(marker in lowered for marker in INFRA_MARKERS):
        return "INFRA_ERROR", "infra"
    return "INFRA_ERROR", "infra"


def execute(
    argv: tuple[str, ...],
    *,
    cwd: Path,
    env_overrides: dict[str, str] | None = None,
    timeout_s: float | None = None,
    max_output_bytes: int = 8 << 20,
) -> tuple[ProcessOutcome, str]:
    """Ejecuta el argv y devuelve (resultado, salida completa capturada)."""
    env = dict(os.environ)
    env.update({"CARGO_TERM_COLOR": "never", "NO_COLOR": "1", "RUST_BACKTRACE": "1"})
    if env_overrides:
        env.update(env_overrides)
    started = time.monotonic()
    timed_out = False
    try:
        proc = subprocess.Popen(
            list(argv),
            cwd=str(cwd),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            env=env,
            start_new_session=True,
        )
    except OSError as exc:
        return (
            ProcessOutcome(
                status="INFRA_ERROR",
                exit_code=-1,
                counts=CaseCounts(),
                duration_s=0.0,
                classification="infra",
                detail=f"no se pudo lanzar el proceso: {exc}",
            ),
            "",
        )
    try:
        output, _ = proc.communicate(timeout=timeout_s)
        exit_code = proc.returncode
    except subprocess.TimeoutExpired:
        timed_out = True
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
        except (OSError, ProcessLookupError):
            proc.kill()
        try:
            output, _ = proc.communicate(timeout=30)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            except (OSError, ProcessLookupError):
                proc.kill()
            output, _ = proc.communicate()
        exit_code = proc.returncode if proc.returncode is not None else -15
    duration = time.monotonic() - started
    if len(output) > max_output_bytes:
        output = output[-max_output_bytes:]
    text = output.decode("utf-8", "replace")
    status, classification = classify(exit_code, text, timed_out)
    detail = text.strip().splitlines()[-1] if text.strip() else "sin salida"
    return (
        ProcessOutcome(
            status=status,
            exit_code=exit_code,
            counts=parse_counts(text),
            duration_s=duration,
            classification=classification,
            detail=detail[:400],
        ),
        text,
    )
