# Proposal: Expose Edge Provenance & Confidence through MCP

## Intent

The `CallGraph` stores per-edge `(Provenance, f64)` metadata since Phase 1,
but the MCP serialization path drops it at the `SymbolRepository` trait boundary.
External agents consuming MCP views see every relation as equally trustworthy
(`confidence: 1.0`) and lack provenance attribution. Expose this metadata so
downstream agents can weigh evidence quality and apply per-edge trust heuristics.

## Scope

### In Scope
- Add `provenance: Option<String>` and `confidence: Option<f64>` to `TypedRelation`
- Add `provenance: Option<String>` to `EvidenceBlock`
- Downcast repo to `MetadataAwareRepository` in call-graph view builders
- Populate per-relation trust data from existing edge metadata
- Replace hardcoded `confidence: Some(1.0)` with per-evidence values

### Out of Scope
- New MCP tools (no `explorer_call_graph_edges`)
- Changing `ExplorerService.repo` type (`Arc<dyn SymbolRepository>` stays)
- Enriching non-call-graph views (scope, file, module views)
- Postgres flag changes — bridge already loads metadata-rich graphs
- Exposing metadata in MoldQL executor

## Capabilities

### New Capabilities
- `mcp-edge-metadata`: MCP views carry per-relation provenance and confidence.
  Consumers receive optional provenance strings and 0.0–1.0 confidence scores
  per `TypedRelation` and per `EvidenceBlock`. Absent metadata serializes as
  `null`. Backward-compatible via `#[serde(default)]`.

### Modified Capabilities
None

## Approach

**DTO enrichment with optional fields + trait downcast at view-build time.**

1. `TypedRelation` gains `provenance: Option<String>`, `confidence: Option<f64>`
   (serde default = `None`)
2. `EvidenceBlock` gains `provenance: Option<String>` (serde default = `None`)
3. View builders receive a `&dyn SymbolRepository` reference. At call sites,
   attempt downcast to `&dyn MetadataAwareRepository` via
   `CallGraphRepository::as_metadata_aware()` pattern (or `Any` downcast).
4. On success: populate provenance/confidence per relation from
   `callees_with_metadata()` / `dependencies_with_metadata()`.
   On failure (mock repos): leave fields as `None`. Log a warning once per view build.
5. Update `relation_for()` helper to accept optional `(Provenance, f64)`.
6. Set `EvidenceBlock.confidence` from actual edge confidence instead of `1.0`.

~150 lines, additive, no signature changes to existing methods.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | Modified | Add 3 optional fields with `#[serde(default)]` |
| `crates/cognicode-explorer/src/domain/views.rs` | Modified | Downcast in `build_callgraph()`, `build_scope_dependencies()`, `relation_for()` |
| `crates/cognicode-explorer/src/domain/evidence.rs` | Modified | Accept provenance from caller, pass per-evidence confidence |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Downcast fails silently on new adapters | Low | Log warning once per view build; `Option` fields are safe `None` |
| Serde breakage for old JSON consumers | Low | `#[serde(default)]` + test round-trip with both populated and absent fields |
| Confidence semantics change surprises agents | Low | Document: per-relation confidence is authoritative; evidence-level is aggregate |
| Performance impact from metadata lookups | Low | `HashMap` lookup is O(1); metadata already in-memory |

## Rollback Plan

Revert commit. `#[serde(default)]` ensures old payloads deserialize cleanly.
No database migration, no config changes, no flag changes.

## Dependencies

- `MetadataAwareRepository` trait (exists, implemented by `CallGraphRepository`)
- `CallGraphRepository::as_metadata_aware()` helper (exists)
- `RelationTargetWithMetadata`, `EdgeWithMetadata` types (exist)

## Success Criteria

- [ ] `TypedRelation` JSON payloads include `provenance` and `confidence` fields (non-null for call-graph views)
- [ ] `EvidenceBlock` JSON payloads include `provenance` field
- [ ] `confidence: 1.0` no longer appears in any view builder
- [ ] Mock repo tests produce `None` metadata fields without panicking
- [ ] MCP `inspect_object` returns enriched relations for symbols with known edges
- [ ] Serde round-trip preserves all fields (old payloads deserialize, new payloads survive re-serialize)

## Entropy Budget

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) | ~0.5 | < 1.0 | ✅ OCP compliant |
| H(Δ_new) | ~1.6 | > 0 | ✅ Pure extension |
| New connascence pairs | 0 | < 3 | ✅ |
| DQS impact | neutral | — | ✅ No structural change |

**Method**: Heuristic (CogniCode unavailable). **Confidence**: Estimated.

**Verdict**: 🟢 — Additive change, zero connascence introduced, pure extension via optional fields.
