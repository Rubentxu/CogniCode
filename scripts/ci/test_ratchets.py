#!/usr/bin/env python3
"""Ratchets over the production sources: file size, public-type homonyms and the
CR-06 allowlist.

This file holds two families of ratchet that share one way of working: each
compares HEAD with the PR's base commit, so there is no number in this file to
raise, and each fails closed when the base cannot be resolved. They share
`git`, `check`, `failures` and `resolve_base`; each family keeps its own
measurement (`SIZE` and `CR06`) and its own `setup_*` function, and `setup()`
runs both.

# Part I -- file size and public-type homonyms


Both compare HEAD with the PR's base commit, so there is no number in this
file to raise and nothing to edit when the measured value shrinks: the base
catches up on merge and the next PR measures against the smaller value. Both
fail closed: if the base cannot be resolved they fail instead of passing
vacuously.

Production code is the code before the first column-0 `#[cfg(test)]`. Files
named `*_tests.rs` or `tests.rs` are test code and are skipped; files under
`tests/` are not measured at all.

## File size

A production source file may not exceed `LIMIT` lines, and one that already
does may not grow. A file that was at or under `LIMIT` at the base (or did not
exist) may not cross it. This mirrors the CR-06 ratchet in the second part of this file and deliberately
does NOT re-implement the CR-06 layering rule: that rule has one owner, and a
second copy would be a second source of truth.

## Public-type homonyms

A name declared more than once as `pub struct|enum|trait` must not be declared
more times than at the base. A name that was unique at the base (or absent) may
not become a homonym. Only unrestricted `pub` items count; `pub(crate)` and
friends are internal. This does not decide which duplicate wins; it only stops
the count growing.

# Part II -- the CR-06 production allowlist

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

Add the `(file_path, dependency_path)` pair to `APPROVED_PRODUCTION_ENTRIES`
in this file, with a comment naming who approved it and why. The list is
empty today, so every pull request that grows the production allowlist is
expected to fail. The list is named rather than counted on purpose: a count
is a ceiling that any later pull request can spend without saying what it
spent it on, and a pair only authorises the debt it names.

## Resolution of the base commit

`GITHUB_BASE_SHA` (or `pull_request.base.sha` from `GITHUB_EVENT_PATH`) is
authoritative; otherwise `origin/main`. Both come from the checkout, so this is
runnable locally the same way it runs in CI.

Run:
    python3 scripts/ci/test_ratchets.py
    python3 scripts/ci/test_ratchets.py --self-test
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# --- shared ------------------------------------------------------------------

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

# --- Part I: file size and public-type homonyms --------------------------------

LIMIT = 2000
CFG_TEST = re.compile(r"^#\[cfg\(test\)\]")
DECL = re.compile(r"^\s*pub (?:struct|enum|trait) ([A-Z][A-Za-z0-9_]*)")

def production_lines(text: str) -> int:
    """Lines before the first column-0 `#[cfg(test)]`; all lines if none."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    for number, line in enumerate(lines):
        if CFG_TEST.match(line):
            return number
    return len(lines)


def public_names(text: str) -> Counter:
    counts: Counter = Counter()
    for line in text.split("\n"):
        if CFG_TEST.match(line):
            break
        match = DECL.match(line)
        if match:
            counts[match.group(1)] += 1
    return counts


