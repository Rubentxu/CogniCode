# Design: M5.1b — Statement-level extraction

> Change: `m5-1b-statement-extraction` | Cycle: `p-c1fac1fea05615c6/m5-1b-statement-extraction` | Phase: design

## Architecture overview

```
                   ┌─────────────────────────────┐
                   │ tree-sitter AST (Rust)      │
                   └──────────────┬──────────────┘
                                  │ DFS
                                  ▼
        ┌─────────────────────────────────────────┐
        │ extract_file (existing, M5.1)           │
        │  • nodes   : Vec<GraphNode>             │
        │  • edges   : Vec<ExtractionEdge>        │
        │  + statements_by_function               │ ← NEW (M5.1b)
        │       : BTreeMap<NodeId, Vec<Statement>>│
        └──────────────┬──────────────────────────┘
                       │ ExtractionResult
                       ▼
        ┌─────────────────────────────────────────┐
        │ ast_lift::lift                          │
        │  (Function, calls) → FunctionLocalView  │
        │  + statements from map → .statements    │ ← CHANGED (M5.1b)
        └──────────────┬──────────────────────────┘
                       │ (Vec<FunctionLocalView>, Vec<Vec<usize>>)
                       ▼
        ┌─────────────────────────────────────────┐
        │ ProgramAnalysisService::dispatch        │
        │  (unchanged — dispatcher takes whatever │
        │   statements the lift produces)         │
        └─────────────────────────────────────────┘
```

## Module changes

### 1. `crates/cognicode-core/src/application/ingest/types.rs`

**Change:** add a single field to `ExtractionResult`.

```rust
use std::collections::BTreeMap;
use cognicode_graph_algos::algorithms::Statement;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub path: PathBuf,
    pub content_hash: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<ExtractionEdge>,
    #[serde(default)]
    pub statements_by_function: BTreeMap<NodeId, Vec<Statement>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
```

`BTreeMap` (not `HashMap`) for deterministic JSON serialization — required
by REQ-STMT-04. `#[serde(default)]` keeps M5.1 callers and stored fixtures
serializable across the format bump.

`ExtractionResult::ok` factory grows a 6th parameter (or — preferred — uses
a builder pattern that defaults to empty map).

### 2. `crates/cognicode-core/src/application/ingest/extractor.rs`

**New private function** `extract_statements_from_node`:

```rust
fn extract_statements_from_node(
    function_node: &Node,
    source: &[u8],
) -> Vec<Statement> {
    let mut statements: Vec<Statement> = Vec::new();
    let mut next_id: usize = 0;

    fn walk(
        node: &Node,
        source: &[u8],
        statements: &mut Vec<Statement>,
        next_id: &mut usize,
    ) {
        // ... branch/loop/block: recurse, do not emit
        // ... let/assignment/expr/return: emit one Statement, advance id
    }

    walk(function_node, source, &mut statements, &mut next_id);
    statements
}
```

**Wired into `extract_file`** at the same place `extract_calls_from_node`
is called for a function symbol:

```rust
let stmts = std::panic::catch_unwind(
    std::panic::AssertUnwindSafe(|| {
        extract_statements_from_node(&node, source_bytes)
    })
).unwrap_or_default();

if !stmts.is_empty() {
    result.statements_by_function
        .insert(symbol_id, stmts);
}
```

`catch_unwind` satisfies REQ-STMT-06: malformed function body → empty Vec,
rest of file proceeds (ADR-023 error isolation).

### 3. `crates/cognicode-core/src/application/program_analysis/ast_lift.rs`

**Change:** in the same loop that creates `FunctionLocalView`, copy the
statements from the map if present.

```rust
let mut view = FunctionLocalView {
    function_id: idx,
    calls: Vec::new(),   // populated in pass 2 (M5.1 logic)
    statements: result
        .statements_by_function
        .get(&node.id)
        .cloned()
        .unwrap_or_default(),
};
```

(`node.id: NodeId` is already accessible here — it's the FQN key into the
map.)

The M5.1 `node_to_function_view` helper is updated to take an optional
`Vec<Statement>` argument (or we just inject it post-construction in `lift`).
Injecting post-construction preserves the M5.1 test surface.

## Failure isolation pattern (ADR-023)

The extractor wraps `extract_statements_from_node` in
`std::panic::catch_unwind` — the same pattern used for the semantic handler
post-parse pass at `extractor.rs:208`. A tree-sitter walk panic is
recovered, the function's statements are empty, and the rest of the file
proceeds.

## Determinism (REQ-STMT-04)

Three sources of nondeterminism must be controlled:

1. **`BTreeMap` not `HashMap`** for `statements_by_function` — deterministic
   iteration order in JSON.
2. **`defs` and `uses` are sorted** before insertion into `Statement` —
   same variable seen twice in the same statement collapses to one entry
   in deterministic position.
3. **Statement.id is DFS-order**, not visit-counter — two extractions of
   the same source produce identical ids.

## Test layout

### `extractor.rs::tests::statement_extraction`

7 module-level fixture tests (SCN-STMT-01..07):
- empty body
- `let x = 1;`
- multi-statement chain
- `return` clause
- `if` branch
- Statement.id monotonicity
- byte-identical determinism

### `extractor.rs::tests::statement_extraction::failure_isolation`

1 test (SCN-STMT-09): a syntactically broken function body (curly bracket
mismatch) returns empty Statements without panic.

### `ast_lift.rs::tests::statement_lift`

2 tests (SCN-STMT-08):
- Lift populates `FunctionLocalView.statements` from the map.
- Lift falls back to empty `statements` when the map is empty.

### `ast_lift.rs::tests::real_source::conformance_acceptance`

5 new tests (REQ-STMT-08): one per deferred algorithm. Each lifts an
inline Rust source fixture (linear chain for cfg, diamond for dominators,
let-chain for slice_forward, return-from for slice_backward, source-sink
for taint) and asserts the dispatched algorithm's digest matches the
synthetic baseline.

## Files touched

| File | LOC delta | Kind |
|---|---|---|
| `application/ingest/types.rs` | +5 | MODIFY (1 field + 1 import) |
| `application/ingest/extractor.rs` | +150 | MODIFY (1 new fn + 2 call sites + 2 tests) |
| `application/program_analysis/ast_lift.rs` | +20 | MODIFY (statements injection + 2 tests) |
| `application/program_analysis/ast_lift.rs::tests::real_source` | +150 | MODIFY (5 conformance tests) |

**Total: ~325 LOC, 3 stacked-to-main commits.**

## Architectural invariants reaffirmed

- **Hexagonal:** `application/ingest` depends on `domain::aggregates` + `cognicode-graph-algos`; no I/O. ✓
- **Statement type reused** from `cognicode-graph-algos` — no new domain types. ✓
- **No new ports.** ✓
- **ADR-023 error isolation** — `catch_unwind` in extractor, empty Vec fall-through. ✓
- **Determinism** — `BTreeMap`, sorted `defs`/`uses`, DFS-order ids. ✓

## Decision

Proceed to **tasks** with the contracted WU sequence above.
