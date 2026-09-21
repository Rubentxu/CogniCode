# Design: M5.1 — Lift tree-sitter extraction into FunctionLocalView

> Change: `m5-1-ast-lifting` | Cycle: `p-c1fac1fea05615c6/m5-1-ast-lifting` | Phase: design

## Architecture (additive; pure value transform)

```
┌────────────────────────────┐
│  crates/cognicode-core/    │
│  src/application/          │
│  ingest/extractor.rs       │
│  extract_one(FileChange)   │
│  → ExtractionResult        │
└────────────┬───────────────┘
             ▼
┌────────────────────────────┐
│  NEW: crates/cognicode-    │
│  core/src/application/     │
│  program_analysis/         │
│  ast_lift.rs               │
│                            │
│  pub fn lift(              │
│    &[ExtractionResult]     │
│  ) -> (                    │
│    Vec<FunctionLocalView>, │
│    Vec<Vec<usize>>         │
│  )                         │
└────────────┬───────────────┘
             ▼
┌────────────────────────────┐
│  existing M5 dispatch      │
│  ProgramAnalysisService    │
│  ::dispatch(id, params,    │
│   limits)                  │
│  params["functions"] =     │
│   lifted FunctionLocalView[]│
│  params["call_graph"] =    │
│   lifted adjacency         │
└────────────────────────────┘
```

## Decisions

### D1 — Pure value transform, no I/O

**Choice**: `ast_lift::lift` is a pure function over `&[ExtractionResult]` →
`(Vec<FunctionLocalView>, Vec<Vec<usize>>)`. No filesystem, no parser, no
network. Caller is responsible for producing `ExtractionResult`s (typically
via `extract_one` in tests, or via the existing `ingest::extract_stage` in
production).
**Why**: keeps the lift deterministic + testable; mirrors the shape of
`compute_summaries(&call_graph, &functions)` in the graph-algos crate.

### D2 — FunctionLocalView.statements derived from `properties`, not from a fresh tree-sitter parse

**Choice**: when an extractor node has `properties.defines` and
`properties.references` arrays, lift them into `Statement { id, defs, uses }`.
If absent (the current extractor doesn't populate them yet), produce an
empty `statements` list and document it as a known constraint.
**Why**: avoids re-walking the source file; defers tree-sitter statement
extraction to a follow-up cycle (M5.1 just plumbing; M5.1b the real
defs/uses extraction).
**Honest**: documented in the module's docs; the existing synthetic corpus
already covers statement-level shape so no test regression.

### D3 — FunctionLocalView.calls derived from `dependency.calls` edges

**Choice**: scan the input `ExtractionResult.edges` for `kind ==
"dependency.calls"`. For each edge, map `source` → `FunctionLocalView`
index (via a name → index table built from the nodes), and append the
target index to `calls`. Cross-file unresolved callees (TargetRef::Unresolved)
are kept as-is and surfaced in the lift via a separate `unresolved_calls`
field on a sibling struct (out of scope; skip them for M5.1).
**Why**: matches the algorithm-side shape (`FunctionLocalView.calls: Vec<usize>`)
without forcing cross-file resolution.

### D4 — Real-source fixtures are inline `&str`s

**Choice**: embed 4-7 small Rust source snippets as `&'static str` constants
in the `ast_lift` test module, parse them in-test via
`application::ingest::extractor::extract_one`.
**Why**: zero temp-dir setup, instant test execution, no fixtures in
`sandbox/`. Aligns with how `conformance::canonical_corpus()` already
inlines its fixtures.

### D5 — Feature-gate behind `program-analysis-server`

**Choice**: the AST lift module is `pub` always, but the AST-derived
fixtures in `conformance` and the test that exercises them are
`#[cfg(feature = "program-analysis-server")]`-gated. The synthetic
fixtures stay on by default.
**Why**: M5 baseline (22 tests) and M5.2 MCP wiring (8 tests) must
remain green without the feature; the AST fixtures are bonus coverage
that depends on the extractor being compiled in.

### D6 — Reuse the canonical corpus's digests as the acceptance target

**Choice**: the acceptance test in REQ-LIFT-04 runs the canonical synthetic
fixture through `ProgramAnalysisService::dispatch` AND the lifted real-source
view through the same dispatcher, then asserts the digests match.
**Why**: functional-equivalence proof — the real-source view produces the
same algorithm output as the synthetic view, modulo the known
statements[] limitation in D2 (which doesn't affect cfg/dominators/
slice-by-function-id/slice-by-uses results for the chosen source).

## Component → file mapping

| Component | File | Status |
|---|---|---|
| Lift module | `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` | NEW (≤ 200 LOC) |
| Module decl | `crates/cognicode-core/src/application/program_analysis.rs` | MODIFY (+1 line) |
| Real-source fixtures (tests) | `ast_lift.rs::tests::real_source_*` | NEW (≤ 200 LOC tests) |
| Acceptance test | `ast_lift.rs::tests::test_real_source_digest_matches_synthetic` | NEW (≤ 50 LOC) |

## Risks & mitigations

- **R1** — `extract_one` requires `tokio::runtime::Handle` if the parser
  runs async. Mitigation: use the sync path inside the test (or mark the
  test `#[tokio::test]` if it isn't already).
- **R2** — the extractor emits unresolved cross-file callees, which
  would corrupt `function_id` indices. Mitigation: only resolve
  calls whose target is a function symbol IN THE SAME EXTRACTION (D3).
- **R3** — empty `statements[]` (D2) means taint and slice outputs may
  not match the synthetic corpus exactly. Mitigation: pick source
  fixtures whose algorithm output is invariant to statements (e.g. cfg
  ignores statements; dominators too). Document this in REQ-LIFT-04.

## Acceptance gate

| Requirement | Check |
|---|---|
| REQ-LIFT-01 | `lift` unit test asserts return shape |
| REQ-LIFT-02 | `lift` unit test with mixed-kind nodes asserts function-only |
| REQ-LIFT-03 | `lift` unit test with multi-function file asserts per-function 0..N |
| REQ-LIFT-04 | acceptance test: real source + synthetic digest equality |
| REQ-LIFT-05 | acceptance test counts covered algorithm shapes (≥4) |
| REQ-LIFT-06 | empty-source unit test asserts no panic + empty result |
| REQ-LIFT-07 | grep `tokio`/`sqlx`/`reqwest` under `ast_lift` = 0 matches |
| REQ-LIFT-08 | both `#[cfg]` test modules present + both green |

Total: 35 program_analysis tests + 8 mcp_m5_roundtrip tests still green
both with and without `program-analysis-server`; new AST tests only run
with the feature.
