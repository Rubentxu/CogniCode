#!/usr/bin/env python3
"""Every disabled case in the Explorer browser suite says why, and names an owner.

The gap
-------

A browser case that cannot reach its assertion is still a case: it still
costs time, it still shows up in the report as skipped, and it still tells
a reader nothing about why it is not running. This tree had sixteen of them
and not one of them said its reason in a shape a machine could read. Seven
carried a `[DEBT: ...]` marker with no owner attached, seven said something
in prose inside `test.skip(true, "...")`, one said nothing at all, and a
seventeenth deactivation point -- a conditional `test.fixme` -- was not
counted by the audit that produced the sixteen. Debt with no owner is debt
that nobody pays back.

What this contract asserts
--------------------------

  - every deactivating registration in a `*.spec.ts` carries one marker of
    the shape `[DEBT: <reason> - owner: <id>]`, with the reason and the
    owner in the same bracket, so the two cannot drift apart;
  - every owner a case cites is published in `apps/explorer-ui/e2e/DEBT.md`,
    and every published owner is cited by at least one case, so the
    registry cannot rot in either direction;
  - the count of disabled cases does not grow;
  - no deactivation hides behind an alias, and no helper that disables is
    consumed by a spec, so a case can never be disabled without a marker.

What this does not check
------------------------

  - **That the suite passes.** The thirty-five failures in this tree are
    stale `toHaveScreenshot` goldens, which are another unit's work. This
    contract reads source; it does not run a browser.
  - **That the owners do their work.** A named owner is a promise, not a
    result. The tasks live in the private Backlog.md tray.
  - **That the reasons are true.** Whether a mock really lacks an endpoint
    is a question for the case itself, not for this file.

The controls
------------

  - (control) the inventory finds the specs; an empty tree would pass
    forever;
  - (control) the marker is read from the same bracket as the owner;
  - (control) the count notices one more disabled case;
  - (control) an owner that nothing cites is reported;
  - (control) a deactivating API that is referenced but not called is
    reported;
  - (control) a helper that disables and is consumed is reported.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

E2E_TREE = REPO_ROOT / "apps" / "explorer-ui" / "e2e"
REGISTRY = E2E_TREE / "DEBT.md"
RUNNER = "scripts/ci/run-all-contracts.sh"
DISCOVERY_GLOB = "scripts/ci/test_*.py"
SELF_NAME = "test_explorer_e2e_disabled_cases_have_owners.py"

# The deactivating APIs of Playwright, longest first so that the shorter name
# never matches inside the longer one. `test.describe.skip` and
# `test.describe.fixme` disable a block; `test.skip` and `test.fixme`
# disable a case, or a block of cases when they sit inside one.
DEACTIVATING = (
    "test.describe.skip",
    "test.describe.fixme",
    "test.skip",
    "test.fixme",
)

# The measured baseline, per API. The audit of 2026-10-28 counted sixteen
# `test.skip` calls in thirteen files. This contract measured one more
# deactivating point that the audit did not see: a conditional `test.fixme`
# in `responsive-full.spec.ts`. Each API is capped at what it measured, so
# neither can grow by borrowing from the other, and neither grows here.
BASELINE = {"test.skip": 16, "test.fixme": 1}

# The marker. The reason and the owner share one bracket on purpose: a
# reason without an owner is exactly the debt this contract exists to
# close, so the two cannot be written apart. The dash is an em dash, the
# one this repository already writes in prose.
MARK = re.compile(
    r"\[DEBT:\s*(?P<reason>[^\]]+?)\s+—\s+owner:\s*(?P<owner>[A-Z][A-Z0-9]*-\d+)\]"
)

# A row of the registry: `| `TASK-9` | what it takes | 11 |`. The count is
# part of the declaration and it is checked, so the registry cannot claim
# a number the tree does not have.
REGISTRY_ROW = re.compile(
    r"^\|\s*`(?P<owner>[A-Z][A-Z0-9]*-\d+)`\s*\|(?P<what>[^|]*)\|\s*(?P<count>\d+)\s*\|\s*$"
)

# The findings are collected here and `main()` turns them into an exit
# code. A contract that reports a failure and exits with zero reports a
# green that never happened, which is worse than having no contract.
failures: list[str] = []


def check(condition: bool, message: str) -> None:
    """Record a finding when the condition does not hold."""
    if not condition:
        failures.append(message)


def spec_files() -> list[Path]:
    """The files that hold browser cases."""
    return sorted(E2E_TREE.rglob("*.spec.ts"))


def typescript_files() -> list[Path]:
    """Every typescript file under the browser suite, helpers included."""
    return sorted(E2E_TREE.rglob("*.ts"))


def masked_spans(text: str) -> list[tuple[int, int]]:
    """The spans of strings and comments, which are not code."""
    spans: list[tuple[int, int]] = []
    index = 0
    length = len(text)
    while index < length:
        character = text[index]
        if character == "/" and text.startswith("//", index):
            end = text.find("\n", index)
            end = length if end == -1 else end
            spans.append((index, end))
            index = end
            continue
        if character == "/" and text.startswith("/*", index):
            end = text.find("*/", index + 2)
            end = length if end == -1 else end + 2
            spans.append((index, end))
            index = end
            continue
        if character in "\"'`":
            quote = character
            cursor = index + 1
            while cursor < length:
                if text[cursor] == "\\":
                    cursor += 2
                    continue
                if text[cursor] == quote:
                    break
                cursor += 1
            spans.append((index, min(cursor + 1, length)))
            index = cursor + 1
            continue
        index += 1
    return spans


def mask_of(text: str) -> bytearray:
    """A per-character map of what is not code."""
    mask = bytearray(len(text))
    for start, end in masked_spans(text):
        mask[start:end] = b"\x01" * (end - start)
    return mask


def call_arguments(text: str, mask: bytearray, open_index: int) -> list[tuple[int, int]]:
    """The spans of the arguments of a call whose parenthesis is at `open_index`."""
    depth = 0
    arguments: list[tuple[int, int]] = []
    argument_start = open_index + 1
    index = open_index
    length = len(text)
    while index < length:
        if mask[index]:
            index += 1
            continue
        character = text[index]
        if character == "(":
            depth += 1
        elif character == ")":
            depth -= 1
            if depth == 0:
                arguments.append((argument_start, index))
                return arguments
        elif character == "," and depth == 1:
            arguments.append((argument_start, index))
            argument_start = index + 1
        index += 1
    arguments.append((argument_start, length))
    return arguments


def deactivation_message(text: str, arguments: list[tuple[int, int]]) -> str:
    """The text that says why the case is disabled.

    Playwright has two shapes. `test.skip("title", body)` puts the reason in
    the title; `test.skip(true, "reason")` puts it in the second argument.
    A first argument that is not a string is a condition, whatever it
    evaluates to, so the reason is the second argument.
    """
    if not arguments:
        return ""
    first = text[arguments[0][0] : arguments[0][1]].strip()
    if first.startswith(("'", '"', "`")):
        return first
    if len(arguments) >= 2:
        return text[arguments[1][0] : arguments[1][1]].strip()
    return ""


def api_occurrences(text: str, mask: bytearray) -> list[tuple[str, int, bool]]:
    """Every deactivating API in code position, with whether it is called."""
    found: list[tuple[str, int, bool]] = []
    for api in DEACTIVATING:
        for match in re.finditer(re.escape(api), text):
            start = match.start()
            if mask[start]:
                continue
            before = text[start - 1] if start else ""
            after = text[match.end()] if match.end() < len(text) else ""
            if before.isalnum() or before in "_$.":
                continue
            if after.isalnum() or after in "_$":
                continue
            cursor = match.end()
            while cursor < len(text) and text[cursor] in " \t\r\n":
                cursor += 1
            called = cursor < len(text) and text[cursor] == "("
            found.append((api, start, called))
    return found


def line_of(text: str, offset: int) -> int:
    """The line a character sits on."""
    return text.count("\n", 0, offset) + 1


def relative(path: Path) -> str:
    """A path as the reader writes it."""
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def cases_in(path: Path) -> list[dict[str, object]]:
    """The deactivating registrations of one file."""
    text = path.read_text(encoding="utf-8")
    mask = mask_of(text)
    found: list[dict[str, object]] = []
    for api, offset, called in api_occurrences(text, mask):
        if not called:
            continue
        open_index = text.index("(", offset)
        arguments = call_arguments(text, mask, open_index)
        found.append(
            {
                "api": api,
                "offset": offset,
                "line": line_of(text, offset),
                "message": deactivation_message(text, arguments),
            }
        )
    return found


def exported_functions(path: Path) -> dict[str, tuple[int, int]]:
    """The exported functions of a file, with the span of their body."""
    text = path.read_text(encoding="utf-8")
    mask = mask_of(text)
    found: dict[str, tuple[int, int]] = {}
    for match in re.finditer(r"\bexport\s+(?:async\s+)?function\s+(\w+)", text):
        if mask[match.start()]:
            continue
        cursor = text.find("(", match.end())
        if cursor == -1:
            continue
        arguments = call_arguments(text, mask, cursor)
        if not arguments:
            continue
        body_start = arguments[-1][1]
        while body_start < len(text) and text[body_start] not in "{":
            body_start += 1
        if body_start >= len(text):
            continue
        depth = 0
        index = body_start
        while index < len(text):
            if mask[index]:
                index += 1
                continue
            if text[index] == "{":
                depth += 1
            elif text[index] == "}":
                depth -= 1
                if depth == 0:
                    break
            index += 1
        found[match.group(1)] = (match.start(), index)
    return found


def registry_rows() -> list[dict[str, object]]:
    """The owner table of the registry."""
    if not REGISTRY.is_file():
        return []
    rows: list[dict[str, object]] = []
    for line in REGISTRY.read_text(encoding="utf-8").splitlines():
        match = REGISTRY_ROW.match(line.strip())
        if match:
            rows.append(
                {
                    "owner": match.group("owner"),
                    "what": match.group("what").strip(),
                    "count": int(match.group("count")),
                }
            )
    return rows


def test_the_explorer_suite_has_specs() -> None:
    """An inventory that found nothing would pass forever."""
    specs = spec_files()
    check(
        bool(specs),
        "no *.spec.ts under apps/explorer-ui/e2e/. An inventory with no "
        "cases in it cannot report a case that does not say why it is "
        "disabled, and this contract would stay green forever",
    )


def test_every_disabled_case_says_its_reason_and_its_owner() -> None:
    """A case that does not say why is a case nobody can fix."""
    for path in spec_files():
        for case in cases_in(path):
            message = str(case["message"])
            match = MARK.search(message)
            check(
                match is not None,
                f"{relative(path)}:{case['line']} disables a case without a "
                f"marker of the shape [DEBT: <reason> — owner: <id>]. It "
                f"says: {message[:160]!r}",
            )


def test_every_owner_is_published_and_published_owners_are_used() -> None:
    """The registry is the published set of owners, in both directions."""
    check(
        REGISTRY.is_file(),
        f"{relative(REGISTRY)} does not exist. The owners a disabled case "
        f"names have to be published somewhere the tree can be read; a "
        f"marker that names a task nobody can find is not an owner",
    )
    if not REGISTRY.is_file():
        return

    rows = registry_rows()
    check(
        bool(rows),
        f"{relative(REGISTRY)} has no owner table. It is expected to carry "
        f"rows of the shape | `TASK-9` | what it takes | 11 |",
    )
    published = {str(row["owner"]): row for row in rows}

    cited: dict[str, int] = {}
    for path in spec_files():
        for case in cases_in(path):
            match = MARK.search(str(case["message"]))
            if match is None:
                continue
            owner = match.group("owner")
            cited[owner] = cited.get(owner, 0) + 1
            check(
                owner in published,
                f"{relative(path)}:{case['line']} names owner {owner}, which "
                f"{relative(REGISTRY)} does not publish",
            )

    for owner, row in sorted(published.items()):
        check(
            owner in cited,
            f"{relative(REGISTRY)} publishes {owner}, but no disabled case "
            f"names it. An owner with no case behind it is a claim the tree "
            f"does not support",
        )
        check(
            int(row["count"]) == cited.get(owner, 0),
            f"{relative(REGISTRY)} says {owner} takes {row['count']} case(s), "
            f"and {cited.get(owner, 0)} disabled case(s) name it",
        )


def test_no_deactivation_hides_behind_a_reference() -> None:
    """A deactivation nobody calls cannot be counted, so it cannot be owned."""
    for path in typescript_files():
        text = path.read_text(encoding="utf-8")
        mask = mask_of(text)
        for api, offset, called in api_occurrences(text, mask):
            if called:
                continue
            check(
                False,
                f"{relative(path)}:{line_of(text, offset)} references {api} "
                f"without calling it. A deactivation that hides behind an "
                f"alias is a case that stops running without a marker, and "
                f"this contract cannot see it",
            )


def test_no_helper_that_disables_is_consumed() -> None:
    """A helper that disables is a case that stops running without a marker."""
    specs = spec_files()
    for path in typescript_files():
        if path.name.endswith(".spec.ts"):
            continue
        sites = cases_in(path)
        for name, (start, end) in exported_functions(path).items():
            disables = [
                site
                for site in sites
                if start <= int(site["offset"]) <= end
            ]
            if not disables:
                continue
            for spec in specs:
                spec_text = spec.read_text(encoding="utf-8")
                spec_mask = mask_of(spec_text)
                for match in re.finditer(rf"\b{re.escape(name)}\s*\(", spec_text):
                    if spec_mask[match.start()]:
                        continue
                    check(
                        False,
                        f"{relative(spec)}:{line_of(spec_text, match.start())} "
                        f"calls {name}(), which disables cases in "
                        f"{relative(path)}. A case disabled through a helper "
                        f"carries no marker of its own, so the helper's "
                        f"deactivation has to be owned where it is written",
                    )


def test_the_number_of_disabled_cases_does_not_grow() -> None:
    """The count is the ratchet: a new disabled case needs a decision."""
    tally: dict[str, int] = {}
    for path in spec_files():
        for case in cases_in(path):
            api = str(case["api"])
            tally[api] = tally.get(api, 0) + 1
    for api, ceiling in sorted(BASELINE.items()):
        found = tally.get(api, 0)
        check(
            found <= ceiling,
            f"{found} disabled case(s) use {api} and the baseline is {ceiling}. "
            f"Disabling a case needs a decision, and this number cannot grow "
            f"because of one",
        )


def test_the_guard_is_run_by_the_merge_authority() -> None:
    """A guard nothing runs is correct, tested, and invisible."""
    sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
    try:
        import pipeline_authority as authority
    except ImportError:
        check(False, "scripts/ci/pipeline_authority.py could not be imported")
        return

    hosts = authority.invoked_by(RUNNER)
    check(
        authority.MERGE_AUTHORITY in hosts,
        f"{RUNNER} is not invoked by {authority.MERGE_AUTHORITY} "
        f"(found in: {sorted(hosts) or 'no pipeline at all'}). A guard that "
        f"nothing runs is the CP5 failure again: correct, tested, and invisible",
    )

    matched = sorted(REPO_ROOT.glob(DISCOVERY_GLOB))
    check(
        SELF_NAME in [path.name for path in matched],
        f"{SELF_NAME} no longer matches {DISCOVERY_GLOB}, so the runner never "
        f"discovers it. Renaming or moving this file without touching the glob "
        f"silently stops it from running",
    )

    runner_text = (REPO_ROOT / RUNNER).read_text(encoding="utf-8")
    check(
        "test_*.py" in runner_text,
        f"{RUNNER} no longer discovers contracts by glob. It runs whatever it "
        f"is handed, which puts the contract list back into a second place",
    )


def main() -> int:
    for name, function in sorted(globals().items()):
        if name.startswith("test_") and callable(function):
            try:
                function()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    for finding in failures:
        print(f"  FAIL {finding}")
    if failures:
        print(f"explorer e2e disabled cases have owners: {len(failures)} finding(s)")
        return 1
    print("explorer e2e disabled cases have owners: none")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
