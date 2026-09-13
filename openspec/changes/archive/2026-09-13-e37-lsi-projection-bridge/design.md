# Design: E37 LSI Projection Bridge

> Delivery strategy: **auto-chain** — chained work-unit slices WU-1..WU-5, stacked-to-main, no git commits without user approval. Resolved proposal assumptions A1–A5 are implemented here, not re-opened. e36-owned files (`evidence_kernel/ports.rs`, `infrastructure/evidence_kernel/in_memory.rs`, mod wiring) are touched only additively; e37 owns every new file.

## Technical Approach

Parallel fact-sourced projections behind existing seams (exploration Option A, batch-rebuild-first). Deterministic adapters turn `extract_file` output and `CodeIntelligenceProvider` observations into canonical `Fact`s committed to the e36 kernel; `CallGraphProjection::from_facts` feeds the unchanged `CallGraphProjectionPort`, and a minimal `GenericGraphProjectionPort` emits existing `GraphNode`/`GraphEdge`. A Rust equivalence harness proves ≥99% structural equivalence against the legacy graph on the e36 golden fixtures. No cutover; everything behind the off-by-default `evidence-kernel` feature.

## Architecture Decisions

| # | Decision | Alternatives | Rationale |
|---|----------|--------------|-----------|
| D1 | Fact adapters in `application/fact_bridge/` (cfg `evidence-kernel`): `tree_sitter_facts.rs` wraps `extract_file`'s `ExtractionResult` (extractor.rs:26); `lsp_facts.rs` consumes `&dyn CodeIntelligenceProvider` (async). All five `ReferenceKind` variants map onto canonical predicates (Call→calls, Import→imports, Read/Write/Type→references, kind recorded in `provenance.detail` as `ref=<Kind>`); `get_hierarchy` parents→inherits. Producers: tree-sitter=`DeterministicAnalyzer`, LSP=`RuntimeObserver` (umbrella authority table). | Own AST re-walk in infrastructure; new crate | `application::ingest` already imports `infrastructure::parser` types (extractor.rs:12) — same dependency direction, zero new crate deps, extractor output reused verbatim |
| D2 | Canonical bootstrap set pinned to exactly six predicates (`core:calls/imports/contains/defines/inherits/references`) via new `domain/evidence_kernel/bootstrap.rs::bootstrap_registry`; idempotent: `AlreadyRegistered` with an identical spec is success. `UsesGeneric`/`AnnotatedBy` stay unregistered (no M2 producer; append-only registry admits them later). | Larger set; stores self-register | Spec phase hinted six; registry is append-only (e36 D6) so re-calls from runtime, tests, and harness must be safe |
| D3 | Entity identity = raw id string in `FactValue::Text` (A1): symbols use the legacy FQN `"{file}:{name}:{line}"`, files their path. `EntityId(u64)` subjects come from `EntityIdTable` — snapshot-scoped sorted unique id strings mapped to sequential 1..N; no hashing, cross-entity references stay `Text` (no `Ref`). Symbol kind rides the `core:defines` fact's `provenance.detail` as `kind=<SymbolKind>` (serde name). `FactBatchBuilder::finish()` sorts all records by (subject-id, predicate, object) before assigning `EntityId`/`FactId`, so fact sets are byte-identical across runs regardless of walk order. Collisions impossible: the table is scoped per `(workspace, snapshot)` key. | Hashed FQN→u64 `Ref`; 7th `core:kind` predicate; structured Text object | Default-hash u64s are process-random (breaks golden identity); a 7th predicate deviates from the hinted six; `provenance.detail` is the e36-declared free-form metadata slot |
| D4 | `CallGraphProjection::from_facts(&[Fact]) -> Self` — ADDED constructor in `infrastructure/graph/call_graph_projection.rs` (cfg-gated), error-free and order-independent (BTreeMap internals). Nodes = `core:defines` facts (`SymbolId(fqn)`; `Symbol` reconstructed via `Symbol::new(name, kind-from-detail, Location)` whose computed fqn reproduces the identity string). Edges = `core:calls` facts only (legacy CallGraph carries only Calls edges — analysis_service.rs build path); callee resolution by lowercase name with deterministic tie-break: lexicographically smallest FQN wins (replaces legacy walk-order first-file-wins at analysis_service.rs:349); unresolved callees dropped and counted, mirroring legacy `unresolved_edges`. | Resolve at extraction time; project Imports/Contains edges too; return Result | Build-time resolution matches the legacy pipeline shape and keeps facts raw; extra edge predicates would break multiset equivalence against the Calls-only legacy graph; orphan-skip mirrors `from_call_graph` |
| D5 | `GenericGraphProjectionPort` in `domain/ports/generic_graph_projection.rs`, gated `all(feature = "evidence-kernel", feature = "multimodal")` (GraphNode/GraphEdge are multimodal-gated); adapter `FactGenericGraphProjection` (`infrastructure/graph/generic_graph_projection.rs`) reads `facts_in_snapshot`, emits `GraphNode` per entity (file kind `Symbol(File)`, symbols from defines facts) and `GraphEdge` per relational fact (`EdgeKind::Dependency(DependencyType::…)`; endpoints = entity id strings; unresolved-callee and self-loop edges skipped via `GraphEdge::new` error). Empty snapshot → empty output; rebuild is deterministic by construction. | Gate on `evidence-kernel` alone; emit dangling NodeIds | Type availability forces the dual gate; dangling ids create phantom nodes, breaking the FactStore-ignorance output contract (UAT-U11) |
| D6 | Additive `facts_in_snapshot(ws, snap) -> Vec<Fact>` on the kernel `FactStore` port, implemented by `InMemoryFactStore` (commit order; consumers sort). No default trait body. | Default empty body; scan/iterator API | Only one impl exists — explorer/ladybug implement the legacy `EvidenceStore`, a different port (verified) — so the additive change breaks nobody; a silent-empty default would mask bugs |
| D7 | Harness = Rust integration test `crates/cognicode-core/tests/equivalence_harness.rs` (file-level `#![cfg(feature = "evidence-kernel")]`). Legacy oracle: `AnalysisService::new().build_project_graph(fixture)` → `get_project_graph()` → `from_call_graph`. Fact side: `extract_stage` over fixture → `FactBatchBuilder` → bootstrap + `InMemoryFactStore::commit` → `facts_in_snapshot` → `from_facts`. Comparator: sorted node/edge multisets, per-fixture Jaccard ≥ 0.99 (declared constant), named report; `multi-lang-types` CallGraph comparison declared quarantined up front (legacy node set is walk-order unstable — e36 WU-1 finding); R2 = rebuild from re-read facts equals the prior projection exactly. e36 goldens stay the legacy byte-stability gate (`just lsi-fixtures check`), not re-implemented. New justfile recipe `lsi-equivalence`. | Python/script harness; new bin + JSON report | e36 already proves legacy stability byte-for-byte; a Rust test calls the store/projection types directly with no serialization layer and fails inline naming fixture and score |
| D8 | Perf gate: e36 `lsi_bench_baseline.py compare --fail-above 10` over the unchanged 24 default-path benchmarks proves feature-off no-regression. New e37-owned `benches/fact_bridge_benchmarks.rs` (fact commit 1000 files, `from_facts`, generic projection build) captures an advisory first-run baseline — no threshold in M2; it becomes the M3 baseline. | Hard 10% gate on the bridge path; extending e36's bench file | The M2 exit gate protects the default build; the bridge has no prior baseline, so a hard gate would be fabricated — advisory capture is the honest form |

