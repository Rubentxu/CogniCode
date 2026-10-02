#!/usr/bin/env python3
"""Nothing in a PipelineK stage body is inert.

The first end-to-end run of `merge-gate.pipeline.kts` on 2026-10-02 built three
release binaries successfully and then failed its own verification stage:

    FAIL: target/release/cognicode was not built

Nothing was wrong with the build. `~/.cargo/config.toml` on this machine sets
`build.target-dir = /var/home/rubentxu/cargo-targets`, because the checkout is
one of several sharing a machine. The stage that built the binaries asked cargo
where to put them; the stage that verified them had the directory hardcoded. A
verifier that hardcodes where the artifact lands is not verifying the artifact —
it is checking a directory nothing was ever going to write. On a machine with no
`build.target-dir` it would have passed while checking nothing at all, which is
the direction that matters: the bug was invisible exactly where it was harmless.

The same run surfaced the sibling defect, found by reading rather than by
failing. Inside a Kotlin raw string, `$cd` interpolates the Kotlin value, and
`${'$'}cd` emits a literal `$cd` for the shell. `cd` is a shell builtin, not a
variable, so the shell expanded it to nothing and this line:

    ${'$'}cd || exit 1

reached bash as

     || exit 1

— a guard that guards nothing, on a stage that never changed directory. It read
exactly like the guard used by the other 59 stages in the file, and it existed in
seven of them. Those seven passed only because the pipeline happened to be
launched from the repository root. Measured with `pipelinek` 0.46.0 rather than
assumed; the probe is `/tmp`-shaped and cheap to repeat.

Both are the shape the repository already has a name for: a written guarantee no
mechanism applies, the N+66 ghost-filter defect. The difference is that this
contract can see it. Both live in one file because they were found in the same
run and share a root — a stage body containing text that looks like it
constrains something and does not.

What is pinned:

- No pipeline may name a cargo artifact directory directly. Artifact paths are
  resolved through `scripts/ci/target-dir.sh`, which asks cargo rather than
  re-implementing the layering of `CARGO_TARGET_DIR` over repository config over
  user config.
- No pipeline may write `${'$'}cd` expecting a directory change.
- The resolver must exist and be executable, because stages run it as a command
  rather than through `bash` and a missing bit fails at run time, not at
  validation time.

Comments are exempt from the first two. The fix for each defect is a comment
naming it, and a contract that failed on those comments would be failing on the
record of why the defect existed.

Run:
    python3 scripts/ci/test_pipeline_stage_bodies.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# The six lanes. Discovered from the directory rather than listed, so a pipeline
# added tomorrow is covered tomorrow. A hand-maintained list is the same ghost
# filter this file exists to prevent.
PIPELINE_GLOB = "*.pipeline.kts"

RESOLVER_REL = "scripts/ci/target-dir.sh"

# A direct path into a cargo output directory. `target/release/…` is where cargo
# writes only when nothing overrides it; `$TARGET_DIR/release/…` is the resolved
# form and is deliberately not matched, because the variable name is what
# separates "asked cargo" from "assumed".
HARDCODED_ARTIFACT = re.compile(
    r"(?<![\w/\-])target/(?:release|debug|[a-z0-9_]+-unknown-[a-z0-9\-]+)/(?:release/)?[\w.\-]+"
)

# The Kotlin escape for a shell variable, applied to a shell builtin.
INERT_CD = re.compile(r"\$\{'\$'\}cd\b")


def pipelines() -> list[Path]:
    return sorted(p for p in REPO_ROOT.glob(PIPELINE_GLOB) if p.is_file())


def code_lines(source: str) -> list[tuple[int, str]]:
    """Lines that are code rather than commentary, with their 1-based numbers.

    Two comment syntaxes live in these files. `//` is Kotlin's, and a raw string
    body can also carry a shell `#` comment — several stages explain themselves
    inside the script they run. A contract that failed on those would be failing
    on the very comments that record why the defect existed, and a linter with a
    known false positive is worse than no linter: it teaches its reader to
    ignore it. Both are skipped only when they start the line.

    A trailing `//` or `#` after code is not skipped, because the code part is
    what runs. This is the conservative direction: it can miss a path hidden
    behind a trailing comment, which would be a false negative, and cannot
    invent one, which would be a false positive.
    """
    out: list[tuple[int, str]] = []
    for number, line in enumerate(source.splitlines(), start=1):
        stripped = line.lstrip()
        if stripped.startswith("//") or stripped.startswith("#"):
            continue
        out.append((number, line))
    return out


def test_the_target_dir_resolver_is_runnable() -> None:
    """Stages run the resolver as a command, so it has to be one.

    `pipelinek validate` compiles the script and checks the stage graph. It does
    not execute a stage, so a resolver that is missing or lost its executable
    bit produces a pipeline that validates cleanly and fails on the first stage
    that needs a built binary — which is the same silent-until-late shape as the
    hardcoded path this file exists to catch.
    """
    resolver = REPO_ROOT / RESOLVER_REL
    assert resolver.is_file(), (
        f"{RESOLVER_REL} is missing. Every pipeline resolves cargo's target "
        f"directory through it, so without it each artifact path in this "
        f"repository is a guess again."
    )
    assert resolver.stat().st_mode & 0o111, (
        f"{RESOLVER_REL} is not executable. Stages invoke it directly rather "
        f"than through `bash`, so a missing bit fails at run time and not at "
        f"validation time."
    )


def test_no_stage_contains_an_inert_cd_guard() -> None:
    """`${'$'}cd` is not a directory change; it is a shell variable that is unset.

    Written that way it expands to nothing and the `|| exit 1` that follows
    guards a command that was never issued. The stage then runs in whatever
    directory `pipelinek` was launched from, which is a fact about the
    invocation and not about the pipeline.
    """
    offenders: list[str] = []
    for path in pipelines():
        for number, line in code_lines(path.read_text(encoding="utf-8")):
            if INERT_CD.search(line):
                offenders.append(
                    f"  {path.name}:{number}: `${{'$'}}cd` expands to a shell "
                    f"variable named `cd`, which does not exist, so the line runs "
                    f"as `|| exit 1` and the stage never changes directory. Write "
                    f"`$cd` so Kotlin interpolates the real path, or drop the guard."
                )
    assert not offenders, (
        f"{len(offenders)} inert cd guard(s) in pipeline stage bodies:\n"
        + "\n".join(offenders)
    )


def test_no_pipeline_hardcodes_a_cargo_artifact_path() -> None:
    """`target/release/` is an assumption about the machine, not a path.

    `CARGO_TARGET_DIR` and any `.cargo/config.toml` move it, and the release
    tool, the three release binaries and the cross-compiled per-target
    directories all move together. A stage that hardcodes it is asserting that
    the build wrote somewhere it was never told to write.
    """
    offenders: list[str] = []
    for path in pipelines():
        for number, line in code_lines(path.read_text(encoding="utf-8")):
            match = HARDCODED_ARTIFACT.search(line)
            if match:
                offenders.append(
                    f"  {path.name}:{number}: names cargo's output directory "
                    f"directly (`{match.group(0)}`). Cargo writes there only when "
                    f"nothing overrides it; `CARGO_TARGET_DIR` and any "
                    f"`.cargo/config.toml` move it, and this repository is built "
                    f"on a machine that sets one. Resolve it with "
                    f"`{RESOLVER_REL}` instead."
                )
    assert not offenders, (
        f"{len(offenders)} hardcoded cargo artifact path(s) in pipeline stage "
        f"bodies:\n" + "\n".join(offenders)
    )


def main() -> int:
    failures: list[str] = []
    tests = [
        test_the_target_dir_resolver_is_runnable,
        test_no_stage_contains_an_inert_cd_guard,
        test_no_pipeline_hardcodes_a_cargo_artifact_path,
    ]
    for func in tests:
        try:
            func()
        except AssertionError as exc:
            failures.append(f"{func.__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(failure)
        return 1

    print(
        f"PASS — {len(pipelines())} pipelines: no inert stage text, no hardcoded "
        f"cargo artifact paths, resolver present and executable."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
