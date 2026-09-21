# Archive Manifest — e59.1 — graph + M5 taint correctness

> Cycle: A-lite | Milestone: M6 (entry to e60) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e59.1-lsi-graph-and-taint-correctness` |
| Path | A-lite |
| Final status | **ARCHIVED** |
| Base SHA | `a298ab0b` (post-e59) |
| Verify verdict | **PASS** |

## What was delivered

| Item | Fix |
|------|-----|
| Graph exclusion | enforced **during** traversal (shortest valid witness), not post-hoc |
| Multi-FLOW | `0` → diagnostic, `1` → run, `>1` → `BackendError::UnsupportedIr` |
| **M5 T1** | witness reconstruction constrained to nodes the origin actually reached (previously could cross an untainted sanitizer) |
| **M5 T2** | a node is re-enqueued on every new origin (previously later origins were recorded but never propagated) |

Both M5 defects were **characterized first** (T1/T2 failed against `main`),
then fixed **in M5** — M6 was untouched for them.

## Evidence

- `taint_characterization`: 2 failed → 2 passed.
- graph: 7 unit + 4 E2E tests; findings: 97; AST E2E: 6.
- `cargo check --workspace --all-targets` 0 errors; fmt clean.
- known-failure baseline unchanged (41, exit 0).

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification commands

```
cargo test -p cognicode-graph-algos --test taint_characterization
cargo test -p cognicode-core --lib domain::findings
python3 scripts/check_known_failures.py
```
