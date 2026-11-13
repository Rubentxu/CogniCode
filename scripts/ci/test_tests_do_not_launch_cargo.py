#!/usr/bin/env python3
"""Contract: a test under `crates/*/tests` never launches `cargo`.

A test that runs `cargo build` or `cargo metadata` pays for it on EVERY run of
the suite, and the suite runs on every iteration and on every mutant of the
gate (AGENTS.md, hard rule 6: a test that compiles the workspace is a task
dressed as a test). It also asks the wrong question. `cargo metadata` answers
"where is the target directory of whoever is running me", and the binary that
sits there may belong to another checkout that shares the directory: the test
then exercises code that is not in this tree and reports on it.

Cargo already answers the right question. For a package that declares a
`[[bin]]`, it builds the binary before its integration tests and hands each one
the path in `CARGO_BIN_EXE_<name>`, fixed at compile time. A test that needs a
binary of ANOTHER package finds it next to its own executable, which is in
`<target>/<profile>/deps/`; no subprocess is needed for that either.

The scan is syntactic: a line is an offence when it launches `cargo` itself
(`Command::new("cargo")`) or reads the variable cargo exports for that purpose
(`CARGO`, via `env!`, `option_env!`, `var` or `var_os`). It does not flag
`CARGO_BIN_EXE_*`, `CARGO_MANIFEST_DIR` or `env_remove("CARGO")`, none of which
runs cargo, and it ignores comments.

Out of scope, and declared where the repository declares it: the packages the
root `Cargo.toml` lists under `[workspace] exclude`. Those are not members of
the workspace, so no gate runs them; they are run on purpose by a `just` recipe
that names them, and the contract checks that the recipe exists.

Run: python3 scripts/run_contract_tests.py scripts/ci/test_tests_do_not_launch_cargo.py
"""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

CI_DIR = Path(__file__).resolve().parent
ROOT = CI_DIR.parents[1]
CRATES = ROOT / "crates"

LAUNCHES_CARGO = re.compile(
    r"""
      Command::new\(\s*"cargo"\s*\)                    # the launch itself
    | \b(?:option_env!|env!)\(\s*"CARGO"\s*\)          # the path cargo exports
    | \bvar(?:_os)?\(\s*"CARGO"\s*\)                   # the same, read at runtime
    """,
    re.VERBOSE,
)


def launches(source: str) -> list[int]:
    """Lines of `source` that launch cargo, comments excluded."""
    return [
        number
        for number, line in enumerate(source.splitlines(), start=1)
        if not line.lstrip().startswith("//") and LAUNCHES_CARGO.search(line)
    ]


def excluded_packages() -> set[str]:
    """Directory names under `crates/` that the workspace does not contain."""
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    declared = manifest["workspace"].get("exclude", [])
    return {Path(path).name for path in declared if Path(path).parent.name == "crates"}


def scanned_test_files() -> list[Path]:
    """Every Rust file under `crates/<member>/tests`, nested directories included."""
    assert CRATES.is_dir(), "crates/ no existe: el contrato no puede quedar vacio en silencio"
    skipped = excluded_packages()
    return sorted(
        path
        for crate in CRATES.iterdir()
        if crate.is_dir() and crate.name not in skipped
        for path in (crate / "tests").rglob("*.rs")
    )


def test_the_scan_sees_each_way_of_launching_cargo() -> None:
    source = "\n".join(
        [
            'let a = std::process::Command::new("cargo")',  # 1
            'let b = Command::new( "cargo" )',  # 2
            'let c = std::env::var_os("CARGO")',  # 3
            'let d = env::var("CARGO").unwrap()',  # 4
            'let e = env!("CARGO")',  # 5
            'let f = option_env!("CARGO")',  # 6
        ]
    )
    assert launches(source) == [1, 2, 3, 4, 5, 6], launches(source)


def test_the_scan_does_not_flag_what_does_not_launch_cargo() -> None:
    source = "\n".join(
        [
            'Command::new(env!("CARGO_BIN_EXE_cognicode-mcp"))',
            'let dir = env!("CARGO_MANIFEST_DIR");',
            'let target = std::env::var_os("CARGO_TARGET_DIR");',
            'cmd.env_remove("CARGO");',
            '// Command::new("cargo") appears here only as a comment',
            '/// let cargo = env!("CARGO");',
            'Command::new(&cargo_deny)',
        ]
    )
    assert launches(source) == [], launches(source)


def test_the_scan_reaches_nested_test_directories() -> None:
    names = {path.relative_to(ROOT).as_posix() for path in scanned_test_files()}
    for needed in (
        "crates/cognicode-mcp/tests/it/b6_w1_mcp_robustness.rs",
        "crates/cognicode-cli/tests/it/a015_onboarding_gate.rs",
        "crates/cognicode-cli/tests/release/prf_f6_w1_release_coherence.rs",
        "crates/cognicode-core/tests/it/b3_w2_progressive_context.rs",
    ):
        assert needed in names, f"{needed} no se recorre"


def test_an_excluded_package_is_run_by_a_recipe_that_names_it() -> None:
    skipped = excluded_packages()
    assert skipped, "el workspace ya no excluye ningun paquete: retira esta excepcion"
    justfile = (ROOT / "justfile").read_text(encoding="utf-8")
    unowned = sorted(name for name in skipped if f"crates/{name}/Cargo.toml" not in justfile)
    assert not unowned, (
        f"paquetes excluidos del workspace que ninguna receta del justfile ejecuta: {unowned}; "
        "fuera del workspace nadie los corre, y fuera de esta lista cargo es suyo"
    )


def test_no_test_launches_cargo() -> None:
    offenders = [
        f"{path.relative_to(ROOT)}:{line}"
        for path in scanned_test_files()
        for line in launches(path.read_text(encoding="utf-8"))
    ]
    assert not offenders, (
        f"{len(offenders)} lugares de tests que lanzan cargo: " + ", ".join(offenders)
        + ". Un binario del propio paquete es env!(\"CARGO_BIN_EXE_<nombre>\"); uno de otro "
        "paquete esta junto al ejecutable del test (std::env::current_exe())."
    )
