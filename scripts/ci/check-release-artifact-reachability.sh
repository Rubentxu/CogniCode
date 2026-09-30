#!/usr/bin/env bash
# QW-09 — release-artifact reachability guard.
#
# WHY THIS EXISTS
# ---------------
# Run 36682728317 / 36684133482 of PR-CI. The `merge-gate` job failed with
#
#     cli_and_mcp_processes_agree_on_symbols_and_edges --- FAILED
#     falta binario CLI: .../target/release/cognicode
#
# for two consecutive "fixes", because of a property of GitHub Actions that
# is easy to read past:
#
#   `needs:` means "wait for that job to finish". It does NOT mean
#   "inherit that job's artifacts".
#
# `build-binary` uploaded `cognicode-bins-release`; `test-pr` downloaded it.
# Both green. `merge-gate` waited on `build-binary`, saw success, and received
# zero files, because every job runs on its own fresh runner. The failing
# assertion was `cli_bin().exists()` — on that runner the file had never
# existed. `test-pr`'s download step sat at line 261; `merge-gate` started at
# line 365 and had no download step at all.
#
# A second, subtler instance of the same class: `merge-gate` DID contain a
# "Release-profile binaries" step running `cargo build --release`, but at
# line ~576 — AFTER the `prf_cli_04` step at line 514. In file order, steps
# run in order. Having the build somewhere in the job is not the same as
# having it before the step that needs it.
#
# WHAT IT CHECKS
# --------------
# For every job in the workflow, if any step in that job needs a release
# binary (built by `cargo build --release`, executed directly, or asserted on
# by a black-box suite that shells out to `target/release/...`), then that
# same job must, BEFORE the first such step, either:
#
#   (a) download an artifact into `target/release`, or
#   (b) build the release profile itself.
#
# Exit 0 when every job that needs binaries can obtain them in time.
# Exit 1 otherwise, naming the job, the step that needs the binaries, and the
# first step that would have provided them.
#
# This is a static check over the workflow text. It cannot prove a runner
# actually has the file; it pins that the job is wired to obtain it before
# use, which is the defect that bit twice.

set -euo pipefail

ROOT="${1:-$(git rev-parse --show-toplevel)}"
WORKFLOW="$ROOT/.github/workflows/pr-ci.yml"

if [ ! -f "$WORKFLOW" ]; then
  echo "QW-09: workflow not found at $WORKFLOW" >&2
  exit 1
fi

python3 - "$WORKFLOW" <<'PY'
import re
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as fh:
    lines = fh.read().splitlines()

# --- split into jobs, tracking the line index of every step ---------------
# A step is `      - name: <label>` optionally followed by `uses:` and
# `with:`. The ACTION lives on the `uses:` line, not on the `name:` line, so
# a step's identity is the pair of lines until the next step. Reading only
# `name:` is what made the first version of this guard miss the very step it
# was written to check (`actions/download-artifact` sits under a `uses:`).
JOBS_RE = re.compile(r"^  ([A-Za-z0-9_-]+):\s*$")
STEP_START_RE = re.compile(r"^      - (?:name|uses):\s*(.*)$")
NAME_STRIP = re.compile(r"^(?:name|uses):\s*")
SUBLINE_RE = re.compile(r"^        ([A-Za-z_-]+):\s*(.*)$")

jobs = {}          # job name -> list of (line_no, joined_label)
current = None
for idx, raw in enumerate(lines, start=1):
    m = JOBS_RE.match(raw)
    if m:
        current = m.group(1)
        jobs[current] = []
        continue
    if current is None:
        continue
    s = STEP_START_RE.match(raw)
    if s:
        label = NAME_STRIP.sub("", s.group(1)).strip()
        label = label.split(" #")[0].strip().strip('"').strip("'")
        jobs[current].append((idx, label))
        continue
    # A `uses:` / `with:` sub-line belongs to the step we just opened.
    sub = SUBLINE_RE.match(raw)
    if sub and jobs[current]:
        key, val = sub.group(1), sub.group(2)
        if key in ("uses", "with", "run"):
            val = val.split(" #")[0].strip()
            jobs[current][-1] = (jobs[current][-1][0], jobs[current][-1][1] + " " + val)

# --- classify steps --------------------------------------------------------
BUILD_RE = re.compile(r"cargo\s+build\b[^\n]*--release|--release[^\n]*cargo\s+build")
DOWNLOAD_RE = re.compile(r"download-artifact")


def provides_bins(labels):
    """True if any of these step labels would put binaries in target/release."""
    for label in labels:
        if DOWNLOAD_RE.search(label):
            return True
    return False


def builds_release(labels):
    for label in labels:
        if BUILD_RE.search(label):
            return True
    return False


def needs_bins(label):
    """True if this single step cannot succeed without a release binary."""
    # Black-box MCP/CLI suites that shell out to target/release/*.
    if "black-box" in label.lower():
        return True
    if re.search(r"PRF-CLI-04|prf_cli_04", label):
        return True
    if re.search(r"Release-profile binaries", label):
        return True
    # A step that executes a built binary directly.
    if re.search(r"\./target/release/", label):
        return True
    return False


problems = []
for job, steps in jobs.items():
    if not steps:
        continue
    need_idx = None
    for line_no, label in steps:
        if needs_bins(label):
            need_idx = (line_no, label)
            break
    if need_idx is None:
        continue

    need_line, need_label = need_idx
    # Everything this job does BEFORE the first step that needs binaries.
    before = [lbl for ln, lbl in steps if ln < need_line]
    if provides_bins(before) or builds_release(before):
        continue

    # Nothing in this job can supply binaries in time. Describe the gap.
    later = [
        (ln, lbl) for ln, lbl in steps
        if ln > need_line and (DOWNLOAD_RE.search(lbl) or BUILD_RE.search(lbl))
    ]
    hint = ""
    if later:
        ln, lbl = later[0]
        hint = (
            f"\n     NOTE: this job does provide binaries, but at line {ln} "
            f"({lbl[:60]}), AFTER the step that needs them. "
            f"Steps run in file order."
        )

    problems.append(
        f"  job '{job}': step at line {need_line} needs a release binary "
        f"({need_label[:60]}), but this job never downloads an artifact or "
        f"builds --release before it.{hint}"
    )

if problems:
    print("QW-09: FAILED — jobs that need release binaries cannot obtain them in time.")
    print()
    for p in problems:
        print(p)
    print()
    print("  `needs:` makes a job WAIT for another job. It does not move")
    print("  artifacts between runners. Each job that uses target/release/*")
    print("  must download it or build it itself, before the first step")
    print("  that needs it.")
    sys.exit(1)

print("OK: every job that needs release binaries obtains them before use.")
PY