## Data Flow

```
sandbox/fixtures/{python-hello, rust-hello, multi-lang-types}
   │
   ├─ legacy: AnalysisService::build_project_graph ─► CallGraph ─► CallGraphProjection::from_call_graph
   │
   └─ facts:  extract_file (per file) ─┐
              CodeIntelligenceProvider ┴─► FactBatchBuilder ─► Vec<Fact> (sorted, EntityId/FactId 1..N)
                          │
                          ▼
        bootstrap_registry(InMemorySchemaRegistry) ─► InMemoryFactStore::commit(ws, snap, batch)
                          │
                          ▼
        facts_in_snapshot(ws, snap) ─► CallGraphProjection::from_facts ─► CallGraphProjectionPort (unchanged)
                                    └► FactGenericGraphProjection ─► GraphNode/GraphEdge
                          │
                          ▼
        comparator: sorted node/edge multisets, Jaccard ≥ 0.99, quarantine report
```

```mermaid
sequenceDiagram
    participant H as Harness test
    participant A as AnalysisService (legacy)
    participant B as FactBatchBuilder
    participant R as SchemaRegistry
    participant F as FactStore
    participant P as CallGraphProjection
    H->>A: build_project_graph(fixture)
    A-->>H: CallGraph → from_call_graph
    H->>B: add_extraction(files) / add_provider(observations)
    B->>B: sort records, assign EntityId + FactId
    H->>R: bootstrap_registry (idempotent, 6 core:*)
    H->>F: commit(ws, snap, batch)
    H->>F: facts_in_snapshot(ws, snap)
    F-->>P: facts → from_facts
    H->>H: compare multisets, Jaccard ≥ 0.99; rebuild → equal
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/evidence_kernel/ports.rs` | Modify | Additive `facts_in_snapshot` on `FactStore` (e36-owned file; e37 owns this edit) |
| `crates/cognicode-core/src/domain/evidence_kernel/bootstrap.rs` | Create | Canonical `core:*` set + idempotent `bootstrap_registry` |
| `crates/cognicode-core/src/domain/evidence_kernel/mod.rs` | Modify | `pub mod bootstrap;` |
| `crates/cognicode-core/src/infrastructure/evidence_kernel/in_memory.rs` | Modify | Implement `facts_in_snapshot` (e36-owned file; additive) |
| `crates/cognicode-core/src/application/fact_bridge/{mod,entity_table,batch_builder,tree_sitter_facts,lsp_facts}.rs` | Create | EntityId table, batch builder, both adapters |
| `crates/cognicode-core/src/application/mod.rs` | Modify | cfg-gated `pub mod fact_bridge;` |
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | Modify | Added `from_facts` constructor (cfg-gated) |
| `crates/cognicode-core/src/domain/ports/generic_graph_projection.rs` | Create | Port + `GenericProjection` (dual-gated) |
| `crates/cognicode-core/src/infrastructure/graph/generic_graph_projection.rs` | Create | `FactGenericGraphProjection` adapter |
| `crates/cognicode-core/src/domain/ports/mod.rs`, `infrastructure/graph/mod.rs` | Modify | cfg-gated wiring |
| `crates/cognicode-core/tests/equivalence_harness.rs` | Create | Harness + quarantine constants |
| `crates/cognicode-core/benches/fact_bridge_benchmarks.rs` | Create | Advisory bridge benchmarks |
| `crates/cognicode-runtime/src/lib.rs` | Modify | Gated no-op kernel wiring (stores + bootstrap when feature on) |
| `justfile` | Modify | `lsi-equivalence` recipe (e36 owns `lsi-fixtures`/`lsi-baseline` lines) |
| `crates/cognicode-core/Cargo.toml` | Modify | Only if auto-discovery needs a `[[test]]`/`[[bench]]` entry; no feature changes |

