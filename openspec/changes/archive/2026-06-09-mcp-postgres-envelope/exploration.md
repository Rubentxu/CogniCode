# Exploration: mcp-postgres-envelope — Exposing Provenance + Confidence through MCP

## Current State

The `CallGraph` aggregate (in `cognicode-core`) stores per-edge `(Provenance, f64)` metadata on every call-edge since Phase 1 (`explorer-graph-foundation`). The edge storage uses `HashMap<(SymbolId, DependencyType), (Provenance, f64)>`.

The `MetadataAwareRepository` sub-trait (in `cognicode-explorer/src/ports/symbol_repository.rs`) exposes three metadata-carrying methods:
- `callees_with_metadata(id) -> Vec<RelationTargetWithMetadata>`
- `dependencies_with_metadata(id) -> Vec<RelationTargetWithMetadata>`
- `edges_with_metadata() -> Vec<EdgeWithMetadata>`

`CallGraphRepository` fully implements `MetadataAwareRepository`. The enriched types (`RelationTargetWithMetadata`, `EdgeWithMetadata`) carry `{target, dependency_type, provenance, confidence}`.

**HOWEVER**, the entire service → view → MCP pipeline operates exclusively on the base `SymbolRepository` trait (`callees()`/`callers()` return bare `RelationTarget` — zero metadata). The `ExplorerService` holds `Arc<dyn SymbolRepository>`, not `Arc<dyn MetadataAwareRepository>`. No code in the service, views, or MCP layer downcasts to `MetadataAwareRepository`.

The serialization path:
1. MCP handler `dispatch()` → calls `service.inspect_object()` / `service.contextual_view()` / `service.apply_lens()`
2. Service builds `ContextualView` via view builders in `src/domain/views.rs`
3. View builders call `repo.callees()` / `repo.callers()` — **metadata dropped here**
4. `TypedRelation` DTO has fields: `{relation_type, direction, target_object_id, target_label, evidence_ids}` — **NO provenance, NO confidence**
5. `EvidenceBlock` has `confidence: Option<f32>` but is hardcoded to `Some(1.0)` in every single view builder — **13 occurrences, all `Some(1.0)`**
6. `EvidenceBlock` has **NO `provenance` field** at all
7. MCP handler serializes via `serde_json::to_string_pretty()` — what the views produce is what MCP consumers see
8. External agents see `TypedRelation` without per-edge trust information and `EvidenceBlock` with fake/uniform confidence

The `--postgres` flag in `bin/mcp.rs` only affects graph loading (`open_graph_from_postgres` loads a full `CallGraph` from PG, drops the pool, wraps in `Arc`). After loading, the same `CallGraphRepository` adapter is used — the MCP layer sees identical data regardless of source.

## Affected Areas

| File | Why |
|------|-----|
| `crates/cognicode-explorer/src/dto.rs` | `TypedRelation` lacks `provenance` and `confidence` fields; `EvidenceBlock` lacks `provenance` |
| `crates/cognicode-explorer/src/domain/views.rs` | All view builders call `SymbolRepository::callees()`/`callers()` (no metadata); `build_callgraph` (lines 62–100) and `build_scope_dependencies` (lines 896–982) construct relations without metadata; `relation_for()` helper (line 198) takes `&RelationTarget` and produces `TypedRelation` — zero metadata pass-through |
| `crates/cognicode-explorer/src/service.rs` | `ExplorerService.repo` is `Arc<dyn SymbolRepository>` (line 33); no downcast to `MetadataAwareRepository` anywhere |
| `crates/cognicode-explorer/src/ports/symbol_repository.rs` | `MetadataAwareRepository` trait exists and is fully implemented but unused in the service/views/MCP pipeline; `RelationTarget` is metadata-free by design |
| `crates/cognicode-explorer/src/mcp.rs` | MCP handler delegates to service; 8 tools registered, none expose call-graph edges directly |
| `crates/cognicode-explorer/src/moldql/executor.rs` | `MoldQLView` holds `Arc<dyn SymbolRepository>` (line 485), same limitation |
| `crates/cognicode-explorer/src/bin/mcp.rs` | `--postgres` flag is loading-only, no impact on metadata exposure |
| `crates/cognicode-explorer/tests/metadata_aware_repository.rs` | Existing test suite validates `MetadataAwareRepository` contract — confirms metadata is correct at the adapter level |