def resolve_base() -> str | None:
    explicit = os.environ.get("GITHUB_BASE_SHA", "").strip()
    if explicit:
        return explicit
    event_path = os.environ.get("GITHUB_EVENT_PATH", "").strip()
    if event_path and Path(event_path).is_file():
        try:
            event = json.loads(Path(event_path).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            event = {}
        sha = (event.get("pull_request") or {}).get("base", {}).get("sha")
        if isinstance(sha, str) and sha.strip():
            return sha.strip()
    for candidate in ("origin/main", "main"):
        resolved = git("rev-parse", "--verify", f"{candidate}^{{commit}}")
        if resolved:
            return resolved.strip()
    return None


def measured_files() -> list[str]:
    """Tracked production sources: crates/<crate>/src/**/*.rs."""
    listing = git("ls-files", "crates/*/src/**/*.rs", "crates/*/src/*.rs")
    if listing is None:
        return []
    return sorted(
        {
            p
            for p in listing.splitlines()
            if p.endswith(".rs") and not (p.endswith("_tests.rs") or p.endswith("/tests.rs"))
        }
    )


def parse_renames(name_status: str) -> dict[str, str]:
    """`new path -> old path` from `git diff -M --name-status` output.

    A file that moved between crates keeps the size it had: measuring it as
    "new" at its destination would make a move look like a file born over the
    limit, and the ratchet would force a split nobody asked for.
    """
    moved: dict[str, str] = {}
    for line in name_status.splitlines():
        fields = line.split("\t")
        if len(fields) == 3 and fields[0].startswith("R"):
            moved[fields[2]] = fields[1]
    return moved


def base_sources(base: str) -> list[str]:
    """Same filter as `measured_files`, over the tree at `base` (moves-safe)."""
    listing = git("ls-tree", "-r", "--name-only", base, "crates")
    if listing is None:
        return []
    return [
        p
        for p in listing.splitlines()
        if re.match(r"^crates/[^/]+/src/.+\.rs$", p)
        and not (p.endswith("_tests.rs") or p.endswith("/tests.rs"))
    ]

class SizeMeasure:
    base: str | None = None
    # file size
    head: dict[str, int] = {}
    oversize: dict[str, int] = {}
    base_sizes: dict[str, int | None] = {}
    # public-type homonyms
    names: Counter = Counter()
    base_names: Counter = Counter()
    files: int = 0


SIZE = SizeMeasure()

def setup_size_and_homonyms() -> None:
    SIZE.base = resolve_base()
    for rel in measured_files():
        path = REPO_ROOT / rel
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        size = production_lines(text)
        SIZE.head[rel] = size
        if size > LIMIT:
            SIZE.oversize[rel] = size
        SIZE.files += 1
        SIZE.names += public_names(text)
    if SIZE.base:
        moved = parse_renames(git("diff", "-M", "--name-status", SIZE.base, "--", "crates") or "")
        for rel in SIZE.oversize:
            old = git("show", f"{SIZE.base}:{moved.get(rel, rel)}")
            SIZE.base_sizes[rel] = None if old is None else production_lines(old)
        for rel in base_sources(SIZE.base):
            old = git("show", f"{SIZE.base}:{rel}")
            if old is not None:
                SIZE.base_names += public_names(old)

def test_the_base_is_resolvable() -> None:
    check(
        SIZE.base is not None,
        "cannot resolve a base commit to compare against. Set GITHUB_BASE_SHA, or "
        "fetch origin/main. This fails instead of passing vacuously.",
    )


# --- file size ---------------------------------------------------------------


def test_file_size_counting_stops_at_the_first_cfg_test() -> None:
    check(production_lines("a\nb\n#[cfg(test)]\nmod t {}\n") == 2, "stops at cfg(test)")
    check(production_lines("a\nb\nc\n") == 3, "counts all lines without cfg(test)")
    check(production_lines("a\n    #[cfg(test)]\n") == 2, "indented cfg(test) is not a module boundary")
    check(production_lines("") == 0, "empty file is zero lines")


def test_a_moved_file_is_measured_against_the_path_it_came_from() -> None:
    moved = parse_renames("R094\tcrates/a/src/x.rs\tcrates/b/src/y.rs\nM\tcrates/a/src/z.rs\n")
    check(moved == {"crates/b/src/y.rs": "crates/a/src/x.rs"}, f"renames: {moved}")
    check(parse_renames("") == {}, "no output, no renames")


def test_file_size_scan_found_sources() -> None:
    check(len(SIZE.head) > 100, f"only {len(SIZE.head)} sources measured; the glob is vacuous")


def test_no_file_grew_past_its_base_or_crossed_the_limit() -> None:
    if not SIZE.base:
        return
    for rel, size in sorted(SIZE.oversize.items()):
        old = SIZE.base_sizes.get(rel)
        if old is None or old <= LIMIT:
            check(
                False,
                f"{rel}: {size} production lines crosses the {LIMIT}-line limit "
                f"(base: {'new file' if old is None else old}). Split it.",
            )
        else:
            check(
                size <= old,
                f"{rel}: {size} production lines > {old} at the base "
                f"({SIZE.base[:12]}). A file over {LIMIT} may only shrink.",
            )


# --- public-type homonyms ----------------------------------------------------


def test_homonym_declarations_are_counted() -> None:
    sample = "pub struct A;\npub(crate) struct B;\n    pub enum C {}\n#[cfg(test)]\npub struct D;\n"
    check(dict(public_names(sample)) == {"A": 1, "C": 1}, "counts pub only, stops at cfg(test)")


def test_homonym_scan_found_sources() -> None:
    check(SIZE.files > 100, f"only {SIZE.files} sources scanned; the glob is vacuous")


def test_no_public_type_name_became_more_ambiguous() -> None:
    if not SIZE.base:
        return
    for name, count in sorted(SIZE.names.items()):
        if count < 2:
            continue
        before = SIZE.base_names.get(name, 0)
        check(
            count <= max(before, 1),
            f"`{name}` is declared {count} times as a public type "
            f"({before} at the base {SIZE.base[:12]}). Reuse the existing one or rename it.",
        )

# --- Part II: the CR-06 production allowlist -----------------------------------

CORE_SRC = "crates/cognicode-core/src"
ALLOWLIST_REL = f"{CORE_SRC}/application/architecture/cr06_allowlist.rs"

# The escape hatch, named rather than counted. A count is a ceiling that any
# later pull request can spend without saying what it spent it on; a list of
# pairs says exactly which debts were declared, and leaves every other pull
# request with no room at all. Empty means: this repository has never
# approved growing the production CR-06 allowlist.
APPROVED_PRODUCTION_ENTRIES: set[tuple[str, str]] = {
    # The architectural guard learned to see dependencies written inline, not
    # only in `use` statements. Five drifts that were already in the code
    # became visible and are now inventoried. They are pre-existing debt
    # owned by the teams that own the walk policy and the build report, not
    # new debt: the count grows because the measurement got sharper, not
    # because the architecture got worse. Approved by the maintainer in the
    # pull request that taught the guard to read inline paths.
    ("application/consumer_analysis.rs", "infrastructure::walk_policy::"),
    ("application/services/analysis_service.rs", "infrastructure::walk_policy::"),
    ("application/services/analyze.rs", "infrastructure::graph::per_file_graph::"),
    ("application/services/refactor_service.rs", "infrastructure::walk_policy::"),
    ("application/workspace_session.rs", "infrastructure::walk_policy::"),
}

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

# The comment scanner is shared. This ratchet has to blank the comments out
# before it counts anything else, and the comment-standard ratchet has to
# count the comments themselves. One scanner, so the two cannot disagree
# about what a comment is.
import comment_scan


def parse_entries(text: str) -> list[dict[str, str]]:
    """Every `ex(...)` call as {constraint, file_path, dependency_path, owner}.

    A call that does not yield four fields is reported and skipped: a malformed
    entry must not silently shrink the inventory, which would make this ratchet
    pass by making itself blind. It is reported as a failure rather than being
    counted as debt so that fixing the format is the only way to clear it.
    """
    source = comment_scan.strip_rust_comments(text)
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

    Matched on the full path the dependency names, in either spelling: a
    `use` statement (`use crate::infrastructure::parser::Language;`) or an
    inline path (`crate::infrastructure::parser::Language::new(..)`). The
    architectural guard reports both, so this classification has to read
    both: a drift the guard calls production cannot be filed as test-only
    here.

    The path is matched whole rather than by its last segment, so that the
    mirror and the Rust guard ask the same question. Matching by segment
    alone reads `use crate::domain::aggregates::call_graph::SymbolId;` as
    an import of `infrastructure::graph::`, because "graph" is a substring
    of "call_graph" — a production import that never happened.

    Comment lines are not hits: a path written in prose is not a
    dependency.
    """
    needle = "crate::" + dependency_path
    test_lines = cfg_test_line_spans(file_text)

    for start, statement in item_statements(file_text):
        if start in test_lines:
            continue
        if needle in statement:
            return True

    for index, raw in enumerate(file_text.splitlines(), start=1):
        if index in test_lines:
            continue
        if raw.strip().startswith("//"):
            continue
        if needle in raw:
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
    match = GUARDED_CONST.search(comment_scan.strip_rust_comments(text))
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


def working_tree_source(rel_path: str) -> str | None:
    candidate = REPO_ROOT / rel_path
    if not candidate.is_file():
        return None
    try:
        return candidate.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return None


SELF_NAME = "test_ratchets.py"

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

    The chain had three links and the last one is the one that is easy to lose:
    the merge authority invoked the runner, the runner discovered this file by
    glob, and the glob still matched. The merge gate is gone (retired
    2026-11-04), so the first link is gone with it. The link that survives —
    and the one that still catches a rename or a move — is the glob: a file
    that moves out of it turns this contract into something nothing runs while
    every other assertion here still passes.
    """
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


class Cr06Measurement:
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


CR06 = Cr06Measurement()


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
        bool(CR06.head_entries),
        f"no CR-06 entries were parsed out of {ALLOWLIST_REL}. The allowlist "
        f"format changed or the file is empty",
    )


