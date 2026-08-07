# Design: Expose Edge Provenance & Confidence through MCP

## Technical Approach

Add optional `provenance` and `confidence` fields to `TypedRelation` and `EvidenceBlock` DTOs, then enrich the two call-graph-aware view builders (`build_callgraph`, `build_scope_dependencies`) with a trait-level downcast to `MetadataAwareRepository`. The downcast uses a default method on `SymbolRepository` — no `Any` hack. When the concrete adapter supports metadata, per-edge `(Provenance, f64)` flows into the JSON payload. When it doesn't (mock repos), fields serialize as `null`.

## Architecture Decisions

### Decision: Downcast mechanism

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Default method `as_metadata_aware()` on `SymbolRepository` | Zero-cost for non-implementors; mock repos inherit `None`; no `Any` import | **Chosen** |
| `Any`-based downcast in view builders | Works but requires `std::any::Any` in the trait object; more boilerplate at every call site | Rejected |
| Change `ExplorerService.repo` to `Arc<dyn MetadataAwareRepository>` | Breaking: all mocks, MoldQL, test fixtures must implement the sub-trait | Rejected (spec says out-of-scope) |

**Rationale**: The default-method pattern is already idiomatic here. `CallGraphRepository` overrides to return `Some(self)`. Every other impl (mocks, future adapters) inherits `None` automatically — zero code change required in tests.

### Decision: `confidence` type in `TypedRelation`

| Option | Tradeoff | Decision |
|--------|----------|----------|
| `Option<f64>` | Matches domain `f64`; same precision as `ConfidenceRules` output | **Chosen** |
| `Option<f32>` | Matches existing `EvidenceBlock.confidence: Option<f32>` | Rejected — loses precision from `ConfidenceRules` output |

**Rationale**: `f64` is the domain standard (edges store `f64`). The `EvidenceBlock.confidence` is already `f32` — we leave it as-is for backward compat, but cast `f64→f32` when populating it. No data loss in practice (values are 0.0–1.0 with ≤2 decimal places).

### Decision: Provenance serialization format

| Option | Tradeoff | Decision |
|--------|----------|----------|
| `Provenance::Display` string ("Extracted", "Inferred", "Ambiguous") | Human-readable; matches DB column; round-trips through `FromStr` | **Chosen** |
| serde JSON enum (`"extracted"`, `"inferred"`) | Canonical for Rust enums | Rejected — breaks the existing convention where the PG bridge stores `Display` form |

**Rationale**: The Postgres bridge already writes `Provenance::to_string()` into the `provenance` column. Using `Display` in the DTO keeps the MCP output consistent with the persistence layer.

## Data Flow

```
Service (contextual_view_*)
  │
  ├─ repo.as_ref() ──→ &dyn SymbolRepository
  │                         │
  │          ┌──────────────┴──────────────────┐
  │          │ repo.as_metadata_aware()         │
  │          │ → Some(&dyn MetadataAwareRepo)   │
  │          │ → None (mock/unknown adapter)    │
  │          └──────────────┬──────────────────┘
  │                         │
  │   callees_with_metadata(id) → Vec<RelationTargetWithMetadata>
  │                         │
  └─ build_callgraph() ◄───┘
       │
       ├─ relation_for(.., metadata: Option<(Provenance, f64)>)
       │     → TypedRelation { provenance, confidence, .. }
       │
       └─ EvidenceBlock { provenance, confidence: edge_value }
             (NOT hardcoded 1.0)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | Modify | Add `provenance: Option<String>` + `confidence: Option<f64>` to `TypedRelation`; add `provenance: Option<String>` to `EvidenceBlock` |
| `crates/cognicode-explorer/src/ports/symbol_repository.rs` | Modify | Add `fn as_metadata_aware(&self) -> Option<&dyn MetadataAwareRepository> { None }` default method to `SymbolRepository` trait |
| `crates/cognicode-explorer/src/adapters/call_graph_repository.rs` | Modify | Override `as_metadata_aware()` to return `Some(self)` (replaces standalone inherent method) |
| `crates/cognicode-explorer/src/domain/views.rs` | Modify | Update `build_callgraph()` and `build_scope_dependencies()` to attempt downcast; update `relation_for()` helper to accept `Option<(Provenance, f64)>`; replace all `confidence: Some(1.0)` in call-graph evidence with per-edge values |
| `crates/cognicode-explorer/src/domain/evidence.rs` | Modify | Add `provenance: None` to evidence blocks that don't carry edge metadata |

## Interfaces / Contracts

### `TypedRelation` (modified)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedRelation {
    pub relation_type: String,
    pub direction: RelationDirection,
    pub target_object_id: String,
    pub target_label: String,
    pub evidence_ids: Vec<String>,
    #[serde(default)]
    pub provenance: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}
```

