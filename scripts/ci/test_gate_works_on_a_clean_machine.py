#!/usr/bin/env python3
"""El gate tiene que dar el mismo resultado en una maquina limpia.

MEDIDO 2026-10-21, dos veces, en el mismo corte.

La primera: la primera corrida del `merge-gate` de la rama `release/v0.101.24`
murio en la etapa `cognicode-ladybug` con `Disk quota exceeded` —32 de 56
etapas— porque el gate aisla su compilacion en el tmpfs de `/tmp`, que en esta
maquina tiene 48G, y el build debug mas release del workspace no cabe.

La segunda, y mas dura de explicar: la primera corrida del gate de candidato
de `v0.101.25` —la lane que certifica lo que se publica— murio en
`host-build-and-verify` con

    FAIL: .../release/cognicode was not built

El arbol estaba bien. El corte anterior habia pasado el mismo gate. Lo que
faltaba era que esa etapa **verifica los tres binarios release antes de
construirlos**: la comprobacion esta al principio del bloque de shell y los
`cargo build --release` estan despues. La lane solo pasa si el directorio de
destino ya tenia los binarios de antes. En una maquina limpia no los tiene, y
la lane muere sin haber construido nada.

Y la tercera forma del mismo defecto: `preflight-clean-clone.sh` crea el clon
con `mktemp -d`, que cae en `/tmp`, y luego fuerza
`CARGO_TARGET_DIR="$WORK_DIR/clone/target"`. El build debug del workspace
—con treinta y pico gramaticas de tree-sitter— tiene que caber ahi dentro. En
esta maquina no cabe: el enlazador muere con `ld: Bus error` y el mensaje que
queda es `IO failure on output stream: Disk quota exceeded`, a mitad, sin
decir que el problema era el disco.

## Por que un contrato y no un comentario

Los tres comentarios ya existen, y los tres son correctos. El de `target-dir.sh`
explica que un verificador que dice donde va el artefacto no esta verificando
el artefacto; el del `preflight` explica por que el target dir se fija dentro
del clon. Y el gate seguia roto, porque ninguno de los tresODY es el problema:
el problema es que la lane **depende del estado previo de la maquina**, y eso
no se arregla con prosa.

Este contrato fija tres propiedades:

1. **La lane construye antes de verificar.** Ninguna comprobacion de
   `TARGET_DIR/release/<bin>` puede aparecer antes del `cargo build --release`
   que lo produce. Se lee el cuerpo del stage, no una constante.
2. **El preflight dice donde va a trabajar y se puede mover.** El seam
   `--print-work-dir` imprime el directorio que usaria, y `PREFLIGHT_WORK_DIR`
   manda sobre `TMPDIR`, que a su vez manda sobre `/tmp`.
3. **El preflight dice que no entra antes de empezar.** Con el espacio
   insuficiente, falla con un mensaje que nombra el volumen, el sitio que
   falta y la manera de moverlo —no a mitad del enlazado.

Los seams existen para que este contrato no pague los 20 minutos del preflight.
Se mide el tiempo, como hace `test_preflight_clone_ref.py`.

Run:
    python3 scripts/ci/test_gate_works_on_a_clean_machine.py
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PREFLIGHT = REPO_ROOT / "scripts" / "ci" / "preflight-clean-clone.sh"
CANDIDATE_PIPELINE = REPO_ROOT / "release-candidate.pipeline.kts"

WORK_DIR_SEAM = "--print-work-dir"
VOLUME_SEAM = "--check-work-dir"

# El preflight completo se declara en 8-15 minutos en su cabecera, y en esta
# maquina mas: el build debug del workspace pesa mas de 20G. Un seam que paga
# eso deja de ser un seam.
SEAM_BUDGET_SECONDS = 30

# El requisito de espacio. MEDIDO: el clon limpio de este workspace ocupa mas
# de 20G solo con el perfil debug. El default es un margen por encima, porque
# un margen que se queda corto solo cambia el momento del fallo, no el fallo.
DEFAULT_MIN_FREE_BYTES = 30 * 1024 * 1024 * 1024


def run_seam(args: list[str], env: dict[str, str] | None = None) -> tuple[int, str, str, float]:
    started = time.monotonic()
    done = subprocess.run(
        ["bash", str(PREFLIGHT), *args],
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
        timeout=120,
        env=env,
    )
    return done.returncode, done.stdout, done.stderr, time.monotonic() - started


def strip_comments(text: str) -> str:
    """Code without the comments.

    A comment that quotes a command is indistinguishable from code that runs
    it, in both directions. Counting the quoted command as code would make this
    contract fail on its own documentation, which is how a linter ends up being
    deleted instead of fixed.
    """
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )


def test_the_candidate_lane_builds_before_it_verifies() -> None:
    """The regression: a lane that verifies artefacts it has not built yet.

    The stage is one shell body: the verification of the three release binaries
    and the four `cargo build --release` calls that produce them. The property
    is about their order, not about their text.
    """
    text = CANDIDATE_PIPELINE.read_text(encoding="utf-8")
    body = text.split('stage("host-build-and-verify")', 1)
    assert len(body) == 2, (
        "the candidate lane no longer has a `host-build-and-verify` stage, so "
        "this contract no longer describes anything"
    )
    stage_body = body[1].split('stage("', 1)[0]

    first_build = stage_body.find("cargo build --release")
    assert first_build != -1, (
        "the `host-build-and-verify` stage no longer builds the release "
        "binaries, so there is nothing to order against"
    )
    first_check = stage_body.find("TARGET_DIR/release/")
    assert first_check != -1, (
        "the stage no longer checks the release binaries at all; a stage that "
        "builds and never checks is not a verification"
    )
    assert first_build < first_check, (
        "the stage verifies `TARGET_DIR/release/<bin>` before the "
        "`cargo build --release` that produces it. The lane then only passes "
        "on a machine whose target directory already had the binaries: on a "
        "clean machine it dies with 'FAIL: .../release/cognicode was not "
        "built' before it has built anything. Build first, verify after."
    )


def test_the_preflight_says_where_it_works_and_can_be_moved() -> None:
    """The work directory is a decision, so the operator can make it.

    `mktemp -d` honours `TMPDIR`, and the machine's tmpfs is the reason the
    lane died. A knob that exists but is not discoverable is not a knob.
    """
    code = strip_comments(PREFLIGHT.read_text(encoding="utf-8"))
    assert WORK_DIR_SEAM in code, (
        f"the preflight does not implement {WORK_DIR_SEAM}, so this contract "
        "cannot exercise where the clone would land without running the whole "
        "preflight"
    )

    with tempfile.TemporaryDirectory() as raw:
        wanted = Path(raw) / "somewhere-else"
        wanted.mkdir()
        code_rc, out, err, elapsed = run_seam(
            [WORK_DIR_SEAM], env={"PATH": "/usr/bin:/bin", "PREFLIGHT_WORK_DIR": str(wanted)}
        )
        assert code_rc == 0, (
            f"the seam failed with exit {code_rc}\nstdout: {out!r}\nstderr: {err!r}"
        )
        assert elapsed < SEAM_BUDGET_SECONDS, (
            f"{WORK_DIR_SEAM} took {elapsed:.1f}s, over the {SEAM_BUDGET_SECONDS}s "
            "budget. The seam is meant to answer where the clone would land "
            "without running the preflight"
        )
        assert out.strip() == str(wanted), (
            f"PREFLIGHT_WORK_DIR was set to {str(wanted)!r} and the seam "
            f"answered {out.strip()!r}. The knob that moves the clone off a "
            "tmpfs does not work"
        )

    with tempfile.TemporaryDirectory() as raw:
        wanted = Path(raw) / "via-tmpdir"
        wanted.mkdir()
        code_rc, out, _err, _elapsed = run_seam(
            [WORK_DIR_SEAM], env={"PATH": "/usr/bin:/bin", "TMPDIR": str(wanted)}
        )
        assert code_rc == 0, f"the seam failed with exit {code_rc}"
        assert str(wanted) in out, (
            f"TMPDIR was set to {str(wanted)!r} and the seam answered "
            f"{out.strip()!r}. `mktemp -d` honours TMPDIR, and that is the "
            "knob that moves the clone to a volume with room"
        )


def test_the_preflight_refuses_before_it_starts_when_the_volume_is_too_small() -> None:
    """The failure has to arrive before the build, and say what is wrong.

    `Disk quota exceeded` in the middle of a link step names the symptom and
    not the cause. The operator learns the truth only by reading the pipeline.
    """
    code = strip_comments(PREFLIGHT.read_text(encoding="utf-8"))
    assert VOLUME_SEAM in code, (
        f"the preflight does not implement {VOLUME_SEAM}, so this contract "
        "cannot exercise the space check without running the whole preflight"
    )

    with tempfile.TemporaryDirectory() as raw:
        code_rc, out, err, elapsed = run_seam(
            [VOLUME_SEAM],
            env={
                "PATH": "/usr/bin:/bin",
                "PREFLIGHT_WORK_DIR": raw,
                "PREFLIGHT_MIN_FREE_BYTES": str(1 << 62),
            },
        )
        assert elapsed < SEAM_BUDGET_SECONDS, (
            f"{VOLUME_SEAM} took {elapsed:.1f}s, over the {SEAM_BUDGET_SECONDS}s budget"
        )
        assert code_rc != 0, (
            "the preflight accepted a volume with less free space than it "
            "needs. On this machine that is the state that killed the lane "
            "twice, with a message that named the symptom and not the cause"
        )
        message = (out + err).lower()
        for word in ("volume", "space"):
            assert word in message, (
                f"the refusal does not mention {word!r}: {(out + err).strip()!r}. "
                "The message has to name the volume and what it lacks"
            )
        assert "preflight_work_dir" in message or "tmpdir" in message, (
            f"the refusal does not say how to move the work directory: "
            f"{(out + err).strip()!r}"
        )


def test_a_volume_with_room_is_accepted() -> None:
    """The check is not a gate that refuses everything.

    A check that always says no is indistinguishable from a broken one, and it
    would turn a lane that works into a lane that never runs.
    """
    with tempfile.TemporaryDirectory() as raw:
        code_rc, out, err, _elapsed = run_seam(
            [VOLUME_SEAM],
            env={
                "PATH": "/usr/bin:/bin",
                "PREFLIGHT_WORK_DIR": raw,
                "PREFLIGHT_MIN_FREE_BYTES": "1",
            },
        )
        assert code_rc == 0, (
            f"a volume with room was refused (exit {code_rc})\n"
            f"stdout: {out!r}\nstderr: {err!r}"
        )


def main() -> int:
    failures: list[str] = []
    for func in (
        test_the_candidate_lane_builds_before_it_verifies,
        test_the_preflight_says_where_it_works_and_can_be_moved,
        test_the_preflight_refuses_before_it_starts_when_the_volume_is_too_small,
        test_a_volume_with_room_is_accepted,
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
        "PASS — the lane builds before it verifies, the preflight says where "
        "it works and can be moved, and it refuses a volume without room "
        "before it starts."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
