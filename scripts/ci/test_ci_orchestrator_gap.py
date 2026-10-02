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

# Differences between the two orchestrators that are not a missing gate.
#
# Each entry is a gate Actions runs and PipelineK does not, with the reason it
# is not one. They are declared rather than filtered out silently: every entry
# is printed on every run, so a reviewer sees the same list the contract is
# reasoning about, and challenging one is a one-line edit. The ratchet still
# measures the raw gap against the base commit, so adding a gate here does not
# hide a growth — it only stops a justified difference being reported as a
# regression.
ACCEPTED_DIFFERENCES: dict[str, str] = {
    "cargo build --release --bin cognicode-mcp": (
        "Inside the step ci.yml disables with `if: false`, pending a running API "
        "server. Migrating a step nothing executes would put a gate in the "
        "inventory that has never run, which is the failure this migration "
        "exists to remove. It moves here when it is enabled there."
    ),
    "target/release/cognicode-mcp --cwd . --tools file_read,file_write 2>&1 | head -5": (
        "The second half of that same disabled step."
    ),
    "cargo fmt --check": (
        "Same gate, different spelling: PipelineK runs "
        "`cargo fmt --all -- --check`, which covers every workspace member. The "
        "un-suffixed form Actions uses is the looser of the two."
    ),
}