### `EvidenceBlock` (modified)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceBlock {
    // ... existing fields unchanged ...
    pub confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
}
```

### `SymbolRepository` trait (modified)

```rust
pub trait SymbolRepository: Send + Sync {
    // ... existing methods unchanged ...

    /// Optional downcast to the metadata-aware surface.
    /// Returns `None` for adapters that don't implement
    /// [`MetadataAwareRepository`].
    fn as_metadata_aware(&self) -> Option<&dyn MetadataAwareRepository> {
        None
    }
}
```

### `relation_for()` helper (modified)

```rust
fn relation_for(
    relation_type: &str,
    direction: RelationDirection,
    target: &RelationTarget,
    evidence_id: &str,
    metadata: Option<(Provenance, f64)>,
) -> TypedRelation {
    TypedRelation {
        relation_type: relation_type.to_string(),
        direction,
        target_object_id: mvp_id_from_target(target),
        target_label: format!("{} ({})", target.name, target.kind.name()),
        evidence_ids: vec![evidence_id.to_string()],
        provenance: metadata.map(|(p, _)| p.to_string()),
        confidence: metadata.map(|(_, c)| c),
    }
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | DTO serde backward compat | Deserialize legacy JSON `{\"source\":\"a\",\"target\":\"b\",\"kind\":\"calls\"}` into updated `TypedRelation`; verify `provenance: None, confidence: None` |
| Unit | DTO serde round-trip | Serialize `TypedRelation { provenance: Some(\"Extracted\"), confidence: Some(0.9), .. }`, deserialize back, assert equality |
| Unit | Downcast returns `None` for `MockRepo` | Call `as_metadata_aware()` on mock; assert `None` |
| Unit | Downcast returns `Some` for `CallGraphRepository` | Build a graph with edges, call `as_metadata_aware()`, assert `Some`, then call `callees_with_metadata()` and verify values |
| Unit | `build_callgraph()` with metadata-aware repo | Wire a `CallGraphRepository` with known `(Provenance::Inferred, 0.85)` edge; verify JSON output has `\"provenance\": \"Inferred\"`, `\"confidence\": 0.85` |
| Unit | `build_callgraph()` with mock repo | Use existing `MockRepo`; verify `provenance: None`, `confidence: None` in output; no panic |
| Unit | `build_scope_dependencies()` with metadata | Same pattern as callgraph test but for scope deps |
| Unit | `EvidenceBlock` confidence not hardcoded | Verify evidence block from `build_callgraph()` uses edge confidence, not `1.0` |
| Integration | `inspect_object` returns enriched relations | Full service stack with `CallGraphRepository`; call `inspect_object` on a symbol; verify the call-graph view has populated metadata |
| Spec scenario | All 7 spec scenarios | Each scenario maps to a focused unit test |

### Test ↔ Spec Scenario Mapping

| Test | Spec Scenario |
|------|---------------|
| `typed_relation_metadata_populated_from_aware_repo` | REQ1: "View builder populates metadata from a metadata-aware repository" |
| `typed_relation_metadata_null_for_mock_repo` | REQ1: "View builder leaves metadata null for a mock repository" |
| `evidence_block_reports_per_evidence_confidence` | REQ2: "Evidence block reports per-evidence confidence" |
| `evidence_block_degrades_gracefully` | REQ2: "Evidence block degrades gracefully without metadata" |
| `downcast_succeeds_on_call_graph_repo` | REQ3: "Downcast succeeds on a Postgres-backed repository" |
| `downcast_fails_on_mock_repo` | REQ3: "Downcast fails on a mock repository" |
| `legacy_payload_deserializes_into_updated_dto` | REQ4: "Legacy payload deserializes into updated DTO" |
| `enriched_payload_round_trips` | REQ4: "Enriched payload round-trips through serde" |

## Migration / Rollout

No migration required. `#[serde(default)]` on all new fields ensures backward compat. No feature flags needed — the downcast is a pure function of the concrete adapter type. The MCP binary (`bin/mcp.rs`) already wires `CallGraphRepository`, which will automatically return metadata. No config or flag changes.

## Open Questions

None. All decisions resolved by the exploration and proposal phases.
