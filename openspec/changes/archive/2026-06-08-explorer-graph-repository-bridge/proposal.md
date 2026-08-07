# Proposal: Repository Trait Consolidation & Metadata-Aware Bridge

## Intent

The `SymbolRepository` trait in `cognicode-explorer` is metadata-free — `callees()` returns bare `RelationTarget` with no provenance or confidence. The adapter's `callees_with_metadata()` exists but sits outside the trait, invisible to downstream consumers (MCP, views). Meanwhile, the `cognicode-store-traits` crate is dead code — zero dependents, identical to `GraphStore` in core. This slice bridges the gap: expose edge trust metadata through the port while cleaning up the duplicated trait.

## Why Now

Phase 1 (`explorer-graph-foundation`, archived) delivered `Provenance` + `confidence` on every `CallGraph` edge. The metadata EXISTS but has no trait-level access path. Unblocking PostgreSQL (next slice) requires a stable metadata-aware Repository trait as its seam. The MCP envelope slice also needs this surface. Delaying means PostgreSQL lands on an unstable foundation.

## Scope

### In Scope
- Remove dead `cognicode-store-traits` crate from workspace
- Add `MetadataAwareRepository` sub-trait on `SymbolRepository` with `callees_with_metadata()`, `dependencies_with_metadata()`, `edges_with_metadata()`
- Add optional `provenance: Option<Provenance>`, `confidence: Option<f64>` to `RelationTarget` (backward-compatible)
- Implement `MetadataAwareRepository` on `CallGraphRepository` (delegates to existing `CallGraph` methods)
- Introduce `#[async_trait]` Repository trait in `cognicode-core` (async-ready, PostgreSQL-targeted; implementation deferred)
- Contract tests for the sub-trait

### Out of Scope
- PostgreSQL adapter (`sqlx`, `PgPool`, schema DDL)
- New node/edge kinds (Component, Container, System, part_of, deployed_as)
- MCP envelope changes
- ExplorerQL grammar, Explorer UI
- `CallGraphV1` removal (defer one more release cycle — auto-grill escalation E1)
- JSON snapshots, `cognicode-mcp` crate changes

## Capabilities

### New Capabilities
- `metadata-aware-repository`: Sub-trait on `SymbolRepository` exposing provenance/confidence per edge, with optional `provenance` + `confidence` fields on `RelationTarget`.
- `repository-trait-core`: Async-ready `Repository` trait in `cognicode-core` extending `GraphStore` with `async` read methods — structurally ready for PostgreSQL.

### Modified Capabilities
- None — additive to existing `SymbolRepository` trait via sub-trait (OCP-compliant).

## Approach

1. **Delete `cognicode-store-traits`** — remove from workspace members, delete crate directory. Zero dependents confirmed via `cognicode_find_usages`.
2. **Sub-trait pattern**: `MetadataAwareRepository: SymbolRepository` — implementors opt in. Mock repos and tests keep implementing the base trait only. H(Δ_existing) = 0.5 bits vs 1.58 bits for direct extension.
3. **Optional metadata on `RelationTarget`**: `provenance: Option<Provenance>`, `confidence: Option<f64>`. Existing code creates `RelationTarget::from(&ResolvedSymbol)` — metadata stays `None`. Only `CallGraphRepository` populates `Some(...)`.
4. **`Repository` trait in core**: Uses `#[async_trait]` (already workspace dep). Extends `GraphStore` with async read methods. PostgreSQL slice implements this trait.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-store-traits/` | Removed | Dead crate — identical `GraphStore`, zero dependents |
| `Cargo.toml` (workspace) | Modified | Remove member + workspace dep |
| `cognicode-explorer/src/ports/symbol_repository.rs` | Modified | +sub-trait, +optional fields on `RelationTarget` |
| `cognicode-explorer/src/adapters/call_graph_repository.rs` | Modified | Implement `MetadataAwareRepository` |
| `cognicode-core/src/domain/traits/repository.rs` | New | Async-ready `Repository` trait |
| `cognicode-core/src/domain/traits/mod.rs` | Modified | Export new trait |
| `cognicode-explorer/tests/metadata_aware_repository.rs` | New | Contract tests |

## Entropy Budget (Protocol B)

**Method**: Heuristic (±1 bit confidence). CogniCode graph build cancelled — using code reading.

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) — files modified | log2(4) ≈ 2.0 bits | < 1.0 | ⚠️ AMBER (foundational) |
| H(Δ_new) — new trait+sub-trait | log2(2) ≈ 1.0 bits | > 0 | ✅ |
| New connascence pairs | 2 (MetadataAwareRepository↔CallGraph, Repository↔GraphStore) | < 3 | ✅ |
| OCP compliant? (sub-trait approach) | Yes — H(Δ)extend ≈ 0.5 bits vs direct ≈ 1.58 bits | yes | ✅ |

**Verdict**: AMBER — 4 files touched pushes H(Δ_existing) above 1.0 bit threshold, but this is expected for foundational trait consolidation. Sub-trait pattern minimizes OCP violation. No critical connascence pairs.

**Design Quality Score (pre-slice)**: ~0.65/1.0 (ACCEPTABLE)

## Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| `RelationTarget` `PartialEq` break with new `Option` fields | Low | `Option<Provenance>` impls `PartialEq`; `Option<f64>` also impls `PartialEq` — derives unchanged |
| `cognicode-store-traits` removal breaks hidden dep | Low | CI gate: `cargo check --workspace` immediately after removal |
| `async_trait` boxing overhead on Repository | Low | PostgreSQL-bound; InMemory impl trivial; boxing acceptable for trait object path |
| Sub-trait discoverability | Low | Document in `cognicode-explorer` crate-level docs; MCP handler doc points to `MetadataAwareRepository` |

**Rollback**: Revert commit. `SymbolRepository` unchanged — zero behavioral change for existing consumers. `cognicode-store-traits` re-addable from git history.

## Success Criteria

- [ ] `cargo check --workspace` passes after removing `cognicode-store-traits`
- [ ] `MetadataAwareRepository` compiles on `CallGraphRepository` with existing `CallGraph` methods
- [ ] `RelationTarget` with optional fields does not break existing test assertions
- [ ] Contract tests pass: metadata-aware methods return correct provenance/confidence
- [ ] Existing 295 tests pass (core 153 + db 23 + explorer 210 + integration 30 + e2e 2)
- [ ] `Repository` trait in core compiles with `#[async_trait]`

## Open Questions

None — all design decisions resolved:
- Sub-trait over widening: resolved (OCP, auto-grill E2 with OS=0.82)
- Eliminate over deprecate `cognicode-store-traits`: resolved (zero dependents)
- `#[async_trait]` over `Box<dyn Future>`: resolved (workspace already depends on async-trait 0.1)
- `CallGraphV1` deferral: resolved (auto-grill E1 with OS=0.78)

The spec phase can proceed immediately.
