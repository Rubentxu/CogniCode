# Proposal: MoldQL Pattern Profile v1

## Intent
Deliver ADR-014 §2's read-only Pattern Profile — the first GQL/openCypher-inspired surface over the E28.1 `GraphPlan` algebra. Today graph primitives compile to PG/snapshot but the user-facing language only exposes ExplorerQL clauses (`PATH`, `NEIGHBORS`, …) with flat results. v1 adds typed patterns, bounded quantifiers, predicates, typed result shapes, aggregation/ordering/limits, and bounded shortest paths — gated behind a **freeze until `e28-2-runtime-closure` passes**, and never claiming Cypher/GQL compatibility.

## Scope

### In Scope
- Typed node + relationship patterns; direction (incoming/outgoing/both).
- Bounded path quantifiers (`*1..N`, `+`, `?` mapped to existing `PathQuantifier` with `max_hops: Some`).
- Property / provenance / confidence predicates.
- Typed row / node / edge / path result projections.
- Aggregation, ordering, limits; bounded shortest paths.
- **Published supported-feature matrix.**
- REST / MCP / Explorer interaction parity (ADR-012 surfaces).

### Out of Scope
- Mutations; unbounded variable-length paths.
- Cypher / openCypher / ISO GQL compatibility claims.
- Analytics registry (E28.4), Neo4j production, WASM canonical executor.

## Capabilities

### New Capabilities
- `moldql-pattern-profile`: grammar, lowering to `GraphPlan`, typed results, feature matrix, parity surfaces.

### Modified Capabilities
- `explorerql-grammar`: accept pattern syntax as new dispatch; existing 7 keywords parse unchanged.
- `explorerql-filters`: property predicates beyond provenance/confidence (typed field/value).
- `explorerql-targets`: typed node labels for pattern anchors.
- `moldql`: intent lowering of lowercase pattern fragments.

## Approach
Layer syntax over the shipped E28.1 algebra without rewriting the
`GraphPlan` contract; extend `PathProjection`/`PathPredicate` only if gap
analysis proves necessary. Parser is a strict superset. Unsupported constructs
fail before execution (`UnsupportedConstruct`). New `PlanVersion` bump; result
`ProvenanceEnvelope` inherited.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `cognicode-explorer/src/moldql/parser*`, `compile.rs` | Modified | Pattern parse + lowering over `GraphPlan` |
| `cognicode-core/src/domain/plan/graph_plan.rs` | Preserved | Shipped E28.2 contract; extend only through an explicit delta |
| `crates/cognicode-mcp`, REST handlers | Modified | Parity surfaces |
| `openspec/specs/moldql-pattern-profile/` | New | Feature matrix + result contracts |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Expand before E28.2 equivalence holds | Med | Hard gate: no merge until `e28-2-runtime-closure` green |
| `GraphPlan` contract drift | Low | Extend only through an explicit versioned delta |
| Implicit Cypher claim | Med | Feature matrix + zero-compat language enforced |

## Entropy Budget
- Target Design Quality ≥ 0.72. Connascence: add meaning-based pattern AST → `GraphPlan` (Connascence of Name, acceptable, single mapping).
- SOLID: pattern profile = one adapter (SRP); OCP via superset grammar, no legacy rewrite.
- Information Bottleneck: pattern AST interface ≤ 7 public methods.

## Rollback Plan
Profile is feature-flagged behind the pattern keyword set. Rollback = disable keyword dispatch in `parse()` → existing ExplorerQL path restored unchanged. Versioned via `PlanVersion`; old serialized queries re-resolve to pre-profile grammar. No schema migration; results are ephemeral/read-only — nothing to un-migrate.

## Dependencies
- **`e28-2-runtime-closure` MUST pass first** (PR4 conformance + equivalence green).
- ADR-014 (PROPOSED → accepted trigger).
- Shipped `GraphPlan` contract remains unchanged or is extended by an explicit delta.

## Success Criteria
- [ ] Supported-feature matrix published; zero Cypher/GQL compatibility claim.
- [ ] Parser, lowering, differential (PG↔snapshot), REST/MCP, Explorer interaction tests pass.
- [ ] No mutation; no unbounded path accepted (rejected with typed error).
- [ ] Shipped `graph_plan.rs` contract preserved (no in-place rewrite).
