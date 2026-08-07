# Tasks: Explorer Graph Foundation — Edge Provenance & Confidence

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~400-500 (production + tests + golden fixtures) |
| 400-line budget risk | Medium |
| Chained PRs recommended | Yes |
| Suggested split | Phase 1 → Phase 2 → Phase 3+4 (parallel) → Phase 5 |
| Delivery strategy | auto-chain |
| Chain strategy | stacked-to-main |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: Medium

## Implementation Order

Phase 1 (no deps) → Phase 2 (needs Phase 1) → Phase 3 (needs Phase 2) + Phase 4 (needs Phase 2, parallel to Phase 3) → Phase 5 (needs Phase 3 + Phase 4).

## Phase 1: Foundation Types (Slice 1 — ~80 lines)

- [ ] 1.1 Create `crates/cognicode-core/src/domain/value_objects/provenance.rs` with `Provenance` enum (Extracted, Inferred, Ambiguous) deriving Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default = Extracted
- [ ] 1.2 Add `pub mod provenance; pub use provenance::Provenance;` to `crates/cognicode-core/src/domain/value_objects/mod.rs`
- [ ] 1.3 Unit test: `Provenance::default() == Extracted` and serde roundtrip for all 3 variants
- [ ] 1.4 Create `crates/cognicode-core/src/domain/services/confidence_rules.rs` with `ExtractionContext` enum (DirectExtraction | Heuristic { score: f64 } | Unresolved) and `ConfidenceError` (OutOfRange, NotANumber, Infinite)
- [ ] 1.5 Implement `ConfidenceRules::assign(ctx: &ExtractionContext) -> Result<(Provenance, f64), ConfidenceError>` with mapping: DirectExtraction→(Extracted,1.0), Heuristic{score}→(Inferred,score.clamp(0.5,0.9)), Unresolved→(Ambiguous,0.3); reject NaN/inf
- [ ] 1.6 Add `pub mod confidence_rules;` to `crates/cognicode-core/src/domain/services/mod.rs`
- [ ] 1.7 Golden tests: frozen fixtures for (DirectExtraction, Heuristic{0.5}, Heuristic{0.7}, Heuristic{0.9}, Unresolved) → bit-exact (Provenance, f64)
- [ ] 1.8 Validation tests: NaN, +inf, -inf, 1.2, -0.1 all return ConfidenceError

**Validation**: `cargo test -p cognicode-core --lib provenance confidence_rules` — all green.

## Phase 2: CallGraph Storage Upgrade (Slice 2 — ~120 lines changed)

Depends on Phase 1.

- [ ] 2.1 Change `edges: HashMap<SymbolId, HashSet<(SymbolId, DependencyType)>>` → `HashMap<SymbolId, HashMap<(SymbolId, DependencyType), (Provenance, f64)>>` in `crates/cognicode-core/src/domain/aggregates/call_graph.rs`
- [ ] 2.2 Refactor `add_dependency(&mut self, source, target, dep_type)` to route through `ConfidenceRules::assign(ExtractionContext::DirectExtraction)` — preserve signature
- [ ] 2.3 Add `add_dependency_with_provenance(source, target, dep_type, ctx: ExtractionContext) -> Result<(), CallGraphError>`
- [ ] 2.4 Add `edges_with_metadata() -> impl Iterator<Item = (SymbolId, SymbolId, DependencyType, Provenance, f64)>` and `callees_with_metadata(id) -> Vec<(SymbolId, DependencyType, Provenance, f64)>`
- [ ] 2.5 Update `dependencies()`, `callees()`, `callers()` iterators to read the new HashMap values; signatures unchanged
- [ ] 2.6 Run `cargo test -p cognicode-core --lib call_graph` — all pre-existing tests pass unmodified

**Validation**: full call_graph test suite green; no API signature change observable from outside.

## Phase 3: Bincode Versioning (Slice 3 — ~100 lines new)

Depends on Phase 2.

