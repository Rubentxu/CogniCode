#!/usr/bin/env python3
"""Contract: no test may mutate process env without a serial guard.

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

Run:
    python3 scripts/ci/test_serial_env_contract.py
"""

from __future__ import annotations

import re
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


def main() -> int:
    tests = [
        (name, obj)
        for name, obj in sorted(globals().items())
        if name.startswith("test_") and callable(obj)
    ]
    failed = 0
    for name, fn in tests:
        try:
            fn()
        except AssertionError as exc:
            failed += 1
            print(f"FAIL {name}")
            for line in str(exc).splitlines():
                print(f"     {line}")
        except Exception as exc:  # noqa: BLE001
            failed += 1
            print(f"ERROR {name}: {exc!r}")
        else:
            print(f"ok   {name}")
    print(f"\n{len(tests) - failed} passed, {failed} failed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
