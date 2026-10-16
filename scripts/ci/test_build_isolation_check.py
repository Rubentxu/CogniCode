#!/usr/bin/env python3
"""The build-isolation check must be wired in, and it must be able to say no.

MEDIDO 2026-10-15. Seis checkouts de este repositorio — el principal y cinco
worktrees — compilan al mismo target dir, porque `build.target-dir` está en la
config global de cargo. Cargo decide frescura por mtime, y un mtime no dice de
qué checkout salió un binario. Dos corridas del merge gate fallaron por eso y
no por el código: un test paniqueó en la línea 900 de un fichero de 861
nombrando un test que no existe en este árbol, y otro comparó su
`source_commit (d5082848…)` contra `git rev-parse HEAD (d73e7b08…)`, que es el
HEAD de `worktree/test-harness`.

`scripts/ci/check-build-isolation.sh` es el preflight que evita que el gate
produzca un veredicto no atribuible. Este contrato le exige tres cosas:

  * que exista y que el merge gate lo nombre — un script correcto que nadie
    invoca no evita nada;
  * que pueda **negar**. La primera versión del script comparaba
    `[ -z "$target_dir" ] || [ "$target_dir" = "$target_dir" ]`, creyendo que
    la segunda cláusula detectaba un fallo de extracción. Es una comparación
    de una variable consigo misma: siempre verdadera, así que el script salía
    por la rama de «un solo checkout» en cada ejecución y **no podía negarse
    nunca**. Verde siempre es exactamente el defecto que el script existe para
    no tener, y por eso se comprueba con comportamiento, no leyendo el texto;
  * que no bloquee cuando no hay motivo. La segunda versión contaba como
    bloqueo cualquier `cargo` de la máquina, así que en un negativo con dos
    checkouts y nadie compilando salía con 1. Un gate que no puede pasar es un
    gate que nadie lee.

Los dos lados se ejecutan de verdad, con `git`, `cargo` y `ps` de mentira en un
directorio temporal. Nada de esto necesita una corrida del gate.

Run:
    python3 scripts/ci/test_build_isolation_check.py
"""
from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_REL = "scripts/ci/check-build-isolation.sh"
AUTHORITY = "merge-gate.pipeline.kts"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


# --------------------------------------------------------------------------
# Harness: a throwaway cargo project with one sibling worktree, and stubs that
# put whatever we want in `git worktree list` and `ps`.
# --------------------------------------------------------------------------

STUB_GIT = """#!/usr/bin/env bash
if [ "$1" = "worktree" ]; then
  echo "worktree {self}"
  echo "worktree {other}"
  echo
fi
"""

STUB_CARGO = """#!/usr/bin/env bash
echo '{{"target_directory":"/tmp/shared-target","packages":[]}}'
"""

STUB_PS = """#!/usr/bin/env bash
cat <<'PS'
{lines}
PS
"""


def build_fixture(tmp: Path, ps_lines: str) -> Path:
    """A repo with two checkouts, and a `ps` that reports `ps_lines`."""
    repo = tmp / "repo"
    (repo / "src").mkdir(parents=True)
    (repo / "Cargo.toml").write_text(
        '[package]\nname="probe"\nversion="0.0.0"\nedition="2021"\n'
    )
    (repo / "src" / "main.rs").write_text("fn main(){}\n")
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    subprocess.run(
        ["git", "-C", str(repo), "add", "-A"], check=True, capture_output=True
    )
    subprocess.run(
        [
            "git", "-C", str(repo),
            "-c", "user.email=probe@example.invalid",
            "-c", "user.name=probe",
            "commit", "-qm", "init",
        ],
        check=True,
        capture_output=True,
    )
    other = tmp / "other"
    other.mkdir()
    subprocess.run(
        ["git", "-C", str(repo), "worktree", "add", "-q", str(other), "-b", "other"],
        check=True,
        capture_output=True,
    )

    scripts = repo / "scripts" / "ci"
    scripts.mkdir(parents=True)
    # Destination is the FILENAME: passing SCRIPT_REL again would nest a second
    # `scripts/ci/` inside the directory we just made.
    shutil.copy(REPO_ROOT / SCRIPT_REL, scripts / Path(SCRIPT_REL).name)

    stub_bin = tmp / "stub-bin"
    stub_bin.mkdir()
    for name, template in (
        ("git", STUB_GIT),
        ("cargo", STUB_CARGO),
        ("ps", STUB_PS),
    ):
        path = stub_bin / name
        path.write_text(template.format(self=repo, other=other, lines=ps_lines))
        path.chmod(0o755)
    return repo


def run_check(
    repo: Path,
    stub_bin: Path,
    env_extra: dict[str, str] | None = None,
) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env["PATH"] = f"{stub_bin}:{env['PATH']}"
    # The real machine may already export CARGO_TARGET_DIR; the fixtures below
    # mean "unset" unless they say otherwise.
    env.pop("CARGO_TARGET_DIR", None)
    if env_extra:
        env.update(env_extra)
    return subprocess.run(
        ["bash", "scripts/ci/check-build-isolation.sh"],
        cwd=repo,
        capture_output=True,
        text=True,
        env=env,
        check=False,
    )


# --------------------------------------------------------------------------
# Tests
# --------------------------------------------------------------------------


