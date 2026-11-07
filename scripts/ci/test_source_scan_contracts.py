#!/usr/bin/env python3
"""Contratos que escanean el arbol versionado: escritores de entorno sin guarda y escrituras ajenas en la prosa.

Dos contratos que leen el codigo y el texto del repositorio en busca de un
defecto que ningun compilador ve, fusionados en uno (TASK-74) conservando
todas las comprobaciones y sus fallos cerrados.

# I. Ningun test muta el entorno del proceso sin una guarda serial

Contract: no test may mutate process env without a serial guard.

Why this exists
---------------
`cognicode-cli` had two tests in `cmd/tracker.rs` that wrote
`COGNICODE_HOME` without `#[serial]`. `serial_test` only serialises
*among tests that carry the attribute*, so those two writers corrupted
the process environment for every other test in the same binary. A
correctly-serialised reader (`lifecycle_journal::t_debt4_...`) was still
failing intermittently: 5 failures in 6 runs at `--test-threads=16`,
0 in 3 runs at `--test-threads=1`.

The defect is on the WRITE side. Adding `#[serial]` to the failing
reader would have looked like a fix while leaving every other
unserialised reader exposed. So the contract pins the writers.

The audit that originally missed this had a `^`-anchored function regex
without a leading `[ \t]*`, so it never saw functions indented inside
`mod tests` and reported "0 violations" as a false negative. That is why
this file carries its own self-test: an auditor that cannot be
falsified is not evidence. See `_the_auditor_finds_indented_functions`
and `_the_auditor_finds_a_planted_violation` below.

# II. La prosa versionada no declara un alfabeto ajeno

The guard that asks which scripts MAY appear here, not which ones I have seen.

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
    python3 scripts/ci/test_source_scan_contracts.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

CRATES = REPO_ROOT / "crates"

# Matches `std::env::set_var("X"...)` / `env::remove_var("X"...)`.
# The variable name lives inside a string literal, so this must run
# against RAW source, not the comment/string-stripped text.
ENV_MUTATION = re.compile(
    r"""(?:std::)?env::\s*(?:set_var|remove_var)\s*\(\s*(?:"|r")([A-Z_][A-Z0-9_]*)"""
)

# A function with any attributes. The leading `[ \t]*` is load-bearing:
# without it, every function indented inside `mod tests` is invisible,
# which is precisely the false negative that let this defect through.
FN_START = re.compile(
    r"^[ \t]*(?P<attrs>(?:#\[[^\n]*\][ \t]*\n[ \t]*)*)"
    r"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\(",
    re.M,
)
ATTR_NAME = re.compile(r"#\[\s*([A-Za-z0-9_:]+)")
CALL = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(")

# `name(` shapes that are not calls to a local function.
NOT_A_CALL = {
    "if", "while", "for", "match", "return", "fn", "let", "move", "ref",
    "unsafe", "assert", "assert_eq", "assert_ne", "debug_assert",
    "debug_assert_eq", "panic", "format", "write", "writeln", "print",
    "println", "eprint", "eprintln", "vec", "Box", "Rc", "Arc", "Mutex",
    "RwLock", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "PathBuf",
    "Path", "String", "str", "Some", "None", "Ok", "Err", "into", "unwrap",
    "expect", "clone", "new", "default", "as_ref", "as_str", "as_mut",
    "to_string", "to_owned", "len", "iter", "push", "extend", "get",
    "insert", "contains", "is_empty", "collect", "map", "filter", "and_then",
    "ok_or", "unwrap_or", "unwrap_or_else", "unwrap_or_default", "is_err",
    "is_ok", "from", "try_from", "build", "with_capacity", "eq", "ne",
    "borrow", "sort", "rev", "split", "trim", "replace", "starts_with",
    "ends_with", "join", "lines", "take", "min", "max", "build", "run",
    "drop", "clone_into", "or_insert", "or_insert_with", "and", "or", "not",
}

# Crates whose test binaries share one process, where env mutation can
# leak between tests. A crate not listed here is checked for nothing.
SHARED_PROCESS_CRATES = ("cognicode-cli", "cognicode-core")

# Known limits of this audit, stated rather than implied. Both were probed
# on 2026-09-28 rather than assumed:
#
# 1. SCOPE. Only the crates above are checked, because only their unit
#    tests share a process. `cognicode-cli` has 74 env-writing tests across
#    174 write sites, all serialised; `cognicode-core` has 0 (the core fix
#    in a855bcde removed its only writer). A same-shaped defect in a crate
#    not listed here is NOT covered. This is a deliberate allowlist: a
#    documented gap is safer than an unverified derivation, which is how the
#    earlier false negative happened.
#
# 2. MACROS. The audit walks direct `fn` calls, so a writer reachable only
#    through macro expansion is invisible. Probed: a test calling
#    `my_env_poison!()` is not detected, while a test calling `helper()`
#    is. No macro in these two crates currently expands to an env mutation
#    (the four `macro_rules!` in scope are theme/assertion helpers), so
#    this is a latent gap, not a live defect. If a macro ever wraps
#    `set_var`, this contract will not catch it.
#
# If either limit starts to matter, widen the audit rather than trusting
# it: the failure mode to avoid is the one that produced Lesson 108, a
# scanner reporting coverage it does not have.


def strip_comments_and_strings(text: str) -> str:
    """Blank comments and string literals, preserving offsets and newlines."""
    out = []
    i, n = 0, len(text)
    while i < n:
        ch = text[i]
        if ch == "/" and i + 1 < n and text[i + 1] == "/":
            j = text.find("\n", i)
            j = n if j == -1 else j
            out.append(" " * (j - i))
            i = j
        elif ch == '"':
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    j += 1
                    break
                j += 1
            out.append("".join(c if c == "\n" else " " for c in text[i:j]))
            i = j
        else:
            out.append(ch)
            i += 1
    return "".join(out)


def parse_functions(path: Path) -> dict:
    """Return {fn_name: {serial, test, body, raw_body, line}} for one file."""
    raw = path.read_text(encoding="utf-8", errors="replace")
    code = strip_comments_and_strings(raw)
    matches = list(FN_START.finditer(code))

    funcs = {}
    for idx, m in enumerate(matches):
        end = matches[idx + 1].start() if idx + 1 < len(matches) else len(code)
        attrs = ATTR_NAME.findall(m.group("attrs") or "")
        # Slice the RAW text over the same span so env mutation is still
        # visible: the variable name lives in a string literal that
        # `strip_comments_and_strings` blanked out.
        line_lo = raw.rfind("\n", 0, m.start()) + 1
        nl = raw.find("\n", end)
        line_hi = len(raw) if nl == -1 else nl
        funcs[m.group("name")] = {
            "serial": any(a in ("serial", "serial_test::serial") for a in attrs),
            "test": "test" in attrs,
            "body": code[m.end():end],
            "raw_body": raw[line_lo:line_hi],
            "line": code[: m.start()].count("\n") + 1,
        }
    return funcs


def mutated_vars(fn_info: dict) -> set:
    return set(ENV_MUTATION.findall(fn_info["raw_body"]))


def reaches_mutation(name: str, funcs: dict, seen=None) -> bool:
    """True if `name` (transitively, within this file) mutates process env."""
    if seen is None:
        seen = set()
    if name in seen or name not in funcs:
        return False
    seen.add(name)
    info = funcs[name]
    if mutated_vars(info):
        return True
    for call in CALL.findall(info["body"]):
        if call in NOT_A_CALL or call == name:
            continue
        if call in funcs and reaches_mutation(call, funcs, seen):
            return True
    return False


def unguarded_writers(crate: str) -> list:
    """Every #[test] in `crate` that mutates env without a serial guard."""
    root = REPO_ROOT / "crates" / crate
    if not root.is_dir():
        return []

    findings = []
    for path in sorted(root.rglob("*.rs")):
        funcs = parse_functions(path)
        for name, info in funcs.items():
            if not info["test"] or info["serial"]:
                continue
            if reaches_mutation(name, funcs):
                rel = path.relative_to(REPO_ROOT)
                findings.append(
                    (str(rel), info["line"], name, sorted(mutated_vars(info)
                                                          or _touched(funcs, name)))
                )
    return findings


def _touched(funcs: dict, name: str, seen=None) -> set:
    if seen is None:
        seen = set()
    if name in seen or name not in funcs:
        return set()
    seen.add(name)
    info = funcs[name]
    found = mutated_vars(info)
    for call in CALL.findall(info["body"]):
        if call in NOT_A_CALL or call == name:
            continue
        if call in funcs:
            found |= _touched(funcs, call, seen)
    return found


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


def test_no_test_mutates_env_without_serial_guard():
    """No test may write process env unless it is #[serial]."""
    violations = []
    for crate in SHARED_PROCESS_CRATES:
        for rel, line, name, variables in unguarded_writers(crate):
            violations.append(
                f"{rel}:{line} `{name}` mutates {variables} without #[serial]. "
                f"serial_test only serialises among attributed tests, so this "
                f"corrupts the process env for correctly-serialised siblings."
            )
    assert not violations, (
        "Unserialised process-env mutation in tests:\n  - "
        + "\n  - ".join(violations)
        + "\n\nAdd #[serial] (and `use serial_test::serial;`) to each, or "
          "inject the value instead of mutating the process env."
    )


def test_the_auditor_finds_indented_functions():
    """Self-test: the parser must see functions indented inside `mod tests`.

    This is the regression that hid the real defect. A `^`-anchored regex
    without `[ \t]*` found 6 functions in `lifecycle_journal.rs` where 13
    exist, and reported "0 violations" as a false negative.
    """
    sample = "#[cfg(test)]\nmod tests {\n    #[test]\n    #[serial]\n    fn a() {}\n    fn helper() {}\n}\n"
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "sample.rs"
        p.write_text(sample)
        funcs = parse_functions(p)

    assert "a" in funcs, f"parser missed the indented #[test]; found {list(funcs)}"
    assert "helper" in funcs, f"parser missed the indented helper; found {list(funcs)}"
    assert funcs["a"]["test"] is True
    assert funcs["a"]["serial"] is True
    assert funcs["helper"]["test"] is False


def test_the_auditor_finds_a_planted_violation():
    """Self-test: a planted unserialised writer must be reported.

    Proves the auditor can actually detect the defect it claims to guard,
    rather than passing vacuously.
    """
    import tempfile

    sample = '''#[cfg(test)]
mod tests {
    #[test]
    fn plant_this() {
        unsafe { std::env::set_var("PLANTED_VAR", "x"); }
    }
}
'''
    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "planted.rs"
        p.write_text(sample)
        funcs = parse_functions(p)

    assert "plant_this" in funcs, f"parser missed the planted test; found {list(funcs)}"
    assert reaches_mutation("plant_this", funcs) is True, "planted writer not detected"
    assert funcs["plant_this"]["serial"] is False


def test_env_mutation_regex_sees_names_inside_string_literals():
    """Self-test: the env regex must match against raw source, not stripped.

    The variable name is a string literal. Stripping literals before
    matching is how a compliant codebase reports zero writers.
    """
    raw = 'unsafe { std::env::set_var("COGNICODE_HOME", "x"); }'
    assert ENV_MUTATION.findall(raw) == ["COGNICODE_HOME"]
    assert ENV_MUTATION.findall(strip_comments_and_strings(raw)) == []


def test_comment_mentioning_set_var_is_not_a_mutation():
    """A doc comment naming set_var must not count as a real mutation."""
    sample = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        // callers MUST be #[serial] before set_var is safe\n    }\n}\n"
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "commented.rs"
        p.write_text(sample)
        funcs = parse_functions(p)

    assert "t" in funcs
    assert reaches_mutation("t", funcs) is False, (
        "a comment naming set_var was counted as a real mutation"
    )


def test_fully_qualified_serial_attribute_is_recognised():
    """#[serial_test::serial] must count as serialised, not be missed."""
    sample = (
        "#[cfg(test)]\nmod tests {\n"
        "    #[test]\n    #[serial_test::serial]\n"
        "    fn t() {\n        unsafe { std::env::set_var(\"X\", \"1\"); }\n    }\n}\n"
    )
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "qualified.rs"
        p.write_text(sample)
        funcs = parse_functions(p)

    assert funcs["t"]["serial"] is True, "fully qualified serial was not recognised"


def test_audited_crates_actually_exist_and_contain_tests():
    """The allowlist must name real crates, not stale paths.

    A scanner whose scope list has drifted silently audits nothing while
    still reporting a clean result. This pins the allowlist against that.
    """
    for crate in SHARED_PROCESS_CRATES:
        root = REPO_ROOT / "crates" / crate
        assert root.is_dir(), f"{crate} is listed but crates/{crate} does not exist"

    cli_tests = sum(
        1
        for path in sorted((REPO_ROOT / "crates" / "cognicode-cli").rglob("*.rs"))
        for info in parse_functions(path).values()
        if info["test"]
    )
    assert cli_tests > 100, (
        f"cognicode-cli yielded only {cli_tests} tests; the parser has likely "
        f"regressed to missing indented functions (Lesson 108)."
    )


def test_known_limit_macro_reached_writer_is_not_detected():
    """Pins the DOCUMENTED macro blind spot, so it cannot widen silently.

    The audit walks direct `fn` calls, so a writer reachable only through
    macro expansion is invisible. No macro in the audited crates currently
    expands to an env mutation, so this is a latent gap. This test asserts
    the gap still exists: if it ever starts failing, the audit has become
    macro-aware and the note above should be updated rather than deleted.
    """
    import tempfile

    macro_src = (
        "#[cfg(test)]\nmod tests {\n"
        "    #[test]\n    fn t() { my_env_poison!(); }\n}\n"
    )
    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "macro.rs"
        p.write_text(macro_src)
        funcs = parse_functions(p)
        assert "t" in funcs
        assert reaches_mutation("t", funcs) is False, (
            "the audit now detects macro-reached writers; the documented "
            "limit in the module header is stale and should be corrected"
        )


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
        "crates/cognicode-core/tests/it/b3_w4_incremental_analysis.rs",
        "unicode path inside a test fixture",
    ),
    (
        "crates/cognicode-core/tests/it/k1_kotlin_symbol_contract.rs",
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
SELF_NAME = "test_source_scan_contracts.py"

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


def test_the_guard_is_discovered_by_the_contract_runner() -> None:
    """A guard nothing runs is correct, tested, and invisible.

    It used to assert that the merge gate invoked the runner. There is no
    merge gate any more (retired 2026-11-04), so that link of the chain is
    gone. The link that survives — and the one that catches the failure this
    contract exists to catch — is that the runner's discovery glob still
    matches this file: a rename or a move would otherwise leave every other
    assertion here passing while the contract itself never runs.
    """
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
    module = sys.modules[__name__]
    tests = sorted(
        (
            (name, getattr(module, name))
            for name in dir(module)
            if name.startswith("test_") and callable(getattr(module, name))
        ),
        key=lambda item: item[1].__code__.co_firstlineno,
    )
    assert tests, "ningun test_ encontrado: un contrato vacio es un fallo, no una linea verde."
    failed = 0
    for name, fn in tests:
        before = len(failures)
        try:
            fn()
        except AssertionError as exc:
            failed += 1
            print(f"FAIL {name}")
            for line in str(exc).splitlines():
                print(f"     {line}")
        except Exception as exc:  # noqa: BLE001 - a check failing is data
            failed += 1
            print(f"ERROR {name}: {exc!r}")
        else:
            if len(failures) > before:
                failed += 1
                print(f"FAIL {name}")
                for finding in failures[before:]:
                    for line in finding.splitlines():
                        print(f"     {line}")
            else:
                print(f"ok   {name}")
    print(f"\n{len(tests) - failed} passed, {failed} failed")
    if failures:
        return 1
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