## Interfaces / Contracts

```rust
// domain/evidence_kernel/ports.rs — ADDITIVE
async fn facts_in_snapshot(&self, ws: &WorkspaceId, snap: &SnapshotId)
    -> Result<Vec<Fact>, KernelError>;

// domain/evidence_kernel/bootstrap.rs — NEW
pub const CORE_RELATIONS: [&str; 6] = ["core:calls", "core:imports", "core:contains",
    "core:defines", "core:inherits", "core:references"];
pub fn bootstrap_registry(registry: &dyn SchemaRegistry) -> Result<(), SchemaError>;

// application/fact_bridge — NEW (cfg evidence-kernel)
pub struct EntityIdTable;                       // sorted id strings → EntityId(1..N), per snapshot
pub struct FactBatchBuilder { /* snap, entity strings, relation records */ }
impl FactBatchBuilder {
    pub fn new(snap: SnapshotId) -> Self;
    pub fn add_extraction(&mut self, result: &ExtractionResult);
    pub async fn add_provider(&mut self, p: &dyn CodeIntelligenceProvider, files: &[PathBuf]);
    pub fn finish(self) -> Vec<Fact>;           // canonical sort → ids assigned
}

// infrastructure/graph/call_graph_projection.rs — ADDED constructor
#[cfg(feature = "evidence-kernel")]
pub fn from_facts(facts: &[Fact]) -> Self;      // facts from one pinned snapshot

// domain/ports/generic_graph_projection.rs — NEW (cfg all(evidence-kernel, multimodal))
pub struct GenericProjection { pub nodes: Vec<GraphNode>, pub edges: Vec<GraphEdge> }
#[async_trait]
pub trait GenericGraphProjectionPort: Send + Sync {
    async fn project(&self, ws: &WorkspaceId, snap: &SnapshotId)
        -> Result<GenericProjection, KernelError>;
}
```

