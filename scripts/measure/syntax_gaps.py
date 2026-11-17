#!/usr/bin/env python3
"""Measure how often the bundled grammars report a syntax fault on real code.

An instrument, not a suite test. It lists the source files of one or more
corpora, hands them to the `measure_syntax_gaps` example of cognicode-core
(which runs the same fault detector the extraction gap uses), and prints, per
corpus and language, how many files were read and how many carry a fault.

A fault is not yet a false gap: a file can be broken, or valid code the grammar
cannot parse. Telling them apart needs a compiler or a person, so this script
only lists the faulty files (`--faults`) for that judgement; it never decides.

Corpora are named `LABEL=PATH`:
  --git LABEL=REPO   tracked files of a git repository (`git ls-files`)
  --tree LABEL=DIR   every file under a directory (vendored or registry sources)

Example:
  scripts/measure/syntax_gaps.py --git workspace=. \\
      --tree registry=$HOME/.cargo/registry/src --faults faults.tsv
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import subprocess
import sys

EXTENSIONS = (".rs", ".java", ".kt", ".kts", ".go")
LANGUAGE_FILTER = {"Rust", "Java", "Kotlin", "Go"}
LISTING_DEADLINE_SECONDS = 120
EXAMPLE_DEADLINE_SECONDS = 900


def corpus_files(kind: str, root: str) -> list[str]:
    if kind == "git":
        out = subprocess.run(
            ["git", "-C", root, "ls-files", "-z"],
            check=True,
            capture_output=True,
            timeout=LISTING_DEADLINE_SECONDS,
        ).stdout.decode()
        return [
            os.path.join(root, rel)
            for rel in out.split("\0")
            if rel.endswith(EXTENSIONS)
        ]
    found = []
    for base, _dirs, names in os.walk(root):
        found.extend(os.path.join(base, n) for n in names if n.endswith(EXTENSIONS))
    return found


def run_example(binary: str, paths: list[str]) -> list[list[str]]:
    proc = subprocess.run(
        [binary],
        input="\n".join(paths).encode(),
        capture_output=True,
        check=True,
        timeout=EXAMPLE_DEADLINE_SECONDS,
    )
    return [line.split("\t") for line in proc.stdout.decode().splitlines()]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--git", action="append", default=[], metavar="LABEL=REPO")
    parser.add_argument("--tree", action="append", default=[], metavar="LABEL=DIR")
    parser.add_argument(
        "--example",
        default=os.path.join(
            os.environ.get("CARGO_TARGET_DIR", "target"), "debug/examples/measure_syntax_gaps"
        ),
        help="built measure_syntax_gaps example (cargo build -p cognicode-core --example measure_syntax_gaps)",
    )
    parser.add_argument("--faults", help="write the faulty files to this TSV")
    parser.add_argument("--json", action="store_true", help="print the table as JSON")
    args = parser.parse_args()

    corpora = [("git", spec) for spec in args.git] + [("tree", spec) for spec in args.tree]
    if not corpora:
        parser.error("give at least one --git or --tree corpus")

    table = collections.OrderedDict()
    faults = []
    for kind, spec in corpora:
        label, _, root = spec.partition("=")
        if not root or not os.path.isdir(root):
            sys.exit(f"corpus {spec!r}: directory not found")
        records = run_example(args.example, corpus_files(kind, root))
        for language, status, path, where, detail in records:
            if language not in LANGUAGE_FILTER:
                continue
            row = table.setdefault((label, language), collections.Counter())
            row["files"] += 1
            row[status] += 1
            if status == "fault":
                faults.append((label, language, path, where, detail))

    rows = [
        {
            "corpus": label,
            "language": language,
            "files": c["files"],
            "faults": c["fault"],
            "unreadable": c["unreadable"],
            "fault_rate_pct": round(100.0 * c["fault"] / c["files"], 3),
        }
        for (label, language), c in sorted(table.items())
    ]
    if args.json:
        print(json.dumps(rows, indent=2))
    else:
        print(f"{'corpus':<14}{'language':<8}{'files':>7}{'faults':>8}{'unread':>8}{'rate%':>9}")
        for r in rows:
            print(
                f"{r['corpus']:<14}{r['language']:<8}{r['files']:>7}{r['faults']:>8}"
                f"{r['unreadable']:>8}{r['fault_rate_pct']:>9.3f}"
            )
    if args.faults:
        with open(args.faults, "w") as out:
            for row in faults:
                out.write("\t".join(row) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
