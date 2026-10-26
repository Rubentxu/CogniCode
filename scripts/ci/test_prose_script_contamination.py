#!/usr/bin/env python3
"""The guard that asks which scripts MAY appear here, not which ones I have seen.

Why this exists
---------------
The writing system of this repository is Latin, and Greek appears in it as
mathematics. Every other script has to be declared, and every declaration has
to be justified. That is the whole rule:

    a character from a script other than Latin or Greek may only appear in a
    file that is exempted, and the exemption says why.

The alternative rule — "the scripts I have seen before" — is what a grep for
one range gives you, and it fails in the direction that matters: the next
contamination arrives in a script the grep never looked at. The lesson this
repository already wrote down says exactly that, and it is the reason the
range list below is long and explicit rather than short and familiar.

What it found
-------------
The first run of this guard over the tracked tree found contamination in
places no inventory had recorded: a Spanish word with three Arabic letters in
it, an English comment with two CJK characters in the middle of it, a YAML
plan whose word "distinguish" had been replaced by a CJK word, and a spec
that glossed a rule name in Chinese. Every one of them was a single word
replaced by a word from another alphabet — the same shape, always.

What it does not do
-------------------
It does not repair anything. A file that carries contamination it cannot be
repaired in —an archived record, a certification receipt— is declared in
`EXEMPTIONS` with the reason it stays, and its findings are reported and never
rewritten. Reconstructing historical evidence so that the guard passes would
be falsifying the evidence, which is worse than the defect it hides.

Run:
    python3 scripts/ci/test_prose_script_contamination.py
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# ---------------------------------------------------------------------------
# The declaration.
# ---------------------------------------------------------------------------

# Latin and Greek are the writing systems this repository uses. Latin is the
# language; Greek is the notation of its graph mathematics. Everything below
# is a script that has to be declared before it may appear, and none of them
# has a legitimate reason to appear in prose unless a file says so.
#
# The list is explicit and long on purpose. A short list of the scripts that
# were seen once is a memory, not a guard.
FORBIDDEN_SCRIPT_RANGES: tuple[tuple[str, int, int], ...] = (
    ("Cyrillic", 0x0400, 0x052F),
    ("Armenian", 0x0530, 0x058F),
    ("Hebrew", 0x0590, 0x05FF),
    ("Arabic", 0x0600, 0x06FF),
    ("Syriac", 0x0700, 0x074F),
    ("Thaana", 0x0780, 0x07BF),
    ("NKo", 0x07C0, 0x07FF),
    ("Samaritan", 0x0800, 0x083F),
    ("Mandaic", 0x0840, 0x085F),
    ("Devanagari", 0x0900, 0x097F),
    ("Bengali", 0x0980, 0x09FF),
    ("Gurmukhi", 0x0A00, 0x0A7F),
    ("Gujarati", 0x0A80, 0x0AFF),
    ("Oriya", 0x0B00, 0x0B7F),
    ("Tamil", 0x0B80, 0x0BFF),
    ("Telugu", 0x0C00, 0x0C7F),
    ("Kannada", 0x0C80, 0x0CFF),
    ("Malayalam", 0x0D00, 0x0D7F),
    ("Sinhala", 0x0D80, 0x0DFF),
    ("Thai", 0x0E00, 0x0E7F),
    ("Lao", 0x0E80, 0x0EFF),
    ("Tibetan", 0x0F00, 0x0FFF),
    ("Myanmar", 0x1000, 0x109F),
    ("Georgian", 0x10A0, 0x10FF),
    ("Hangul Jamo", 0x1100, 0x11FF),
    ("Ethiopic", 0x1200, 0x137F),
    ("Cherokee", 0x13A0, 0x13FF),
    ("Canadian Aboriginal", 0x1400, 0x167F),
    ("Ogham", 0x1680, 0x169F),
    ("Runic", 0x16A0, 0x16FF),
    ("Khmer", 0x1780, 0x17FF),
    ("Mongolian", 0x1800, 0x18AF),
    ("Limbu", 0x1900, 0x194F),
    ("Tai Le", 0x1950, 0x197F),
    ("Balinese", 0x1B00, 0x1B7F),
    ("Sundanese", 0x1B80, 0x1BBF),
    ("Batak", 0x1BC0, 0x1BFF),
    ("Lepcha", 0x1C00, 0x1C4F),
    ("Ol Chiki", 0x1C50, 0x1C7F),
    ("Phags-pa", 0xA840, 0xA87F),
    ("CJK radicals", 0x2E80, 0x2EFF),
    ("CJK punctuation", 0x3000, 0x303F),
    ("Hiragana", 0x3040, 0x309F),
    ("Katakana", 0x30A0, 0x30FF),
    ("Bopomofo", 0x3100, 0x312F),
    ("CJK unified extension", 0x3400, 0x4DBF),
    ("CJK unified", 0x4E00, 0x9FFF),
    ("Hangul syllables", 0xAC00, 0xD7AF),
    ("Hangul jamo extended", 0xA960, 0xA97F),
    ("CJK compatibility", 0xF900, 0xFAFF),
    ("Alphabetic presentation", 0xFB00, 0xFB4F),
    ("Arabic presentation A", 0xFB50, 0xFDFF),
    ("Arabic presentation B", 0xFE70, 0xFEFF),
)

# The suffixes of the files that carry writing. A binary file has no writing
# to contaminate, and reading one as text would produce noise.
TEXT_SUFFIXES = frozenset(
    {
        ".md", ".rs", ".py", ".kts", ".toml", ".sh", ".yaml", ".yml",
        ".json", ".txt", ".kt", ".js", ".ts", ".tsx", ".jsx", ".html", ".css",
        ".cfg", ".ini", ".gradle", ".sql", ".xml",
    }
)

# The files that are allowed to contain what they contain, and why. Each
# entry is a path prefix and a reason. The reason is not decoration: an
# exemption without a reason is a hole, and a hole that nobody can explain
# is the thing this guard exists to stop.
EXEMPTIONS: tuple[tuple[str, str], ...] = (
    (
        "crates/cognicode-core/src/application/portable_execution/paths.rs",
        "test fixture with a non-ASCII path",
    ),
    (
        "crates/cognicode-core/src/interface/mcp/security.rs",
        "test that a non-Latin filename is rejected",
    ),
    (
        "crates/cognicode-sandbox/src/main.rs",
        "localized error messages",
    ),
    (
        "crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs",
        "unicode identifier inside a parser test",
    ),
    (
        "crates/cognicode-core/src/infrastructure/refactor/inline_strategy.rs",
        "localized error message",
    ),
    (
        "crates/cognicode-core/tests/b3_w4_incremental_analysis.rs",
        "unicode path inside a test fixture",
    ),
    (
        "crates/cognicode-core/tests/k1_kotlin_symbol_contract.rs",
        "unicode identifier inside a test that proves the published code "
        "does not shift its column",
    ),
    (
        "sandbox/fixtures/kotlin/Unicode.kt",
        "unicode fixture whose characters are the subject of the test",
    ),
    (
        "apps/explorer-ui/src/components/MermaidRenderer.test.tsx",
        "mermaid diagram whose node labels are not Latin",
    ),
)

# The frozen expedition is expected to hold exactly this much contamination.
# It is frozen, so the number cannot change; if it does, somebody edited
# frozen evidence and the guard has to say so rather than shrug.

RUNNER = "scripts/ci/run-all-contracts.sh"
DISCOVERY_GLOB = "scripts/ci/test_*.py"
SELF_NAME = "test_prose_script_contamination.py"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    """Record a finding. The runner reads the list; nothing raises."""
    if not condition:
        failures.append(message)


def script_of(char: str) -> str | None:
    """The forbidden script a character belongs to, or None if it is allowed."""
    code = ord(char)
    for name, low, high in FORBIDDEN_SCRIPT_RANGES:
        if low <= code <= high:
            return name
    return None


def findings_in(text: str) -> list[tuple[int, str, int]]:
    """Every forbidden character in the text, as (line, script, codepoint)."""
    found: list[tuple[int, str, int]] = []
    for number, line in enumerate(text.splitlines(), start=1):
        for char in line:
            script = script_of(char)
            if script is not None:
                found.append((number, script, ord(char)))
    return found


def tracked_text_files() -> list[str]:
    """The tracked files that carry writing, relative to the repository root."""
    try:
        completed = subprocess.run(
            ["git", "ls-files", "-z"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except (OSError, subprocess.SubprocessError):
        return []
    if completed.returncode != 0:
        return []
    names = completed.stdout.split("\0")
    return sorted(
        name
        for name in names
        if name and Path(name).suffix.lower() in TEXT_SUFFIXES
    )


def summarize(found: list[tuple[int, str, int]]) -> str:
    """The findings of one file, one entry per source line.

    The count comes first because a truncated report reads like a small
    problem: the reader needs the scale before the details.
    """
    by_line: dict[int, list[str]] = {}
    for line, _script, code in found:
        by_line.setdefault(line, []).append("U+%04X" % code)
    entries = [
        "line %d: %s" % (line, ", ".join(codes))
        for line, codes in sorted(by_line.items())
    ]
    head = "; ".join(entries[:6])
    if len(entries) > 6:
        head += "; and %d more line(s)" % (len(entries) - 6)
    return "%d character(s) on %d line(s): %s" % (
        len(found),
        len(by_line),
        head,
    )


def is_exempt(relative_path: str) -> bool:
    """Whether the file may contain what it contains, and why not."""
    return any(
        relative_path == prefix or relative_path.startswith(prefix)
        for prefix, _ in EXEMPTIONS
    )


def read(relative_path: str) -> str | None:
    try:
        return (REPO_ROOT / relative_path).read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return None


# ---------------------------------------------------------------------------
# The checks.
# ---------------------------------------------------------------------------


def test_versioned_text_declares_no_foreign_script() -> None:
    """No tracked file may contain a forbidden script without an exemption."""
    offenders: list[str] = []
    frozen: list[str] = []
    for relative_path in tracked_text_files():
        text = read(relative_path)
        if text is None:
            continue
        found = findings_in(text)
        if not found:
            continue
        if is_exempt(relative_path):
            frozen.extend(found)
            continue
        summary = summarize(found)
        offenders.append(f"{relative_path}: {summary}")

    check(
        not offenders,
        "a versioned file contains a script that its prose has no business "
        "using. Declare it in EXEMPTIONS with a reason, or repair the text:\n  "
        + "\n  ".join(offenders),
    )


def test_every_exemption_still_exempts_something() -> None:
    """An exemption that exempts nothing is a hole with no reason attached."""
    stale: list[str] = []
    for prefix, reason in EXEMPTIONS:
        text = read(prefix)
        if text is None:
            stale.append(f"{prefix}: the file does not exist")
            continue
        if not findings_in(text):
            stale.append(
                f"{prefix}: no forbidden character left, so the exemption "
                f"({reason}) no longer exempts anything. Remove it"
            )
    check(
        not stale,
        "an exemption outlived what it exempted:\n  " + "\n  ".join(stale),
    )


def test_the_scanner_sees_a_contamination_and_spares_greek() -> None:
    """The rule has to detect what it claims and leave what it allows alone."""
    cyrillic = findings_in("una palabra " + chr(0x0431) + " dentro de otra")
    check(
        len(cyrillic) == 1 and cyrillic[0][1] == "Cyrillic",
        f"the scanner does not see a Cyrillic character inside a Spanish "
        f"sentence, which is the exact shape of every defect this guard "
        f"exists to catch. It saw {cyrillic!r}",
    )

    greek = findings_in("la communities modularity of the graph, Q = A")
    check(
        not greek,
        f"the scanner flags Greek, which is the notation of the graph "
        f"mathematics this repository writes. It saw {greek!r}",
    )

    latin = findings_in("una palabra española con acentos y eñes")
    check(
        not latin,
        f"the scanner flags plain Latin prose. It saw {latin!r}",
    )


def test_the_guard_is_run_by_the_merge_authority() -> None:
    """A guard nothing runs is correct, tested, and invisible."""
    sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
    try:
        import pipeline_authority as authority
    except ImportError:
        check(False, f"scripts/ci/pipeline_authority.py could not be imported")
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
        SELF_NAME in [p.name for p in matched],
        f"{SELF_NAME} no longer matches {DISCOVERY_GLOB}, so the runner never "
        f"discovers it. Renaming or moving this file without touching the glob "
        f"silently stops it from running",
    )

    runner_text = read(RUNNER) or ""
    check(
        "test_*.py" in runner_text,
        f"{RUNNER} no longer discovers contracts by glob. It runs whatever it "
        f"is handed, which puts the contract list back into a second place",
    )


def main() -> int:
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    for finding in failures:
        print(f"  FAIL {finding}")
    if failures:
        print(f"prose script contamination: {len(failures)} finding(s)")
        return 1
    print("prose script contamination: none")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
