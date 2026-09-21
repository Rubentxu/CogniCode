# Exploration Report — M5.1b — Statement-level extraction

> Cycle: `p-c1fac1fea05615c6/m5-1b-statement-extraction`
> Phase: explore
> Date: 2026-09-14

## Problem statement

M5.1 (`m5-1-ast-lifting`) shipped a pure value-transform lift that maps
`ExtractionResult[]` → `(Vec<FunctionLocalView>, Vec<Vec<usize>>)` for real
source. It serves `interproc_summary` end-to-end but explicitly defers the
remaining 5 algorithms (`cfg_per_function`, `dominators_cfg`, `slice_forward`,
`slice_backward`, `taint_flow`) to M5.1b because the `tree_sitter_facts`
extractor does not emit statement-level def/use facts.

M5.1b closes this gap: emit `Vec<Statement>` per function from tree-sitter,
populate `FunctionLocalView.statements`, and enable conformance coverage of
all 6 algorithm shapes from real source.

## Codebase reconnaissance

### Statement type (`cognicode-graph-algos`)

`crates/cognicode-graph-algos/src/algorithms/dfg.rs:34` defines:

```rust
pub struct Statement {
    pub id: usize,
    pub kind: String,
    pub defs: Vec<String>,
    pub uses: Vec<String>,
}
```

`Statement` already serializes/deserializes (`serde::Serialize + Deserialize`),
so the lift can populate `FunctionLocalView.statements: Vec<Statement>` via a
plain JSON round-trip or direct struct copy.

### Extractor (`application/ingest/extractor.rs`)

`extract_file` performs an iterative DFS over the tree-sitter AST. For each
function-symbol node it calls `extract_calls_from_node` to emit `dependency.calls`
edges. There is **no** analogous statement walker.

Per-language AST shape (Rust, the M5.1 default):

| Tree-sitter node kind | Statement intent |
|---|---|
| `let_declaration` | def: binding-pattern identifiers |
| `assignment_expression` | def: lhs identifier; uses: rhs identifiers |
| `expression_statement` | uses: all identifiers in the expression |
| `return_expression` | uses: identifiers in the return value |
| `if_expression` | branch; recurse into condition + then + else |
| `match_expression` / `match_arm` | branch; recurse into scrutinee + arm bodies |
| `for_expression` / `while_expression` / `loop_expression` | loop header (uses); recurse into body |
| `block` | recurse into statements |

### ExtractionResult shape (`application/ingest/types.rs`)

`ExtractionResult { nodes, edges, error, ... }` has no per-function
statement container. Two viable shapes:

- **Option A (preferred):** grow `ExtractionResult` with a third field
  `statements_by_function: BTreeMap<String, Vec<Statement>>` keyed by the
  function's symbol `NodeId`. Zero-cost for callers that don't read it
  (empty map default).
- **Option B:** emit new edge kinds `data_flow.defines` / `data_flow.uses`
  and let the lift rebuild `Statement[]` from the edge stream. Backward
  compatible but double-bookkeeping.

**Decision:** Option A — explicit per-function container is more discoverable,
testable, and keeps the lift pure.

### Lift (`application/program_analysis/ast_lift.rs`)

The lift currently produces `FunctionLocalView.statements = Vec::new()` for
every lifted function. M5.1b's lift change is a 3-line extension: when
`ExtractionResult.statements_by_function` is non-empty, copy the entry keyed
by the function's node-id into `FunctionLocalView.statements`. The existing
`lift_assigns_function_id_in_discovery_order` test + `lift_skips_unresolved_calls`
test continue to apply (they don't touch statements).

### Dispatcher (`application/program_analysis.rs`)

`ProgramAnalysisService::dispatch(interproc_summary, ...)` already passes
`functions: Vec<FunctionLocalView>` (including `statements`) into the
algorithm. After M5.1b, the same path serves `cfg_per_function`,
`slice_forward`, etc. — no dispatcher change required.

## Risk analysis

| Risk | Severity | Mitigation |
|---|---|---|
| Per-language statement extraction shapes diverge | medium | Scope M5.1b to **Rust only** (the M5.1 default + the M5.1 conformance corpus is Rust); generalize in a later multi-language cycle |
| Statement-id assignment must be per-function, 0..N-1 (per REQ-LIFT-03) | low | Count statements as we walk; reset per function |
| `Statement` ordering must be deterministic across extractions | medium | Iterator order matches tree-sitter DFS; statements indexed in source-line order; no sorting required post-walk |
| Tree-sitter node-kind mismatch on syntax errors | low | Wrap per-statement walk in `try-catch` (return Partial Vec on parse error) — fail-soft per ADR-023 |
| New `ExtractionResult` field breaks external consumers | medium | Field is `#[serde(default)]` with empty map default; all existing serialization tests still pass |

## Acceptance boundary

After M5.1b, the conformance corpus expands from **1 of 6 algorithm shapes**
covered by real source (M5.1: `interproc_summary`) to **all 6 of 6**:
`cfg_per_function`, `dominators_cfg`, `slice_forward`, `slice_backward`,
`taint_flow`, `interproc_summary` — each tested via `extract_file` →
`ast_lift::lift` → `ProgramAnalysisService::dispatch` and digest-compared
against the synthetic corpus baseline.

## Scope estimate

- ~150 LOC: `extract_statements_from_node` walker in `extractor.rs` (Rust-only)
- ~30 LOC: `ExtractionResult` field + serde default + 2 unit tests
- ~15 LOC: `ast_lift.rs` change to populate `statements` from the new field
- ~80 LOC: 5 new conformance acceptance tests (one per remaining algorithm)
- ~25 LOC: change artifacts (spec.md delta + design.md delta + tasks.md)

**Total ~300 LOC, 3 stacked-to-main commits.** A-lite path appropriate.

## Architectural invariants reaffirmed

- Domain layer (`crates/cognicode-core/src/domain/`) does NOT import I/O crates (AGENTS.md).
- Statements flow through the existing `Statement` type in `cognicode-graph-algos` (no new domain types).
- No new ports, no new dependencies.
- Failure isolation per ADR-023: malformed function body → empty `Vec<Statement>` for that function, rest of file proceeds.

## Decision

Proceed to **specify** with the following contracted shape:

1. `ExtractionResult` grows `statements_by_function: BTreeMap<NodeId, Vec<Statement>>` (serde-default empty map).
2. New private function `extract_statements_from_node(function_node, source_bytes) -> Vec<Statement>` walks the function body.
3. `lift()` populates `FunctionLocalView.statements` from the map.
4. Five new acceptance tests prove all 5 deferred algorithms now run on real source.
