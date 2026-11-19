#!/usr/bin/env python3
"""Cost of the parts of a change that the in-process benchmark does not time.

The mode-equivalence benchmark (crates/cognicode-core/tests/it/mode_equivalence)
times the analysis pipelines. This instrument times what surrounds them, on the
same corpus (crates/cognicode-core/testdata/mode_equivalence):

  corpus-gates   the gate stages that exist for each language of a scenario
                 (compile, lint, format, test), cold and after the edit
  guard-delta    `cognicode-guard candidate delta` over each scenario's edit
  worktree       git branch and worktree operations, first use and repeat
  pr-checks      what the hosted check and the PR lifecycle cost, read back
                 from the GitHub API for the most recent merged pull requests

Disciplines the numbers obey:
  - Every figure is a wall time on one shared host. The report records the
    load average before and after, the cores and the memory, and it says so.
  - Time a build spent waiting for another build's file lock is reported
    apart (`lock_wait_ms`) and `compute_ms` excludes it. The wait is read from
    the lock message cargo prints, so it is an estimate of the wait, not a
    clock on the lock.
  - Percentiles are nearest-rank, like the benchmark: with fewer than 20 runs
    the p95 is the maximum, and the report says how many runs it has.
  - A tool that is not installed is `not_evaluated` with the reason. Nothing is
    estimated, and no run that failed is folded into a number as if it passed:
    exit codes are counted next to the times.
  - Page cache cannot be dropped without privileges, so "cold" means a fresh
    directory that nothing has built or read yet, never a cold disk.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from collections import Counter
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Callable

LOCK_MARKER = "Blocking waiting for file lock"
# A measured command that has not finished by now is killed and counted as a
# failed run; a hung tool must not hang the measurement.
DEADLINE_SECONDS = 900
QUICK_SECONDS = 120
CORPUS = Path("crates/cognicode-core/testdata/mode_equivalence")

# What each part of the corpus has as gates on a developer machine, cheapest
# first. Every stage runs whatever the earlier ones concluded, because the
# corpus is analysis test data and is not clean by construction (the base
# crate itself fails `clippy -D warnings`): each stage is costed on its own
# and its exit codes are reported beside the time. `clippy` therefore runs
# without `-D warnings`, so that it costs the whole lint pass.
STAGES: dict[str, list[tuple[str, list[str], str]]] = {
    "rust": [
        ("fmt", ["cargo", "fmt", "--check"], "cargo"),
        ("clippy", ["cargo", "clippy"], "cargo"),
        ("build", ["cargo", "build"], "cargo"),
        ("test", ["cargo", "test"], "cargo"),
    ],
    "helm": [
        ("lint", ["helm", "lint", "chart"], "helm"),
        ("template", ["helm", "template", "chart"], "helm"),
    ],
}
NO_GATE_REASON = "no gate for this language is defined here: {tool} is not installed on this host"
LANGUAGE_TOOLS = {"java": "javac", "kotlin": "kotlinc", "go": "go", "terraform": "terraform"}


# ---- pure core ---------------------------------------------------------


def percentile(values: list[float], p: int) -> float:
    """Nearest-rank percentile, the definition the Rust benchmark uses."""
    if not values:
        raise ValueError("a percentile of nothing is not a measurement")
    ordered = sorted(values)
    rank = max(-(-p * len(ordered) // 100), 1)
    return ordered[rank - 1]


@dataclass(frozen=True)
class Sample:
    wall_ms: float
    lock_wait_ms: float
    exit_code: int

    @property
    def compute_ms(self) -> float:
        return max(self.wall_ms - self.lock_wait_ms, 0.0)


def lock_wait_ms(stamped: list[tuple[float, str]], end: float) -> float:
    """Milliseconds between each lock message and the line that followed it."""
    total = 0.0
    for index, (at, line) in enumerate(stamped):
        if LOCK_MARKER in line:
            following = stamped[index + 1][0] if index + 1 < len(stamped) else end
            total += following - at
    return total * 1000.0


def distribution(values: list[float]) -> dict[str, float | int] | None:
    if not values:
        return None
    return {
        "n": len(values),
        "p50_ms": percentile(values, 50),
        "p95_ms": percentile(values, 95),
        "min_ms": min(values),
        "max_ms": max(values),
    }


def summarise(samples: list[Sample]) -> dict:
    """The distribution of a set of runs, with the exit codes counted beside it."""
    return {
        "runs": len(samples),
        "exit_codes": dict(Counter(str(s.exit_code) for s in samples)),
        "wall_ms": distribution([s.wall_ms for s in samples]),
        "compute_ms": distribution([s.compute_ms for s in samples]),
        "lock_wait_ms_total": sum(s.lock_wait_ms for s in samples),
        "runs_that_waited_for_a_lock": sum(1 for s in samples if s.lock_wait_ms > 0),
    }


def run_total(stages: list[Sample]) -> Sample:
    """One run of a whole gate: the stages summed, failing if any stage failed."""
    failed = next((s.exit_code for s in stages if s.exit_code != 0), 0)
    return Sample(
        sum(s.wall_ms for s in stages), sum(s.lock_wait_ms for s in stages), failed
    )


def seconds_between(start: str, end: str) -> float:
    parse = lambda text: datetime.fromisoformat(text.replace("Z", "+00:00"))  # noqa: E731
    return (parse(end) - parse(start)).total_seconds()


def pr_cost(pulls: list[dict]) -> dict:
    """Check-run and lifecycle durations (in ms) over merged pull requests.

    Each pull is `{"created_at", "merged_at", "check_runs": [{name, started_at,
    completed_at, conclusion}]}`. A check run that did not finish is counted,
    not timed.
    """
    by_check: dict[str, list[float]] = {}
    conclusions: dict[str, Counter] = {}
    unfinished = 0
    for pull in pulls:
        for run in pull["check_runs"]:
            conclusions.setdefault(run["name"], Counter())[str(run["conclusion"])] += 1
            if not run.get("completed_at") or not run.get("started_at"):
                unfinished += 1
                continue
            by_check.setdefault(run["name"], []).append(
                seconds_between(run["started_at"], run["completed_at"]) * 1000.0
            )
    lifecycle = [seconds_between(p["created_at"], p["merged_at"]) * 1000.0 for p in pulls]
    return {
        "pull_requests": len(pulls),
        "check_runs": {
            name: {**(distribution(values) or {}), "conclusions": dict(conclusions[name])}
            for name, values in by_check.items()
        },
        "unfinished_check_runs": unfinished,
        "open_to_merge": distribution(lifecycle),
    }


# ---- the corpus, as files ---------------------------------------------


def scenarios(corpus: Path) -> list[tuple[str, str, Path]]:
    """(part, class, overlay) for every overlay directory, in a stable order."""
    found = []
    for part in sorted((corpus / "edits").iterdir()):
        for overlay in sorted(part.iterdir()):
            if overlay.is_dir():
                found.append((part.name, overlay.name, overlay))
    return found


def apply_overlay(overlay: Path, dest: Path) -> None:
    deletions = overlay / "DELETE"
    if deletions.is_file():
        for line in deletions.read_text().splitlines():
            if line.strip():
                (dest / line.strip()).unlink()
    for source in sorted(overlay.rglob("*")):
        relative = source.relative_to(overlay)
        if source.is_dir() or relative.parts[0] in ("DELETE", "ORACLE") and len(relative.parts) == 1:
            continue
        (dest / relative).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, dest / relative)


def edits_json(overlay: Path) -> dict[str, str | None]:
    """The overlay as the `candidate` edits object: path to new text, null to delete."""
    edits: dict[str, str | None] = {}
    deletions = overlay / "DELETE"
    if deletions.is_file():
        edits.update({line.strip(): None for line in deletions.read_text().splitlines() if line.strip()})
    for source in sorted(overlay.rglob("*")):
        relative = source.relative_to(overlay)
        if source.is_file() and relative.as_posix() not in ("DELETE", "ORACLE"):
            edits[relative.as_posix()] = source.read_text()
    return edits


# ---- the edge: processes, git, GitHub ---------------------------------


def run_timed(argv: list[str], cwd: Path, env: dict[str, str] | None = None) -> Sample:
    started = time.monotonic()
    process = subprocess.Popen(
        argv,
        cwd=cwd,
        env={**os.environ, **(env or {})},
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    watchdog = threading.Timer(DEADLINE_SECONDS, process.kill)
    watchdog.start()
    try:
        stamped = [(time.monotonic(), line) for line in process.stderr]  # type: ignore[union-attr]
        code = process.wait()
    finally:
        watchdog.cancel()
    end = time.monotonic()
    return Sample((end - started) * 1000.0, lock_wait_ms(stamped, end), code)


def git_identity(repo: Path) -> tuple[str, str]:
    """Read the admitted identity from the repository running the measurement."""
    values = []
    for key in ("user.name", "user.email"):
        result = subprocess.run(
            ["git", "config", "--get", key], cwd=repo, capture_output=True, text=True
        )
        value = result.stdout.strip()
        if result.returncode != 0 or not value:
            raise RuntimeError("fixture commits require a configured Git author identity")
        values.append(value)
    return values[0], values[1]


def git(repo: Path, *args: str, identity: tuple[str, str] | None = None) -> str:
    env = os.environ.copy()
    for key in ("GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL"):
        env.pop(key, None)
    env.update({"GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_SYSTEM": "/dev/null"})
    if identity is not None:
        env.update(
            {
                "GIT_AUTHOR_NAME": identity[0],
                "GIT_AUTHOR_EMAIL": identity[1],
                "GIT_COMMITTER_NAME": identity[0],
                "GIT_COMMITTER_EMAIL": identity[1],
            }
        )
    done = subprocess.run(
        ["git", *args], cwd=repo, env=env, capture_output=True, text=True, check=True,
        timeout=QUICK_SECONDS,
    )
    return done.stdout.strip()


def fixture_repo(
    corpus: Path,
    dest: Path,
    overlay: Path | None,
    *,
    identity_source: Path,
    language: str | None = None,
) -> None:
    """Commit the corpus base and overlay, optionally scoped to one language."""
    author_name, author_email = git_identity(identity_source)
    dest.mkdir(parents=True, exist_ok=True)
    git(dest, "init", "-q", "-b", "main")
    git(dest, "config", "user.name", author_name)
    git(dest, "config", "user.email", author_email)
    remote = dest.with_name(f"{dest.name}-origin.git")
    git(dest.parent, "init", "--bare", "-q", str(remote))
    git(dest, "remote", "add", "origin", str(remote))
    source = corpus / "base" if language is None else corpus / "base" / language
    shutil.copytree(source, dest if language is None else dest / language, dirs_exist_ok=True)
    git(dest, "add", "-A")
    git(
        dest,
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-q",
        "-m",
        "chore: establish benchmark base",
        identity=(author_name, author_email),
    )
    if overlay is not None:
        apply_overlay(overlay, dest)
        git(dest, "add", "-A")
        git(
            dest,
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "feat: apply benchmark edit",
            identity=(author_name, author_email),
        )


def host(load_before: str) -> dict:
    meminfo = dict(line.split(":", 1) for line in Path("/proc/meminfo").read_text().splitlines())
    return {
        "logical_cpus": os.cpu_count(),
        "mem_total_kib": int(meminfo["MemTotal"].split()[0]),
        "load_average_before": load_before,
        "load_average_after": Path("/proc/loadavg").read_text().split()[0:3],
        "kernel": platform.release(),
        "shared_host": "other builds and agents run on this host; every timing is a noisy single-host measurement",
    }


def load_now() -> str:
    return " ".join(Path("/proc/loadavg").read_text().split()[0:3])


def toolchain_env(repo_root: Path, workdir: Path) -> None:
    """Pins the tools the way the repository does, so a shim resolves in a temp dir."""
    versions = repo_root / ".tool-versions"
    if versions.is_file():
        shutil.copy2(versions, workdir / ".tool-versions")


def pinned_versions() -> dict[str, str]:
    """`ASDF_<TOOL>_VERSION` for tools the temp directory has no `.tool-versions` for.

    A shim resolves the version from the directory it runs in; the corpus copy
    lives in a temp directory where nothing pins helm. Uses the newest
    installed version, so the tool measured is one that exists on this host.
    """
    pins: dict[str, str] = {}
    if shutil.which("asdf") is None:
        return pins
    for tool in {t for stages in STAGES.values() for _n, _a, t in stages}:
        listed = subprocess.run(["asdf", "list", tool], capture_output=True, text=True, timeout=QUICK_SECONDS).stdout.split()
        if listed:
            pins[f"ASDF_{tool.upper()}_VERSION"] = listed[-1].lstrip("*")
    return pins


def gate_run(part: str, workdir: Path, target: Path) -> dict[str, Sample]:
    """Runs every stage of `part`, each timed on its own."""
    env = {"CARGO_TARGET_DIR": str(target), **pinned_versions()}
    return {name: run_timed(argv, workdir / part, env) for name, argv, _tool in STAGES[part]}


def corpus_gates(repo_root: Path, runs: int, only: set[str] | None) -> list[dict]:
    corpus = repo_root / CORPUS
    report = []
    for part, klass, overlay in scenarios(corpus):
        if only and part not in only:
            continue
        gates_part = "rust" if part in ("rust", "cross") else part
        scenario = f"{part}/{klass}"
        if gates_part not in STAGES:
            tool = LANGUAGE_TOOLS.get(part, "its toolchain")
            report.append({"scenario": scenario, "not_evaluated": NO_GATE_REASON.format(tool=tool)})
            continue
        tool = STAGES[gates_part][0][2]
        if shutil.which(tool) is None:
            report.append({"scenario": scenario, "not_evaluated": NO_GATE_REASON.format(tool=tool)})
            continue
        for state in ("cold", "after_edit"):
            per_stage: dict[str, list[Sample]] = {}
            totals: list[Sample] = []
            for _ in range(runs):
                with tempfile.TemporaryDirectory(prefix="scenario-cost-") as tmp:
                    workdir, target = Path(tmp) / "repo", Path(tmp) / "target"
                    workdir.mkdir()
                    toolchain_env(repo_root, workdir)
                    shutil.copytree(corpus / "base", workdir, dirs_exist_ok=True)
                    if state == "cold":
                        apply_overlay(overlay, workdir)
                    elif gates_part == "rust":
                        run_timed(["cargo", "build"], workdir / "rust", {"CARGO_TARGET_DIR": str(target)})
                    if state == "after_edit":
                        apply_overlay(overlay, workdir)
                    done = gate_run(gates_part, workdir, target)
                for name, sample in done.items():
                    per_stage.setdefault(name, []).append(sample)
                totals.append(run_total(list(done.values())))
            report.append({
                "scenario": scenario,
                "state": state,
                "stages": {name: summarise(samples) for name, samples in per_stage.items()},
                "all_stages": summarise(totals),
            })
    return report


def delta_outcome(stdout: str) -> dict:
    """What the delta concluded, so a fast answer is never read as a good one."""
    delta = json.loads(stdout)["delta"]
    return {
        "status": delta["status"],
        "fully_observed": delta["fully_observed"],
        "impact_status": delta["candidate"]["impact_status"],
        "summary": delta["summary"],
    }


def guard_delta(repo_root: Path, guard_bin: Path, runs: int, only: set[str] | None) -> list[dict]:
    corpus = repo_root / CORPUS
    pack = {
        "pack_id": "native", "pack_version": "1.0.0", "description": "",
        "rules": [{
            "id": "native.complexity", "version": "1.0.0", "kind": {"kind": "metric_threshold"},
            "requirements": [{
                "language": "rust", "provider": "cognicode-core",
                "semantic_precision": "structural_only", "snapshotted": False, "budget": None,
            }],
            "fix_support": "none", "severity": "error", "tags": [],
            "description": json.dumps({"tag": "max_complexity", "metric": "cyclomatic", "max": 2}),
        }],
    }
    report = []
    for part, klass, overlay in scenarios(corpus):
        if only and part not in only:
            continue
        with tempfile.TemporaryDirectory(prefix="scenario-cost-") as tmp:
            repo, support = Path(tmp) / "repo", Path(tmp) / "support"
            support.mkdir()
            fixture_repo(
                corpus,
                repo,
                None,
                identity_source=repo_root,
                language="rust" if part == "rust" else None,
            )
            (support / "pack.json").write_text(json.dumps(pack))
            (support / "edits.json").write_text(json.dumps(edits_json(overlay)))
            argv = [
                str(guard_bin), "candidate", "delta", "--pack", str(support / "pack.json"),
                "--workspace", str(repo), "--edits-file", str(support / "edits.json"),
            ]
            samples = [run_timed(argv, repo) for _ in range(runs + 1)]
            answer = subprocess.run(argv, cwd=repo, capture_output=True, text=True, timeout=QUICK_SECONDS)
        report.append({
            "scenario": f"{part}/{klass}",
            "first_run_ms": samples[0].wall_ms,
            "after_first": summarise(samples[1:]),
            "answer_bytes": len(answer.stdout),
            "answer": delta_outcome(answer.stdout),
        })
    return report


def worktree_costs(source: Path, runs: int, far: str | None) -> dict:
    """Branch and worktree operations on a fresh clone of `source`.

    `first` is the first use of a clone nothing has touched; `repeat` is every
    later use. `far` is a revision to switch to, to cost a checkout across
    real file churn.
    """
    ops: dict[str, dict[str, list[Sample]]] = {}
    with tempfile.TemporaryDirectory(prefix="scenario-cost-") as tmp:
        clone = Path(tmp) / "clone"
        subprocess.run(
            ["git", "clone", "-q", "--local", "--no-checkout", str(source), str(clone)],
            check=True, timeout=DEADLINE_SECONDS,
        )
        git(clone, "checkout", "-q", "HEAD")
        files = len(git(clone, "ls-files").splitlines())
        for index in range(runs):
            phase = "first" if index == 0 else "repeat"
            branch, tree = f"bench-{index}", Path(tmp) / f"tree-{index}"
            steps = [
                ("branch_create", ["git", "branch", branch], clone),
                ("worktree_add", ["git", "worktree", "add", "-q", str(tree), branch], clone),
            ]
            if far:
                steps.append(("switch_across_history", ["git", "checkout", "-q", far], tree))
            steps += [
                ("worktree_remove", ["git", "worktree", "remove", "--force", str(tree)], clone),
                ("branch_delete", ["git", "branch", "-q", "-D", branch], clone),
            ]
            for name, argv, cwd in steps:
                ops.setdefault(name, {}).setdefault(phase, []).append(run_timed(argv, cwd))
    return {
        "tracked_files": files,
        "operations": {n: {p: summarise(s) for p, s in phases.items()} for n, phases in ops.items()},
    }


def gh_json(*args: str) -> object:
    return json.loads(subprocess.run(["gh", "api", *args], capture_output=True, text=True, check=True, timeout=QUICK_SECONDS).stdout)


def pr_checks(repo: str, count: int) -> dict:
    listed = gh_json(f"repos/{repo}/pulls?state=closed&per_page={min(count * 2, 100)}")
    merged = [p for p in listed if p.get("merged_at")][:count]  # type: ignore[union-attr]
    pulls = [
        {
            "created_at": p["created_at"],
            "merged_at": p["merged_at"],
            "check_runs": gh_json(f"repos/{repo}/commits/{p['head']['sha']}/check-runs")["check_runs"],  # type: ignore[index]
        }
        for p in merged
    ]
    return {"repo": repo, **pr_cost(pulls)}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("corpus-gates", "guard-delta"):
        sp = sub.add_parser(name)
        sp.add_argument("--runs", type=int, default=10)
        sp.add_argument("--parts", help="comma-separated corpus parts; default all")
        if name == "guard-delta":
            sp.add_argument("--guard-bin", type=Path, required=True)
    wt = sub.add_parser("worktree")
    wt.add_argument("--source", type=Path, default=Path("."))
    wt.add_argument("--runs", type=int, default=10)
    wt.add_argument("--far", help="revision to switch a worktree to")
    pr = sub.add_parser("pr-checks")
    pr.add_argument("--repo", required=True)
    pr.add_argument("--count", type=int, default=30)
    args = parser.parse_args(argv)

    repo_root = Path(__file__).resolve().parents[2]
    only = set(args.parts.split(",")) if getattr(args, "parts", None) else None
    load_before = load_now()
    if args.command == "corpus-gates":
        result: object = corpus_gates(repo_root, args.runs, only)
    elif args.command == "guard-delta":
        result = guard_delta(repo_root, args.guard_bin.resolve(), args.runs, only)
    elif args.command == "worktree":
        result = worktree_costs(args.source.resolve(), args.runs, args.far)
    else:
        result = pr_checks(args.repo, args.count)
    json.dump(
        {"command": " ".join(sys.argv), "host": host(load_before),
         "percentiles": "nearest-rank; with fewer than 20 runs the p95 is the maximum", "result": result},
        sys.stdout, indent=2,
    )
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
