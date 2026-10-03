#!/usr/bin/env python3
"""`preflight-clean-clone.sh` has to survive being run from a tag checkout.

MEDIDO 2026-10-03. La lane `release-candidate` callmurio en 0.3s al certificar
el tag v0.101.0, en el stage `clean-clone`, con:

    fatal: Rama remota HEAD no encontrada en upstream origin

La causa era una sola linea, y la linea tenia un fallback escrito para cubrir
justo el caso que no cubria:

    --branch "$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo main)"

En un checkout detached — que es lo que se obtiene al hacer `git checkout
<tag>`, o sea como se corta una release — `git rev-parse --abbrev-ref HEAD`
imprime la cadena literal `HEAD` y **sale con codigo 0**. El `|| echo main`
depende de que el comando falle, y no falla. Asi que se pasaba `--branch HEAD` a
`git clone`, que busca una rama llamada `HEAD`, no la encuentra, y muere.

Que una release no se pueda certificar desde un tag no es un defecto de entorno:
es el caso normal de este gate. Un gate que solo se puede ejecutar desde una rama
es un gate que no se puede usar para publicar.

## Por que un contrato y no un comentario

El comentario anterior ya explicaba la intencion del fallback y era correcto, y
el gate seguia roto. Un fallback cuya condicion de disparo nunca se cumple no es
un fallback: es prosa que parece una guarda. Este contrato comprueba la
propiedad, que es la que importa: **la referencia que el clon va a nombrar tiene
que existir, en cualquier estado del checkout.**

## Que se comprueba

- Desde una rama, la resolucion devuelve el nombre de la rama. Sin esto, la
  correccion mas simple —devolver siempre vacio— dejaria el clon sin
  optimizacion y pasaria igual.
- Desde un checkout detached, no devuelve nada, y en particular no devuelve la
  cadena `HEAD`.
- Lo que la resolucion devuelve se puede pasar a `git clone --branch`. Esto es
  la propiedad de extremo a extremo: la resolucion y el comando que la consume,
  juntos.
- La regla esta escrita una sola vez. El stage 1 llama a `resolve_clone_ref`, y
  no hay una segunda resolucion `--abbrev-ref` repartida por el script: dos
  copias de la regla es una de ellas equivocada dentro de un mes.
- El seam es barato. `--print-clone-ref` existe para que este contrato no
  tenga que pagar los 8-15 minutos del preflight. Si alguien lo quita, o el
  script lo ejecuta entero antes de responder, este test pasaria a tardar 15
  minutos y a dejar de vigilarse. Se mide el tiempo.

Run:
    python3 scripts/ci/test_preflight_clone_ref.py
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "ci" / "preflight-clean-clone.sh"
SEAM = "--print-clone-ref"

# El preflight completo declara 8-15 minutos en su cabecera. El seam tiene que
# ser mucho mas barato que eso, y con holgura: si el limite se acerca al
# documento, el seam ha dejado de ser un seam.
SEAM_BUDGET_SECONDS = 30

# Y lo que cuesta cuando el seam NO existe. Sin el seam, el script interpreta
# `--print-clone-ref` como un SHA, sigue por su curso y clona el repositorio
# entero de verdad: medido, un contrato en rojo encendia 5 clones de 1.6G y sus
# logs en /tmp antes de que este test pudiera decir nada. Un contrato que al
# fallar hace mas trabajo que el gate que vigila es un contrato que nadie va a
# correr en un PR. Con el seam ausente se falla por inspeccion, sin ejecutar nada.
PROBE_TIMEOUT_SECONDS = 20


def git(cwd: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(cwd), *args], capture_output=True, text=True, timeout=120
    )


def make_repo(root: Path) -> Path:
    """A repo with one commit, one tag and one branch."""
    subprocess.run(
        ["git", "init", "-q", "-b", "main", str(root)],
        check=True,
        capture_output=True,
        timeout=120,
    )
    git(root, "-c", "user.email=contract@example.invalid", "-c", "user.name=c",
        "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "x")
    git(root, "tag", "v1")
    return root


def resolve_clone_ref(cwd: Path) -> str:
    """What the script would pass to `git clone --branch`, from `cwd`.

    Runs the script itself rather than reimplementing the rule: a contract that
    reimplemented the thing it checks would keep passing after the thing broke.

    The seam is verified by inspection first. Without that check, an unfixed
    script reads `--print-clone-ref` as a SHA and runs the whole preflight —
    including a real clone of this repository — so the contract's red state
    would be the most expensive thing in the suite.
    """
    source = SCRIPT.read_text(encoding="utf-8")
    code = "\n".join(
        line for line in source.splitlines() if not line.lstrip().startswith("#")
    )
    if SEAM not in code:
        raise AssertionError(
            f"the script does not implement {SEAM}, so its ref resolution cannot "
            "be exercised without running the preflight end to end. Add the seam "
            "next to resolve_clone_ref; without it this contract either clones "
            "the repository to find out it is broken, or stops testing the rule"
        )

    started = time.monotonic()
    try:
        done = subprocess.run(
            ["bash", str(SCRIPT), SEAM],
            cwd=str(cwd),
            capture_output=True,
            text=True,
            timeout=PROBE_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        raise AssertionError(
            f"{SEAM} did not answer within {PROBE_TIMEOUT_SECONDS}s. The seam is "
            "meant to print one ref and exit; if it runs the preflight first, "
            "this contract silently becomes a 15-minute test"
        ) from None
    elapsed = time.monotonic() - started
    if done.returncode != 0:
        raise AssertionError(
            f"the script does not answer {SEAM} (exit {done.returncode}). Either "
            f"the seam was removed or the script fails before reaching it.\n"
            f"stdout: {done.stdout!r}\nstderr: {done.stderr!r}"
        )
    if elapsed > SEAM_BUDGET_SECONDS:
        raise AssertionError(
            f"{SEAM} took {elapsed:.1f}s, over the {SEAM_BUDGET_SECONDS}s budget. "
            "The seam is supposed to answer the ref resolution without running "
            "the preflight; if it runs the preflight, this contract silently "
            "becomes a 15-minute test and stops being run"
        )
    return done.stdout.strip()


def test_a_detached_checkout_names_no_branch() -> None:
    """The regression: a tag checkout must not hand `HEAD` to the clone.

    This is the exact state a release cut lives in, and the exact state that
    killed the lane.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = make_repo(Path(raw))
        git(root, "checkout", "-q", "--detach", "v1")
        assert git(root, "rev-parse", "--abbrev-ref", "HEAD").stdout.strip() == "HEAD", (
            "the fixture is not a detached checkout, so this test would pass "
            "without exercising anything"
        )
        ref = resolve_clone_ref(root)
        assert ref != "HEAD", (
            "a detached checkout still resolves to the literal 'HEAD', which "
            "git clone rejects with 'Remote HEAD branch not found'. The "
            "`|| echo main` fallback never fires because --abbrev-ref exits 0"
        )
        assert ref == "", (
            f"a detached checkout should name no branch at all, got {ref!r}"
        )


