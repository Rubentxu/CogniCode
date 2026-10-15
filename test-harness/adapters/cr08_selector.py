"""Adaptador del selector CR-08 (`scripts/ci/select-suites.sh`).

CR-08 es el oráculo vigente del repositorio: decide, a partir de una lista de
paths, qué suites deberían correr. Mientras el harness no demuestre paridad, la
decisión del selector manda, así que este adaptador existe para **comparar**, no
para sustituir.

Dos límites reales del oráculo que conviene tener escritos porque afectan a la
comparación:

  * Su vocabulario es grueso (`core`, `explorer`, `mcp`, `cli`, `ladybug`,
    `merge`), no una obligación por suite. La comparación se hace por *scope*:
    lo que CR-08 pide se traduce a paquetes, y se comprueba que el harness no se
    queda por debajo.
  * Recibe los paths como una cadena separada por espacios, así que un fichero
    con espacios lo rompería. Eso no se arregla aquí: se detecta y se declara
    (el harness no puede fingir que la comparación fue limpia).
"""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from pathlib import Path

SELECTOR = Path("scripts/ci/select-suites.sh")

STRATEGY_CHANGED = "changed"
STRATEGY_FALLBACK = "fallback"
STRATEGY_NOOP = "noop"


@dataclass(frozen=True)
class SelectorDecision:
    """Lo que CR-08 decidió, con la evidencia de cómo se obtuvo."""

    strategy: str
    reason: str
    suites: tuple[str, ...]
    ok: bool = True
    problem: str | None = None
    argv: tuple[str, ...] = ()

    @property
    def requires_everything(self) -> bool:
        return self.strategy == STRATEGY_FALLBACK

    def to_json(self) -> dict:
        payload = {
            "strategy": self.strategy,
            "reason": self.reason,
            "suites": list(self.suites),
            "ok": self.ok,
            "argv": list(self.argv),
        }
        if self.problem:
            payload["problem"] = self.problem
        return payload


def unknown(problem: str, *, argv: tuple[str, ...] = ()) -> SelectorDecision:
    """Oráculo ilegible. No es `noop`: es «no se pudo comprobar»."""
    return SelectorDecision(
        strategy="unknown", reason="oráculo no disponible", suites=(), ok=False, problem=problem, argv=argv
    )


def run_selector(repo_root: Path, paths: tuple[str, ...], timeout_s: float = 120.0) -> SelectorDecision:
    """Ejecuta CR-08 con esos paths y normaliza la respuesta.

    Fail-closed en dos frentes: si no hay script, no se pudo ejecutar o la salida
    no es JSON, el resultado es `ok=False` con el motivo. Un oráculo ilegible no
    reduce obligaciones: el plan queda indeterminado.
    """
    script = repo_root / SELECTOR
    if not script.exists():
        return unknown(f"no existe {SELECTOR} en este checkout")

    spaced = [path for path in paths if " " in path]
    argv = ("bash", str(SELECTOR), "--paths", " ".join(paths))
    try:
        proc = subprocess.run(
            list(argv),
            cwd=str(repo_root),
            capture_output=True,
            text=True,
            check=False,
            timeout=timeout_s,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return unknown(f"no se pudo ejecutar {SELECTOR}: {exc}", argv=argv)

    if proc.returncode != 0:
        return unknown(f"{SELECTOR} salió con {proc.returncode}: {proc.stderr.strip()[:200]}", argv=argv)
    try:
        payload = json.loads(proc.stdout.strip().splitlines()[-1])
    except (json.JSONDecodeError, IndexError) as exc:
        return unknown(f"la salida del selector no es JSON: {exc}", argv=argv)

    problem = None
    if spaced:
        problem = (
            "CR-08 recibe los paths separados por espacios, así que un nombre con "
            f"espacios lo parte: {spaced[0]!r}. La comparación de esos paths no es limpia"
        )
    return SelectorDecision(
        strategy=payload.get("strategy", "unknown"),
        reason=payload.get("reason", ""),
        suites=tuple(payload.get("suites", ())),
        ok=True,
        problem=problem,
        argv=argv,
    )
