# Tasks: ExplorerQL Grammar

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~1350 (1200 prod + 150 tests) |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR 1 (AST+Cursor+bool) → PR 2 (5 parsers) → PR 3 (compile+exec+MCP+NL) |
| Delivery strategy | exception-ok (size:exception accepted by orchestrator prompt) |
| Chain strategy | single batch (size:exception) |

Decision needed before apply: Yes (resolved by orchestrator prompt)
Chained PRs recommended: Yes (resolved by size:exception)
Chain strategy: size:exception
400-line budget risk: High (mitigated by size:exception)

### Work Units

| # | Goal | PR | Base | LOC |
|---|------|----|------|-----|
| 1 | AST + Cursor refactor + boolean scaffold | PR 1 | main | ~320 |
| 2 | 5 clause parsers + dispatch + filters | PR 2 | PR 1 | ~400 |
| 3 | compile + executor + MCP + NL + e2e | PR 3 | PR 2 | ~630 |

User must pick: `stacked-to-main` | `feature-branch-chain` | `size:exception` before `sdd-apply`.

---

## Phase 1 — Foundation (PR 1)

- [x] **1.1** RED: `ast_tests::query_variants_roundtrip` — 6 new `MoldQLQuery` variants exist (Path/Neighbors/Subgraph/Cluster/Explain/Boolean) and are `Debug+Clone+PartialEq`.
- [x] **1.2** GREEN 1.1: add `PathQuery`, `NeighborsQuery`, `SubgraphQuery`, `ClusterQuery`, `ExplainQuery`, `BooleanQuery`, `TraversalDirection{In,Out,Both}`, `ClusterMethod{Scc,Connected}`, `BooleanOp{And,Or}` in `moldql/ast.rs` (~+120 LOC). Extend `MoldQLQuery` enum.
- [x] **1.3** RED: `cursor_tests` in new `moldql/cursor.rs` — verify `peek/consume_keyword/position/is_eof` behavior matches current parser.
- [x] **1.4** GREEN 1.3: create `moldql/cursor.rs` (~110 LOC). Move `Cursor` verbatim from `parser.rs`. `pub(crate)`. Add `pub mod cursor;` + re-export in `moldql/mod.rs`.
- [x] **1.5** Update `parser.rs` `use` to `use crate::moldql::cursor::Cursor;`. Delete inline `Cursor`. Validate: `cargo test -p cognicode-explorer moldql` → 32 existing pass.
- [x] **1.6** RED: `parse_boolean_and_or` — `"FIND x AND EXPLORE y"` → `Boolean{And,…}`. Empty `moldql/parser_explorerql.rs` with test module.
- [x] **1.7** GREEN 1.6: in `parser_explorerql.rs` add `parse_atom` (delegate to existing `parse_query`), `parse_and_chain`, `parse_or_chain` (~120 LOC). Wire `parse_query` in `parser.rs` to call `parse_or_chain`.

**Gate P1**: `cargo test -p cognicode-explorer moldql::` → 32 + 7 = 39 pass.

---

## Phase 2 — 5 Clause Parsers (PR 2)

- [x] **2.1** RED: `parse_path_basic` — `"PATH FROM a TO b"`.
- [x] **2.2** GREEN: `parse_path_after_keyword` (~40 LOC) + dispatch match. `MAX HOPS <n>` optional.
- [x] **2.3** RED: 4 `parse_neighbors_*` tests (basic, INCOMING, OUTGOING, BOTH, DEPTH n).
- [x] **2.4** GREEN: `parse_neighbors_after_keyword` (~60 LOC). Defaults Both/1; cap 5.
- [x] **2.5** RED: `parse_subgraph_basic` + `parse_subgraph_min_confidence`.
- [x] **2.6** GREEN: `parse_subgraph_after_keyword` (~55 LOC). RADIUS 1–5.
- [x] **2.7** RED: `parse_cluster_scc` + `parse_cluster_connected` (with `WHERE cluster_id=42`).
- [x] **2.8** GREEN: `parse_cluster_after_keyword` (~60 LOC).
- [x] **2.9** RED: 3 `parse_explain_*` (CYCLES/PATH/CONNECTIVITY).
- [x] **2.10** GREEN: `parse_explain_after_keyword` (~80 LOC) with private `ExplainKind`.
- [x] **2.11** RED: 5 `where_filter_*` tests for `provenance`/`confidence`.
- [x] **2.12** GREEN: `parse_explorerql_where_clauses` reusing `Condition` AST (~30 LOC).
- [x] **2.13** RED: `boolean_precedence_parens` + `boolean_not`. (12+1 boolean total.)
- [x] **2.14** GREEN: `parse_not` + parens handling (~30 LOC).
- [x] **2.15** Update `moldql/mod.rs` re-exports: `BooleanQuery, BooleanOp, PathQuery, NeighborsQuery, SubgraphQuery, ClusterQuery, ClusterMethod, ExplainQuery, TraversalDirection`.