- [ ] 3.1 Create `VersionedBlob` in `cognicode-db` (or `cognicode-core`) with `MAGIC: [u8;4] = b"CCG1"`, `encode_v2(&CallGraph) -> Vec<u8>` (= MAGIC + 0x02 + bincode), `decode(&[u8]) -> Result<CallGraph, StoreError>`
- [ ] 3.2 Create `CallGraphV1` shadow struct in `call_graph.rs` mirroring old `HashMap<SymbolId, HashSet<(SymbolId, DependencyType)>>` shape, marked `#[deprecated]`
- [ ] 3.3 Implement `CallGraphV1::into_v2() -> CallGraph` assigning `(Extracted, 1.0)` to every legacy edge
- [ ] 3.4 Update `cognicode-db/src/graph.rs::save_graph` to use `VersionedBlob::encode_v2`
- [ ] 3.5 Update `cognicode-db/src/graph.rs::load_graph`: if starts with MAGIC → versioned path (v2 deserialize direct; v1 → CallGraphV1::into_v2; unknown version → StoreError::Corrupted); if no magic → legacy v1 fallback
- [ ] 3.6 Integration test: bincode v2 roundtrip preserves (Provenance, f64) for all edges
- [ ] 3.7 Integration test: legacy v1 blob (no magic, old shape) loads with (Extracted, 1.0) defaults
- [ ] 3.8 Integration test: blob with `CCG1` magic but version 99 returns `StoreError::Corrupted`

**Validation**: `cargo test -p cognicode-db` — all bincode + migration tests green.

## Phase 4: SQLite Migration (Slice 4 — ~40 lines)

Depends on Phase 2. Can run in parallel to Phase 3.

- [ ] 4.1 Add `migrate_v1_to_v2(db: &Connection)` in `cognicode-db/src/schema.rs` with guard `if schema_version(db) < 2 { ...; set_schema_version(db, 2); }`
- [ ] 4.2 Inside migration: `ALTER TABLE call_edges ADD COLUMN provenance TEXT NOT NULL DEFAULT 'Extracted'` and `ADD COLUMN confidence REAL NOT NULL DEFAULT 1.0`
- [ ] 4.3 Fix column name mismatch: CREATE TABLE uses `caller_id`/`callee_id`; rewrite INSERTs in `populate_edges` to use matching names
- [ ] 4.4 Update `populate_edges` in `cognicode-db/src/graph.rs` to insert `provenance` and `confidence` alongside `caller_id`/`callee_id`/`dep_type`
- [ ] 4.5 Integration test: fresh SQLite → `initialize_schema` creates columns with defaults; second run is idempotent and preserves existing rows
- [ ] 4.6 Integration test: `populate_edges` with metadata + `SELECT caller_id, callee_id, dep_type, provenance, confidence` returns all 5 columns populated

**Validation**: `cargo test -p cognicode-db` — all schema + populate tests green.

## Phase 5: Explorer Adapter + Integration (Slice 5 — ~60 lines)

Depends on Phase 3 and Phase 4.

- [ ] 5.1 Add `callees_with_metadata(&self, id: &SymbolId) -> Vec<(SymbolId, DependencyType, Provenance, f64)>` passthrough in `crates/cognicode-explorer/src/adapters/call_graph_repository.rs`
- [ ] 5.2 End-to-end integration test: SQLite call_edges populated → bincode save+load → adapter read returns consistent metadata
- [ ] 5.3 Add invariant post-condition helper: every edge in any test CallGraph satisfies `confidence ∈ [0.0, 1.0] && !is_nan()`; apply to all call_graph tests
- [ ] 5.4 Run `cargo test --workspace` and `cargo clippy --workspace` — all green, no new warnings

**Validation**: workspace-wide test suite + clippy clean.

## Dependencies Between Tasks

- Phase 1 has no internal deps.
- Phase 2 depends on Phase 1 (uses `Provenance`, `ConfidenceRules`, `ExtractionContext`).
- Phase 3 depends on Phase 2 (operates on the new `CallGraph` shape).
- Phase 4 depends on Phase 2 (reads/writes the new edge metadata) but not on Phase 3.
- Phase 5 depends on both Phase 3 and Phase 4 (end-to-end through both stores).

## External Dependencies

None — every file to be created or modified already exists in the repo. No new crate dependencies, no new toolchain, no schema changes outside `call_edges`.
