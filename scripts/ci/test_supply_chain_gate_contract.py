#!/usr/bin/env python3
"""Contracts for the supply-chain gate: it must exist, it must be pinned, and
something a merge waits on must run it.

`cargo deny check advisories` and `cargo deny check licenses` are the only
things standing between a known-vulnerable or unvetted-licence dependency and
a published binary. They ran, and they were not on the merge path.

Measured 2026-10-02. `grep -rn 'cargo deny' .github/workflows/` returned hits in
exactly two files:

    release.yml          on: push, tags: v*
    release-validate.yml on: workflow_dispatch

Neither fires on a pull request, so a dependency with a published advisory
merged unnoticed and the first thing to look at it was the next person who
pushed a version tag — at which point there is no cheap way back. Fixed in
05c66126 by adding a `supply-chain` job to the merge path.

## The authority moved

These contracts used to resolve the merge gate from `pr-ci.yml` and check the
`needs:` list. They now resolve it from
`scripts/ci/pipeline_authority.py`, which names the merge-authority pipeline in
one place so that fourteen contracts cannot disagree about it.

The property is unchanged. "Runs in a job merge-gate needs" becomes "runs in a
stage of the merge authority": a merge cannot pass without the capability either
way. The mechanism is what changed, and `needs:` is a GitHub concept that does
not exist here, so it is not re-implemented.

What is asserted:

1. The gate runs in the merge authority. Looked up through the invoked
   `sh()` body rather than the file text, so a comment quoting the command
   cannot satisfy it.
2. Both declared axes are covered. `deny.toml` carries `[advisories]` and
   `[licenses]`; running one leaves the other's policy unexercised on every
   merge while the diff still looks like a security gate.
3. The cargo-deny version is fixed. `cargo install cargo-deny --locked` pins
   cargo-deny's *dependencies* but floats the *tool*, so the gate's behaviour
   would change with no commit. The version asserted here is the one the policy
   in `deny.toml` was measured against.

Not asserted: how cargo-deny is installed, or that the stage lives in this
pipeline rather than another. Both are free to change as long as a merge runs
it.

Run:
    python3 scripts/ci/test_supply_chain_gate_contract.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import pipeline_authority as authority  # noqa: E402

# The tool version this contract pins the merge path to. 0.20.2 is the version
# the current `deny.toml` policy was measured against on 2026-10-02, and the one
# whose behaviour (`unused-ignored-advisory = "deny"`, the `[advisories] ignore`
# list) the advisory-ignore backing contract assumes.
PINNED_CARGO_DENY = "0.20.2"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def test_the_supply_chain_gate_runs_in_the_merge_authority() -> None:
    """The step must exist, and a merge must not be able to pass without it.

    Looked up through the invoked `sh()` body rather than the name of the
    pipeline it sits in, because the failure being guarded against is a step
    quietly moving to a lane that does not gate merges.
    """
    for axis in ("advisories", "licenses"):
        invocation = f"cargo deny check {axis}"
        hosts = authority.invoked_by(invocation)
        check(
            bool(hosts),
            f"no pipeline runs `{invocation}`; the supply-chain gate is not on "
            f"the merge path, so a dependency with a published advisory merges "
            f"unnoticed and is first examined at tag time",
        )
        # The property is that a merge cannot pass without the gate -- not
        # that only one pipeline runs it. The previous form computed
        # `hosts - {MERGE_AUTHORITY}` and failed when that was non-empty, so
        # running the gate in `release-candidate` as well read as a
        # violation. It is a violation of nothing: the same advisory check at
        # tag time is defence in depth, and the module docstring has always
        # said the stage is free to live in any pipeline "as long as a merge
        # runs it".
        #
        # This was not visible until 2026-10-03. It arrived with b651774a
        # (#338), the commit that re-anchored these contracts off `pr-ci.yml`
        # onto the merge authority, and it failed on every run from then on
        # without failing the gate: see the runner fix in the same commit.
        check(
            authority.MERGE_AUTHORITY in hosts,
            f"`{invocation}` does not run in the merge authority "
            f"({authority.MERGE_AUTHORITY}); it runs in "
            f"{sorted(hosts) or 'no pipeline'}. A merge can pass without a "
            f"dependency with a published advisory being checked, and the "
            f"first thing to look at it is whoever pushes the next version "
            f"tag",
        )


def test_the_gate_covers_both_axes_the_policy_declares() -> None:
    """Advisories alone is half a supply-chain gate.

    `deny.toml` carries both an `[advisories]` section and a `[licenses]`
    allow-list, and the licence allow-list took real work to build (CP2-DEBT-04
    landed the shared `--check` semantics; the workspace `license` fields came
    in a separate commit). A wiring that runs only the advisory axis leaves that
    work unexercised on every merge while still looking like a security gate in
    the diff.
    """
    body = "\n".join(authority.pipeline_steps(authority.MERGE_AUTHORITY))
    for axis in ("advisories", "licenses"):
        check(
            f"cargo deny check {axis}" in body,
            f"the merge authority does not run the {axis} axis; deny.toml "
            f"declares a policy for it that no merge currently exercises",
        )


def test_the_cargo_deny_version_is_fixed() -> None:
    """`--locked` pins cargo-deny's dependencies, not cargo-deny itself.

    `cargo install cargo-deny --locked` resolves cargo-deny's own Cargo.lock, so
    the tool still floats to whatever version is newest the day it runs. A
    security gate whose behaviour can change without a commit is a gate whose
    behaviour nobody reviewed.
    """
    body = "\n".join(authority.pipeline_steps(authority.MERGE_AUTHORITY))
    installs = re.findall(r"cargo install cargo-deny[^\n]*", body)
    check(
        bool(installs),
        f"{authority.MERGE_AUTHORITY} does not install cargo-deny at all; a "
        f"gate that runs it would depend on whatever the runner image happens to "
        f"carry, and the runner is not a declared input to this repository",
    )
    for command in installs:
        check(
            "--version" in command,
            f"cargo-deny is installed without a fixed version: `{command}`. "
            "--locked pins its dependencies but floats the tool, so the gate's "
            "behaviour can change with no commit in this repository",
        )
        check(
            PINNED_CARGO_DENY in command,
            f"cargo-deny is not pinned to the version this policy was measured "
            f"against ({PINNED_CARGO_DENY}): `{command}`",
        )


def test_the_gate_installs_the_lsp_server_it_needs() -> None:
    """El gate tiene que dejar de saltarse los tests que necesitan un LSP.

    MEDIDO 2026-10-04. `merge-gate.pipeline.kts` corre
    `cargo test -p cognicode-core --lib`, y en esa suite vive
    `test_hierarchy_falls_through_within_the_bounded_readiness`, que empieza
    con un `println!` y un `return` cuando `rust-analyzer` no esta en el PATH.
    El gate se saltaba el test y daba verde.

    Y no era solo que faltara el binario: con rustup instalado, el shim
    `~/.cargo/bin/rust-analyzer` es un symlink a `rustup` que responde
    `Unknown binary` con codigo 1. Esta en el PATH, es ejecutable, y no
    funciona. Un gate que lo comprobara con `command -v` pasaria el paso de
    instalacion y se saltaria los tests igual, que es el defecto que se quiere
    cerrar.

    Por eso se afirman DOS cosas y no una: que el gate lo instala, y que
    despues lo PREGUNTA. Preguntar es lo unico que distingue "instalado" de
    "utilizable".
    """
    body = "\n".join(authority.pipeline_steps(authority.MERGE_AUTHORITY))
    check(
        "rustup component add rust-analyzer" in body,
        f"{authority.MERGE_AUTHORITY} never installs rust-analyzer. The core suite "
        f"contains a test that returns early when the binary is absent, so the "
        f"gate reports green without ever running it. Measured: nobody in the "
        f"pipeline, the CI scripts or the workflows installed it, and the rustup "
        f"shim exits 1 while sitting in the PATH",
    )
    check(
        body.count("rust-analyzer --version") >= 2,
        "the gate installs rust-analyzer but never asks it whether it answers. "
        "An install step that is not followed by a probe passes on a broken shim, "
        "which is the exact failure this contract exists to catch",
    )


def test_the_merge_authority_exists() -> None:
    """Anti-vacuity: the pipeline every other test reads has to be there.

    Without this, a renamed or deleted merge authority would leave the three
    checks above reporting that no pipeline runs cargo-deny, which reads as a
    genuine supply-chain gap rather than a contract looking in the wrong place.
    """
    path = Path(authority.REPO_ROOT) / authority.MERGE_AUTHORITY
    check(
        path.is_file(),
        f"the declared merge authority does not exist: {path}. Either restore "
        f"it or update MERGE_AUTHORITY in pipeline_authority.py — do not delete "
        f"the constant, because fourteen contracts resolve it from there",
    )


def main() -> int:
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print(
        f"PASS — the supply-chain gate runs in {authority.MERGE_AUTHORITY}, "
        f"covers both declared axes, and pins the tool version"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