**Gate P2**: 32 + 23 + 11 + 12 = 78 tests pass.

---

## Phase 3 — Compilation (PR 3)

- [x] **3.1** Add `petgraph = "0.6"` to `crates/cognicode-explorer/Cargo.toml` `[dependencies]`. Validate: `cargo check` clean.
- [x] **3.2** RED: `compile_path_to_pg` — `Path{from,to,max_hops:None}` → SQL starts `WITH RECURSIVE search_path`.
- [x] **3.3** GREEN scaffold: `moldql/compile.rs` (~80 LOC) with `CompileTarget{Postgres,Petgraph}`, `CompileError`, `CompiledQuery{Postgres(String),Petgraph(PetgraphPlan),Composed(Vec<…>)}`, `pub fn compile(q,t)`.
- [x] **3.4** `pub mod compile;` + re-export in `moldql/mod.rs`.
- [x] **3.5** GREEN 3.2: `compile_postgres` dispatch + `emit_path_pg` (~50 LOC). `WITH RECURSIVE` CTE on `edges`.
- [x] **3.6** RED: 4 more `pg_emit_*` (neighbors, subgraph, cluster_scc, explain_cycles).
- [x] **3.7** GREEN: 4 emit fns (~160 LOC). Use `$1`/`$2` binds — no string interpolation.
- [x] **3.8** RED: `pg_no_string_interpolation` — regex scan for `'{.*}'` and unsafe concat.
- [x] **3.9** GREEN 3.8: validation via `sqlx::query::Query` (gated on `postgres` feature) or manual check.
- [x] **3.10** RED: `petgraph_compile_path` → `PetgraphPlan::Bfs{roots,targets,max_hops}`.
- [x] **3.11** GREEN: `compile_petgraph` + path mapper (~120 LOC).
- [x] **3.12** RED: 4 more `petgraph_compile_*`.
- [x] **3.13** GREEN: 4 petgraph plans (~120 LOC).
- [x] **3.14** RED: `compile_boolean_pg` — `Boolean{And,FIND,PATH}` → SQL `INTERSECT`.
- [x] **3.15** GREEN: AND=INTERSECT, OR=UNION, NOT=EXCEPT in PG. Petgraph: AND=node-intersect, OR=union, NOT=complement (~80 LOC).

**Gate P3**: 20 PG + 8 petgraph + 1 boolean = 29 pass.

---

## Phase 4 — Executor Wiring (PR 3)

- [x] **4.1** RED: `executor_tests::execute_path_uses_compile_then_run`.
- [x] **4.2** GREEN 4.1: 6 new match arms in `MoldQLExecutor::execute` for new variants → `execute_compiled`.
- [x] **4.3** `execute_compiled_pg` (~60 LOC) — run SQL via `SymbolRepository`/sqlx.
- [x] **4.4** `execute_compiled_petgraph` (~60 LOC) — run `PetgraphPlan` over `cognicode_core` graph.
- [x] **4.5** 4 integration tests (path_pg, neighbors_pg, subgraph_petgraph, cluster_petgraph).
- [x] **4.6** Regression: 32 existing tests still pass.

**Gate P4**: 32 + 5 = 37 pass.

---

## Phase 5 — MCP Evolution (PR 3)

- [x] **5.1** RED: `mcp_tests::explorer_query_moldql_accepts_path` — call with `"PATH FROM a TO b"`.
- [x] **5.2** GREEN: extend `TOOL_QUERY_MOLDQL` handler (+25 LOC). Accept optional `target: "pg"|"petgraph"|"auto"` (default auto). Pass to executor.
- [x] **5.3** 3 more tests: neighbors, subgraph, boolean calls.
- [x] **5.4** Update tool docstring at `src/mcp.rs:16` to mention ExplorerQL primitives.
- [x] **5.5** Expose `cognicode_core::graph::CallGraph` to MCP handler (+15 LOC) for petgraph target.

