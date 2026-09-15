# Spec: M5.1b — Statement-level extraction to enable all 6 algorithm shapes from real source

> Change: `m5-1b-statement-extraction` | Cycle: `p-c1fac1fea05615c6/m5-1b-statement-extraction` | Phase: specify
> Goal: extend the tree-sitter extractor + the AST lift to emit per-function `Vec<Statement>` (defs + uses) so the conformance harness can cover all 6 algorithm shapes (`cfg_per_function`, `dominators_cfg`, `slice_forward`, `slice_backward`, `taint_flow`, `interproc_summary`) from real Rust source — closing the M5.1 REQ-LIFT-03 deferral.

## Scope (M5.1b)

Rust-only. The M5.1 conformance corpus is Rust. Generalized multi-language
statement extraction (TS/Go/Java/Python) is a separate cycle.

## Requirements

### REQ-STMT-01 — `ExtractionResult` grows `statements_by_function`

**Given** an existing `ExtractionResult`
**When** M5.1b is applied
**Then** the result has a new field:

```rust
#[serde(default)]
pub statements_by_function: BTreeMap<NodeId, Vec<Statement>>,
```

where `Statement` is the existing `cognicode_graph_algos::algorithms::Statement`
(re-used, no new type). The field defaults to an empty map when missing on
deserialization (`#[serde(default)]`). The map is keyed by the function's
`NodeId` (FQN string).

### REQ-STMT-02 — `extract_statements_from_node` walker

**Given** a tree-sitter `Node` representing a function/method body and a
source buffer
**When** the walker is invoked
**Then** it returns a `Vec<Statement>` indexed 0..N-1 in source order, where
each statement's `defs` is a sorted, deduplicated `Vec<String>` of variable
names bound by the statement, and `uses` is the same for variable names
referenced.

The walker handles these Rust tree-sitter node kinds:

| Kind | Statement model |
|---|---|
| `let_declaration` | def: identifiers in the `pattern` child; uses: identifiers in the `value` child |
| `assignment_expression` | def: identifier on the lhs; uses: identifiers in the rhs |
| `expression_statement` | uses: all identifiers in the expression |
| `return_expression` | uses: all identifiers except `return` keyword |
| `if_expression` / `match_expression` / `match_arm` | recurse into condition + branches + arms; emit one Statement per contained leaf statement |
| `for_expression` / `while_expression` | uses: identifiers in the iterable/condition; recurse into body |
| `block` | recurse into children (DFS, source order) |

Branches, loops, and blocks do **not** emit a Statement of their own; their
contents do. This keeps `Statement.id` per-function linear and deterministic.

### REQ-STMT-03 — Per-function statement.id is 0..N-1

**Given** a function with N statements
**When** `extract_statements_from_node` runs
**Then** `Statement.id` runs 0..N-1 in source-line order (matches the DFS
child-visit order). This satisfies REQ-LIFT-03 from M5.1.

### REQ-STMT-04 — Determinism: same source → same `Vec<Statement>`

**Given** two consecutive calls to `extract_file(RUST_CONFIG, path, source, hash)`
**When** both calls complete successfully
**Then** their `statements_by_function` are byte-equal after JSON
serialization. Same invariant as REQ-LIFT-08 from M5.1.

### REQ-STMT-05 — Lift populates `FunctionLocalView.statements`

**Given** an `ExtractionResult` with non-empty `statements_by_function`
**When** `ast_lift::lift(&[result])` runs
**Then** each lifted `FunctionLocalView` carries the `Vec<Statement>` keyed
by the function's `NodeId`. If the map is empty for a function (e.g. the
function body could not be walked), `statements` remains `Vec::new()`.

### REQ-STMT-06 — Failure isolation per ADR-023

**Given** a function whose body has malformed Rust syntax (rare but possible
in partially-edited files)
**When** `extract_statements_from_node` is invoked
**Then** it returns `Vec::new()` for that function (no panic). The rest of the
file's functions continue to extract normally.

