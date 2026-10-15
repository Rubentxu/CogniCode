"""Ayudas compartidas por los tests del harness (sin dependencias externas).

La suite de contratos del repositorio se ejecuta con
`scripts/run_contract_tests.py`, que descubre funciones `test_*` y no instala
pytest: el runner de CI no lo trae. Así que aquí no hay fixtures ni
parametrización, hay funciones y context managers, y cada test se fabrica su
propio repositorio temporal con `git init` y un workspace mínimo de Cargo.

No es un detalle de estilo. Probar el ChangeSet con estructuras inventadas
probaría la estructura inventada: el valor del harness está en cómo lee git y
cargo de verdad.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator

HARNESS_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = HARNESS_DIR.parent
for _path in (HARNESS_DIR / "src", HARNESS_DIR):
    if str(_path) not in sys.path:
        sys.path.insert(0, str(_path))

HARNESS_BIN = HARNESS_DIR / "bin" / "harness.py"

CORE_LIB = """pub fn add(left: i64, right: i64) -> i64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds() {
        assert_eq!(add(2, 2), 4);
    }
}
"""

APP_LIB = """pub fn greet() -> String {
    format!("suma {}", demo_core::add(1, 1))
}

#[cfg(test)]
mod tests {
    #[test]
    fn greets() {
        assert!(super::greet().starts_with("suma 2"));
    }
}
"""

ROOT_TOML = """[workspace]
resolver = "2"
members = ["crates/core", "crates/app"]
"""

CORE_TOML = """[package]
name = "demo-core"
version = "0.1.0"
edition = "2021"