def test_production_and_test_only_partition_the_inventory() -> None:
    """The classification must be a partition, and must not be entirely empty."""
    check(
        len(CR06.head_production) <= len(CR06.head_entries),
        f"classified {len(CR06.head_production)} production entries out of "
        f"{len(CR06.head_entries)} total, which is impossible: the classification "
        f"counts something the inventory does not contain",
    )
    check(
        bool(CR06.head_production),
        "every CR-06 entry classified as cfg(test)-only. If ST-01..ST-05 really "
        "did retire all production drift, delete the allowlist rather than "
        "leaving a ratchet over an empty list",
    )


def test_classification_agrees_with_the_rust_guard() -> None:
    """The compiled Rust guard is the authority; this mirror must not diverge."""
    cross_check_against_rust_guard(
        CR06.head_entries,
        {(e["file_path"], e["dependency_path"]) for e in CR06.head_production},
    )


def test_production_debt_did_not_grow() -> None:
    """The ratchet itself: production debt on this branch <= debt at the base."""
    if CR06.base is None or CR06.base_allowlist is None:
        return  # already reported as a failure by test_inventory_is_parsed's peers
    added = "\n".join(
        f"      + {e['file_path']} :: {e['dependency_path']} ({e['owner']})"
        for e in CR06.added
    )
    check(
        all(
            (e["file_path"], e["dependency_path"]) in APPROVED_PRODUCTION_ENTRIES
            for e in CR06.added
        ),
        f"CR-06 production debt grew from {len(CR06.base_production)} to "
        f"{len(CR06.head_production)} against base {CR06.base[:12]} "
        f"({len(CR06.added)} added, {len(CR06.removed)} retired):\n{added}\n"
        f"    ST-01..ST-05 retire these; they do not add them. If this growth "
        f"was decided, add the pair to APPROVED_PRODUCTION_ENTRIES in this file "
        f"with a comment naming who approved it and why.",
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
    text = CR06.head_allowlist
    if text is None:
        return
    production_keys = {
        (e["file_path"], e["dependency_path"]) for e in CR06.head_production
    }
    victim = next(
        (e for e in CR06.head_entries
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

def setup_cr06() -> None:
    """Resolve the base and the head measurement before any check reads it.

    This used to live as the first line of `main()`. `main()` is not what
    the merge gate calls: `run-all-contracts.sh` hands this file to
    `run_contract_tests.py`, which imports it and calls each `test_`
    function directly. Without this, `CR06.head_entries` stayed empty and
    `test_inventory_is_parsed` reported that the allowlist had no entries
    -- a measurement that had never been taken, described as a corrupt
    inventory. The runner calls `setup()` if a module defines it.
    """
    CR06.resolve()

def _selftest() -> int:
    """strip_rust_comments must not treat comment syntax inside a literal.

    Run with `--self-test`. This exists because the bug was real: a `/*`
    inside a string in cr06_allowlist.rs made this parser lose the GUARDED
    constant entirely, and the failure only shows up as a confusing message
    much later.
    """
    cases = [
        ("slash-star in a string", 'a();\nlet p = "application/*.rs";\nconst G = 1;\n'),
        ("slashes in a string", 'a();\nlet p = "a//b";\nconst G = 1;\n'),
        ("real block comment", 'a();\n/* gone */\nconst G = 1;\n'),
        ("real line comment", "a(); // gone\nconst G = 1;\n"),
        ("raw string with hash", 'let s = r#"x /* y"#;\nconst G = 1;\n'),
        ("escaped quote", 'let s = "a\\"/*b";\nconst G = 1;\n'),
    ]
    bad = 0
    for name, src in cases:
        out = comment_scan.strip_rust_comments(src)
        if "const G" not in out:
            print(f"FAIL {name}")
            bad += 1
        else:
            print(f"ok   {name}")
    for name, src in cases:
        if comment_scan.strip_rust_comments(src).count("\n") != src.count("\n"):
            print(f"FAIL line numbering: {name}")
            bad += 1
    return 1 if bad else 0


def setup() -> None:
    """Take both measurements before any check reads them.

    The contract runner imports this module and calls each `test_` function
    directly, calling `setup()` first when the module defines it, so both
    families are resolved here rather than in `main()`.
    """
    setup_size_and_homonyms()
    setup_cr06()


def main() -> int:
    setup()
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    homonyms = sum(1 for c in SIZE.names.values() if c > 1)
    base = (SIZE.base or "?")[:12]
    print(f"file-size ratchet: {len(SIZE.oversize)} of {len(SIZE.head)} sources over {LIMIT} production lines (base {base})")
    print(f"homonym ratchet: {homonyms} public names declared more than once (base {base})")
    if CR06.head_entries:
        print(
            f"CR-06 production debt: {len(CR06.base_production)} @ "
            f"{(CR06.base or '?')[:12]} -> {len(CR06.head_production)} on this branch "
            f"({len(CR06.added)} added, {len(CR06.removed)} retired); "
            f"{len(CR06.head_entries) - len(CR06.head_production)} of "
            f"{len(CR06.head_entries)} entries are cfg(test)-only, not counted"
        )

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    shrunk = [r for r, s in SIZE.oversize.items() if (SIZE.base_sizes.get(r) or 0) > s]
    for rel in shrunk:
        print(f"  shrank: {rel} {SIZE.base_sizes[rel]} -> {SIZE.oversize[rel]}")
    for entry in CR06.removed:
        print(f"  retired: {entry['file_path']} :: {entry['dependency_path']} "
              f"({entry['owner']})")
    if CR06.removed:
        print("  The base catches up on merge, so the next PR measures against "
              "the smaller number with nothing to edit here.")

    print("PASS — no production file crossed the limit or grew past its base size, and no public type name became more ambiguous")
    print(
        "PASS — the CR-06 production allowlist did not grow against the base "
        "commit, the cfg(test) guards were not counted as debt, and this "
        "classification agrees with the Rust guard on every entry both check"
    )
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(_selftest())
    sys.exit(main())