### REQ-STMT-07 — No domain I/O, no new ports

**Given** the change is in `application/ingest/extractor.rs` + `application/program_analysis/ast_lift.rs`
**When** the change is applied
**Then** it does not import `tokio`/`sqlx`/`reqwest`; it does not add new
domain traits or ports; the `Statement` type re-used from `cognicode-graph-algos`
is unchanged.

### REQ-STMT-08 — All 6 algorithm shapes covered by real source

**Given** the M5.1 conformance corpus + the M5.1b-extracted real source
**When** the conformance harness runs
**Then** all 6 algorithm IDs are tested end-to-end from `extract_file` →
`lift` → `dispatch`:
- `cfg_per_function`
- `dominators_cfg`
- `slice_forward`
- `slice_backward`
- `taint_flow`
- `interproc_summary`

For each algorithm, the digest from the lifted real source is compared
against the synthetic corpus baseline digest (same shape as M5.1's
`real_source_digest_for_interproc_matches_synthetic` test, generalized).

## Scenarios

| ID | Scenario | Requirement |
|---|---|---|
| SCN-STMT-01 | Empty function body → `Vec::new()` | REQ-STMT-02 |
| SCN-STMT-02 | `let x = 1;` → 1 Statement with `defs=[x]`, `uses=[]` | REQ-STMT-02 |
| SCN-STMT-03 | `let y = x + 1;` after `let x = 1;` → 2 statements; second has `defs=[y]`, `uses=[x]` | REQ-STMT-02 |
| SCN-STMT-04 | `return x + y;` → 1 Statement with `defs=[]`, `uses=[x, y]` (sorted) | REQ-STMT-02 |
| SCN-STMT-05 | `if cond { use(z); }` → 1 leaf Statement (`use(z)`); the `if` itself emits nothing | REQ-STMT-02 |
| SCN-STMT-06 | Statement.id runs 0..N-1 in source order | REQ-STMT-03 |
| SCN-STMT-07 | Two extractions of same source → byte-identical `Vec<Statement>` | REQ-STMT-04 |
| SCN-STMT-08 | Lift carries Statements into `FunctionLocalView.statements` | REQ-STMT-05 |
| SCN-STMT-09 | Malformed function body → empty Statements for that function, rest of file proceeds | REQ-STMT-06 |

## Acceptance contract

`ast_lift` and `extractor` test modules MUST:

1. Cover SCN-STMT-01..09 with both synthetic (fixture-driven) tests and
   real-source (gated `#[cfg(feature = "program-analysis-server")]`) tests.
2. Add 5 new conformance acceptance tests (one per deferred algorithm), each
   lifting an inline Rust source fixture (linear chain, diamond, branch,
   loop, taint source + sink), dispatching to the algorithm, and asserting
   the digest matches the synthetic baseline.
3. Update M5.1's `real_source_lift_mirrors_canonical_corpus_shape` test
   comment to reflect that `statements` is now populated.
4. The existing 35-test M5 + M5.2 + 49-test M5.1 suite MUST remain green.

## Coverage delta vs M5.1 close

| Surface | M5.1 close | M5.1b close | Δ |
|---|---|---|---|
| `program_analysis` lib tests | 49 | 49 + ~9 (extractor stmt tests) + ~5 (lift stmt tests) + ~5 (conformance) | +~19 |
| Real-source algorithm coverage | 1 of 6 | 6 of 6 | +5 |
| Forbidden I/O imports | 0 | 0 | 0 |
| Synthetic corpus (retained) | 7 fixtures | 7 fixtures | 0 |

## Out of scope

- Multi-language statement extraction (TS/Go/Java/Python). Deferred to a
  later cycle.
- Control-flow-aware statement grouping (e.g. basic blocks). The current
  `Statement` model is flat per-function; CFG-sensitive grouping is a
  `cfg_per_function` algorithm concern, not an extractor concern.
- Type-aware def/use (e.g. field reads vs function calls). Both emit as
  `uses: Vec<String>` for now; richer typing is deferred.