def test_a_branch_checkout_still_names_its_branch() -> None:
    """The fix is not "always return nothing".

    Dropping `--branch` unconditionally would make the two failing cases pass and
    cost a full default-branch clone on every run. This is the assertion that
    keeps the optimisation alive.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = make_repo(Path(raw))
        ref = resolve_clone_ref(root)
        assert ref == "main", (
            f"a checkout on branch 'main' resolved to {ref!r}; the branch name "
            "is the whole point of the optimisation and it must survive the fix"
        )


def test_the_named_ref_is_clonable() -> None:
    """End to end: whatever the resolver returns, the clone accepts.

    The two unit tests above pin the resolver. This one pins the pair, which is
    the property that actually failed: a ref that is not clonable, from any
    state, is a broken release path.
    """
    for state, setup in (
        ("on a branch", ["checkout", "-q", "main"]),
        ("detached at a tag", ["checkout", "-q", "--detach", "v1"]),
    ):
        with tempfile.TemporaryDirectory() as raw:
            root = make_repo(Path(raw) / "src")
            git(root, *setup)
            ref = resolve_clone_ref(root)
            args = ["--no-tags"]
            if ref:
                args += ["--branch", ref]
            with tempfile.TemporaryDirectory() as dst:
                done = subprocess.run(
                    ["git", "clone", "-q", *args, str(root), f"{dst}/clone"],
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                assert done.returncode == 0, (
                    f"cloning a repository {state} failed with ref={ref!r}:\n"
                    f"{done.stderr.strip()}"
                )


def test_the_rule_is_written_once() -> None:
    """One definition, called by the stage that clones.

    Two copies of a resolution rule is one of them wrong inside a month, and the
    symptom would be a release path that breaks again the day someone edits the
    copy that is not the one the contract reads.
    """
    text = SCRIPT.read_text(encoding="utf-8")
    assert "resolve_clone_ref()" in text, (
        "the resolver function is gone; the stage and the seam must share one "
        "definition rather than each inlining the rule"
    )
    assert 'clone_ref="$(resolve_clone_ref)"' in text, (
        "the clone stage no longer calls the shared resolver, so the rule the "
        "contract exercises and the rule the clone uses could disagree"
    )
    # One definition, one call site — counted in code, not in prose. The comment
    # above the resolver deliberately quotes the old line verbatim so the reason
    # for the change survives; counting that would make the test fail on its own
    # documentation, which is how a linter ends up being deleted instead of fixed.
    # A comment that quotes a command is indistinguishable from code that runs it,
    # in both directions — the same reason `pipeline_authority.strip_line_comments`
    # exists.
    code = "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )
    occurrences = code.count("--abbrev-ref")
    assert occurrences == 1, (
        f"--abbrev-ref appears {occurrences} times in executable code; the "
        "detached-HEAD rule is written in more than one place, so the copy the "
        "contract reads and the copy the clone uses can drift apart"
    )


def test_the_branch_flag_is_only_passed_when_a_branch_exists() -> None:
    """The clone must not be handed an empty `--branch`.

    A bash array expanded unguarded under `set -u` can yield an empty first
    argument, which `git clone` reads as the branch name. The guarded expansion
    is what makes the omitted case safe, and it is worth pinning.
    """
    text = SCRIPT.read_text(encoding="utf-8")
    assert "clone_branch_args=()" in text, (
        "the clone no longer builds its --branch arguments conditionally"
    )
    guarded = '${clone_branch_args[@]+"${clone_branch_args[@]}"}'
    assert guarded in text, (
        f"the array is expanded unguarded. Under `set -u`, {guarded} is what "
        "keeps an omitted --branch from being passed as an empty branch name"
    )
    assert '--branch "$clone_ref"' in text, (
        "the --branch flag is no longer derived from the resolved ref"
    )


def main() -> int:
    failures: list[str] = []
    for func in (
        test_a_detached_checkout_names_no_branch,
        test_a_branch_checkout_still_names_its_branch,
        test_the_named_ref_is_clonable,
        test_the_rule_is_written_once,
        test_the_branch_flag_is_only_passed_when_a_branch_exists,
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
        "PASS — the preflight names a branch only when one exists, and the ref "
        "it names is clonable from a branch and from a detached tag checkout."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
