# Proposal: ExplorerQL Grammar

## Intent

Extend MoldQL (FIND + EXPLORE) with ExplorerQL: a graph-native sub-language for path queries, neighborhood traversal, subgraph extraction, cluster membership, and structural explanation. ExplorerQL replaces current multi-tool orchestration and NL routing hacks for graph questions ("what connects X to Y?", "find low-confidence symbols").

## Scope

### In Scope
- 5 graph primitives: PATH, NEIGHBORS, SUBGRAPH, CLUSTER, EXPLAIN
- Provenance/confidence WHERE filters
- Boolean composition: AND, OR, NOT across primitives
- Dual compilation: PostgreSQL SQL + petgraph
- STRICT superset — all existing MoldQL parses unchanged

### Out of Scope
- Full Cypher/GQL compliance; streaming traversal; query planner optimization; frontend UI

## Capabilities

### New Capabilities
- `explorerql-grammar`: 5 graph primitives with syntax rules
- `explorerql-filters`: provenance/confidence field filtering
- `explorerql-boolean`: AND/OR/NOT clause composition
- `explorerql-compilation`: PostgreSQL + petgraph backends

### Modified Capabilities
None. No existing OpenSpec specs — ExplorerQL is additive syntax.

## Approach

Extend the hand-written recursive-descent parser (858 lines) with 5 clause parsers following existing idioms. Add ExplorerQL AST variants to `MoldQLQuery`. Wire into `explorer_query_moldql` — tool signature stable; dispatch selects MoldQL vs ExplorerQL by leading keyword. Dual compilation in new `moldql/compile.rs`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `moldql/ast.rs` | Modified | New query variants in MoldQLQuery |
| `moldql/parser.rs` | Modified | 5 clause parsers (~400 lines) |
| `moldql/compile.rs` | New | PG + petgraph compilation |
| `moldql/executor.rs` | Modified | Execute compiled queries |
| `src/mcp.rs` | Modified | Evolve explorer_query_moldql |
| `ask/patterns.rs` | Modified | 2–3 NL patterns → ExplorerQL |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Grammar scope creep | Medium | Strict spec contract before parser coding |
| Parser complexity in recursive descent | Low | Extract sub-parsers; existing code already modular |
| PG vs petgraph compilation divergence | Medium | Shared test suite; shared AST→query-builder layer |

## Rollback Plan

ExplorerQL is additive. Revert: remove new AST variants, delete clause parsers, remove compile.rs, revert MCP tool to reject unknown keywords. Existing MoldQL is NEVER touched — rollback is pure reverse-diff.

## Dependencies

- `petgraph` already in Cargo.toml; PostgreSQL adapter in `explorer/src/db.rs`

## Success Criteria

- [ ] 32 existing MoldQL tests pass unchanged
- [ ] Each primitive parses/compiles to PG SQL and petgraph
- [ ] Provenance/confidence filters apply correctly
- [ ] Boolean composition (AND/OR/NOT) works across primitives
- [ ] 2–3 NL patterns route to ExplorerQL

### Entropy Budget

**Method**: Heuristic

| Metric | Estimate | Threshold | Status |
|--------|----------|-----------|--------|
| H(Δ_existing) | 0.92 bits | < 1.0 | ✅ OK |
| H(Δ_new) | 3.46 bits | > 0 | ✅ |
| New connascence pairs | 2 | < 3 | ✅ |
| OCP compliant? | Yes | Yes | ✅ |

4 files modified (parser, AST, executor, MCP) — all additive changes (new variants, match arms). No signature breaks. New `compile.rs` self-contained with clear AST→SQL / AST→petgraph contracts.

**Verdict**: 🟡 ACCEPTABLE — low existing modification cost, new code well-contained.