def test_the_check_exists_and_the_merge_gate_names_it() -> None:
    check(
        (REPO_ROOT / SCRIPT_REL).is_file(),
        f"{SCRIPT_REL} does not exist. Without it the gate has no way to "
        f"notice that its verdict is not attributable to this checkout.",
    )
    if not (REPO_ROOT / AUTHORITY).is_file():
        failures.append(f"{AUTHORITY} is missing")
        return
    authority = (REPO_ROOT / AUTHORITY).read_text(encoding="utf-8")
    check(
        "check-build-isolation.sh" in authority,
        f"{AUTHORITY} never runs {SCRIPT_REL}. A correct script that nothing "
        "invokes prevents nothing: the gate would go on producing verdicts "
        "another checkout's binaries can decide.",
    )
    # It has to run BEFORE the work, or the refusal comes 11 minutes late —
    # which is exactly the cost the preflight exists to avoid.
    stage = authority.find('stage("build-isolation")')
    check(
        stage != -1,
        f'{AUTHORITY} has no stage("build-isolation")',
    )
    if stage != -1:
        first_test = min(
            (authority.find(m) for m in (
                'stage("core-lib")',
                'stage("core-unrestricted-evidence-kernel")',
                'stage("build-binary")',
            ) if authority.find(m) != -1),
            default=-1,
        )
        check(
            first_test == -1 or stage < first_test,
            f'{AUTHORITY} runs build-isolation AFTER the first test/build '
            "stage. A refusal that arrives after the work has been done "
            "saves nothing.",
        )


def test_the_check_can_refuse() -> None:
    """Positive: a sibling checkout of THIS repo is compiling. Must exit 1."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)
        repo = build_fixture(
            tmp,
            ps_lines=(
                f"  4242 cargo test --workspace\n"
                f"  4243 /home/u/.cargo-targets/debug/build/x\n"
            ),
        )
        # Put a compiling process that belongs to the sibling checkout.
        (tmp / "other" / "marker").write_text("x")
        proc = run_check_with_named_ps(repo, tmp, name=f"{tmp / 'other'}/cargo")
        check(
            proc.returncode == 1,
            "the check did NOT refuse with a sibling checkout compiling. If it "
            "cannot say no, it is decoration: it would have stayed silent "
            "through both of the contaminated runs it exists to prevent. "
            f"stdout={proc.stdout!r} stderr={proc.stderr[:200]!r}",
        )
        check(
            "REFUSING" in proc.stderr or "REFUSING" in proc.stdout,
            "the check refused without saying so. A silent exit 1 reads as a "
            "broken stage, not as an environment the operator can act on.",
        )


def test_the_check_stays_quiet_when_there_is_no_reason() -> None:
    """Negative: two checkouts, nobody compiling. Must exit 0.

    MEDIDO: the second version of the script counted every `cargo` on the
    machine, so this exact case exited 1. A gate that cannot pass is a gate
    nobody reads.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)
        repo = build_fixture(
            tmp,
            # A cargo process belonging to THIS checkout: unrelated work.
            ps_lines=f"  7777 cargo test\n",
        )
        proc = run_check_with_named_ps(repo, tmp, name=str(tmp / "repo" / "cargo"))
        check(
            proc.returncode == 0,
            "the check refused with two checkouts and nobody from another "
            "checkout compiling. Over-blocking is its own failure mode: the "
            f"gate would be red every day for no reason. stdout={proc.stdout!r} "
            f"stderr={proc.stderr[:300]!r}",
        )


def test_a_declared_isolated_target_dir_wins() -> None:
    """MEDIDO 2026-10-15: the preflight rejected its own remedy.

    The message it prints when it refuses tells the operator to run
    `CARGO_TARGET_DIR=/tmp/... bash scripts/ci/publish-merge-gate.sh`. The
    script checked `git worktree list` and `ps` and never looked at
    `CARGO_TARGET_DIR`, so it refused that run too — while a sibling
    checkout was compiling, which under an isolated destination cannot
    happen at all. A gate that refuses the only way to make it trustworthy
    is worse than having no preflight at all.

    So a declared, different target directory must short-circuit the
    refusal.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)
        repo = build_fixture(tmp, ps_lines="")
        proc = run_check_with_named_ps(
            repo, tmp, name=f"{tmp / 'other'}/cargo", env_extra={"CARGO_TARGET_DIR": "/tmp/its-own-target"}
        )
        check(
            proc.returncode == 0,
            "the check refused a run that declared its own CARGO_TARGET_DIR, "
            "even with a sibling checkout compiling. That is the remedy the "
            "refusal message recommends, so following it makes the gate "
            "permanently red. "
            f"stdout={proc.stdout!r} stderr={proc.stderr[:300]!r}",
        )

    # And the same declaration must NOT let a run through when it points at the
    # shared destination it was supposed to escape from.
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)
        repo = build_fixture(tmp, ps_lines="")
        proc = run_check_with_named_ps(
            repo, tmp, name=f"{tmp / 'other'}/cargo", env_extra={"CARGO_TARGET_DIR": "/tmp/shared-target"}
        )
        check(
            proc.returncode == 1,
            "the check accepted CARGO_TARGET_DIR pointing at the very shared "
            "directory the run was trying to escape. Declaring the shared "
            "path is not isolation; if it counted, the variable would be a "
            "way to switch the check off.",
        )


def run_check_with_named_ps(
    repo: Path,
    tmp: Path,
    name: str,
    env_extra: dict[str, str] | None = None,
) -> subprocess.CompletedProcess:
    """run_check, but the fake `ps` reports one process whose args contain `name`."""
    stub_bin = tmp / "stub-bin"
    ps = stub_bin / "ps"
    ps.write_text(
        "#!/usr/bin/env bash\n"
        f"cat <<'PS'\n  1234 {name} --release\nPS\n"
    )
    ps.chmod(0o755)
    return run_check(repo, stub_bin, env_extra=env_extra)


def main() -> int:
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a failing check is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print(
        "PASS — the gate refuses when a sibling checkout is building, and "
        "stays quiet when there is no reason to refuse"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())