Fact grammar (per extracted file `F`, symbol `S`): `(F, core:contains, Text(fqn))`; `(S, core:defines, Text(fqn))` with detail `kind=<K>`; `(S, core:calls, Text(callee_name))`; `(F, core:imports, Text(module))`; `(S, core:references, Text(type_name))`; LSP hierarchy: `(typeE, core:inherits, Text(parent))`.

## Testing Strategy

| Layer | What | Command |
|-------|------|---------|
| Unit kernel | `facts_in_snapshot` isolation; idempotent bootstrap; unregistered-predicate rejection still holds | `cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel` |
| Unit bridge | Double-run identical fact sets (spec scenario); EntityIdTable determinism; ReferenceKind mapping exhaustiveness; LlmAgent still rejected | same filter, `fact_bridge` |
| Unit projections | `from_facts` vs `from_call_graph` on a synthetic graph; generic mapping, empty snapshot, clear→rebuild; self-loop skip | `cargo test -p cognicode-core --lib call_graph_projection --features evidence-kernel` |
| Integration | Harness: 2 fixtures ≥ 0.99, `multi-lang-types` quarantined+reported, below-threshold failure names fixture | `just lsi-equivalence` |
| Guard | Default build unchanged both feature states; fmt; scoped clippy green for new code (pre-existing macros/generic_graph red is known) | `cargo check -p cognicode-core` ± `--features evidence-kernel`; `cargo clippy -p cognicode-core --lib --features evidence-kernel` |
| Perf | Feature-off no-regression ≤ 10% on 24 benchmarks; advisory bridge baseline captured | `just lsi-baseline compare threshold=10`; `cargo bench --bench fact_bridge_benchmarks` |

## Threat Matrix

| Boundary | Applicability |
|----------|---------------|
| Shell/subprocess | Applicable (limited): new justfile recipe runs `cargo test`/`cargo bench` with fixed args, `shell=False`-class, in-repo constant fixture roots, no user-supplied paths; failure = non-zero exit + named report. Same class as e36 D8; no new product shell boundary → no extra product RED tests |
| Routing, git selection, commit/push state, PR commands, executable-file classification | N/A — no VCS/PR automation, routing change, or executable classification |

## Migration / Rollout

No data migration (in-memory only, no persisted format). Feature off by default; runtime wiring is a gated no-op; legacy path untouched and cutover forbidden (spec "No silent cutover" — gate-off scenario covered by `just lsi-fixtures check` byte-stability). Rollback per WU table: delete new modules, revert the three additive e36-file edits and the justfile/runtime lines.

## Work-Unit Slices (auto-chain)

| WU | Scope | Finish / Verify | Rollback |
|----|-------|-----------------|----------|
| 1 Kernel additions | `facts_in_snapshot` port + in-memory impl; `bootstrap.rs` six-predicate set; mod wiring | kernel lib tests green (isolation, idempotency); default `cargo check` unchanged | revert the 3 additive edits |
| 2 Fact bridge | `entity_table`, `batch_builder`, `tree_sitter_facts`, `lsp_facts` + determinism/round-trip tests | `fact_bridge` tests green; scoped clippy clean | delete `application/fact_bridge` + mod.rs line |
| 3 Projections | `from_facts`; `GenericGraphProjectionPort` + adapter + tests (mapping, empty, rebuild) | feature lib tests green; `cargo check` both feature states | delete new files + revert `call_graph_projection.rs` edit |
| 4 Harness | `tests/equivalence_harness.rs` + quarantine constants + `just lsi-equivalence` | harness green (2 scored ≥ 0.99, 1 quarantined reported); `just lsi-fixtures check` still byte-stable | delete test file + justfile recipe |
| 5 Wiring + perf | runtime gated no-op; advisory bridge bench baseline; e36 `compare --fail-above 10`; TESTING-STATE handoff | compare OK; bench captured; handoff recorded | delete bench file + revert runtime edit |

## Open Questions

- [ ] LSP adapter determinism when only `TreesitterFallbackProvider` is available in CI — accepted as deterministic; formal UAT-U10/U11 runs may be deferred with honest reporting (A5).
- [ ] `SymbolKind` fidelity of reconstructed `Symbol`s (provenance-detail convention) — validated against goldens only if a formal UAT-U10 pass runs; non-blocking for the harness score (SymbolId/edge multisets only).