## Approaches

### 1. Enrich existing views with optional metadata fields (Recommended)
Add `provenance` and `confidence` fields to `TypedRelation` and `EvidenceBlock`, modify view builders to downcast to `MetadataAwareRepository` and populate per-edge metadata.

- **Pros**: No new MCP tools; backward-compatible (new fields are additive with `#[serde(default)]`); existing agent workflows keep working; leverages existing `MetadataAwareRepository` implementation; low line-count change (~150-200 lines)
- **Cons**: Downcast from `&dyn SymbolRepository` to `&dyn MetadataAwareRepository` at view-build time adds a runtime check; must handle the case where the repo does NOT implement `MetadataAwareRepository` (e.g., mocks in tests) gracefully — return `None` / `Option`
- **Effort**: Low-Medium

### 2. Dedicated MCP tool (`explorer_call_graph_edges`)
Add a new MCP tool that returns `Vec<EdgeWithMetadata>` directly, bypassing the view layer entirely.

- **Pros**: Clean separation; doesn't touch existing view code; metadata-rich response by construction; no downcast needed
- **Cons**: Requires either changing `ExplorerService.repo` type or adding a new service method; agents must learn a new tool; partial metadata (old views still return bare relations)
- **Effort**: Medium

### 3. Upgrade `ExplorerService.repo` to `Arc<dyn MetadataAwareRepository>`
Change the service's repo type and propagate metadata through all view builders.

- **Pros**: Eliminates downcast; metadata available everywhere; type-safe at compile time
- **Cons**: Breaking change to all view builders; all mock implementations in tests must also implement `MetadataAwareRepository`; affects MoldQL executor; high blast radius
- **Effort**: High

## Recommendation

**Approach 1 — Enrich existing views with optional metadata fields.** This is the minimal change that delivers the value. The `MetadataAwareRepository` trait already exists and `CallGraphRepository` implements it. The view builders just need to:

1. Add `provenance: Option<String>` and `confidence: Option<f64>` to `TypedRelation` (serde default = None)
2. Add `provenance: Option<String>` to `EvidenceBlock` (serde default = None)
3. In `build_callgraph()` and `build_scope_dependencies()`, attempt downcast `repo` to `&dyn MetadataAwareRepository`; on success, populate provenance/confidence per relation; on failure (mock), leave as `None`
4. Update the `relation_for()` helper to accept optional `(Provenance, f64)`
5. Set per-evidence confidence from actual edge data instead of hardcoded `1.0`

The downcast can be done with a simple helper function. Since `ExplorerService` holds `Arc<dyn SymbolRepository>`, the view builders that receive `&dyn SymbolRepository` can attempt the downcast at the call site. In production, the concrete type is always `CallGraphRepository` which implements both traits.

## Risks

- **Downcast fragility**: If a future adapter implements `SymbolRepository` but not `MetadataAwareRepository`, metadata will silently be `None`. Mitigation: log a warning on first failed downcast per view build.
- **Serde backward compatibility**: Adding `#[serde(default)]` fields keeps old clients working, but needs explicit testing for JSON round-trips with both populated and absent metadata.
- **Evidence confidence semantics**: Currently `EvidenceBlock.confidence` is `Some(1.0)` everywhere. Changing to per-evidence confidence may surprise consumers that assumed `1.0` = "trusted". Mitigation: keep the evidence-level confidence but document that per-relation confidence is the authoritative value.

## Ready for Proposal

**Yes.** The gap is well-understood: metadata exists in the domain model but is dropped at the `SymbolRepository` trait boundary in the view layer. The fix is additive (new optional fields on existing DTOs + downcast in view builders). No new MCP tools required. The `--postgres` flag needs no changes — the bridge already loads metadata-rich graphs; the gap is only in the views→MCP serialization path.
