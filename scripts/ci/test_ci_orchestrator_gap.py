#!/usr/bin/env python3
"""Ratchet: the PipelineK coverage gap may only shrink.

GitHub Actions is not the target orchestrator for this repository. The
decision is that PipelineK becomes the single execution authority for CI/CD,
and the closure criterion is deliberate and absolute:

    executable GitHub Actions workflows in CogniCode == 0

That criterion is unmeasurable as it stands. "How much of CI has migrated" is
not a number anyone wrote down, so a migration can stall for months while every
dashboard stays green — the gates keep running, just from the wrong engine, and
nothing registers as regression.

## What this measures

Two orchestrators, one inventory of gates:

- **Actions** — every `run:` command in `.github/workflows/*.yml` that invokes
    a known gate tool (`cargo test|clippy|fmt|deny|build`, a `scripts/**/*.py`,
    a `scripts/**/*.sh`, or a `just` recipe).
- **PipelineK** — every `sh(...)` step in `*.pipeline.kts` invoking the same.

The difference is the gap. This contract asserts the gap did not grow against
the base commit, the same shape as `test_cr06_ratchet.py`: a comparison against
the base, not a constant, so there is no number in this file to raise.

## Two normalisations this file exists because of

Both were found by running it and reading the wrong number, not by reading the
code. Without them this contract reports a migration state that is not the
repository's.

**`for t in ... do` is N gates, not one.** `merge-gate.pipeline.kts` runs the 24
MCP suites in a shell loop, while `pr-ci.yml` spells each one out as its own
step. Counting the loop as one line reported 24 gates that the Kotlin pipeline
does run.

**The loop variable reaches bash through Kotlin's escaped-dollar form.**
Inside a raw Kotlin string the loop body is `"${'$'}t"`, not `"${t}"`.
Substituting only the latter expands the loop to nothing.

**`${{ matrix.args }}` is a parameter, not a gate.** Left in place, every
YAML-matrix step looks like a gate the Kotlin side lacks.

## Measured baseline (main bf0ff1ae, 2026-10-02)

    Actions  90 distinct gates
    PipelineK 51 distinct gates
    covered 50, gap 40

    pr-ci.yml           61 gates, 50 covered, 11 in the gap
    the other 5         33 gates,  2 covered, 31 in the gap

The 11 `pr-ci.yml` gates in the gap are the most recent additions:
`cargo deny check advisories`, `cargo deny check licenses` (the supply-chain
job, #332), `validate_skills.py`, `verify-skills.sh` (CP5),
`run_contract_tests.py` (the contract runner, which now includes the CR-06
ratchet), `check-bin-tracking.sh`, `check-release-artifact-reachability.sh`,
the `cognicode` and `cognicode-cli` release builds, and
`cr07_metrics_exposition_contract`.

The other 31 are the entire release path — SBOM, tag coherence, clean-clone
preflight, install smoke, platform payloads, musl, `cogh`,
`cognicode-release` — plus the sandbox nightly. PipelineK has no release lane
at all.

The gap failing is the point. It is not this contract's job to say the
migration is done; it is this contract's job to make "not done" impossible to
mistake for "done", and to notice the day the gap gets worse.

Run:
    python3 scripts/ci/test_ci_orchestrator_gap.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"

# The tools whose invocations count as gates.
#
# A gate is a command whose non-zero exit changes a verdict. The boundary is
# drawn there, not at "looks like a build command", because the first version
# of this list was `cargo (test|clippy|fmt|deny|build)` and it missed real
# gates that were running in Actions the whole time:
#
#   cargo llvm-cov --lib -p cognicode-core --summary-only \
#     --fail-under-lines 75.00 --fail-under-regions 71.00
#
# That is the CR-09 coverage gate: blocking, with a numeric threshold, and the
# inventory reported a gap of 40 while being unable to see it. A measurement
# that cannot see a gate cannot report on gates.
#
# `cargo` is matched whole rather than per-subcommand so the list cannot go
# stale again when a new subcommand becomes a gate.
#
# Anchored to the start of the command. Searching anywhere in the line made
# `chmod +x target/release/cognicode` and `test -x target/release/...` count
# as gates: they mention a binary, they do not run one. The pipe form is the
# one place a gate is not the head — `echo "$subject" | commitlint` runs
# commitlint — so it is matched separately rather than by anchoring.
GATE_HEAD = re.compile(
    r"^(?:cargo|just|python3|bash|commitlint|gh|rustup|npm|"
    r"target/release/|\./target/release/)\b"
)
GATE_PIPE = re.compile(r"\|\s*(?:commitlint|gh|cargo|just|python3|bash)\b")


def is_gate(line: str) -> bool:
    # A prerequisite is not a verdict. `python3 -m pip install` sets the stage
    # for a gate that runs later; counting it would mean the Kotlin pipeline
    # "covered" a gate merely by installing the thing that checks it.
    if line.startswith("python3 -m "):
        return False
    return bool(GATE_HEAD.match(line) or GATE_PIPE.search(line))

RUN_BLOCK = re.compile(r"run:\s*\|\s*\n((?:\s{6,}.*\n|\n)+)")
RUN_INLINE = re.compile(r"run:\s*(\S.*)$", re.MULTILINE)
SH_RAW = re.compile(r'sh\(\s*"""(.*?)"""', re.DOTALL)
SH_LINE = re.compile(r'sh\(\s*"(.*?)"', re.DOTALL)
FOR_LOOP = re.compile(r"for\s+\w+\s+in\s+\\?\s*\n(.*?)\n\s*do", re.DOTALL)
DONE = re.compile(r"^\s*done\s*$", re.MULTILINE)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def normalise(command: str) -> str:
    """A comparable identity for one gate invocation."""
    command = " ".join(command.split())
    # The inline `run:` capture keeps the key.
    command = re.sub(r"^run:\s*", "", command)
    command = re.sub(r"^cd\s+\S+\s*&&\s*", "", command)
    command = re.sub(r"^set -e\s+", "", command)
    command = command.strip().rstrip("\\").strip()
    # `VAR=$(some gate)` is the same gate with its output captured, not a
    # different one. Both orchestrators capture the selector's output this way
    # and write the variable with a different spelling, which read as a gap in
    # a pipeline that runs the command. Parsed rather than stripped in two
    # steps: an earlier pair of regexes left a stray quote behind and produced
    # `'SELECT_PATHS=...sh)'`, still not matching.
    assignment = re.match(
        r'^[A-Za-z_][A-Za-z0-9_]*="?\$\((.*)\)"?$', command, re.DOTALL
    )
    if assignment:
        command = assignment.group(1).strip()
    # A YAML matrix value is a parameter, not a different gate.
    command = re.sub(r"\$\{\{[^}]*\}\}", "<param>", command)
    return command


def gate_key(command: str) -> str:
    """Drop the wrapper that differs between the two orchestrators.

    The Kotlin side has no `cd`, and resolves the repository root once; the YAML
    side prefixes most steps with it. Those are packaging differences, not
    different gates.
    """
    command = command.replace("$cd && ", "")
    command = re.sub(r"^\$\{?cd[^&|]*&&\s*", "", command)
    command = command.replace('"$repoRoot/', "").replace("./", "")
    return command.strip()


def expand_for_loops(text: str) -> list[str]:
    """One `for t in a b c do` step is N gates, not one.

    The loop body is what runs per item. The `for ... do` header ends at `do`,
    so the body starts right after it.
    """
    lines: list[str] = []
    for body in SH_RAW.findall(text):
        loop = FOR_LOOP.search(body)
        if not loop:
            lines.extend(body.splitlines())
            continue
        items = [
            token
            for token in re.split(r"[\s\\]+", loop.group(1))
            if token and not token.startswith("#")
        ]
        after_do = body[loop.end() :]
        inner = DONE.split(after_do, maxsplit=1)[0]
        template = inner if inner.strip() else after_do
        for item in items:
            for line in template.splitlines():
                lines.append(
                    line.replace('"${\'$\'}t"', item).replace("${t}", item)
                )
    return lines


def actions_gates(workflow_texts: dict[str, str]) -> set[str]:
    gates: set[str] = set()
    for text in workflow_texts.values():
        candidates: list[str] = []
        for block in RUN_BLOCK.findall(text):
            candidates.extend(
                line.strip()
                for line in block.splitlines()
                if line.strip() and not line.strip().startswith("#")
            )
        candidates.extend(RUN_INLINE.findall(text))
        for candidate in candidates:
            # Normalise first, then ask. Every Kotlin step is written as
            # `$cd && <gate>`, so anchoring the head check before the wrapper is
            # stripped matched nothing on the PipelineK side and reported the
            # whole pipeline as missing.
            keyed = gate_key(normalise(candidate))
            if is_gate(keyed):
                gates.add(keyed)
    return gates


def pipelinek_gates(pipeline_texts: dict[str, str]) -> set[str]:
    gates: set[str] = set()
    for text in pipeline_texts.values():
        # The loop is expanded first, while the source still carries Kotlin's
        # escaped dollar, because that is what the loop placeholder looks like.
        # Unescaping before this point would rewrite the placeholder and expand
        # the loop to nothing — which reads as 24 MCP suites vanishing from a
        # pipeline that runs them.
        candidates = [line.strip() for line in expand_for_loops(text)]
        candidates.extend(SH_LINE.findall(text))
        for candidate in candidates:
            # Inside a Kotlin raw string, `${'$'}` is how a literal dollar
            # reaches the emitted shell script. This contract reads the .kts
            # source, not the emitted script, so the escape is still visible
            # here and would make every step using a shell variable look like a
            # different gate.
            candidate = candidate.replace("${'$'}", "$")
            keyed = gate_key(normalise(candidate))
            if is_gate(keyed):
                gates.add(keyed)
    return gates


def read_workflows() -> dict[str, str]:
    if not WORKFLOW_DIR.is_dir():
        return {}
    return {
        path.name: path.read_text(encoding="utf-8")
        for path in sorted(WORKFLOW_DIR.glob("*.yml"))
    }


def read_pipelines() -> dict[str, str]:
    return {
        path.name: path.read_text(encoding="utf-8")
        for path in sorted(REPO_ROOT.glob("*.pipeline.kts"))
    }


def git(*args: str) -> str | None:
    try:
        done = subprocess.run(
            ["git", *args],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return done.stdout if done.returncode == 0 else None


def at_ref(ref: str) -> tuple[dict[str, str], dict[str, str]] | None:
    """Both orchestrators as they existed at `ref`, or None if unavailable."""
    listing = git("ls-tree", "-r", "--name-only", ref)
    if listing is None:
        return None
    names = listing.splitlines()
    workflows = {
        name: git("show", f"{ref}:{name}")
        for name in names
        if name.startswith(".github/workflows/") and name.endswith(".yml")
    }
    pipelines = {
        name: git("show", f"{ref}:{name}")
        for name in names
        if name.endswith(".pipeline.kts")
    }
    if not workflows or any(text is None for text in workflows.values()):
        return None
    if any(text is None for text in pipelines.values()):
        return None
    return (
        {name: text or "" for name, text in workflows.items()},
        {name: text or "" for name, text in pipelines.items()},
    )


def resolve_base() -> str | None:
    for candidate in ("origin/main", "main"):
        resolved = git("rev-parse", "--verify", f"{candidate}^{{commit}}")
        if resolved:
            return resolved.strip()
    return None


def test_both_orchestrators_are_readable() -> None:
    """Anti-vacuity: a gap of zero because nothing was read is not a gap of zero."""
    workflows = read_workflows()
    pipelines = read_pipelines()
    check(
        bool(workflows),
        f"no workflows read from {WORKFLOW_DIR}. The gap would read as zero "
        f"because nothing was inventoried",
    )
    check(
        bool(pipelines),
        "no *.pipeline.kts found at the repository root. If the pipelines "
        "moved, update this contract's read_pipelines()",
    )
    check(
        bool(actions_gates(workflows)),
        "the Actions side produced zero gates; the inventory is not reading the "
        "workflows correctly",
    )
    check(
        bool(pipelinek_gates(pipelines)),
        "the PipelineK side produced zero gates; the inventory is not reading "
        "the pipelines correctly",
    )


def test_the_gap_did_not_grow() -> None:
    """The ratchet: coverage of Actions gates by PipelineK may not regress."""
    base = resolve_base()
    if base is None:
        check(
            False,
            "the base commit could not be resolved, so this ratchet has "
            "nothing to compare against. Fetch main (`git fetch origin main`). "
            "A ratchet that skips when it cannot see the baseline is the "
            "silent-green failure this repository keeps refusing to ship",
        )
        return

    base_state = at_ref(base)
    if base_state is None:
        check(
            False,
            f"could not read the orchestrators at base {base[:12]}; the gap "
            f"cannot be compared",
        )
        return

    base_workflows, base_pipelines = base_state
    head_workflows = read_workflows()
    head_pipelines = read_pipelines()

    base_actions = actions_gates(base_workflows)
    head_actions = actions_gates(head_workflows)
    base_covered = base_actions & pipelinek_gates(base_pipelines)
    head_covered = head_actions & pipelinek_gates(head_pipelines)

    base_gap = base_actions - base_covered
    head_gap = head_actions - head_covered

    newly_uncovered = sorted(head_gap - base_gap)
    newly_covered = sorted(base_gap - head_gap)

    check(
        not newly_uncovered,
        f"the PipelineK coverage gap grew against base {base[:12]}: "
        f"{len(base_gap)} -> {len(head_gap)} gates. Newly in the gap:\n"
        + "\n".join(f"      - {g}" for g in newly_uncovered)
        + "\n    A gate added to Actions is not automatically added to "
        "PipelineK. Add it to the matching pipeline in the same commit, or "
        "state why the Kotlin side does not need it.",
    )

    print(
        f"orchestrator gap: {len(head_gap)} of {len(head_actions)} Actions gates "
        f"not covered by PipelineK (base {base[:12]}: {len(base_gap)}); "
        f"{len(newly_covered)} newly covered"
    )
    for gate in newly_covered:
        print(f"  covered: {gate}")


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
        "PASS — the PipelineK coverage gap did not grow against the base "
        "commit. The gap is expected to be non-zero and to shrink: the "
        "closure criterion is zero executable Actions workflows, and this "
        "contract is the instrument that makes progress towards it visible."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
