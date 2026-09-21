# Tasks — cycle e58 — known-failure baseline

> Cycle: B-direct (housekeeping) | Date: 2026-09-15

### WU-1 — Baseline
- `scripts/known_failures.yaml` (41 entries, category
  `environment_dependency`, with reasons).

### WU-2 — Checker
- `scripts/check_known_failures.py` (run + compare; `--update`;
  exit 1 on new or unexpectedly-fixed).

### WU-3 — Recipes
- `just check-known-failures`, `just update-known-failures`.

## Acceptance gate
- `python3 scripts/check_known_failures.py` exits 0 with the current tree.
- Drift (missing entry) exits 1.
- Conventional commit, no AI trailers.
