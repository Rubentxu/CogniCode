# Tasks: mcp-postgres-envelope

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~200 (additive, 5 files; 1 trait method, 3 DTO fields, 1 helper signature, evidence tweaks, 8 unit tests + 1 integration test) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | single PR |
| Delivery strategy | auto-chain (caller's hint) — but change is small enough to ship as a single PR; no chain needed |
| Chain strategy | single-pr |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: single-pr
400-line budget risk: Low

### Suggested Work Units

Single work unit — additive change, no new types, no signature breakage in public API.

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | DTO enrichment + trait downcast + 2 view builders + 8 unit + 1 integration test | PR 1 | Base = main; all tests + docs land with the code |

## Phase 1: DTO & Trait Foundation (interface changes)

- [ ] 1.1 In `crates/cognicode-explorer/src/dto.rs`, add `#[serde(default)] pub provenance: Option<String>` and `#[serde(default)] pub confidence: Option<f64>` to `TypedRelation` (struct at L72–78).
- [ ] 1.2 In the same file, add `#[serde(default, skip_serializing_if = "Option::is_none")] pub provenance: Option<String>` to `EvidenceBlock` (struct at L88–101), mirroring the existing `freshness` pattern.
- [ ] 1.3 In `crates/cognicode-explorer/src/ports/symbol_repository.rs`, add a default method to the `SymbolRepository` trait (after L114): `fn as_metadata_aware(&self) -> Option<&dyn MetadataAwareRepository> { None }`. Do not change the existing methods.

## Phase 2: Adapter Override & Helper Signature (downcast wiring)

- [ ] 2.1 In `crates/cognicode-explorer/src/adapters/call_graph_repository.rs`, delete the inherent `as_metadata_aware` method (L48–57) — the default-method override on the trait will cover it.
- [ ] 2.2 In the same file, add `impl SymbolRepository for CallGraphRepository` override: `fn as_metadata_aware(&self) -> Option<&dyn MetadataAwareRepository> { Some(self as &dyn MetadataAwareRepository) }`. This replaces the inherent helper with the trait override so the downcast is reachable from `&dyn SymbolRepository`.

## Phase 3: View Builder Enrichment (core wiring)

- [ ] 3.1 In `crates/cognicode-explorer/src/domain/views.rs`, update the `relation_for` helper (L198–211) to accept `metadata: Option<(Provenance, f64)>` as a 5th parameter and populate `provenance` (via `p.to_string()`) and `confidence` on the returned `TypedRelation`. Import `Provenance` from `cognicode_core::domain::value_objects::Provenance`.
- [ ] 3.2 In `build_callgraph` (L62–129), attempt downcast via `repo.as_metadata_aware()`. If `Some(aware)` → for each caller/callee, look up `(Provenance, f64)` from `aware.callees_with_metadata(&symbol.id)` (key by target `SymbolId`) and pass it as the new `metadata` arg to `relation_for`. If `None` → log a `warn!` once via `tracing` and call `relation_for(.., None)`; add `use tracing::warn;` at the top of the file.
- [ ] 3.3 In the same function, replace the hardcoded `confidence: Some(1.0)` on the `cg_evidence` block (L77) with the per-edge confidence when the downcast succeeds; when it fails, set `confidence: None` and `provenance: None` (matches mock-repo path). Reuse the downcast result computed in 3.2 — do not re-call.
- [ ] 3.4 In `build_scope_dependencies` (L896–982), apply the same downcast pattern: on `Some(aware)`, fold per-edge `(Provenance, f64)` into the cross-scope buckets and add an `evidence` block `provenance` field; on `None`, log a `warn!` once and emit `provenance: None`, `confidence: None`. The block at L962–971 gets the same `provenance`/`confidence` treatment as the callgraph evidence.
- [ ] 3.5 In `crates/cognicode-explorer/src/domain/evidence.rs`, add `provenance: None` to every `EvidenceBlock` constructor in this file (L44, L61, L80, L115) — the file does not own edge metadata, so the field stays `None`. Compile-only; no test changes here.

## Phase 4: Tests (validation)

All unit tests below are added to the `mod tests` block in `crates/cognicode-explorer/src/domain/views.rs` unless noted. The integration test lands in `crates/cognicode-explorer/tests/mcp_edge_metadata.rs` (new file).

- [ ] 4.1 **Unit — DTO serde backward compat** in `views.rs` mod tests: `legacy_payload_deserializes_into_updated_dto` — parse `{"relation_type":"CALLS","direction":"outgoing","target_object_id":"x","target_label":"x","evidence_ids":[]}` into `TypedRelation`; assert `provenance: None` and `confidence: None`. Covers REQ4 spec scenario.
- [ ] 4.2 **Unit — DTO round-trip** in `views.rs` mod tests: `enriched_payload_round_trips` — build `TypedRelation { provenance: Some("Extracted".into()), confidence: Some(0.9), .. }`, serialize, deserialize, assert equality. Covers REQ4.
- [ ] 4.3 **Unit — Downcast returns None on mock** in `views.rs` mod tests: `downcast_fails_on_mock_repo` — call `as_metadata_aware()` on the existing `MockRepo`; assert `None`. Covers REQ3.
- [ ] 4.4 **Unit — Downcast returns Some on real repo** in `views.rs` mod tests: `downcast_succeeds_on_call_graph_repo` — build a `CallGraphRepository` from an `Arc<CallGraph>` with a seeded `(Provenance::Inferred, 0.85)` edge, call `as_metadata_aware()`, assert `Some(_)`, then call `callees_with_metadata` and verify the edge tuple. Covers REQ3.
- [ ] 4.5 **Unit — `build_callgraph` with metadata-aware repo** in `views.rs` mod tests: `typed_relation_metadata_populated_from_aware_repo` — wire a `CallGraphRepository` with one edge `(Provenance::CallSite, 0.85)`, call `build_callgraph`, assert the emitted `TypedRelation` has `provenance: Some("Extracted")` (or whatever the seeded Display form is) and `confidence: Some(0.85)`. Covers REQ1.
- [ ] 4.6 **Unit — `build_callgraph` with mock repo** in `views.rs` mod tests: `typed_relation_metadata_null_for_mock_repo` — keep the existing `MockRepo` setup from `callgraph_populates_relations`; assert every emitted `TypedRelation` has `provenance: None`, `confidence: None`; no panic. Covers REQ1.
- [ ] 4.7 **Unit — Evidence block confidence not hardcoded** in `views.rs` mod tests: `evidence_block_reports_per_evidence_confidence` — wire a `CallGraphRepository` with one edge at `0.72`; call `build_callgraph`; assert the `cg_evidence` block has `confidence: Some(0.72)` (or `Some(0.72_f32)` after the f64→f32 cast) and `provenance` populated. Covers REQ2.
- [ ] 4.8 **Unit — `build_scope_dependencies` with metadata** in `views.rs` mod tests: `evidence_block_degrades_gracefully` — same as 4.6 but for `build_scope_dependencies`; assert the evidence block has `provenance: None`, `confidence: None` on the mock path, and that the downcast `warn!` is emitted (use `tracing-test` or a captured logger; if not available, assert the public shape only). Covers REQ2.
- [ ] 4.9 **Integration — `inspect_object` end-to-end** in new file `crates/cognicode-explorer/tests/mcp_edge_metadata.rs`: `inspect_object_returns_enriched_relations` — build a `CallGraph` with two symbols and a known edge, wrap it in a `CallGraphRepository`, drive the full service stack (`ExplorerService`), call `inspect_object` on the source symbol with `view_id = "call-graph"`, parse the JSON, assert the relation has non-null `provenance` and `confidence`. Covers the spec acceptance criterion #5.

## Phase 5: Verification & Cleanup

- [ ] 5.1 Run `cargo test -p cognicode-explorer` and confirm all 8 new unit tests + 1 integration test pass.
- [ ] 5.2 Run `cargo clippy -p cognicode-explorer --all-targets -- -D warnings` and resolve any new lints.
- [ ] 5.3 Run `cargo build -p cognicode-explorer` and confirm the binary still compiles with no `unused_import` or `dead_code` warnings (the deleted inherent `as_metadata_aware` in 2.1 must not leave dangling references).
- [ ] 5.4 Spot-check: grep the repo for `confidence: Some(1.0)` to confirm no call-graph view builder still emits the hardcoded value (`crates/cognicode-explorer/src/domain/views.rs` and `evidence.rs` should be clean).

## Dependency Map

```
1.1, 1.2 (DTO fields)        ── must precede ──> 3.1, 3.3, 3.4 (view builders consume new fields)
1.3 (default trait method)   ── must precede ──> 2.2 (override) and 3.2, 3.4 (call sites call as_metadata_aware)
2.1, 2.2 (adapter override)  ── must precede ──> 4.4, 4.5, 4.7, 4.9 (tests need a working downcast)
3.1 (helper signature)       ── must precede ──> 3.2, 3.4 (call sites pass new arg) and 4.5, 4.6 (tests assert)
3.2, 3.3, 3.4 (view wiring)  ── must precede ──> 4.5, 4.6, 4.7, 4.8, 4.9
3.5 (evidence.rs tweaks)     ── independent — compile-only
4.1, 4.2 (serde tests)       ── independent of view builders — can run after 1.1, 1.2
```

## Line-Count & PR Sizing Estimate

| Phase | Lines changed (approx) | Files |
|-------|------------------------|-------|
| Phase 1 | ~10 (3 fields + 1 default method) | `dto.rs`, `ports/symbol_repository.rs` |
| Phase 2 | ~6 (delete inherent + add override) | `adapters/call_graph_repository.rs` |
| Phase 3 | ~80 (downcast + per-edge wiring + evidence tweaks) | `domain/views.rs`, `domain/evidence.rs` |
| Phase 4 | ~150 (8 unit tests + 1 integration test) | `domain/views.rs` mod tests, new `tests/mcp_edge_metadata.rs` |
| Phase 5 | 0 (verification) | — |
| **Total** | **~250** | **5** |

400-line budget: **comfortably under** — no chained PR needed.

## Files Touched

| File | Action | Reference |
|------|--------|-----------|
| `crates/cognicode-explorer/src/dto.rs` | Modify | 1.1, 1.2 |
| `crates/cognicode-explorer/src/ports/symbol_repository.rs` | Modify | 1.3 |
| `crates/cognicode-explorer/src/adapters/call_graph_repository.rs` | Modify | 2.1, 2.2 |
| `crates/cognicode-explorer/src/domain/views.rs` | Modify | 3.1, 3.2, 3.3, 3.4, 4.1–4.8 |
| `crates/cognicode-explorer/src/domain/evidence.rs` | Modify | 3.5 |
| `crates/cognicode-explorer/tests/mcp_edge_metadata.rs` | Create | 4.9 |
