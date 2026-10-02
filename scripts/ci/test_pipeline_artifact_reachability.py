#!/usr/bin/env python3
"""QW-09 for PipelineK: no step may consume a path a later step produces.

QW-09 was written for GitHub Actions and the defect it guards is real: two
19-minute merge-gate runs failed because `merge-gate` asserted on
`target/release/cognicode` while never downloading the artifact `build-binary`
uploaded. `needs:` makes a job *wait* for another job; it does not move
artifacts between runners, because every job gets a fresh one.

That failure mode does not exist in PipelineK, so re-anchoring the old guard at
`*.pipeline.kts` would have pinned the wrong thing. Measured 2026-10-02 under
pipelinek 0.46.0, with a two-stage probe and no `cd` prefix:

    WRITER_PWD      = <repository root>
    READER_PWD      = <repository root>
    RELATIVE_SHARED = yes
    REPO_VISIBLE    = yes

Stages share the filesystem and the working directory. A file one stage writes
at a relative path is visible to the next.

So the invariant is not "each stage must obtain its own binaries", which would
force every consumer to rebuild, and would be a topology invented to satisfy a
constraint that is not there. It is:

    a step that consumes a produced path must come after a step that produces
    it, within the same pipeline

That is the whole property. It is the same order-sensitivity that broke
`merge-gate` twice, expressed in the topology that actually exists.

## Execution order

Steps are read in file order. That matches execution for these scripts: every
`sh()` lives in a leaf stage, and the probe showed sibling leaf stages running
in declaration order. A step placed inside a stage that also contains other
stages runs after them, so a script that mixes both would need this checker to
model nesting — it does not, and it says so rather than pretending.

Not asserted: that a runner really has the file at run time. This is a static
check over the pipeline text, the same limit the YAML version had.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# A consumed path: a build artifact the script expects to already exist.
CONSUMES = re.compile(r"target/(?:release|debug)/[\w./-]+")

# A step that produces one. `--release` alone is not enough, because
# `cargo test --release` produces test output rather than a binary the next
# step executes, and treating it as a provider hides the real gap.
#
# There is deliberately no "runs a binary" clause here. A first version had
# one, on the theory that executing `target/release/x` meant something had
# produced it — which makes the checker self-satisfying: the ABSENT case, a
# step running a binary nothing built, then counts as its own provider and goes
# green. It was caught by running the mutation, not by reading the pattern.
PRODUCES = re.compile(
    r"cargo\s+build\b[^\n]*--release"          # a release build
    r"|cargo\s+install\b"                       # cargo-deny, cargo-llvm-cov
    r"|rustup\s+target\s+add"                   # a cross target
    r"|\bcp\s+[^\n]*target/"                    # staging a payload
)
# `cargo test --release` compiles release binaries as a side effect. Counted
# separately so the failure message can say which kind of provider is missing.
PROVIDES_INDIRECTLY = re.compile(r"cargo\s+test\b[^\n]*--release")

TRIPLE_QUOTE = '"' * 3
# Each alternative pairs its own opener with its own closer.
#
# A first version matched the opener and then searched forward for whichever
# delimiter opened it. That desynchronises: a single-line `sh("...")` whose body
# or whose following raw string contains a stray `"` terminates early, the scan
# resumes from the wrong offset, and the remaining steps are read as garbage. It
# reported 45 bodies for a 45-step script and not one of them contained the
# string `target/` — a checker that passed because it saw nothing, which is the
# worst kind of green. Matching the whole call in one regex cannot drift.
STEP = re.compile(
    'sh\\(\\s*' + TRIPLE_QUOTE + '(.*?)' + TRIPLE_QUOTE + r"\s*\.trimIndent\(\)"
    r'|sh\(\s*"(.*?)"\s*\)',
    re.DOTALL,
)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def steps_of(text: str) -> list[str]:
    # Raw and single-line bodies are both captured by STEP, which pairs each
    # call's own opener with its own closer. Splitting on `sh(` alone would also
    # match the word inside a comment, which is how the first version of this
    # checker read its own documentation as a step.
    bodies: list[str] = []
    for match in STEP.finditer(text):
        body = match.group(1) if match.group(1) is not None else match.group(2)
        if body is None:
            continue
        # A raw Kotlin string needs its escaped dollar resolved before the
        # shell patterns can see it.
        bodies.append(body.replace("${'$'}", "$"))
    return bodies


def stage_of(text: str, position: int) -> str:
    """The nearest enclosing `stage("...")` name, for the failure message."""
    names = [
        (m.start(), m.group(1))
        for m in re.finditer(r'stage\(\s*"([^"]+)"\s*\)', text)
    ]
    enclosing = [name for start, name in names if start < position]
    return enclosing[-1] if enclosing else "(top level)"


def check_pipeline(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    bodies = steps_of(text)
    problems: list[str] = []
    for index, body in enumerate(bodies):
        consumes = CONSUMES.findall(body)
        if not consumes:
            continue
        earlier = "\n".join(bodies[:index])
        later = "\n".join(bodies[index + 1 :])
        if PRODUCES.search(earlier):
            continue
        hint = ""
        if PRODUCES.search(later):
            hint = (
                f"\n       the step that produces it comes LATER in this script. "
                f"Steps run in order."
            )
        elif PROVIDES_INDIRECTLY.search(earlier):
            hint = (
                f"\n       an earlier `cargo test --release` compiles release "
                f"binaries as a side effect, which is not the same as a stage "
                f"that builds one. Build it explicitly."
            )
        problems.append(
            f"  {path.name}: the step consuming {sorted(set(consumes))[0]} "
            f"runs before anything in this pipeline produces it.{hint}"
        )
    return problems


def pipelines() -> list[Path]:
    """The pipelines, read on demand.

    Not cached in a module global filled by another test: the runner executes
    `test_*` functions in sorted name order, so a global populated by
    `test_pipelines_are_readable` is still empty when
    `test_every_consumer_has_an_earlier_producer` runs first — and a test that
    iterates an empty list passes. That is the same vacuous green this checker
    has already produced twice.
    """
    return sorted(REPO_ROOT.glob("*.pipeline.kts"))


def test_pipelines_are_readable() -> None:
    """Anti-vacuity: no pipelines found would mean this checker read nothing.

    A checker that parses zero steps and reports zero reachability problems is
    indistinguishable from a correct one. The first version of `steps_of` did
    exactly that — it reported 45 bodies for a 45-step script and not one of
    them contained the string it was looking for, and the contract passed.
    """
    found = pipelines()
    check(
        bool(found),
        "no *.pipeline.kts found at the repository root; this contract would "
        "pass without reading a single pipeline",
    )
    for path in found:
        bodies = steps_of(path.read_text(encoding="utf-8"))
        check(
            bool(bodies),
            f"{path.name}: no sh() step was parsed out of a pipeline that "
            f"contains them. The step regex has drifted from the DSL",
        )


def test_steps_are_actually_parsed() -> None:
    """The parser must recover a body, and it must recover the whole of it.

    A step parser that silently truncates reports fewer problems than exist,
    and a contract that only ever reports zero is indistinguishable from one
    that works. This asserts a known body is found verbatim.
    """
    fixture = (
        'sh("$cd && cargo build --release --bin thing")\n'
        'sh("""\n'
        "    ${'$'}cd || exit 1\n"
        "    ./target/release/thing --version\n"
        '    """.trimIndent())\n'
    )
    bodies = steps_of(fixture)
    check(
        len(bodies) == 2,
        f"expected 2 steps from the fixture, parsed {len(bodies)}: {bodies!r}",
    )
    if len(bodies) != 2:
        return
    check(
        "cargo build --release" in bodies[0],
        f"the single-line body was not recovered intact: {bodies[0]!r}",
    )
    check(
        "target/release/thing" in bodies[1] and "cd || exit 1" in bodies[1],
        f"the raw body was not recovered intact: {bodies[1]!r}",
    )


def test_every_consumer_has_an_earlier_producer() -> None:
    """The invariant: no step consumes a build artifact before it exists."""
    for path in pipelines():
        failures.extend(check_pipeline(path))


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
        print()
        print("  PipelineK stages share the filesystem (measured 2026-10-02),")
        print("  so ordering is the whole invariant: a step that executes a")
        print("  build artifact needs an earlier step that built it. Unlike")
        print("  GitHub Actions there is no download step to forget, and there")
        print("  is also no need for every consumer to rebuild.")
        return 1

    print(
        f"PASS — {len(pipelines())} pipelines checked, every step that consumes a "
        f"build artifact has an earlier step that produces it "
        f"({len(failures)} reachability problems)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
