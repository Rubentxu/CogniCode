#!/usr/bin/env python3
"""Every test surface either runs in a lane, or says why it does not.

The gap
-------

A gate runs what its lane says. A suite that no gate names is not a suite
that never runs: the integration lane runs `cargo test --workspace`, and
that sweeps every integration test in the crates. What does not exist is a
declaration of which lane owns a surface, so nobody can say that a
surface is watched.

Measured in this tree, with the rule this contract applies:

    196 integration tests under the crates    run by `cargo test --workspace`
     46 contracts under `scripts/ci`          run by the contract runner
      6 contracts under `scripts/product`      run by the contract runner
     83 unit tests under the explorer         no lane runs them
     41 browser tests under the explorer       no lane runs them
      4 browser tests under `tests/e2e`        no lane runs them
      8 python tests under the sandbox         no lane runs them
      3 unit tests under the backstage plugin  no lane runs them

The ones on the bottom are not deleted. They stay, and this contract
declares them: the reason they stay that way, and how many of them there
are. The number of surfaces that nobody runs becomes a fact in the tree
instead of a guess about it.

The rule
--------

Every test surface in the repository belongs to one of two worlds:

  - **it has a lane**: a pipeline that really runs it;
  - **it does not**: it is declared here, with the reason it stays that
    way and how many of them there are.

The counts are checked, not trusted. Adding a file to a declared tree
turns this contract red, so a surface cannot grow without someone
deciding what to do with it. Taking a lane out of the pipelines turns it
red too, so a surface cannot stop running without anyone noticing.

Another contract already covers the opposite direction: that every suite
a pipeline names exists under the crate it names.

The controls
------------

  - (control) the inventory finds surfaces; an inventory that found
    nothing would pass forever;
  - (control) a surface in an undeclared tree is reported;
  - (control) the count notices one more file;
  - (control) a lane that no pipeline runs is reported;
  - (control) a comment that names a lane does not keep it alive.

What this does not check
------------------------

  - **That a lane passes.** Whether a lane runs green is measured by the
    lane itself, in its own run.
  - **That leaving a surface without a lane is the right call.** The
    table says why it stays that way; the decision is a human one, and
    the reason is what survives.
  - **What the tests contain.** Neither how good they are nor whether
    they would pass if they ran.
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# A file is a test when its name says so: the python convention of a
# `test_` prefix or a `_test` suffix, and the javascript convention of a
# `.test` or `.spec` infix. Rust integration tests do not follow a naming
# convention, so they are recognised by where they live.
TEST_NAME = re.compile(
    r"(^test_[^/]*\.py$)"
    r"|(_test\.py$)"
    r"|(\.(?:test|spec)\.[^./]+$)"
)

# Directories that are not the repository: build output, installed
# dependencies and tool metadata. Walking them would measure the disk
# instead of the code.
SKIPPED_DIRECTORIES = frozenset(
    {
        ".git",
        ".pipelinek",
        ".venv",
        "__pycache__",
        "build",
        "dist",
        "node_modules",
        "target",
        "venv",
    }
)

# The lanes, and the text a pipeline has to contain for the lane to exist.
# A lane that is only written down is not a lane: the trees it covers stop
# being run without anyone noticing, so the text is looked for in the
# pipelines themselves.
LANES = {
    "crates/": "cargo test --workspace",
    "scripts/ci/": "scripts/ci/run-all-contracts.sh",
    "scripts/product/": "scripts/ci/run-all-contracts.sh",
}

# The surfaces that no lane runs. They are not deleted: they stay, and
# this table says why. The count is part of the declaration, and it is
# checked: a tree that grows past its declared count turns this contract
# red, so the surface cannot grow in silence.
WITHOUT_LANE = {
    "apps/explorer-ui/src/": (
        "the explorer unit suite needs a javascript toolchain and a test "
        "runner that no pipeline in this repository installs, so no gate "
        "executes it; the tests stay as reference material and are not "
        "presented as a guarantee",
        83,
    ),
    "apps/explorer-ui/e2e/": (
        "the browser suite needs a running dev server and a browser, and "
        "no pipeline in this repository installs or drives either; the "
        "specs stay as reference material and are not presented as a "
        "guarantee",
        41,
    ),
    "tests/e2e/": (
        "the browser suite at the root of the repository needs a running "
        "dev server and a browser, and no pipeline in this repository "
        "installs or drives either; the specs stay as reference material "
        "and are not presented as a guarantee",
        4,
    ),
    "sandbox/scripts/tests/": (
        "these python checks assert against a sandbox that only the "
        "sandbox lane assembles, and no pipeline runs them; they stay as "
        "reference material until a lane can host them",
        8,
    ),
    "integrations/backstage/": (
        "the backstage plugin is a separate package with its own "
        "toolchain, and no pipeline in this repository installs or runs "
        "it; the tests stay as reference material and are not presented as "
        "a guarantee",
        3,
    ),
}

# Trees that are not test surfaces of this repository. Each one says what
# it actually is, because an exclusion without a reason is a hole with a
# lid on it.
EXCLUDED = {
    "sandbox/fixtures/": "example projects the sandbox exercises, not tests of this repository",
}

# The findings are collected here and `main()` turns them into an exit
# code. A contract that reports a failure and exits with zero reports a
# green that never happened, which is worse than having no contract.
failures: list[str] = []


def is_test_file(relative: Path) -> bool:
    """Whether a file is a test, by its name or by where it lives."""
    if TEST_NAME.search(relative.name):
        return True
    parts = relative.parts
    return (
        parts[0] == "crates"
        and "tests" in parts
        and relative.suffix == ".rs"
    )


def is_excluded(relative: Path) -> bool:
    """Whether a file belongs to a tree that is not a test surface."""
    text = str(relative)
    return any(text.startswith(prefix) for prefix in EXCLUDED)


def surfaces() -> list[Path]:
    """Every test surface in the repository, as a relative path."""
    found: list[Path] = []
    for directory, subdirectories, names in os.walk(REPO_ROOT):
        subdirectories[:] = sorted(
            name for name in subdirectories if name not in SKIPPED_DIRECTORIES
        )
        for name in sorted(names):
            relative = (Path(directory) / name).relative_to(REPO_ROOT)
            if is_excluded(relative):
                continue
            if is_test_file(relative):
                found.append(relative)
    return found


def class_of(relative: Path, classes: tuple[str, ...]) -> str | None:
    """The class a surface belongs to: the longest prefix that matches."""
    text = str(relative)
    best: str | None = None
    for prefix in classes:
        if text.startswith(prefix) and (best is None or len(prefix) > len(best)):
            best = prefix
    return best


def unclassified(paths: list[Path]) -> list[str]:
    """The surfaces that belong to no declared class."""
    classes = tuple(LANES) + tuple(WITHOUT_LANE)
    return sorted(str(path) for path in paths if class_of(path, classes) is None)


def counts(paths: list[Path]) -> dict[str, int]:
    """How many surfaces each declared class holds."""
    classes = tuple(LANES) + tuple(WITHOUT_LANE)
    tally = {prefix: 0 for prefix in classes}
    for path in paths:
        prefix = class_of(path, classes)
        if prefix is not None:
            tally[prefix] += 1
    return tally


def strip_comments(text: str) -> str:
    """The pipeline text without its comments.

    A comment can name a lane that no pipeline runs. Counting that would
    keep the check green while the lane is gone, and a check that a
    sentence can satisfy is not a check. So the comments come out before
    the lanes are looked for.
    """
    kept: list[str] = []
    for line in text.split("\n"):
        if line.strip().startswith("//") or line.strip().startswith("#"):
            continue
        cut = len(line)
        inside_quotes = False
        position = 0
        while position < len(line):
            character = line[position]
            if character == '"':
                inside_quotes = not inside_quotes
            elif (
                character == "/"
                and not inside_quotes
                and line.startswith("//", position)
            ):
                cut = position
                break
            position += 1
        kept.append(line[:cut])
    return "\n".join(kept)


def pipelines_text() -> str:
    """Everything the pipelines run, in one text."""
    return strip_comments(
        "\n".join(
            path.read_text(encoding="utf-8", errors="replace")
            for path in sorted(REPO_ROOT.glob("*.pipeline.kts"))
        )
    )


def lanes_without_a_pipeline(lanes: dict[str, str], text: str) -> list[str]:
    """The lanes that no pipeline mentions."""
    return sorted(
        f"{prefix} (expects `{lane}` in a pipeline)"
        for prefix, lane in lanes.items()
        if lane not in text
    )


def test_every_test_surface_belongs_to_a_declared_class() -> None:
    """No test surface is left outside the declaration."""
    missing = unclassified(surfaces())
    assert not missing, (
        "these test surfaces belong to no declared class, so nobody can say "
        "whether a lane runs them or why they stay without one:\n  "
        + "\n  ".join(missing)
        + "\nEither give the tree a lane, or declare it with its reason and "
        "its count."
    )


def test_every_lane_is_really_run_by_a_pipeline() -> None:
    """A lane that no pipeline runs is not a lane."""
    missing = lanes_without_a_pipeline(LANES, pipelines_text())
    assert not missing, (
        "these lanes are declared here but no pipeline mentions them, so the "
        "surfaces they cover stopped being run without anyone noticing:\n  "
        + "\n  ".join(missing)
    )


def test_every_declared_count_matches_the_tree() -> None:
    """The declared count is the count, not a memory."""
    tally = counts(surfaces())
    wrong = [
        f"{prefix}: declared {declared}, found {tally.get(prefix, 0)}"
        for prefix, (_, declared) in sorted(WITHOUT_LANE.items())
        if tally.get(prefix, 0) != declared
    ]
    assert not wrong, (
        "the declared counts do not match the tree:\n  "
        + "\n  ".join(wrong)
        + "\nA surface that nobody runs has grown or shrunk behind the "
        "declaration. Decide what to do with it and declare the new count."
    )


def test_every_tree_without_a_lane_states_why() -> None:
    """A surface without a lane that does not say why says nothing."""
    silent = [
        prefix
        for prefix, (reason, _) in sorted(WITHOUT_LANE.items())
        if not reason.strip()
    ]
    assert not silent, (
        "these trees are declared without a lane and without a reason:\n  "
        + "\n  ".join(silent)
        + "\nA retirement without a reason is not a decision, it is a name."
    )


def test_the_inventory_is_not_empty() -> None:
    """Control: the inventory has to find surfaces.

    A contract that finds nothing passes forever. This control measures
    that the inventory is alive, so that an empty result is a fact and not
    a silent failure.
    """
    found = surfaces()
    assert found, (
        "the inventory found no test surface at all; if the inventory "
        "stopped looking, this contract would pass forever"
    )


def test_a_surface_in_an_undeclared_tree_is_reported() -> None:
    """Control: a surface outside the declaration has to be reported.

    Measured on a tree of its own, with a surface that no class claims:
    if the rule does not see it, the rule is not looking at what it says
    it looks at.
    """
    missing = unclassified([Path("tools/probe.test.ts")])
    assert missing, (
        "FAIL: a surface in an undeclared tree was not reported. The rule is "
        "not looking at what it says it looks at."
    )


def test_the_count_notices_one_more_file() -> None:
    """Control: the count has to count.

    Measured on a tree of its own: one more file has to move the number.
    A count that cannot move is a number that was written once and never
    checked.
    """
    before = counts([Path("apps/explorer-ui/e2e/one.spec.ts")])
    after = counts(
        [
            Path("apps/explorer-ui/e2e/one.spec.ts"),
            Path("apps/explorer-ui/e2e/two.spec.ts"),
        ]
    )
    assert before["apps/explorer-ui/e2e/"] == 1, (
        "FAIL: the count of a tree did not count the file it holds."
    )
    assert after["apps/explorer-ui/e2e/"] == 2, (
        "FAIL: one more file did not move the count. A count that cannot "
        "move cannot notice a surface growing."
    )


def test_a_lane_without_a_pipeline_is_reported() -> None:
    """Control: a lane that no pipeline runs has to be reported.

    Measured on a lane of its own, against a text that does not mention
    it: the check has to name it, or a lane that quietly stopped running
    would look exactly like one that never did.
    """
    missing = lanes_without_a_pipeline(
        {"crates/": "cargo test --workspace"}, "sh('$cd && echo hello')\n"
    )
    assert missing, (
        "FAIL: a lane that no pipeline mentions was not reported. The check "
        "would pass on a lane that stopped running."
    )


def test_a_comment_cannot_keep_a_lane_alive() -> None:
    """Control: a comment that names a lane is not a lane.

    Measured on a text of its own, where the only mention of the lane is
    inside a comment: the lane has to be reported as missing, or a
    sentence about it would keep the check green while it stopped
    running.
    """
    text = strip_comments('sh("""\n// cargo test --workspace\n""")\n')
    missing = lanes_without_a_pipeline(
        {"crates/": "cargo test --workspace"}, text
    )
    assert missing, (
        "FAIL: a comment kept a lane alive. A sentence about a lane is not "
        "the lane."
    )


def main() -> int:
    """Runs the checks and returns 1 if any of them fails.

    The contract of the contracts requires this shape: a file that
    accumulates its findings in a list has to turn that list into an exit
    code other than zero, or running it by hand would report a success
    that did not happen.
    """
    for name, function in sorted(globals().items()):
        if not name.startswith("test_") or not callable(function):
            continue
        try:
            function()
            print(f"PASS: {name}")
        except AssertionError as error:
            failures.append(f"FAIL: {name}: {error}")
            print(f"FAIL: {name}: {error}", file=sys.stderr)
        except Exception as error:  # noqa: BLE001 — the contract reports, it does not abort
            failures.append(f"ERROR: {name}: {error}")
            print(f"ERROR: {name}: {error}", file=sys.stderr)

    if failures:
        print(f"\n{len(failures)} contract(s) failed", file=sys.stderr)
        return 1
    print("\nall contracts passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