**Gate P5**: 4 new + all existing pass.

---

## Phase 6 — NL Patterns (PR 3)

- [x] **6.1** RED: `pattern_what_connects` — `"what connects A to B?"` → `Path`.
- [x] **6.2** GREEN: 2 new `PatternMatcher` in `ask/patterns.rs` (+15 LOC): (a) what-connects → PATH MAX HOPS 5; (b) show-neighbors → NEIGHBORS BOTH DEPTH 2.
- [x] **6.3** 2 more tests: `show_neighbors`, `explain_cycles` ("explain cycles in X" → `EXPLAIN CYCLES IN X`).
- [x] **6.4** Update `ask/mod.rs` registry if needed.

**Gate P6**: 3 new tests pass.

---

## Phase 7 — End-to-End (PR 3)

- [x] **7.1** `cargo test --workspace` → 32 + 78 + 29 + 5 + 4 + 3 = 151 pass.
- [x] **7.2** `cargo clippy -p cognicode-explorer -- -D warnings` clean.
- [x] **7.3** `cargo fmt --check` clean.
- [x] **7.4** Add `crates/cognicode-explorer/tests/explorerql_e2e.rs`: 8 PG↔petgraph parity tests on fixture graph.
- [x] **7.5** `cargo test -p cognicode-explorer --test explorerql_e2e` → 8 pass.
- [x] **7.6** Update `moldql/mod.rs` doc (line ~7) to mention ExplorerQL superset (+5 LOC).
- [x] **7.7** Add "ExplorerQL" section to `docs/architecture.md` linking to specs (+10 LOC).

**Gate P7**: 159 tests pass, clippy clean, fmt clean, docs updated. Total ~1350 LOC.

---

## Dependency Graph

```
1.1→1.2; 1.3→1.4→1.5; 1.6→1.7   (P1)
        ↓
2.1→2.2; 2.3→2.4; 2.5→2.6; 2.7→2.8; 2.9→2.10; 2.11→2.12; 2.13→2.14; 2.15   (P2, parallel)
        ↓
3.1; 3.2→3.3→3.4→3.5; 3.6→3.7; 3.8→3.9; 3.10→3.11; 3.12→3.13; 3.14→3.15   (P3)
        ↓
4.1→4.2→4.3→4.4→4.5→4.6   (P4)
        ↓
5.1→5.2→5.3→5.4→5.5   (P5)
        ↓
6.1→6.2→6.3→6.4   (P6)
        ↓
7.1→7.2→7.3→7.4→7.5→7.6→7.7   (P7)
```

## TDD Order (STRICT)

Every RED task lands first; commit fails. Next task (its GREEN) makes it pass. Refactor steps only after GREEN. No implementation PR is mergeable with any RED test still failing.

## Validation Commands

| Gate | Command |
|------|---------|
| P1 | `cargo test -p cognicode-explorer moldql::ast moldql::parser moldql::cursor` |
| P2 | `cargo test -p cognicode-explorer moldql::parser_explorerql` |
| P3 | `cargo test -p cognicode-explorer moldql::compile` |
| P4 | `cargo test -p cognicode-explorer moldql::executor` |
| P5 | `cargo test -p cognicode-explorer mcp::` |
| P6 | `cargo test -p cognicode-explorer ask::` |
| P7 | `cargo test --workspace && cargo clippy -p cognicode-explorer -- -D warnings && cargo fmt --check` |

## Risk Hot-Spots

1. **1.4–1.5 Cursor extraction** — zero-behavior refactor; if 32 tests fail, revert and audit `Cursor` usages.
2. **1.7 dispatch widening** — must NOT change FIND/EXPLORE error messages; 32 tests guard.
3. **3.1 petgraph as direct dep** — open question; alternative is plan-based delegation. Reviewer may force swap.
4. **3.7/3.15 SQL injection** — `pg_no_string_interpolation` is a static safety net; PR review must verify.

## Out-of-Scope

Streaming traversal, query planner optimization, frontend UI, full Cypher/GQL compliance, removing legacy `Direction` enum.