[dependencies]
"""

APP_TOML = """[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
demo-core = { path = "../core" }
"""

PIPELINE = """// PipelineK de prueba: dos obligaciones de test y un check de formato.
pipeline("demo-fast") {
    stage("fmt") {
        sh("$cd && cargo fmt --all -- --check")
    }
    stage("core-lib") {
        sh("$cd && cargo test -p demo-core --lib --quiet")
    }
    stage("app-lib") {
        sh("$cd && cargo test -p demo-app --lib --quiet")
    }
}
"""

CORE_SUITE = "demo-core:lib#-"
APP_SUITE = "demo-app:lib#-"
FMT_SUITE = "workspace:cargo:fmt#-"


def git(root: Path, *args: str) -> str:
    proc = subprocess.run(
        ["git", *args], cwd=str(root), capture_output=True, text=True, check=False
    )
    if proc.returncode != 0:
        raise AssertionError(
            f"git {' '.join(args)} falló con {proc.returncode}: {proc.stderr.strip()}"
        )
    return proc.stdout


@contextmanager
def demo_repo(prefix: str = "th-demo-") -> Iterator[Path]:
    """Workspace de Cargo mínimo con git, dos paquetes y dos pipelines.

    Se aísla `CARGO_TARGET_DIR` en un directorio temporal propio: el workspace
    real comparte caché con otros proyectos de la máquina y no queremos ni
    bloquearla ni depender de su estado.
    """
    with tempfile.TemporaryDirectory(prefix=prefix) as tmp:
        base = Path(tmp)
        root = base / "demo"
        (root / "crates" / "core" / "src").mkdir(parents=True)
        (root / "crates" / "app" / "src").mkdir(parents=True)
        (root / "crates" / "core" / "tests").mkdir(parents=True)
        (root / "docs").mkdir(parents=True)
        (root / "Cargo.toml").write_text(ROOT_TOML, encoding="utf-8")
        (root / "crates" / "core" / "Cargo.toml").write_text(CORE_TOML, encoding="utf-8")
        (root / "crates" / "core" / "src" / "lib.rs").write_text(CORE_LIB, encoding="utf-8")
        (root / "crates" / "app" / "Cargo.toml").write_text(APP_TOML, encoding="utf-8")
        (root / "crates" / "app" / "src" / "lib.rs").write_text(APP_LIB, encoding="utf-8")
        (root / "crates" / "core" / "tests" / "documentacion.rs").write_text(
            "// lo que promete este módulo está en docs/guia.md\n"
            "#[test]\n"
            "fn la_guia_sigue_existiendo() {}\n",
            encoding="utf-8",
        )
        (root / "docs" / "guia.md").write_text("# guía\n", encoding="utf-8")
        (root / "product-fast.pipeline.kts").write_text(PIPELINE, encoding="utf-8")
        # La lane de merge ejecuta las mismas obligaciones, como en el repositorio
        # real: lo que cambia entre lanes es la política, no los comandos.
        (root / "merge-gate.pipeline.kts").write_text(PIPELINE, encoding="utf-8")

        saved_target = os.environ.get("CARGO_TARGET_DIR")
        saved_color = os.environ.get("CARGO_TERM_COLOR")
        os.environ["CARGO_TARGET_DIR"] = str(base / "cargo-target")
        os.environ["CARGO_TERM_COLOR"] = "never"
        try:
            # Cargo.lock se genera y se commitea: un lock falso lo reescribiría la
            # primera ejecución y el ChangeSet cargaría un fichero "modificado".
            subprocess.run(
                ["cargo", "generate-lockfile", "--offline"],
                cwd=str(root),
                check=True,
                capture_output=True,
            )
            git(root, "init", "-q", "-b", "main")
            git(root, "config", "user.email", "harness@example.invalid")
            git(root, "config", "user.name", "Harness Tests")
            git(root, "add", "-A")
            git(root, "commit", "-q", "-m", "base del fixture")
            git(root, "checkout", "-q", "-b", "topic")
            yield root
        finally:
            for key, previous in (
                ("CARGO_TARGET_DIR", saved_target),
                ("CARGO_TERM_COLOR", saved_color),
            ):
                if previous is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = previous


def commit_change(repo: Path, relative: str, content: str = "// cambio\n", message: str = "cambio") -> None:
    """Commit que **añade** contenido si el fichero ya existe.

    Reemplazarlo entero convertiría un cambio en una regresión de compilación de
    los paquetes que dependen de él, y el test mediría eso en vez de lo que
    quería medir.
    """
    target = repo / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        target.write_text(target.read_text(encoding="utf-8") + "\n" + content, encoding="utf-8")
    else:
        target.write_text(content, encoding="utf-8")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", message)


def make_receipt(
    *,
    suite_id: str = CORE_SUITE,
    status: str = "PASSED",
    exit_code: int = 0,
    executed: int = 3,
    passed: int = 3,
    failed: int = 0,
    skipped: int = 0,
    discovered: int = 3,
    fingerprint: str = "a" * 64,
    argv: tuple[str, ...] = ("cargo", "test", "-p", "demo-core", "--lib"),
    features: tuple[str, ...] = (),
    runner_name: str = "cargo-test",
    receipt_id: str = "rcpt-test",
    run_id: str = "run-test",
    log_sha256: str = "b" * 64,
):
    """Recibo coherente; cada test rompe solo el campo que estudia."""
    from harness.domain import Receipt, RunnerInfo, SourceSnapshot

    return Receipt(
        receipt_id=receipt_id,
        run_id=run_id,
        repo_id="demo-repo",
        suite_id=suite_id,
        lane="change",
        status=status,
        input_fingerprint=fingerprint,
        source_snapshot=SourceSnapshot(
            git_head="0" * 40,
            effective_content_digest="c" * 64,
            dirty=False,
            worktree_id="wt-test",
        ),
        runner=RunnerInfo(
            name=runner_name,
            version="cargo 1.0.0",
            argv=tuple(argv),
            features=tuple(features),
            target_triple="x86_64-unknown-linux-gnu",
        ),
        started_at="2026-01-01T00:00:00Z",
        finished_at="2026-01-01T00:00:01Z",
        exit_code=exit_code,
        discovered_cases=discovered,
        executed_cases=executed,
        passed_cases=passed,
        failed_cases=failed,
        skipped_cases=skipped,
        log_sha256=log_sha256,
    )


def make_fingerprint(**overrides):
    from harness.validity import FingerprintInputs

    base = dict(
        suite_id=CORE_SUITE,
        argv=("cargo", "test", "-p", "demo-core", "--lib"),
        features=(),
        target_triple="x86_64-unknown-linux-gnu",
        runner_name="cargo-test",
        runner_version="cargo 1.0.0",
        rustc_version="1.0.0",
        cargo_lock_digest="d" * 64,
        toolchain_digest="e" * 64,
        closure_digest="f" * 64,
        expected_min_cases=1,
    )
    base.update(overrides)
    return FingerprintInputs(**base)


def run_cli(repo: Path, state_dir: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(HARNESS_BIN), "--repo", str(repo), "--state-dir", str(state_dir), *args],
        capture_output=True,
        text=True,
        check=False,
    )


def git_dirty(repo: Path) -> str:
    return git(repo, "status", "--porcelain").strip()
