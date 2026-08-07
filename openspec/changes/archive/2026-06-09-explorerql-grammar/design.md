# Design: ExplorerQL Grammar

## Technical Approach

Extend the existing hand-written recursive-descent parser by extracting the shared `Cursor` into a submodule, then adding 5 clause parsers in a new `parser_explorerql.rs`. Dual compilation (PG SQL + petgraph plan) lives in a new `compile.rs`. The `explorer_query_moldql` MCP tool signature is unchanged — dispatch widens to accept 7 leading keywords. All existing MoldQL tests pass untouched; new tests target ExplorerQL exclusively.

## Architecture Decisions

### Decision: AST Extension Strategy

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Add 5 variants to `MoldQLQuery` + `Boolean(BooleanQuery)` | Simple, single enum. Superset guarantee trivial | **Chosen** |
| Wrapper enum `ExplorerQLQuery` with its own primitives | Isolation but double-dispatch everywhere | Rejected — violates superset contract |

**Rationale**: The spec mandates ExplorerQL is a strict superset. A single `MoldQLQuery` enum makes `parse()` return one type and the compiler/executor match exhaustively. No newtype wrapping needed.

### Decision: Cursor Sharing

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Extract `Cursor` to `moldql/cursor.rs` (pub(crate)) | Clean separation, reusable | **Chosen** |
| Keep `Cursor` private in `parser.rs`, duplicate for ExplorerQL | No refactor, but code duplication | Rejected |
| Keep everything in one 1250-line `parser.rs` | Simple but unmaintainable | Rejected |

**Rationale**: `Cursor` is 110 lines of byte-index machinery. Both `parser.rs` (FIND/EXPLORE) and `parser_explorerql.rs` (5 new clauses) need it. Extracting is a zero-risk refactor — no API change.

### Decision: TraversalDirection vs Direction

| Option | Tradeoff | Decision |
|--------|----------|----------|
| New `TraversalDirection { Incoming, Outgoing, Both }` alongside existing `Direction { Callers, Callees }` | No legacy break, clean separation | **Chosen** |
| Extend `Direction` with `Both` | One type, but `EXPLORE` callers must handle new variant | Rejected — breaks existing match arms |

**Rationale**: Existing `Direction::Callers/Callees` is used by `ExploreQuery` and executor. Adding `Both` forces every match on `Direction` to update. New `TraversalDirection` sidesteps this entirely.

### Decision: Compilation as Separate Module

| Option | Tradeoff | Decision |
|--------|----------|----------|
| New `moldql/compile.rs` with `compile(query, target)` | Clean, testable in isolation | **Chosen** |
| Extend `executor.rs` with compile logic | Couples execution and compilation | Rejected |

**Rationale**: The spec requires dual targets (PG SQL + petgraph plans). The executor runs queries; the compiler translates AST to plans. These are separate concerns. The executor will call `compile()` when it encounters ExplorerQL variants.

### Decision: Boolean AST Shape

`BooleanQuery { left: Box<MoldQLQuery>, op: BooleanOp, right: Box<MoldQLQuery> }` for AND/OR; `Not(Box<MoldQLQuery>)` for NOT. Left-associative. Parens override precedence during parsing.

## Data Flow

```
str ── parse ──► MoldQLQuery (AST)
                  │
         ┌────────┴────────┐
         │                  │
    FIND/EXPLORE      PATH/NEIGHBORS/SUBGRAPH/CLUSTER/EXPLAIN
         │                  │
    executor.rs         compile(query, target)
         │              ┌───┴───┐
         │         PostgresPlan  PetgraphPlan
         │              │       │
         └──────────────┴───────┘
                        │
                   MCP response
```

## File Changes

| File | Action | Lines | Description |
|------|--------|-------|-------------|
| `moldql/ast.rs` | Modify | +120 | 5 query structs, `BooleanQuery`, `TraversalDirection`, `ClusterMethod`, `BooleanOp` |
| `moldql/cursor.rs` | Create | 110 | Extract `Cursor` from parser.rs (zero-risk refactor) |
| `moldql/parser.rs` | Modify | -100/+15 | Remove Cursor, update `parse_query` to call explorerql dispatch |
| `moldql/parser_explorerql.rs` | Create | 400 | 5 clause parsers + boolean + ExplorerQL WHERE |
| `moldql/compile.rs` | Create | 500 | `compile()`, `CompileTarget`, `CompiledQuery`, PG emit fns, petgraph plans |
| `moldql/mod.rs` | Modify | +8 | Add `cursor`, `parser_explorerql`, `compile` modules + re-exports |
| `moldql/executor.rs` | Modify | +180 | ExplorerQL match arms calling `compile()` then executing plans |
| `src/mcp.rs` | Modify | +25 | Evolve TOOL_QUERY_MOLDQL to pass `graph` for petgraph target |
| `ask/patterns.rs` | Modify | +15 | 2-3 NL patterns routing to ExplorerQL primitives |
| `Cargo.toml` | Modify | +1 | Add `petgraph` as direct dependency |

## Interfaces / Contracts

```rust
// ast.rs additions
pub enum MoldQLQuery {
    Find(FindQuery),
    Explore(ExploreQuery),
    // NEW:
    Path(PathQuery),
    Neighbors(NeighborsQuery),
    Subgraph(SubgraphQuery),
    Cluster(ClusterQuery),
    Explain(ExplainQuery),
    Boolean(BooleanQuery),
}

pub struct PathQuery {
    pub from: String, pub to: String,
    pub max_hops: Option<u32>,
    pub conditions: Vec<Condition>,
}

pub enum TraversalDirection { Incoming, Outgoing, Both }
pub enum ClusterMethod { Scc, Connected }
pub enum BooleanOp { And, Or }

pub struct BooleanQuery {
    pub op: BooleanOp,
    pub left: Box<MoldQLQuery>,
    pub right: Box<MoldQLQuery>,
}

// compile.rs
pub enum CompileTarget { Postgres, Petgraph }
pub enum CompiledQuery { /* PG plans + petgraph plans + Composed */ }

pub fn compile(query: &MoldQLQuery, target: CompileTarget)
    -> Result<CompiledQuery, CompileError>;
```

## Testing Strategy

| Layer | What to Test | Approach | Count |
|-------|-------------|----------|-------|
| Unit | 5 primitive parses + errors | `#[cfg(test)] mod explorerql_tests` in `parser_explorerql.rs` | 23 |
| Unit | Filter parses (provenance, confidence) | Same module | 11+1 |
| Unit | Boolean composition + precedence | Same module | 12+1 |
| Unit | PG SQL emission per primitive | `compile.rs` tests | 20 |
| Integration | PG vs petgraph parity on fixture graph | `compile_parity.rs` | 8 |
| Static | No string interpolation in SQL output | `compile.rs` test | 1 |
| Regression | All 32 existing MoldQL tests pass | No changes needed | 32 |

**TDD RED Gate**: All 76+ new tests written first, must FAIL, then implementation begins.

## Migration / Rollout

No migration required. ExplorerQL is purely additive — no existing types change signature, no data migration, no feature flags needed. Rollback is a reverse diff.

## Open Questions

- [ ] Should `petgraph` become a direct dep of `cognicode-explorer` or should compilation return a plan that `cognicode-core` executes?
- [ ] Provenance source validation: compile-time (const array) vs runtime lookup?
