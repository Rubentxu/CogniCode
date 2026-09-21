# Proposal: Narrative Runtime Cycle 2 — Snapshot Cache

## Intent
Cycle 1 delivered `EmbedResolver` + `ProjectDiaryExecutor` + `ExampleObjectExecutor` wired in narrative shapers — but `ContextualView` outputs are built on-demand and discarded. Cycle 2 adds LadybugDB-backed snapshot persistence so rendered narrative views survive restarts and avoid recomputation.

## Scope

### In Scope
- `NarrativeStore` port trait in `crates/cognicode-core/src/domain/ports/`
- `NarrativeSnapshot` DTO (serialized `ContextualView` + metadata)
- `NarrativeView` LadybugDB NODE TABLE
- `impl NarrativeStore for LadybugStore`
- `RuntimePorts::narrative_store` slot + `bootstrap_with_backend` wiring
- Cache invalidation on source revision mismatch

### Out of Scope
- Authored narrative documents (Lepiter-lite) — deferred to Phase 4
- Changes to narrative shapers (they stay synchronous)
- Frontend changes
- Async/streaming cache population

## Capabilities

> CONTRACT with sddk-spec. Researched against `openspec/specs/`.

### New Capabilities
- `narrative-store`: Port trait, `NarrativeSnapshot` DTO, error types, and LadybugDB-backed CRUD for narrative view cache. Shapers stay sync; I/O lives behind the port.

### Modified Capabilities
- `runtime-ladybug-wiring`: 10 → 11 ports. `NarrativeStore` slot added to `RuntimePorts` DTO and wired in `bootstrap_with_backend`.
- `ladybug-graph-schema`: New `NarrativeView` NODE TABLE added to DDL catalog (`id SERIAL`, `workspace_id`, `view_id`, `object_id`, `view_kind STRING`, `payload STRING`, `created_at INT64`, `source_rev INT64`).

## Approach
Follow the six-step `QualityStore` pattern exactly:

| Step | File | What |
|------|------|------|
| 1 | `ports/narrative_store.rs` | `NarrativeStore` trait + `NarrativeSnapshot` + `NarrativeError` |
| 2 | `ports/mod.rs` | Register module + re-export |
| 3 | `cognicode-ladybug/src/lib.rs` | `narrative_schema_ddls()` DDL helper |
| 4 | `LadybugStore::open()` | Call `init_narrative_schema()` |
| 5 | `lib.rs` (impl block) | `impl NarrativeStore for LadybugStore` |
| 6 | `cognicode-runtime/src/lib.rs` | `narrative_store` slot in `RuntimePorts` + bootstrap wiring |

Invalidation: `invalidate(ws, source_rev)` deletes rows where `source_rev < $rev`, returns deleted count.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/ports/narrative_store.rs` | New | Port trait + DTOs |
| `crates/cognicode-core/src/domain/ports/mod.rs` | Modified | Register module + re-export |
| `crates/cognicode-ladybug/src/lib.rs` | Modified | DDL + `impl NarrativeStore for LadybugStore` |
| `crates/cognicode-runtime/src/lib.rs` | Modified | `RuntimePorts` slot + bootstrap wiring |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Workspace ID type mismatch (generic-graph uses INT64, ports use STRING) | Med | Use STRING consistently; match existing `QualityStore` pattern |
| Cache staleness after re-ingest with new source_rev | Low | `invalidate()` deletes stale rows; callers check `source_rev` before load |
| DDL idempotency failure on restart | Low | `CREATE NODE TABLE IF NOT EXISTS` — same pattern as existing 25+ tables |

## Rollback Plan
1. Remove `NarrativeStore` slot from `RuntimePorts` (no caller dependency)
2. Remove `init_narrative_schema()` call from `LadybugStore::open()`
3. Delete `ports/narrative_store.rs` + module entry
4. Drop `NarrativeView` table — or leave it (no readers)

## Dependencies
- LadybugDB 0.19.0 (already in workspace)
- `serde_json` for payload serialization (already in workspace)
- `generic-graph-model` for workspace_id conventions

## Success Criteria
- [ ] `NarrativeStore` trait compiles in `cognicode-core` with no PG/SQL dependencies
- [ ] `impl NarrativeStore for LadybugStore` passes round-trip test: save → load → same snapshot
- [ ] `invalidate()` deletes rows with stale `source_rev` and returns correct count
- [ ] `runtime-ladybug-wiring` spec scenario count updated (10 → 11 ports)
- [ ] `ladybug-graph-schema` DDL catalog updated with `NarrativeView` table
- [ ] `bootstrap_with_backend` smoke test passes with `narrative_store: Some(ladybug_store)`
