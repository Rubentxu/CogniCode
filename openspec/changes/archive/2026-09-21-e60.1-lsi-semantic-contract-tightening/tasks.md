# Tasks — cycle e60.1 — semantic contract tightening

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

| Id | Content |
|----|---------|
| C1 | `FLOW.source` is the only authoritative reachability source (dataflow backend) |
| C2 | IR rule V11: `FLOW.source` must be declared by a `MATCH` |
| C3 | Regression tests: unrelated `MATCH` cannot become a source (request sites + whole chain) |
| C4 | Enforce `max_visited_nodes`, `max_visited_edges`, `max_path_count`/`max_result_rows` |
| C5 | Exceeded limits stay errors end-to-end |
| C6 | Correct the e60 design contract |

## Acceptance gate
- `domain::findings` 98; `application::findings` 13; AST E2E 6; Graph E2E 4;
  Dataflow E2E 7.
- `cargo check --workspace --all-targets` 0 errors; fmt clean.
- known-failure baseline unchanged.
