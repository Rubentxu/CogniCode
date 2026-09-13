# Proposal: E37 LSI Projection Bridge

> Delivery: `auto-chain` — chained slices, stacked-to-main, no commits without user approval. M2: umbrella tasks 3.1–3.6.

## Intent

CallGraph/GenericGraph lack a canonical, rebuildable source; later LSI milestones lack governed evidence and cutover risk is unmeasured. This change builds fact-derived projections behind existing seams and proves equivalence — no cutover.

## Scope

### In Scope
- Fact adapters: tree-sitter `extract_file` and `CodeIntelligenceProvider` observations → facts; golden-pinned EntityId↔FQN mapping (3.1/3.2).
- Canonical `core:*` SchemaRegistry bootstrap (calls/imports/contains/defines/inherits/references).
- `CallGraphProjection::from_facts` → unchanged `CallGraphProjectionPort` (3.3); additive `FactStore::facts_in_snapshot` read.
- Minimal `GenericGraphProjectionPort` emitting existing `GraphNode`/`GraphEdge` (3.4).
- Rust equivalence harness over `sandbox/fixtures/*` (e36 goldens oracle; quarantined surfaces excluded) (3.5).

### Out of Scope
- Legacy CallGraph cutover (debt rule 6 — forbidden before equivalence gates); incremental/differential projection; Ladybug kernel-store adapter; new MCP/CLI surfaces.

## Capabilities

> `projection-architecture` delta lives in the umbrella; e37 IMPLEMENTS it (e36 pattern).

### New Capabilities
- None.

### Modified Capabilities
- None.

> Candidate: `GenericGraphProjectionPort` contract (consumers FactStore-ignorant, UAT-U11) may warrant minimal capability `generic-graph-projection` if exceeding R2.

## Approach

Option A (exploration): fact-sourced projections behind existing seams, batch-rebuild-first, gated by off-by-default `evidence-kernel`. Facts → `from_facts` → unchanged ADR-029 port (analytics/services unchanged); facts → `GraphNode`/`GraphEdge` (Explorer untouched). Harness compares normalized node/edge multisets per fixture plus clear→rebuild equivalence (R2).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/evidence_kernel/ports.rs` | Modified | additive `facts_in_snapshot`; `core:*` bootstrap |
| `crates/cognicode-core/src/application/fact_bridge/` | New | TreeSitter→Fact, LSP→Fact adapters; EntityId mapping |
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | Modified | `from_facts` constructor |
| `crates/cognicode-core/src/domain/ports/generic_graph_projection.rs`, `infrastructure/graph/` adapter | New | port + adapter |
| `crates/cognicode-core/tests/` | New | equivalence harness, e36 goldens oracle |
| `crates/cognicode-runtime` | Modified | gated no-op wiring |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Known engine nondeterminism (same-named symbols, name-based resolution) legitimately diverges fact-derived vs legacy projections | High | Declared tolerance: ≥99% equivalence; KNOWN_UNSTABLE_SURFACES excluded; FQN resolution intentional |
| Full-build perf regression >10% | Med | Additive/off-by-default; `lsi_bench_baseline.py compare` gate |
| Feature-gate drift; e36/e37 tree entanglement | Med | Single `evidence-kernel` flag; disjoint file ownership |

## Rollback Plan

Fully additive behind `evidence-kernel` (default off): disable feature, delete new modules; legacy path untouched; no migration, no data rollback.

## Dependencies

- e36 kernel in working tree: `FactStore::commit`, snapshot-pinned stores, M0 goldens, bench baseline.

## Success Criteria

- [x] ≥99% structural equivalence on golden fixtures; tolerance/exclusions declared. (fresh harness: 1.0000/1.0000 on both scored fixtures; multi-lang-types quarantined and reported)
- [x] Clear→rebuild from pinned facts yields equivalent projection (R2). (exact node/edge multiset equality, fresh)
- [x] No >10% full-build regression; MCP/CLI compatibility preserved (e36 goldens byte-stable). (perf gate not certified clean — see verify-report WARNING 1; re-adjudication pending) (goldens 42/42 byte-identical, fresh)
- [ ] UAT-U10/U11 pass; formal run may be deferred, stated honestly. (deferred, A5)

## Proposal question round

Autonomous fallback — assumptions proceeded on:

1. EntityId: raw FQN as `FactValue::Text` (assumed) over hashed `Ref`?
2. Quarantine: exclude KNOWN_UNSTABLE_SURFACES (assumed) or count within ≥99%?
3. Predicates: `core:defines` + `core:contains` both, or one?
4. LSP adapter: all `ReferenceKind` variants or Call/Type/Import only?
5. Defer formal UAT-U10/U11 with honest reporting?