# A fourth entry used to live here: `cargo test <param>`, justified as
# "Actions collapses the eight-arm feature matrix into one templated step, so
# the inventory can only see one gate". That stopped being true when the
# extractor learned to join backslash continuations, which is what the YAML
# feature matrix actually uses: the eight arms became individually visible on
# both sides, and the entry became dead weight. It was deleted because the
# ratchet below said so, which is the ratchet doing its job.
#
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
# Prerequisites are counted too, and that is deliberate: `cargo install
# cargo-llvm-cov`, `rustup target add` and `npm install -g @commitlint/cli`
# decide nothing, but a failure in any of them aborts the lane, so one side
# having it and the other not is a real difference in what the two
# orchestrators actually do. What is excluded is setup that cannot fail the
# lane — `python3 -m pip install` is the one case, because a pip failure there
# surfaces as the gate that needed it failing, and counting it separately
# would let a pipeline claim coverage by installing the thing that checks it.
#
# Anchored to the start of the command. Searching anywhere in the line made
# `chmod +x target/release/cognicode` and `test -x target/release/...` count
# as gates: they mention a binary, they do not run one. The pipe form is the
# one place a gate is not the head — `echo "$subject" | commitlint` runs
# commitlint — so it is matched separately rather than by anchoring.
# `$repoRoot/` is the repository root the pipelines resolve once and interpolate,
# so `$repoRoot/target/release/cognicode-release` runs the same binary that
# `target/release/cognicode-release` does. It is in this list because otherwise
# every step invoking the release tool through the pipeline's own variable read
# as no gate at all.
GATE_HEAD = re.compile(
    r"^(?:cargo|just|python3|bash|commitlint|gh|rustup|npm|"
    r"target/release/|\./target/release/|\$repoRoot/)\b"
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
# `\"` inside a single-line Kotlin string is an escaped quote, not the end of
# the argument list. The previous pattern stopped at it, so a step written as
# `sh("$cd && foo.sh \"${VAR}\"")` was read as `foo.sh \` — the argument
# vanished and the gate compared as a shorter, different command.
SH_LINE = re.compile(r'sh\(\s*"((?:[^"\\]|\\.)*)"', re.DOTALL)
FOR_LOOP = re.compile(r"for\s+\w+\s+in\s+\\?\s*\n(.*?)\n\s*do", re.DOTALL)
DONE = re.compile(r"^\s*done\s*$", re.MULTILINE)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def normalise(command: str) -> str:
    """A comparable identity for one gate invocation."""
    # An escaped quote inside a single-line Kotlin string is a quote, not a
    # delimiter. This runs before the backslash rule below, which would
    # otherwise turn the `\` of `\"` into a space and leave the argument with a
    # trailing one.
    command = command.replace('\\"', '"')
    # A YAML `run: |` block continues a command with a trailing backslash, and
    # the backslash survives whitespace joining as a lone token. The Kotlin side
    # writes the same command on one line, so the two never matched.
    command = command.replace("\\", " ")
    command = " ".join(command.split())
    # The inline `run:` capture keeps the key.
    command = re.sub(r"^run:\s*", "", command)
    # A trailing comment is prose about the command, not part of it. Actions
    # writes `just sandbox-pull || true   # || true: digests pueden cambiar`
    # and the comment survived every later rule, so one gate and its PipelineK
    # counterpart read as two.
    command = re.sub(r"\s+#.*$", "", command)
    command = re.sub(r"^cd\s+\S+\s*&&\s*", "", command)
    # The same wrapper written as a shell variable, which is how every stage
    # in these pipelines spells it. It is a `cd`, not a parameter, and mapping
    # it to `<param>` moved the gate off the head of the command: the inventory
    # then saw 102 gaps in a pipeline that runs 99 gates, every one of them a
    # command that had lost its `cd` prefix. A false positive in the instrument
    # that measures the migration is worse than no instrument.
    command = re.sub(r"^\$cd\s*&&\s*", "", command)
    # The repository root the pipeline resolves once, said as a wrapper. It has
    # to go before the `$var` rule below, or the rule turns it into
    # `<param>/target/release/cognicode-release`, the head stops being a gate,
    # and every step that reaches a binary through the pipeline's own variable
    # becomes invisible. Wrapper, not parameter — the same distinction as `$cd`.
    command = command.replace('"$repoRoot/', "").replace("$repoRoot/", "")
    # Selecting which justfile to read is how the recipe is invoked, not which
    # recipe it is. `just --justfile sandbox/justfile sandbox-ci-smoke` and
    # `just sandbox-ci-smoke` are one gate; the sandbox lanes are only reachable
    # from the repository root with the explicit form, because a plain `import`
    # collides on the `build` recipe.
    command = re.sub(r"^just\s+--justfile\s+\S+\s+", "just ", command)
    command = re.sub(r"^set -e\s+", "", command)
    # `if ! <gate>; then` is a gate under a condition. The condition is the
    # orchestrator's business; the gate is the command.
    command = re.sub(r"^if\s+!?\s*", "", command)
    command = command.strip().rstrip("\\").strip()
    # `|| true` is a deliberate tolerance and means the same as the advisory
    # form: the exit code stops being the verdict. The gate is the command.
    command = re.sub(r"\s*\|\|\s*true\s*$", "", command).strip()
    # An advisory stage is written `|| echo 'ADVISORY: ...'` so a failure is
    # reported without failing the lane. That preserves `ci.yml`'s
    # `continue-on-error: true` semantics — and the suffix is the one place
    # the PipelineK side spells a gate differently from Actions, so it has to
    # be removed for the gate to be recognised as the same one. Advisory-ness
    # lives in the pipeline, not in the gate's identity.
    command = re.sub(r"\s*\|\|\s*echo\s+['\"]?ADVISORY:.*$", "", command)
    # The shell scaffolding a gate is wrapped in is not the gate. Actions wraps
    # the coverage gate in `|| { ... }` and the PipelineK side in `; then`,
    # because the two languages spell an error handler differently; the command
    # between them is byte-identical. Left in, the CR-09 coverage gate — the
    # one gate with a numeric threshold — read as two different gates.
    command = re.sub(r"\s*(\|\|\s*\{|&&\s*\{|;\s*then|;\s*do|\}\s*)$", "", command)
    command = command.strip()
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
    # Kotlin escapes a literal `$` inside a raw string as `${'$'}`; the shell
    # receives a plain `$`. Left in place, `${'$'}tool` and `target/release/
    # cognicode-release` are the same command with two spellings, and the
    # inventory reported the pipeline as not running a gate it does run.
    command = command.replace("${'$'}", "$")
    # A shell variable is a parameter too, and for the same reason. A pipeline
    # that runs `build-sboms-for-lane.sh $target` runs exactly the gate that
    # Actions runs as `build-sboms-for-lane.sh "${{ matrix.rust_target }}"`.
    # Without this the migration cannot be measured: every stage that names its
    # platform once and interpolates it read as an uncovered gate, which is a
    # false positive in an instrument whose whole job is to be believed.
    #
    # `test_the_normaliser_does_not_fuse_two_different_gates` is the guard on
    # this: losing a distinction is worse than reporting a spurious gap.
    command = re.sub(r"\$\{[A-Za-z_][A-Za-z0-9_]*[^}]*\}", "<param>", command)
    command = re.sub(r"\$[A-Za-z_][A-Za-z0-9_]*", "<param>", command)
    return command


def gate_key(command: str) -> str:
    """Drop the wrapper that differs between the two orchestrators.

    The Kotlin side has no `cd`, and resolves the repository root once; the YAML
    side prefixes most steps with it. Those are packaging differences, not
    different gates.
    """
    # Shell quoting is syntax, not identity: `script.sh "$target"` and
    # `script.sh $target` are the same gate, and making the match depend on
    # the author having quoted identically in both orchestrators is a way to
    # report a covered gate as missing.
    command = re.sub(r"([\"'])<param>\1", "<param>", command)
    command = command.replace("$cd && ", "")
    command = re.sub(r"^\$\{?cd[^&|]*&&\s*", "", command)
    command = (
        command.replace('"$repoRoot/', "")
        .replace("$repoRoot/", "")
        .replace("./", "")
    )
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


def join_continuations(block: str) -> list[str]:
    """Fold a `run: |` block's backslash continuations into one line each.

    A line ending in a backslash is not a command, it is the first half of one. Treated
    as a command on its own it produced an identity with no arguments —
    `target/release/cognicode-release generate` — which can never equal the
    PipelineK side's `... generate --staging staging --out release ...` no
    matter what either orchestrator actually runs. The two spellings of the
    same command are joined here, once, so the comparison sees the command.
    """
    lines = [l.strip() for l in block.splitlines()]
    joined: list[str] = []
    pending = ""
    for line in lines:
        if not line or line.startswith("#"):
            continue
        if pending:
            line = pending + " " + line
            pending = ""
        if line.endswith("\\"):
            pending = line[:-1].rstrip()
            continue
        joined.append(line)
    if pending:
        # A trailing backslash with nothing after it: the command is what there
        # is, and dropping it would lose a gate rather than shorten one.
        joined.append(pending)
    return joined


def actions_commands(workflow_texts: dict[str, str]) -> list[str]:
    """Every command line Actions runs, before any normalisation."""
    candidates: list[str] = []
    for text in workflow_texts.values():
        for block in RUN_BLOCK.findall(text):
            candidates.extend(join_continuations(block))
        candidates.extend(RUN_INLINE.findall(text))
    return candidates


def actions_gates(workflow_texts: dict[str, str]) -> set[str]:
    gates: set[str] = set()
    for candidate in actions_commands(workflow_texts):
        # Normalise first, then ask. Every Kotlin step is written as
        # `$cd && <gate>`, so anchoring the head check before the wrapper is
        # stripped matched nothing on the PipelineK side and reported the
        # whole pipeline as missing.
        keyed = gate_key(normalise(candidate))
        if is_gate(keyed):
            gates.add(keyed)
    return gates


# The type annotation is optional because the pipelines that declare a constant
# mostly write `val tool = "..."` without it, and requiring it would make the
# resolution quietly apply to nothing in this repository.
#
# A value containing a backslash is excluded. `val cd = "cd \"$repoRoot\""` is
# one, and resolving it would rewrite every step's wrapper rather than the gate
# inside it — which is the mistake the `$var` rule already made once. A `$` in
# the value is fine: it is the repo root, and `gate_key` drops that prefix.
VAL_DECL = re.compile(r'^val\s+(\w+)(?:\s*:\s*\w+)?\s*=\s*"([^"\\]*)"\s*$', re.MULTILINE)


def resolve_vals(text: str) -> str:
    """Substitute the pipeline's own string constants before reading steps.

    A pipeline declares `val tool = "$repoRoot/target/release/cognicode-release"`
    and every step then says `$tool generate`. That is the right way to write it
    — one declaration instead of a path repeated in six stages — but a checker
    reading the source sees `$tool` and cannot tell what it runs, so a gate the
    pipeline demonstrably runs reads as uncovered.

    Only `val x: String = "literal"` is resolved. A `val` bound to an expression
    (`System.getenv(...)`, `File(".")`) is left alone, because its value is not
    knowable from the text and pretending otherwise would make the instrument
    assert something it cannot see.
    """
    constants = dict(VAL_DECL.findall(text))
    if not constants:
        return text
    for name, value in constants.items():
        text = re.sub(r"\$\{'\$'\}" + re.escape(name) + r"\b", value, text)
        text = re.sub(r"\$" + re.escape(name) + r"\b", value, text)
    return text


def pipelinek_gates(pipeline_texts: dict[str, str]) -> set[str]:
    gates: set[str] = set()
    for raw in pipeline_texts.values():
        text = resolve_vals(raw)
        # The loop is expanded first, while the source still carries Kotlin's
        # escaped dollar, because that is what the loop placeholder looks like.
        # Unescaping before this point would rewrite the placeholder and expand
        # the loop to nothing — which reads as 24 MCP suites vanishing from a
        # pipeline that runs them.
        candidates = [line.strip() for line in expand_for_loops(text)]
        candidates.extend(SH_LINE.findall(text))
        # A stage written as a Kotlin raw string is a multi-line shell script,
        # and a command in it can wrap with a backslash exactly as one in a
        # `run: |` block can. Reading those line by line splits one command into
        # several, and the halves do not start with a gate head, so the gate
        # disappears rather than being shortened. Joined the same way, so the
        # two orchestrators are compared as commands rather than as spellings.
        for block in SH_RAW.findall(text):
            candidates.extend(join_continuations(block))
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


def test_the_normaliser_keeps_the_program_being_run() -> None:
    """Two commands that run different things must never share a gate identity.

    `normalise` is lossy by design: it erases `cd`, `set -e`, `run:`, the
    `|| echo ADVISORY` suffix, YAML matrix values, shell variables and quoting,
    because all of those are the orchestrator's spelling rather than the gate's.
    Every one of those rules can, if written carelessly, swallow the command
    itself — the `$var` rule first did exactly that, turning `$cd && cargo …`
    into `<param> && cargo …` and reporting 102 gaps in a pipeline running 99
    gates.

    What must survive is the program being invoked. So this checks the real
    inventory: for every gate identity, the first two significant tokens of
    each command that maps to it have to agree. A false match is worse than a
    gap here, because a gap is a to-do and a false match is silence.
    """
    programs: dict[str, set[str]] = {}
    for command in actions_commands(read_workflows()):
        keyed = normalise(command)
        if not is_gate(gate_key(keyed)):
            continue
        head = " ".join(gate_key(keyed).split()[:2])
        programs.setdefault(gate_key(keyed), set()).add(head)

    fused = {k: sorted(v) for k, v in programs.items() if len(v) > 1}
    check(
        not fused,
        "these gate identities are reached by commands that invoke different "
        "programs, so the inventory could claim coverage it does not have: "
        f"{fused}",
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

    # An entry that no longer matches anything is a stale justification, and a
    # stale justification is how an allowlist stops meaning anything.
    stale = sorted(set(ACCEPTED_DIFFERENCES) - head_gap)
    check(
        not stale,
        "these accepted differences no longer describe anything in the gap, so "
        "their justification is dead weight:\n"
        + "\n".join(f"      - {g}" for g in stale)
        + "\n    Delete them, or fix whichever orchestrator change made them "
        "unnecessary.",
    )

    explained = sorted(head_gap & set(ACCEPTED_DIFFERENCES))
    unexplained = head_gap - set(ACCEPTED_DIFFERENCES)

    print(
        f"orchestrator gap: {len(head_gap)} of {len(head_actions)} Actions gates "
        f"not covered by PipelineK (base {base[:12]}: {len(base_gap)}); "
        f"{len(newly_covered)} newly covered"
    )
    for gate in newly_covered:
        print(f"  covered: {gate}")
    for gate in explained:
        print(f"  accepted difference: {gate}")
    print(
        f"  {len(unexplained)} unexplained gap(s) remaining"
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
        "PASS — the PipelineK coverage gap did not grow against the base "
        "commit. The gap is expected to be non-zero and to shrink: the "
        "closure criterion is zero executable Actions workflows, and this "
        "contract is the instrument that makes progress towards it visible."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
