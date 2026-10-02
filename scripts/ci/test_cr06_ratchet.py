#!/usr/bin/env python3
"""Ratchet: the CR-06 production allowlist may only shrink, measured per PR.

CR-06 (`application_no_infrastructure` / `application_no_interface`) became a
real fitness function: `LayerId::Interface` exists, `from_module_path()`
resolves it, and `cr06_synthetic_drift_application_to_interface_is_detected`
proves the rule fires. Measured 2026-10-02, that part is done.

What it measures is a second thing. `cr06_allowlist.rs` carries 39
`TemporaryException` entries that suppress real drift findings, and the count
has only ever gone up:

    38 -> 40 (2026-09-30)   two application_no_interface drifts became
                            visible once LayerId::Interface existed
    40 -> 39 (2026-09-30)   ST-01 closed; infrastructure::vfs:: deleted

The single control over that number is
`inventory_size_is_pinned_at_current_baseline` (cr06_allowlist.rs:548). It
asserts 39, and its own failure message says:

    Update both the allowlist and this test, in the same commit.

So adding a production exception and bumping the pin is a two-line diff and a
green build. The number is pinned, and pinning a number that you are allowed to
raise in the same breath is not a ratchet. It is a changelog.

That matters because the architectural programme ST-01..ST-05 is measured by
this number, and its whole claim is directional:

    production CR-06 exceptions:  36 -> 29 -> 11 -> 0

Treated as prose in a review, that sequence cannot be falsified by the thing it
is trying to catch.

## What this asserts

1. An entry counts as *production debt* only if its dependency path really is
   imported outside `#[cfg(test)]` in the file it names. The classification is
   derived from the source, not from the entry's own `rationale` string — a
   relabelled rationale must not buy an exemption.
2. Production debt on the branch may not exceed production debt at the PR's
   base commit. Compared against the base, not against a constant, so there is
   no number in this file to raise.
3. The comparison is fail-closed. If the base cannot be resolved, this does not
   pass vacuously.

`cfg(test)` entries are excluded from the count on purpose. A behaviour test
that composes the real tree-sitter adapter is not architectural debt, and three
such guards exist today (all `team:st-01`). Counting them would make the only
honest response to "add a regression guard" be to let the ceiling drift.

## Approving growth deliberately

Set `APPROVED_PRODUCTION_GROWTH` to the number of additional production
entries this repository has accepted, with a comment naming who approved it and
why. It starts at 0. It is the one escape hatch, and it is a one-line diff.

## Resolution of the base commit

`GITHUB_BASE_SHA` (or `pull_request.base.sha` from `GITHUB_EVENT_PATH`) is
authoritative; otherwise `origin/main`. Both come from the checkout, so this is
runnable locally the same way it runs in CI.

Run:
    python3 scripts/ci/test_cr06_ratchet.py
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CORE_SRC = "crates/cognicode-core/src"
ALLOWLIST_REL = f"{CORE_SRC}/application/architecture/cr06_allowlist.rs"

# The escape hatch. Zero means: this repository has never approved growing the
# production CR-06 allowlist, and every PR that grows it is expected to fail.
# Raising it is the deliberate act this contract is designed to make visible.
APPROVED_PRODUCTION_GROWTH = 0

# An inventory entry is a call to the local `ex(...)` helper that opens on its
# own line at some indentation. Anchoring on the line start is what keeps the
# helper's own definition — `fn ex(` — out of the inventory; matching `ex(`
# anywhere found it, and a definition is not an exception.
EX_OPEN = re.compile(r"^([ \t]+)ex\($", re.MULTILINE)
QUOTED = re.compile(r'"((?:[^"\\]|\\.)*)"')

# A line that is a real Rust item declaration. Anchored at the start of the
# line so that a `//` comment mentioning `use`, or a string literal containing
# one, cannot be mistaken for an import. Item lines are what the dependency is
# actually named on.
ITEM_LINE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:use|mod|extern\s+crate)\b"
)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def git(*args: str) -> str | None:
    """Run a git command in the repo. None if git is unusable here."""
    try:
        out = subprocess.run(
            ["git", *args],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return out.stdout if out.returncode == 0 else None


def read_at(ref: str, rel_path: str) -> str | None:
    """File contents at `ref`, or None if that ref or path is unavailable."""
    return git("show", f"{ref}:{rel_path}")


def strip_rust_comments(text: str) -> str:
    """Remove `//` line comments and `/* */` blocks, keeping line numbering.

    The allowlist's module doc explains the format in prose and quotes tuples
    inside backticks. Scanning the raw text would count documentation as
    inventory, and this file's own header is longer than its inventory.
    """
    out_lines: list[str] = []
    in_block = False
    for line in text.splitlines():
        if in_block:
            if "*/" in line:
                line = line.split("*/", 1)[1]
                in_block = False
            else:
                out_lines.append("")
                continue
        while "/*" in line:
            head, _, tail = line.partition("/*")
            if "*/" in tail:
                line = head + tail.split("*/", 1)[1]
            else:
                line = head
                in_block = True
                break
        # A `//` inside a string literal would truncate a line that matters.
        # None of the inventory lines contain one, and the cost of being wrong
        # here is a miscounted entry, not a wrong verdict, so the simple split
        # is the honest trade.
        code = line.split("//", 1)[0]
        out_lines.append(code)
    return "\n".join(out_lines)


def parse_entries(text: str) -> list[dict[str, str]]:
    """Every `ex(...)` call as {constraint, file_path, dependency_path, owner}.

    A call that does not yield four fields is reported and skipped: a malformed
    entry must not silently shrink the inventory, which would make this ratchet
    pass by making itself blind. It is reported as a failure rather than being
    counted as debt so that fixing the format is the only way to clear it.
    """
    source = strip_rust_comments(text)
    lines = source.splitlines()
    entries: list[dict[str, str]] = []
    for match in EX_OPEN.finditer(source):
        start = source[: match.end()].count("\n") + 1
        indent = match.group(1)
        closing = f"\n{indent})"
        end = source.find(closing, match.end())
        body = (
            source[match.end() : end]
            if end != -1
            else source[match.end() : match.end() + 400]
        )
        fields = QUOTED.findall(body)
        if len(fields) < 4:
            failures.append(
                f"CR-06 allowlist line {start}: an ex(...) call has "
                f"{len(fields)} quoted fields, expected at least 4 "
                f"(constraint, file_path, dependency_path, owner). The entry "
                f"cannot be classified, so it is NOT counted as production "
                f"debt: {' '.join(body.split())[:80]!r}"
            )
            continue
        entries.append(
            {
                "constraint": fields[0],
                "file_path": fields[1],
                "dependency_path": fields[2],
                "owner": fields[3],
            }
        )
    return entries


def count_braces(line: str) -> int:
    """Net brace delta, ignoring braces inside string literals.

    Mirrors `count_braces` in cr06_allowlist.rs. A `"{"` in a message must not
    open a scope.
    """
    delta = 0
    in_string = False
    previous = ""
    for char in line:
        if char == '"' and previous != "\\":
            in_string = not in_string
        if not in_string:
            if char == "{":
                delta += 1
            elif char == "}":
                delta -= 1
        previous = char
    return delta


def cfg_test_line_spans(text: str) -> set[int]:
    """1-based line numbers that sit inside an open `#[cfg(test)]` scope.

    This mirrors `classify_imports` in cr06_allowlist.rs, and deliberately so:
    the Rust version is the authority for whether an entry is test-only, and a
    second implementation with different semantics would be a second source of
    truth for the same question — the failure mode this repository already paid
    for once, with `EvidenceStore`.

    The first attempt here read "a `#[cfg(test)]` marker followed by a `mod`"
    and marked only the attribute's own line otherwise. That is wrong, and it
    was caught by measurement rather than by reading: in
    `application/workspace_session.rs:193` the import

        #[cfg(test)]
        pub(crate) async fn new(workspace_root: impl AsRef<Path>) -> ... {
            use crate::interface::mcp::security::InputValidator;

    is test-only by scope depth, and a line-based reading called it production.
    The attribute gates whatever item follows it, `mod` or not.
    """
    spans: set[int] = set()
    depth = 0
    open_scopes: list[int] = []
    pending_cfg_test = False
    for index, raw in enumerate(text.splitlines(), start=1):
        trimmed = raw.strip()
        if trimmed == "#[cfg(test)]":
            pending_cfg_test = True
            continue
        if open_scopes:
            spans.add(index)
        if pending_cfg_test:
            open_scopes.append(depth)
            pending_cfg_test = False
        depth += count_braces(trimmed)
        open_scopes = [level for level in open_scopes if level < depth]
    return spans


def dependency_leaves(path: str) -> list[str]:
    """Leaf names a dependency path covers.

    Allowlist matching is by prefix, so `infrastructure::parser` covers
    `infrastructure::parser::Language` and `::TreeSitterParser` from one entry.
    A bare module segment therefore has to match on its last component, or the
    entry would look unused and be misfiled as retired.
    """
    return [part for part in path.split("::") if part]


def item_statements(text: str) -> list[tuple[int, str]]:
    """(start_line, joined_text) for every `use` / `mod` item declaration.

    Statements, not lines. Measured 2026-10-02: 167 lines under
    `cognicode-core/src/application/` open a brace block —

        use crate::infrastructure::graph::{
            GraphCache, TraversalDirection,
        };

    — so the leaf name the allowlist names often sits on a continuation line.
    Reading one line per import found 31 production entries; reading whole
    statements finds more, and the difference is debt this ratchet would have
    failed to see. Under-counting is the one error direction that makes a
    ratchet useless, so the statement is the unit.

    A `#[cfg(test)]` attribute is carried on the statement too: it is a line
    above the item, and the attribute is the only thing marking some test
    imports.
    """
    lines = text.splitlines()
    statements: list[tuple[int, str]] = []
    index = 0
    while index < len(lines):
        line = lines[index]
        if not ITEM_LINE.match(line):
            index += 1
            continue
        parts = [line]
        cursor = index
        # A statement ends at its semicolon. The cap keeps a malformed file
        # from swallowing the rest of the file into one "statement".
        while ";" not in parts[-1] and cursor - index < 30:
            cursor += 1
            if cursor >= len(lines):
                break
            parts.append(lines[cursor])
        start = index + 1
        # Pull in an attribute comment directly above the item.
        if start >= 2 and "cfg(test)" in lines[start - 2]:
            start -= 1
            parts.insert(0, lines[start - 1])
        statements.append((start, " ".join(parts)))
        index = cursor + 1
    return statements


def imported_in_production(file_text: str, dependency_path: str) -> bool:
    """Does the named dependency appear in this file outside `#[cfg(test)]`?

    Matched on the final path segment, which is what a `use` statement names
    (`use crate::infrastructure::parser::Language;`). A dependency path with no
    `::` is matched whole.
    """
    leaves = dependency_leaves(dependency_path)
    if not leaves:
        return False
    leaf = leaves[-1].rstrip(":")
    if not leaf:
        return False

    test_lines = cfg_test_line_spans(file_text)
    for start, statement in item_statements(file_text):
        if start in test_lines:
            continue
        if leaf in statement:
            return True
    return False


# The (file_path, dependency_path) pairs the Rust guard
# `cfg_test_only_entries_really_have_no_production_import` asserts are test-only,
# read straight out of its GUARDED constant so this mirror cannot drift from
# the authority it mirrors. Parsed rather than duplicated: a second hand-kept
# copy of the same list is exactly the drift this is meant to catch.
GUARDED_CONST = re.compile(
    r"const GUARDED: &\[\(&str, &str\)\] = &\[(.*?)\n        \];", re.DOTALL
)
GUARDED_PAIR = re.compile(
    r'\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*,?\s*\)', re.DOTALL
)


def rust_guarded_pairs() -> list[tuple[str, str]]:
    """The Rust guard's own test-only list, read from its source."""
    text = working_tree_source(ALLOWLIST_REL)
    if text is None:
        return []
    match = GUARDED_CONST.search(strip_rust_comments(text))
    if not match:
        failures.append(
            "cannot find the GUARDED constant in cr06_allowlist.rs, so this "
            "contract cannot cross-check itself against the Rust guard. If it "
            "was renamed, update GUARDED_CONST here — do not delete the check, "
            "because two implementations of the same question disagreeing "
            "silently is the failure this ratchet exists to prevent"
        )
        return []
    return GUARDED_PAIR.findall(match.group(1))


def cross_check_against_rust_guard(
    entries: list[dict[str, str]],
    production_keys: set[tuple[str, str]],
) -> None:
    """The mirror and the Rust guard must agree on every shared entry.

    The Rust guard is the authority: it is compiled and run by `cargo test`.
    This contract exists to count debt per PR, which cargo cannot do. Where
    both have an opinion — the entries in GUARDED, which the guard proves are
    test-only — a disagreement means one implementation is wrong, and the only
    honest response is to fail rather than pick a winner.
    """
    guarded = rust_guarded_pairs()
    check(
        bool(guarded),
        "the Rust guard's GUARDED list is empty or unreadable; a cross-check "
        "against nothing is not a cross-check",
    )
    inventory = {(e["file_path"], e["dependency_path"]) for e in entries}
    for pair in guarded:
        check(
            pair in inventory,
            f"the Rust guard lists {pair[0]} :: {pair[1]} as a cfg(test)-only "
            f"entry, but this contract cannot find it in the inventory. One of "
            f"the two is reading a stale list",
        )
        check(
            pair not in production_keys,
            f"the Rust guard proves {pair[0]} :: {pair[1]} has no production "
            f"import, and this contract classified it as production debt. The "
            f"two implementations of the same question disagree",
        )


def production_entries(allowlist_text: str, read_source) -> list[dict[str, str]]:
    """Entries whose dependency is imported outside `#[cfg(test)]`.

    `read_source(rel_path)` supplies file contents for a given ref, so the same
    classification runs against the base commit and the branch head.
    """
    production: list[dict[str, str]] = []
    for entry in parse_entries(allowlist_text):
        file_rel = f"{CORE_SRC}/{entry['file_path']}"
        source = read_source(file_rel)
        if source is None:
            # The file the entry names does not exist at this ref. The Rust-side
            # test `every_entry_points_at_an_existing_file` covers the working
            # tree; here, an unreadable file means the entry cannot be shown to
            # be test-only, so it counts as production.
            entry = dict(entry, unreadable_source=True)
            production.append(entry)
            continue
        if imported_in_production(source, entry["dependency_path"]):
            production.append(entry)
    return production


def resolve_base() -> str | None:
    """The commit the branch's production debt is measured against."""
    explicit = os.environ.get("GITHUB_BASE_SHA", "").strip()
    if explicit:
        return explicit
    event_path = os.environ.get("GITHUB_EVENT_PATH", "").strip()
    if event_path and Path(event_path).is_file():
        try:
            event = json.loads(Path(event_path).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            event = {}
        base_sha = (event.get("pull_request") or {}).get("base", {}).get("sha")
        if isinstance(base_sha, str) and base_sha.strip():
            return base_sha.strip()
    for candidate in ("origin/main", "main"):
        resolved = git("rev-parse", "--verify", f"{candidate}^{{commit}}")
        if resolved:
            return resolved.strip()
    return None


def working_tree_source(rel_path: str) -> str | None:
    candidate = REPO_ROOT / rel_path
    if not candidate.is_file():
        return None
    try:
        return candidate.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return None


SELF_NAME = "test_cr06_ratchet.py"

# This ratchet is not named anywhere. The merge authority runs
# `scripts/ci/run-all-contracts.sh`, which discovers `scripts/ci/test_*.py` by
# glob. So the wiring question sits one level up — is the runner itself reached?
# — and it is asked here rather than assumed, because two gates in this
# repository shipped correct, tested and never run.
RUNNER = "scripts/ci/run-all-contracts.sh"
DISCOVERY_GLOB = "scripts/ci/test_*.py"


def check_gate_is_wired() -> None:
    """Something a merge waits on must reach this file.

    This is the A-013 / CP5 shape, and this contract is not exempt from it. Two
    gates in this repository shipped correct, tested, and unexecuted: the CP5
    skill gate and the cargo-deny supply-chain gate. A ratchet that nothing
    invokes is the same defect wearing a better name, and it is the most likely
    way for this work to be worthless.

    The chain has three links and the last one is the one that is easy to lose:
    the merge authority invokes the runner, the runner discovers this file by
    glob, and the glob still matches. A rename or a move turns this contract
    into something nothing runs while every other assertion here still passes.
    """
    import pipeline_authority as authority

    hosts = authority.invoked_by(RUNNER)
    check(
        authority.MERGE_AUTHORITY in hosts,
        f"{RUNNER} is not invoked by {authority.MERGE_AUTHORITY} "
        f"(found in: {sorted(hosts) or 'no pipeline at all'}). A ratchet that "
        f"nothing runs is the CP5 failure again: correct, tested, and invisible",
    )

    matched = sorted(REPO_ROOT.glob(DISCOVERY_GLOB))
    check(
        SELF_NAME in [p.name for p in matched],
        f"{SELF_NAME} no longer matches {DISCOVERY_GLOB}, so the runner never "
        f"discovers it. Renaming or moving this file without touching the glob "
        f"silently stops it from running",
    )

    # The runner has to execute what it discovered rather than accept a list it
    # was handed, or the glob is decoration and discovery can drift back to a
    # second place to forget a contract.
    runner_text = working_tree_source(RUNNER) or ""
    check(
        "CI_CONTRACTS" in runner_text and "test_*.py" in runner_text,
        f"{RUNNER} no longer discovers contracts by glob. It runs whatever it is "
        f"handed, which puts the contract list back into a second place",
    )


class Measurement:
    """The ratchet's numbers, resolved once and shared by every assertion.

    A contract whose assertions each re-derive the state can disagree with
    itself — one test counting a different inventory than the next. Resolving
    once, and having the unresolvable cases recorded as failures rather than as
    exceptions, keeps the properties independently readable while they share a
    single source of truth for the measurement.
    """

    def __init__(self) -> None:
        self.base: str | None = None
        self.base_allowlist: str | None = None
        self.head_allowlist: str | None = None
        self.base_production: list[dict[str, str]] = []
        self.head_production: list[dict[str, str]] = []
        self.head_entries: list[dict[str, str]] = []
        self.added: list[dict[str, str]] = []
        self.removed: list[dict[str, str]] = []

    def resolve(self) -> None:
        self.head_allowlist = working_tree_source(ALLOWLIST_REL)
        if self.head_allowlist is None:
            failures.append(
                f"allowlist not found: {REPO_ROOT / ALLOWLIST_REL}. The ratchet "
                f"has nothing to measure and refuses to report no-growth"
            )
            return

        self.base = resolve_base()
        if self.base is None:
            failures.append(
                "the base commit could not be resolved, so this ratchet has "
                "nothing to compare against. Set GITHUB_BASE_SHA, or fetch the "
                "branch it is measured against (`git fetch origin main`). A "
                "ratchet that skips when it cannot see the baseline is the "
                "silent-green failure this repository keeps refusing to ship"
            )
            return

        self.base_allowlist = read_at(self.base, ALLOWLIST_REL)
        if self.base_allowlist is None:
            failures.append(
                f"{ALLOWLIST_REL} does not exist at the base commit "
                f"{self.base}. The ratchet cannot classify debt it has no "
                f"baseline for"
            )
            return

        self.base_production = production_entries(
            self.base_allowlist, lambda rel: read_at(self.base, rel)
        )
        self.head_production = production_entries(
            self.head_allowlist, working_tree_source
        )
        self.head_entries = parse_entries(self.head_allowlist)

        base_keys = {(e["file_path"], e["dependency_path"]) for e in self.base_production}
        head_keys = {
            (e["file_path"], e["dependency_path"]) for e in self.head_production
        }
        self.added = [e for e in self.head_production
                      if (e["file_path"], e["dependency_path"]) not in base_keys]
        self.removed = [e for e in self.base_production
                        if (e["file_path"], e["dependency_path"]) not in head_keys]


M = Measurement()


def test_a_merge_waits_on_this_ratchet() -> None:
    """This contract must be invoked by a job merge-gate lists in needs.

    A-013 and CP5 both shipped a gate that was correct, tested, and never run.
    This file is not exempt from that failure mode, and it is the most likely
    way for the whole exercise to be worth nothing.
    """
    check_gate_is_wired()


def test_inventory_is_parsed() -> None:
    """A ratchet that reads zero entries reports no growth while measuring nothing."""
    check(
        bool(M.head_entries),
        f"no CR-06 entries were parsed out of {ALLOWLIST_REL}. The allowlist "
        f"format changed or the file is empty",
    )


def test_production_and_test_only_partition_the_inventory() -> None:
    """The classification must be a partition, and must not be entirely empty."""
    check(
        len(M.head_production) <= len(M.head_entries),
        f"classified {len(M.head_production)} production entries out of "
        f"{len(M.head_entries)} total, which is impossible: the classification "
        f"counts something the inventory does not contain",
    )
    check(
        bool(M.head_production),
        "every CR-06 entry classified as cfg(test)-only. If ST-01..ST-05 really "
        "did retire all production drift, delete the allowlist rather than "
        "leaving a ratchet over an empty list",
    )


def test_classification_agrees_with_the_rust_guard() -> None:
    """The compiled Rust guard is the authority; this mirror must not diverge."""
    cross_check_against_rust_guard(
        M.head_entries,
        {(e["file_path"], e["dependency_path"]) for e in M.head_production},
    )


def test_production_debt_did_not_grow() -> None:
    """The ratchet itself: production debt on this branch <= debt at the base."""
    if M.base is None or M.base_allowlist is None:
        return  # already reported as a failure by test_inventory_is_parsed's peers
    added = "\n".join(
        f"      + {e['file_path']} :: {e['dependency_path']} ({e['owner']})"
        for e in M.added
    )
    check(
        len(M.added) <= APPROVED_PRODUCTION_GROWTH,
        f"CR-06 production debt grew from {len(M.base_production)} to "
        f"{len(M.head_production)} against base {M.base[:12]} "
        f"({len(M.added)} added, {len(M.removed)} retired, "
        f"{APPROVED_PRODUCTION_GROWTH} approved):\n{added}\n"
        f"    ST-01..ST-05 retire these; they do not add them. If this growth "
        f"was decided, set APPROVED_PRODUCTION_GROWTH in this file with a "
        f"comment naming who approved it and why.",
    )


def test_a_relabelled_rationale_does_not_buy_an_exemption() -> None:
    """Classification is derived from the source, never from the entry's text.

    The cheapest way to defeat a ratchet keyed on the allowlist is to relabel a
    production exception's `rationale` as `cfg(test)-only:` — the phrase is
    already in three strings in that file, so it looks like the vocabulary the
    classifier reads. If it were the input, one edit would move entries out of
    the count without retiring any debt.

    Relabels a known production entry in memory and asserts it is still
    classified as production. The allowlist on disk is not touched; only the
    text handed to the classifier changes.
    """
    text = M.head_allowlist
    if text is None:
        return
    production_keys = {
        (e["file_path"], e["dependency_path"]) for e in M.head_production
    }
    victim = next(
        (e for e in M.head_entries
         if (e["file_path"], e["dependency_path"]) in production_keys),
        None,
    )
    if victim is None:
        return

    anchor = f'"{victim["dependency_path"]}",'
    if anchor not in text:
        check(False, f"cannot locate {anchor} in the allowlist source, so the "
                     f"relabel test did not run")
        return
    # Append the phrase to the owner string of that one entry. The owner is the
    # only quoted field adjacent to it that is safe to rewrite without changing
    # the tuple's meaning, and it is the field a reviewer would read.
    relabelled = text.replace(
        f'"{victim["owner"]}",',
        f'"{victim["owner"]} cfg(test)-only: relabelled by the ratchet contract",',
        1,
    )
    check(
        relabelled != text,
        f"could not relabel the owner string of {victim['file_path']} :: "
        f"{victim['dependency_path']}, so the relabel test proved nothing",
    )

    after = {
        (e["file_path"], e["dependency_path"])
        for e in production_entries(relabelled, working_tree_source)
    }
    check(
        (victim["file_path"], victim["dependency_path"]) in after,
        f"relabelling the rationale of {victim['file_path']} :: "
        f"{victim['dependency_path']} to say cfg(test)--only removed it from the "
        f"production count. The classifier is reading the entry's own text "
        f"instead of the source, which is the loophole this ratchet exists to "
        f"close",
    )


def main() -> int:
    M.resolve()

    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    if M.head_entries:
        print(
            f"CR-06 production debt: {len(M.base_production)} @ "
            f"{(M.base or '?')[:12]} -> {len(M.head_production)} on this branch "
            f"({len(M.added)} added, {len(M.removed)} retired); "
            f"{len(M.head_entries) - len(M.head_production)} of "
            f"{len(M.head_entries)} entries are cfg(test)-only, not counted"
        )

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    for entry in M.removed:
        print(f"  retired: {entry['file_path']} :: {entry['dependency_path']} "
              f"({entry['owner']})")
    if M.removed:
        print("  The base catches up on merge, so the next PR measures against "
              "the smaller number with nothing to edit here.")

    print(
        "PASS — the CR-06 production allowlist did not grow against the base "
        "commit, the cfg(test) guards were not counted as debt, and this "
        "classification agrees with the Rust guard on every entry both check"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
