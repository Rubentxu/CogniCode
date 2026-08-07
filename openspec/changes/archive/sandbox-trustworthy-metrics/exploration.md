## Exploration: sandbox-trustworthy-metrics

### Current State
The remaining sandbox gaps are mostly definition and reporting problems, not a single parser defect. Rust repo-backed scenarios in `sandbox/manifests/rust.yaml` mix a repo sub-workspace (`workspace: serde`) with repo-prefixed paths like `serde/src/lib.rs`, which makes the MCP server resolve paths under a nonexistent `sandbox/repos/serde/serde/serde/...` tree. Python scenarios in `sandbox/manifests/python.yaml` target the Click repo from `workspace: src/click`, but their mutation strings do not exist in `__init__.py`, and their validation command (`python -m py_compile ...`) is executed from inside the package directory, where `src/click/types.py` shadows stdlib `types` and breaks Python startup. TypeScript’s only remaining failure is not a parser-validity problem; the “concrete” rename scenario in `sandbox/manifests/ts.yaml` is logically inconsistent because it renames `calculateArea` in `ts-mutation/index.ts` without updating `test_index.ts`, so the scenario cannot truthfully be a passing mutation.

The metrics/reporting layer is also incomplete. `src/bin/sandbox_orchestrator.rs` computes dimension averages and an MCP Health Score at lines `1855-1862` and prints them at `1892-1902`, but `write_summary` at `1439-1444` only writes the legacy `Summary` object to `summary.json`, and current markdown summaries omit health-score sections entirely. `sandbox/history/runs.jsonl` only contains a stale April 9 entry with `health_score: 0.0`, so recent runs are not providing trustworthy persisted health data.

### Affected Areas
- `sandbox/manifests/rust.yaml` — repo-backed Rust paths and mutation targets are inconsistent with the actual Serde repo layout.
- `sandbox/manifests/python.yaml` — Click workspace, target files, ground truth, and validation commands are inconsistent with the repo.
- `sandbox/manifests/ts.yaml` — the concrete TS mutation scenario is internally inconsistent with its tests.
- `src/bin/sandbox_orchestrator.rs` — repo/workspace resolution, baseline validation behavior, summary persistence, and health-score reporting.
- `src/application/services/file_operations.rs` — rejected/no-op edits surface ambiguous `Changed 0 bytes` previews.
- `sandbox/results/summary_20260410T174126.md` — latest Rust repo-backed failure summary.
- `sandbox/results/summary_20260410T174138.md` — latest Python failure summary.
- `sandbox/results/summary_20260410T174308.md` — latest TypeScript failure summary.
- `sandbox/results/**/response.json` and `result.json` for the failing scenarios — contain the exact observable failure messages.
- `sandbox/history/runs.jsonl` — persisted health history is stale and incomplete.

### Approaches
1. **Manifest repair only** — Fix the Rust/Python/TS scenario definitions so paths, workspaces, mutation strings, and validations match the real repos/fixtures, then rerun targeted suites.
   - Pros: Fastest route to improving pass/fail trustworthiness.
   - Cons: Leaves poor diagnostics and missing health-score persistence in place.
   - Effort: Medium

2. **Manifest repair plus reporting/diagnostic fixes** — Correct the manifests and also improve summary persistence and edit-file rejection reporting so future metrics are auditable.
   - Pros: Makes the metrics trustworthy and actionable for application improvements.
   - Cons: Slightly broader implementation scope.
   - Effort: Medium/High

### Recommendation
Choose approach 2. The primary failures are bad scenario definitions, but the application also hides key truth: `edit_file` rejections can look like successful zero-byte changes, and health-score data is computed but not persisted into the main summary artifacts. Fixing both the benchmark definitions and the reporting layer is the most reliable way to satisfy the user’s two goals.

### Risks
- Repo-backed manifests are mixing repo-root and crate/package-root assumptions; fixing only one field may still leave validations or file arguments wrong.
- Python validation from inside a package directory can continue to trigger module-shadowing failures unless the working directory or command style changes.
- TypeScript mutation scenarios may keep appearing “healthy” at validation time if the edit is silently rejected before tests run.
- If health-score fields are not added to the serialized summary format, future runs will still require stdout scraping to recover key metrics.

### Ready for Proposal
Yes — propose a change that (1) repairs manifest correctness for repo-backed Rust/Python/TypeScript scenarios, and (2) improves sandbox metric/reporting fidelity in the orchestrator and `edit_file` diagnostics.
