#!/usr/bin/env python3
"""The pack of a change against `git diff` plus reading files, on one fixture.

The question: to review or edit exactly the lines of a change, what does
`cognicode pack --change-scope working_tree` cost and what does it return,
next to what an agent does today with Git and a file reader?

The fixture is a small Rust project built here, in a throwaway Git repository,
with the truth of each case written by hand from how the fixture was built and
before any tool ran: which symbols a change touches, which consumers a changed
declaration has, which tests exercise it, and which symbols a reader would be
wrong to be handed. The truth is not read from any tool's answer.

The alternatives, each charitable on purpose (a careful agent that already
knows which names to look for; the names are hand-written per case):

  diff        `git diff`. One call. Shows the lines, finds no consumer or test.
  grep_lines  the diff, then one `git grep -n -w <name>` per name. Locations
              only: a line is not attributed to a function, a consumer or a
              test, and a comment that mentions the name is a hit.
  grep_files  the diff, one `git grep -l -w` per name, then the whole of every
              file that matched or changed is read: what an agent that wants to
              see the code actually does.

What is measured, per case, over several runs:

  relevance   `recall`: the share of the required symbols the tool delivers.
              `precision`: the share of what it delivers that is required or
              neutral (not unwanted). `unwanted`: how many symbols the truth
              says a reader should not be handed were delivered. Symbol-level
              for `pack`; file-level for the alternatives, which do not name
              symbols: a file counts as found when it declares a required
              symbol, which is the most charitable reading of reading it.
  bytes       the UTF-8 bytes an agent reads. For `pack`, the text of the
              answer, and apart the bytes of the declaration ranges it names
              (what an agent that goes on to read every selected item reads).
  calls       the calls made to get there. The reads of the ranges of a pack
              are reported as `optional_reads`, not folded into `calls`.
  latency     wall time of the command(s), p50 and the p10-p90 noise, with the
              pack timed cold (a fresh cache each run) and warm (one cache).
              Each run of the CLI builds the graph, so a pack costs more time
              than a grep; the time of an agent's own reasoning between calls
              is not measured and is the cost the alternatives add.
  honesty     the `status` of the pack and the kinds of its gaps: a case whose
              truth has a relation the graph cannot see must not come back
              complete.

Disciplines the numbers obey: one shared host (load average and cores are
recorded before and after); percentiles are nearest-rank and the report says
how many samples it has; a failed call is counted apart and never folded into a
number; nothing is estimated; the binary is whatever `--cognicode` points at,
and a debug build is far slower than a release one, so times compare inside one
binary only.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# ---- the fixture -----------------------------------------------------------

FILES: dict[str, str] = {
    "README.md": (
        "# fixture\n\nReads a configuration with `parse_config` and serves it.\n"
    ),
    "docs/usage.md": (
        "# usage\n\nCall `parse_config` with the text of the file.\n"
    ),
    "src/config.rs": (
        "pub struct Config {\n"
        "    pub entries: Vec<(String, String)>,\n"
        "}\n"
        "\n"
        "fn trim_line(line: &str) -> &str {\n"
        "    line.trim()\n"
        "}\n"
        "\n"
        "pub fn parse_config(text: &str) -> Config {\n"
        "    let mut entries = Vec::new();\n"
        "    for line in text.lines() {\n"
        "        let line = trim_line(line);\n"
        "        if let Some((key, value)) = line.split_once('=') {\n"
        "            entries.push((key.to_string(), value.to_string()));\n"
        "        }\n"
        "    }\n"
        "    Config { entries }\n"
        "}\n"
    ),
    "src/server.rs": (
        "use crate::config::{Config, parse_config};\n"
        "\n"
        "pub fn start(text: &str) -> usize {\n"
        "    let config = parse_config(text);\n"
        "    config.entries.len()\n"
        "}\n"
        "\n"
        "pub fn status() -> &'static str {\n"
        "    \"ready\"\n"
        "}\n"
    ),
    "src/cli.rs": (
        "use crate::config::{Config, parse_config};\n"
        "\n"
        "pub fn read_args(args: &str) -> Config {\n"
        "    parse_config(args)\n"
        "}\n"
    ),
    "src/reload.rs": (
        "use crate::config::{Config, parse_config};\n"
        "\n"
        "pub fn reload(text: &str) -> Config {\n"
        "    parse_config(text)\n"
        "}\n"
    ),
    "src/render.rs": (
        "use crate::config::Config;\n"
        "\n"
        "pub trait Renderer {\n"
        "    fn render(&self, config: &Config) -> String;\n"
        "}\n"
        "\n"
        "pub struct Html;\n"
        "\n"
        "impl Renderer for Html {\n"
        "    fn render(&self, config: &Config) -> String {\n"
        "        format!(\"<p>{}</p>\", config.entries.len())\n"
        "    }\n"
        "}\n"
        "\n"
        "pub struct Plain;\n"
        "\n"
        "impl Renderer for Plain {\n"
        "    fn render(&self, config: &Config) -> String {\n"
        "        config.entries.len().to_string()\n"
        "    }\n"
        "}\n"
    ),
    "src/report.rs": (
        "use crate::config::Config;\n"
        "use crate::render::{Html, Renderer};\n"
        "\n"
        "pub fn report(config: &Config) -> String {\n"
        "    let page = Html;\n"
        "    page.render(config)\n"
        "}\n"
    ),
    "src/metrics.rs": (
        "pub struct Counter {\n"
        "    pub total: u64,\n"
        "}\n"
        "\n"
        "pub fn record(counter: &mut Counter, amount: u64) {\n"
        "    counter.total += amount;\n"
        "}\n"
        "\n"
        "pub fn mean(values: &[u64]) -> u64 {\n"
        "    if values.is_empty() {\n"
        "        return 0;\n"
        "    }\n"
        "    values.iter().sum::<u64>() / values.len() as u64\n"
        "}\n"
    ),
    "src/logging.rs": (
        "// Formats the lines the server prints; what `parse_config` reads is not\n"
        "// logged here.\n"
        "pub fn line(level: &str, message: &str) -> String {\n"
        "    format!(\"[{level}] {message}\")\n"
        "}\n"
        "\n"
        "pub fn banner() -> String {\n"
        "    line(\"info\", \"starting\")\n"
        "}\n"
    ),
    "src/retry.rs": (
        "pub fn backoff(attempt: u32) -> u64 {\n"
        "    100 * 2u64.pow(attempt.min(6))\n"
        "}\n"
        "\n"
        "pub fn should_retry(attempt: u32, limit: u32) -> bool {\n"
        "    attempt < limit\n"
        "}\n"
    ),
    "tests/config_tests.rs": (
        "use fixture::config::parse_config;\n"
        "\n"
        "#[test]\n"
        "fn parses_pairs() {\n"
        "    let config = parse_config(\"a=1\");\n"
        "    assert_eq!(config.entries.len(), 1);\n"
        "}\n"
        "\n"
        "#[test]\n"
        "fn ignores_blank_lines() {\n"
        "    let config = parse_config(\"\\n\");\n"
        "    assert!(config.entries.is_empty());\n"
        "}\n"
    ),
    "tests/server_tests.rs": (
        "use fixture::server::start;\n"
        "\n"
        "#[test]\n"
        "fn start_counts_entries() {\n"
        "    let count = start(\"a=1\");\n"
        "    assert_eq!(count, 1);\n"
        "}\n"
    ),
    "tests/reload_tests.rs": (
        "use fixture::reload::reload;\n"
        "\n"
        "#[test]\n"
        "fn reload_reads_the_text() {\n"
        "    assert_eq!(reload(\"a=1\").entries.len(), 1);\n"
        "}\n"
    ),
    "tests/render_tests.rs": (
        "use fixture::config::parse_config;\n"
        "use fixture::render::{Html, Renderer};\n"
        "\n"
        "#[test]\n"
        "fn renders_html() {\n"
        "    let config = parse_config(\"a=1\");\n"
        "    let page = Html;\n"
        "    let text = page.render(&config);\n"
        "    assert!(text.contains(\"<p>\"));\n"
        "}\n"
    ),
    "tests/metrics_tests.rs": (
        "use fixture::metrics::{Counter, record};\n"
        "\n"
        "#[test]\n"
        "fn counts_what_it_records() {\n"
        "    let mut counter = Counter { total: 0 };\n"
        "    record(&mut counter, 2);\n"
        "    assert_eq!(counter.total, 2);\n"
        "}\n"
    ),
}

# Each case edits the working tree. `edits` are (file, old, new) replacements.
# `required` are the symbols a reader of the change must be handed; `unwanted`
# the ones a narrower change must not hand over; every other symbol is
# neutral. `names` are the names the charitable alternatives search for.
CASES = [
    {
        "id": "api-signature",
        "what": "a public function gains a parameter",
        "edits": [("src/config.rs", "pub fn parse_config(text: &str) -> Config {",
                   "pub fn parse_config(text: &str, strict: bool) -> Config {")],
        "required": {
            "parse_config", "start", "read_args", "reload",
            "parses_pairs", "ignores_blank_lines", "start_counts_entries",
            "reload_reads_the_text", "renders_html",
        },
        "unwanted": {"mean", "record", "backoff", "line"},
        "names": ["parse_config"],
    },
    {
        "id": "private-body",
        "what": "the body of a private helper changes",
        "edits": [("src/config.rs", "    line.trim()\n", "    line.trim_matches(' ')\n")],
        "required": {
            "trim_line", "parses_pairs", "ignores_blank_lines",
            "start_counts_entries", "reload_reads_the_text", "renders_html",
        },
        "unwanted": {"read_args", "mean", "record", "backoff", "line"},
        "names": ["trim_line", "parse_config", "start", "reload"],
    },
    {
        "id": "contract-implementor",
        "what": "a type that implements a trait gains a field",
        "edits": [("src/render.rs", "pub struct Html;\n", "pub struct Html {\n    pub escape: bool,\n}\n")],
        "required": {"Html", "Renderer", "report", "renders_html"},
        "unwanted": {"mean", "record", "backoff", "line", "start"},
        "names": ["Html", "Renderer"],
    },
    {
        "id": "unrelated-body",
        "what": "the body of a leaf function changes",
        "edits": [("src/metrics.rs", "    counter.total += amount;\n", "    counter.total = counter.total.saturating_add(amount);\n")],
        "required": {"record", "counts_what_it_records"},
        "unwanted": {"parse_config", "start", "read_args", "reload", "report"},
        "names": ["record"],
    },
    {
        "id": "docs-only",
        "what": "only documentation changes",
        "edits": [("README.md", "and serves it.", "and serves it, fast.")],
        "required": set(),
        "unwanted": {"parse_config", "start", "record", "mean"},
        "names": [],
    },
]

# ---- pure core -------------------------------------------------------------


def percentile(samples: list[float], q: float) -> float | None:
    """Nearest-rank percentile; `None` when there is nothing to rank."""
    if not samples:
        return None
    ordered = sorted(samples)
    rank = max(1, math.ceil(q / 100 * len(ordered)))
    return ordered[min(rank, len(ordered)) - 1]


def spread(samples: list[float]) -> dict:
    """The median and how noisy it is: the p10-p90 band relative to the p50."""
    p50 = percentile(samples, 50)
    if p50 is None:
        return {"n": 0}
    p10, p90 = percentile(samples, 10), percentile(samples, 90)
    return {
        "n": len(samples),
        "p50": round(p50, 2),
        "p10": round(p10, 2),
        "p90": round(p90, 2),
        "noise": round((p90 - p10) / p50, 3) if p50 else None,
    }


def relevance(delivered: set[str], required: set[str], unwanted: set[str]) -> dict:
    """Recall over what is required, precision over what is delivered.

    With nothing required recall is `None` (there is nothing to recall), and
    with nothing delivered precision is `None`: a tool that delivers nothing is
    not precise, it is silent.
    """
    hit = delivered & required
    bad = delivered & unwanted
    return {
        "delivered": len(delivered),
        "recall": round(len(hit) / len(required), 3) if required else None,
        "precision": round((len(delivered) - len(bad)) / len(delivered), 3) if delivered else None,
        "unwanted": len(bad),
        "missed": sorted(required - delivered),
    }


def declaring_files(names: set[str], declarations: dict[str, set[str]]) -> set[str]:
    """The files that declare at least one of the symbols."""
    return {path for path, declared in declarations.items() if declared & names}


def file_level(read: set[str], required: set[str], unwanted: set[str],
               declarations: dict[str, set[str]]) -> dict:
    """The relevance of a tool that hands over files, not symbols.

    A file is found when it declares a required symbol, which is the most
    charitable reading of having read it; it is unwanted when it declares only
    unwanted symbols.
    """
    needed = declaring_files(required, declarations)
    noise = {p for p in read if declarations.get(p, set()) & unwanted
             and not declarations.get(p, set()) & required}
    return {
        "files_read": len(read),
        "recall": round(len(read & needed) / len(needed), 3) if needed else None,
        "precision": round((len(read) - len(noise)) / len(read), 3) if read else None,
        "unwanted": len(noise),
        "missed": sorted(needed - read),
    }


def lines_of(text: str, start: int, end: int) -> str:
    """Lines `start..=end`, counted from one."""
    return "".join(text.splitlines(keepends=True)[max(start, 1) - 1:end])


# ---- effects ---------------------------------------------------------------


def git(repo: Path, *args: str) -> str:
    env = {"PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": os.devnull,
           "GIT_CONFIG_SYSTEM": os.devnull, "HOME": str(repo)}
    done = subprocess.run(
        ["git", "-c", "user.name=measure", "-c", "user.email=measure@example.invalid",
         "-c", "commit.gpgsign=false", *args],
        cwd=repo, env=env, capture_output=True, text=True, check=False,
    )
    if done.returncode not in (0, 1):  # `git grep` answers 1 for no match
        raise RuntimeError(f"git {args}: {done.stderr.strip()}")
    return done.stdout


def build_repository(root: Path) -> None:
    for relative, text in FILES.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    git(root, "init", "-q", "-b", "main")
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", "base")


def apply(root: Path, case: dict) -> None:
    for relative, old, new in case["edits"]:
        path = root / relative
        text = path.read_text()
        if text.count(old) != 1:
            raise RuntimeError(f"{case['id']}: `{old}` is not in {relative} exactly once")
        path.write_text(text.replace(old, new))


def declarations_of() -> dict[str, set[str]]:
    """Which symbols each file declares, written down from the fixture."""
    declared = {
        "src/config.rs": {"Config", "trim_line", "parse_config"},
        "src/server.rs": {"start", "status"},
        "src/cli.rs": {"read_args"},
        "src/reload.rs": {"reload"},
        "src/render.rs": {"Renderer", "Html", "Plain"},
        "src/report.rs": {"report"},
        "src/metrics.rs": {"Counter", "record", "mean"},
        "src/logging.rs": {"line", "banner"},
        "src/retry.rs": {"backoff", "should_retry"},
        "tests/config_tests.rs": {"parses_pairs", "ignores_blank_lines"},
        "tests/server_tests.rs": {"start_counts_entries"},
        "tests/reload_tests.rs": {"reload_reads_the_text"},
        "tests/render_tests.rs": {"renders_html"},
        "tests/metrics_tests.rs": {"counts_what_it_records"},
    }
    return declared


def timed(command: list[str], cwd: Path, env: dict | None = None) -> tuple[float, subprocess.CompletedProcess]:
    started = time.perf_counter()
    done = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, check=False)
    return (time.perf_counter() - started) * 1000, done


WIDE_TOKENS = 6000


def run_pack(binary: str, root: Path, cache: Path, fmt: str,
             tokens: int | None = None) -> tuple[float, subprocess.CompletedProcess]:
    env = {"PATH": os.environ["PATH"], "HOME": str(cache), "XDG_CACHE_HOME": str(cache)}
    command = [binary, "pack", "--change-scope", "working_tree", "--format", fmt]
    if tokens is not None:
        command += ["--max-tokens", str(tokens)]
    return timed(command, root, env)


def measure_pack(binary: str, root: Path, case: dict, runs: int,
                 tokens: int | None = None) -> dict:
    """The pack, cold (a fresh cache each run) and warm (one cache, first run
    discarded), with the structured answer of the last run."""
    cold, warm, failed = [], [], 0
    answer = {}
    text_bytes = 0
    for _ in range(runs):
        cache = Path(tempfile.mkdtemp(prefix="pack-cache-"))
        try:
            ms, done = run_pack(binary, root, cache, "json", tokens)
            if done.returncode != 0:
                failed += 1
                continue
            cold.append(ms)
            answer = json.loads(done.stdout).get("result", {})
        finally:
            shutil.rmtree(cache, ignore_errors=True)
    cache = Path(tempfile.mkdtemp(prefix="pack-cache-"))
    try:
        run_pack(binary, root, cache, "json", tokens)
        for _ in range(runs):
            ms, done = run_pack(binary, root, cache, "json", tokens)
            if done.returncode != 0:
                failed += 1
                continue
            warm.append(ms)
        _, text = run_pack(binary, root, cache, "text", tokens)
        text_bytes = len(text.stdout.encode()) if text.returncode == 0 else 0
    finally:
        shutil.rmtree(cache, ignore_errors=True)
    items = answer.get("items") or []
    names = {item["name"] for item in items}
    declared = declarations_of()
    ranges = 0
    for item in items:
        path = root / item["file"]
        if path.is_file():
            window = item.get("range") or {}
            ranges += len(lines_of(path.read_text(), window.get("start_line", 1),
                                   window.get("end_line", 1)).encode())
    delivered = names
    return {
        "failed_runs": failed,
        "cold_ms": spread(cold),
        "warm_ms": spread(warm),
        "text_bytes": text_bytes,
        "optional_range_bytes": ranges,
        "calls": 1,
        "optional_reads": len(items),
        "status": answer.get("status"),
        "gap_kinds": sorted({g["kind"] for g in (answer.get("change") or {}).get("gaps", [])}),
        "delivered": sorted(delivered),
        "relevance": relevance(delivered, case["required"], case["unwanted"]),
        "file_level": file_level({i["file"] for i in items}, case["required"],
                                 case["unwanted"], declared),
    }


def measure_alternatives(root: Path, case: dict, runs: int) -> dict:
    declared = declarations_of()
    result = {}
    diff_ms, grep_ms, read_ms = [], [], []
    diff_text = ""
    for _ in range(runs):
        started = time.perf_counter()
        diff_text = git(root, "diff", "--no-color")
        diff_ms.append((time.perf_counter() - started) * 1000)
        started = time.perf_counter()
        lines = {n: git(root, "grep", "-n", "-w", n) for n in case["names"]}
        grep_ms.append((time.perf_counter() - started) * 1000)
        started = time.perf_counter()
        hits = {n: set(git(root, "grep", "-l", "-w", n).split()) for n in case["names"]}
        changed = set(git(root, "diff", "--name-only").split())
        files = changed.union(*hits.values()) if hits else changed
        sizes = {f: len((root / f).read_bytes()) for f in files}
        read_ms.append((time.perf_counter() - started) * 1000)
    diff_bytes = len(diff_text.encode())
    grep_bytes = sum(len(t.encode()) for t in lines.values())
    changed_files = set(git(root, "diff", "--name-only").split())
    grep_files = {f for text in lines.values() for f in (l.split(":", 1)[0] for l in text.splitlines())}
    code = lambda paths: {p for p in paths if p.endswith(".rs")}
    names = set(case["names"])
    result["diff"] = {
        "calls": 1, "bytes": diff_bytes, "ms": spread(diff_ms),
        "file_level": file_level(code(changed_files), case["required"], case["unwanted"], declared),
    }
    result["grep_lines"] = {
        "calls": 1 + len(names), "bytes": diff_bytes + grep_bytes,
        "ms": spread([a + b for a, b in zip(diff_ms, grep_ms)]),
        "file_level": file_level(code(changed_files | grep_files), case["required"],
                                 case["unwanted"], declared),
        "grep_hit_lines": sum(len(t.splitlines()) for t in lines.values()),
    }
    result["grep_files"] = {
        "calls": 1 + len(names) + len(code(files)),
        "bytes": diff_bytes + sum(sizes[f] for f in code(files)),
        "ms": spread([a + b for a, b in zip(diff_ms, read_ms)]),
        "file_level": file_level(code(files), case["required"], case["unwanted"], declared),
    }
    return result


def measure(binary: str, runs: int) -> dict:
    cases = {}
    for case in CASES:
        root = Path(tempfile.mkdtemp(prefix="change-pack-"))
        try:
            build_repository(root)
            apply(root, case)
            cases[case["id"]] = {
                "what": case["what"],
                "required": sorted(case["required"]),
                "pack": measure_pack(binary, root, case, runs),
                "pack_wide": measure_pack(binary, root, case, runs, WIDE_TOKENS),
                **measure_alternatives(root, case, runs),
            }
        finally:
            shutil.rmtree(root, ignore_errors=True)
    return cases


def print_table(report: dict) -> None:
    print(f"runs per case: {report['host']['runs']}")
    head = f"{'case':<22}{'tool':<12}{'calls':>6}{'bytes':>9}{'recall':>8}{'prec':>7}{'unw':>5}  state"
    print(head)
    for case_id, entry in report["cases"].items():
        pack = entry["pack"]
        rel = pack["relevance"]
        print(f"{case_id:<22}{'pack':<12}{pack['calls']:>6}{pack['text_bytes']:>9}"
              f"{str(rel['recall']):>8}{str(rel['precision']):>7}{rel['unwanted']:>5}  "
              f"{pack['status']} gaps={pack['gap_kinds']} cold p50={pack['cold_ms'].get('p50')}ms "
              f"warm p50={pack['warm_ms'].get('p50')}ms ranges={pack['optional_range_bytes']}B")
        wide = entry["pack_wide"]
        rel = wide["relevance"]
        print(f"{'':<22}{'pack-wide':<12}{wide['calls']:>6}{wide['text_bytes']:>9}"
              f"{str(rel['recall']):>8}{str(rel['precision']):>7}{rel['unwanted']:>5}  "
              f"{wide['status']} gaps={wide['gap_kinds']} (--max-tokens {WIDE_TOKENS}) "
              f"warm p50={wide['warm_ms'].get('p50')}ms")
        for tool in ("diff", "grep_lines", "grep_files"):
            level = entry[tool]["file_level"]
            print(f"{'':<22}{tool:<12}{entry[tool]['calls']:>6}{entry[tool]['bytes']:>9}"
                  f"{str(level['recall']):>8}{str(level['precision']):>7}{level['unwanted']:>5}  "
                  f"file-level, p50={entry[tool]['ms'].get('p50')}ms")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cognicode", required=True, help="path to the cognicode binary")
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--out", help="write the full JSON report here")
    args = parser.parse_args()

    host = {
        "cores": os.cpu_count(),
        "platform": platform.platform(),
        "loadavg_before": os.getloadavg(),
        "binary": args.cognicode,
        "runs": args.runs,
    }
    cases = measure(args.cognicode, args.runs)
    host["loadavg_after"] = os.getloadavg()
    report = {"host": host, "cases": cases}
    print_table(report)
    print("host:", json.dumps(host))
    if args.out:
        Path(args.out).write_text(json.dumps(report, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
