# Archive Manifest — e60.1 — semantic contract tightening

> Cycle: A-lite | Milestone: M6 | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e60.1-lsi-semantic-contract-tightening` |
| Path | A-lite |
| Final status | **ARCHIVED** |
| Base SHA | `5516d63e` (post-e60) |
| Verify verdict | **PASS** |

## What was delivered

| Id | Fix |
|----|-----|
| C1 | `M5DataflowBackend` seeds M5 only from `FLOW.source` (matching `GraphBackend`) |
| C2 | IR rule V11: `FLOW.source` must be declared by a `MATCH`, else `UndeclaredFlowSource` |
| C3 | Regression tests: an unrelated `MATCH` cannot become a source (request + full chain) |
| C4 | `taint_flow` enforces `max_visited_nodes`, `max_visited_edges`, `max_path_count`/`max_result_rows` |
| C5 | Exceeded limits → `AnalyticsError::LimitExceeded` → `BackendError::Analysis` → `ExecutionError::Backend` |
| C6 | e60 design contract corrected (`MATCH` is never a source) |

## Evidence

- `domain::findings` 98; `application::findings` 13; AST E2E 6; Graph E2E 4;
  Dataflow E2E 7.
- `cargo check --workspace --all-targets` 0 errors; fmt clean; known-failure
  baseline unchanged (41).

## Result

**AST + Graph + Dataflow are frozen as base infrastructure.** No further
hardening of this seam unless 7.7 (Axiom importer) or U42 (kernel grounding)
produce concrete evidence of another problem.

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.
