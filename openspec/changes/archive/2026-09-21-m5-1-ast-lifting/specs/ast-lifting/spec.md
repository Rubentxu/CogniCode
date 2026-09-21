# Spec: M5.1 — Lift tree-sitter extraction into FunctionLocalView for M5 conformance

> Change: `m5-1-ast-lifting` | Cycle: `p-c1fac1fea05615c6/m5-1-ast-lifting` | Phase: specify
> Goal: replace M5's synthetic flat-slice fixtures with fixtures sourced from real
> Rust source code via the existing tree-sitter extractor. Closes the "synthetic
> corpus" honesty gap recorded in M5 WU7.

## Scope (M5.1 vs M5.1b)

The current `tree_sitter_facts` extractor emits **node-level + edge-level** facts
(function symbols, `dependency.calls` edges) but does **not** emit statement-level
def/use facts (it does not parse `tree_sitter::Parser` `defines` / `references`
predicates yet). This is a clean separation:

- **M5.1 (this change)** — the lift produces `FunctionLocalView` + call-graph
  adjacency. `interproc_summary` (the only algorithm that needs only the call
  graph) is served end-to-end from real source. `cfg_per_function`,
  `dominators_cfg`, `slice_forward`, `slice_backward`, `taint_flow` still rely on
  the synthetic corpus.
- **M5.1b (deferred)** — extends the extractor to emit statement-level def/use
  facts, then widens the lift + the conformance harness to cover the remaining
  five algorithms from real source.

## Requirements

### REQ-LIFT-01 — `ast_lift::lift(extractions) -> (Vec<FunctionLocalView>, Vec<Vec<usize>>)`

**Given** a slice of `ExtractionResult` (one or more files)
**When** `lift` is called
**Then** it returns:
- `Vec<FunctionLocalView>` — one per function or method symbol, ordered by discovery.
  - `function_id` = index in the returned slice (NOT a FQN).
  - `statements` = empty list (M5.1b will populate it once the extractor emits
    statement-level facts).
  - `calls` = list of indices of other `FunctionLocalView`s this function calls,
    derived from `dependency.calls` edges in the input. Targets outside the lift
    are ignored. Duplicates are removed (input order preserved).
- `Vec<Vec<usize>>` — call graph adjacency, indexed by `function_id`,
  identical to the per-function `calls` list (the adjacency is the symmetric
  return value so the dispatcher can pass it as `call_graph`).

### REQ-LIFT-02 — Function-only filtering (function or method)

**Given** an `ExtractionResult` that contains symbols of any kind (file, function, method, class, variable)
**When** `lift` filters the nodes
**Then** only `NodeKind::Symbol(Function | Method)` nodes contribute to `FunctionLocalView`.
File/class/variable nodes are ignored.
**And** this filter is documented and unit-tested.

### REQ-LIFT-03 — Statement.id is per-function (DEFERRED to M5.1b)

> **Deferred to M5.1b.** M5.1 always emits `statements: Vec::new()`. M5.1b will
> parse statement-level def/use facts from the extractor and assign `Statement.id`
> 0..N-1 per function.

### REQ-LIFT-04 — Real-source acceptance: extract → lift → dispatch succeeds for interproc_summary

**Given** a small inline Rust source fixture (a linear-chain or diamond function)
sourced from a real Rust file
**When** the fixture is fed through `extract_file` → `ast_lift::lift` →
`ProgramAnalysisService::dispatch(interproc_summary, { call_graph, functions })`
**Then** the dispatch returns Ok and produces a JSON body with
`summary_count` equal to the number of lifted functions.
**And** this is asserted in a unit test, so a regression in either the lift OR
the call-graph mapping is caught.

> **Scope note.** REQ-LIFT-04 in M5.1 covers only `interproc_summary`. cfg /
> dominators / slice / taint acceptance from real source is REQ-LIFT-04b in M5.1b
> (once `statements` are populated).

### REQ-LIFT-05 — Real-source corpus coverage

**Given** the 6 algorithm IDs (cfg_per_function, dominators_cfg, slice_forward,
slice_backward, taint_flow, interproc_summary)
**When** the real-source acceptance tests run
**Then** at least `interproc_summary` is covered end-to-end (M5.1).
**And** the remaining five are covered in M5.1b.

### REQ-LIFT-06 — Honest fallback when extraction yields no function nodes

**Given** a source file whose extraction produces zero function symbols
**When** `lift` is called
**Then** it returns an empty `Vec<FunctionLocalView>` and an empty call graph,
**And** no panic occurs, so callers can fall back to the synthetic fixture for
that algorithm.

### REQ-LIFT-07 — No new ports, no domain I/O

**Given** `ast_lift` is purely a value-transform function
**When** the module is implemented
**Then** it does NOT import `tokio`, `sqlx`, `reqwest`, or any I/O crate.
**And** it does NOT add new domain traits or ports.

### REQ-LIFT-08 — Feature-gated behind `program-analysis-server`; lift output is deterministic

**Given** the AST lift depends on the tree-sitter extractor
**When** the conformance harness picks a fixture
**Then** the AST-derived fixtures are only used when the
`program-analysis-server` feature is enabled.
**And** the synthetic fixtures remain available without the feature (so the
35-test M5 + M5.2 suite stays green without the feature).
**And** two extractions of the same source produce byte-identical JSON output
(no time-based fields, no set-iteration non-determinism).

## Scenarios

| ID | Scenario | Requirement |
|---|---|---|
| SCN-LIFT-01 | `lift` produces 1 FunctionLocalView per function/method symbol | REQ-LIFT-01 |
| SCN-LIFT-02 | `lift` ignores non-function/non-method symbols | REQ-LIFT-02 |
| SCN-LIFT-03 | `Statement.id` runs 0..N-1 per function | REQ-LIFT-03 (M5.1b) |
| SCN-LIFT-04 | Real Rust source → lift → dispatch(interproc_summary) returns Ok with summary_count | REQ-LIFT-04 |
| SCN-LIFT-05 | Real source corpus covers interproc_summary in M5.1; remaining 5 in M5.1b | REQ-LIFT-05 |
| SCN-LIFT-06 | Empty source file returns empty view without panic | REQ-LIFT-06 |
| SCN-LIFT-07 | No tokio/sqlx/reqwest imports under `ast_lift` | REQ-LIFT-07 |
| SCN-LIFT-08 | AST fixtures gated behind feature flag; lift output is deterministic | REQ-LIFT-08 |

## Acceptance contract

A new test module `application/program_analysis/ast_lift.rs::tests` MUST:

1. Cover SCN-LIFT-01 / SCN-LIFT-02 with fixture-driven tests (no I/O).
2. Cover SCN-LIFT-04 by lifting inline Rust source through the actual
   `extract_file` extractor and asserting the dispatched `interproc_summary`
   returns `summary_count == 1`.
3. Cover SCN-LIFT-06 with an empty source file.
4. Cover SCN-LIFT-08 with two `#[cfg]`-gated test modules: feature-off
   (synthetic only) and feature-on (synthetic + real).
5. Cover REQ-LIFT-08 determinism with a `lift(a) == lift(a)` test.

The existing 35-test M5 + M5.2 suite MUST remain green both with and without
the feature.

## Coverage delta vs M5.2 close

| Surface | M5.2 close | M5.1 close | Delta |
|---|---|---|---|
| `program_analysis` lib tests | 35 | 49 | +14 |
| `mcp_roundtrip_tests` m5 tests | 8 | 8 | 0 |
| Real-source acceptance coverage | 0 algorithms | 1 algorithm (interproc_summary) | +1 |
| Forbidden I/O imports | 0 | 0 | 0 |
