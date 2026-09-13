# Exploration: e37-lsi-projection-bridge (LSI M2 — Projection Bridge)

> Evidence-backed current-state map for umbrella tasks 3.1–3.6. All paths relative to repo root.

## Current State

### 1. Tree-sitter → CallGraph today (legacy source of truth)

- Extraction: `crates/cognicode-core/src/application/ingest/extractor.rs:24` (`extract_file`) walks the tree-sitter AST via `LanguageConfig` and emits `GraphNode` + `ExtractionEdge` (same-file `Calls`, `Imports`, file→symbol `Contains`; unresolved callees as `TargetRef::Unresolved`). Parallel orchestration: `application/ingest/extract_stage.rs:27,41` (`extract_all` rayon / `extract_streaming` bounded-mpsc, ADR-017/021/023).
- **Production CallGraph path is separate**: `application/services/analysis_service.rs:196` `build_project_graph` — `ignore::WalkBuilder` walk → rayon parse via `TreeSitterParser::with_cache` (`:292`) → per-file symbols + `(caller, callee_name)` pairs → **name-based resolution** into `name_to_symbol_id` keyed by `symbol.name().to_lowercase()` (`:349`-area) → `PetGraphStore` → `store.to_call_graph()` → `graph_cache.set()` (`:404`). Symbol identity = FQN `"{file}:{name}:{line}"` (see test helper `call_graph_projection.rs:812-823`).
- Storage/cache: `infrastructure/graph/graph_cache.rs:42-137` (`ArcSwap<Mutex<VersionedGraphCache>>` ring of `Arc<CallGraph>`); read-side port `SnapshotProvider` (`infrastructure/graph/snapshot_provider.rs:62`, `(ws, RevisionId) → Arc<CallGraph>`); reload via `application/ingest/refresh.rs:17` (`refresh_from_pg`).

### 2. GenericGraph today

- Aggregate: `domain/aggregates/generic_graph.rs` (module doc :1-18) — `NodeId(String)`, `GraphNode` (kind `NodeKind`, property map), `GraphEdge` (`EdgeKind`, `Provenance`, confidence [0,1], metadata map). Feature-gated `multimodal`.
- Ingest: `SourceExtractor` port (`domain/traits/source_extractor.rs`) — `DocsExtractor`/`IssuesExtractor` (`infrastructure/extraction/`) feed the Generic Graph Layer through MCP `docs_ingest`/`issues_ingest` (`cognicode-explorer/src/mcp/handler/ingest.rs:94,393`) into `GraphRepository` (`domain/ports/graph_repository.rs:70`; explorer adapters `in_memory_graph_repository.rs`, `call_graph_repository.rs`).
- Persistence: `IngestCommitPort` (`domain/ports/ingest_commit_port.rs:46` `GraphDelta{nodes, edges, deleted_node_ids}`; trait :81) — atomic revision publication, implemented by Ladybug (`cognicode-ladybug/src/lib.rs:1136` `commit_revision`; generic-graph schema DDLs :2438/:2636). Runtime wires it at `cognicode-runtime/src/lib.rs:284`. Postgres was removed (e29-7); Ladybug is the store.
- `RevisionId(u64)`, `NONE=0` sentinel never valid (`domain/value_objects/revision_id.rs:21-28`). e36 `SnapshotId(u64)` is bijective with it per workspace (D4).

### 3. Projection seam (ADR-029)

- Port: `domain/ports/call_graph_projection.rs` — trait `CallGraphProjectionPort` (:120-216, ~19 ops: adjacency builders, topo sort, SCC/WCC, cycles, has_path, dijkstra, impact radius, forward reach, subgraph, explain_path); object-safe factory `project_call_graph(&CallGraph)` (:107). Types `SubgraphView`/`ExplanationView` live in the port.
- Adapter: `infrastructure/graph/call_graph_projection.rs` — `CallGraphProjection::from_call_graph` (:138) snapshots `StableGraph<SymbolId, (DependencyType, f64)>`; `sanitize_confidence` (:105); Dijkstra cost `1-conf` (:120).
- **Projects FROM the in-memory `CallGraph` aggregate only — never from facts.**
- Consumers (all via factory): `application/services/graph_analytics.rs` (7 sites), `graph_insights.rs:93`, `impact_analysis.rs` (9+ sites), and 10 domain analytics descriptors in `domain/analytics/` (pagerank, personalized_pagerank, scc, wcc, kcore, modularity, bridges, articulation, conductance, dominators) → MCP `graph_analyze`/`impact` + Explorer views.
- **No GenericGraph projection port exists.** Generic graph consumers use `GraphRepository` + `cognicode-graph-algos` `GraphBuilder` (`graph_builder.rs:44`). Task 3.4 is new surface; `CURRENT-TO-TARGET-MAPPING.md:10` maps `GenericGraph → KnowledgeGraphProjection` ("mantener compatibilidad").

### 4. FactBatch / kernel gaps (vs e36)

- No `FactBatch` type in code (only umbrella docs). e36 `FactStore::commit(ws, snap, batch: Vec<Fact>)` (`domain/evidence_kernel/ports.rs:69-74`) **is** the atomic batch commit (rejects LlmAgent provenance, unregistered predicate, snapshot mismatch).
- Missing for M2:
  1. **Extract→Fact adapters** (tasks 3.1/3.2) — nothing converts extractor output to `Fact`s yet.
  2. **Relation vocabulary**: `SchemaRegistry` is append-only and ships empty; extraction predicates (`core:calls`, `core:imports`, `core:contains`, `core:defines`, `core:inherits`, `core:references`) must be registered as a canonical bootstrap set.
  3. **EntityId mapping**: `EntityId(u64)`/`FactValue::Ref` vs string `SymbolId` FQNs — needs a deterministic, golden-pinned convention (raw FQN in `FactValue::Text` is the safe default; `Ref` requires an entity table that does not exist yet).
  4. **Port gap**: `FactStore` only reads by subject (`facts_of`); projection rebuild needs "all facts of a snapshot" (scan/iterate). Additive port extension required (only in-memory impl exists, so safe).

### 5. LSP observations (fact candidates)

- `CodeIntelligenceProvider` (`domain/traits/code_intelligence.rs:13-47`): `get_symbols`, `find_references` → `Reference{location, reference_kind: Read|Write|Call|Type|Import, container}` (:51-73), `get_hierarchy` → parents/children (:91-108), `get_definition`, `get_document_symbols`, `hover`.
- Implementations: `CompositeProvider` (`infrastructure/lsp/providers/composite.rs:41,120`, LSP-first + `TreesitterFallbackProvider`), `LspIntelligenceProvider` (`providers/lsp.rs`, JSON-RPC over `LspProcessManager`).
- Umbrella authority table (`cognicode-living-software-intelligence/design.md:74-81`): Tree-sitter adapter and LSP/compiler adapter MAY write Facts (producer `DeterministicAnalyzer` / `RuntimeObserver`).

### 6. Equivalence harness assets (e36 reuse)

- `sandbox/scripts/capture_lsi_fixtures.py`: per-fixture goldens for `cli_graph_full/mermaid/impact/hierarchy/trace_path/entry_points/leaf_functions/on_demand` + `cli_graph_per_file` + `cli_index_outline` (:146-222) + MCP `build_graph`, `analyze_impact`, `get_file_symbols`, `query_symbol_index` (:231-252) over `python-hello`, `rust-hello`, `multi-lang-types` (:66-91); `inventory.json` = 4 sources / 48 consumer surfaces; `KNOWN_UNSTABLE_SURFACES` quarantine (:117).
- `lsi_bench_baseline.py compare --fail-above N` is the perf gate; e36 fresh worst delta +14.18% (`symbol_index_build_1000_files`) within 25% threshold.
- UAT-U10 (`docs/CogniCode_Living_Software_Intelligence/docs/uat/UAT-MILESTONES.md:44-47`): same golden repo, run call-graph tools via legacy AND projection path → structural results match within declared tolerance. UAT-U11 (:49-52): Explorer uses projected CallGraph → critical views keep working **without knowing the FactStore**.

## Umbrella contract (acceptance criteria — do not duplicate in delta)

`openspec/changes/cognicode-living-software-intelligence/specs/projection-architecture/spec.md` has exactly 2 requirements: (R1) **CallGraph projection equivalence** — golden equivalence of normalized nodes/edges vs legacy before cutover; (R2) **Derived projections are rebuildable** — clear projection storage, rebuild from pinned canonical inputs, result equivalent. Plus `evidence-kernel/spec.md` fact-provenance/snapshot-pinning constraints (implemented by e36). Design debt rule (`CURRENT-TO-TARGET-MAPPING.md:29`): "No sustituir CallGraph antes de equivalence harness."

## Approaches

1. **A — Parallel fact-sourced projections behind existing seams (strangler)** — new cfg-gated adapters: TreeSitter→`Vec<Fact>` (reusing `extract_file` output), LSP-observations→`Vec<Fact>`, `CallGraphProjection::from_facts` exposed through `project_call_graph`-style factory, GenericGraphProjection building `Vec<GraphNode>`/`Vec<GraphEdge>` from facts behind a new thin port, structural-equivalence harness as Rust integration tests over `sandbox/fixtures/*` comparing legacy vs fact-derived graphs (normalized sorted node/edge multisets) + e36 goldens as regression gate.
   - Pros: zero default-path risk; satisfies R1+R2 exactly; reuses ADR-029 port, e36 M0 gates and `GraphNode`/`GraphEdge` types (UAT-U11 compat for free).
   - Cons: needs the `facts_in_snapshot` port addition and a pinned EntityId convention.
   - Effort: Medium.
2. **B — Cut CallGraph construction over to FactStore now** — AnalysisService reads facts as its source.
   - Pros: single source of truth immediately.
   - Cons: violates debt rule 6; high blast radius on perf gate, MCP/Explorer, UAT-U10/U11.
   - Effort: High. Not recommended for M2.
3. **C — Shadow bridge subset (3.1+3.3+3.5 only, defer LSP + GenericGraph)**.
   - Pros: smallest slice.
   - Cons: leaves umbrella tasks 3.2/3.4 undone; UAT-U11 weakly covered.
   - Effort: Low-Medium.

Incremental vs batch: **batch rebuild first** (matches R2 "clear → rebuild" literally and e36 snapshot pinning); incremental/differential derivation is a later milestone (umbrella design.md:89 — Differential Dataflow gated by SPIKE-011).

## Recommendation

Option **A**, batch-rebuild-first, gated behind the existing off-by-default `evidence-kernel` cargo feature (facts already live there; ADR-029 port itself stays ungated since e29-3). Implement 3.1/3.2 as deterministic fact producers over the existing extractor/provider surfaces, 3.3 as `from_facts` feeding the unchanged `CallGraphProjectionPort` (all 10 descriptors + services keep working), 3.4 as a minimal `GenericGraphProjectionPort` mirroring ADR-029 that emits `GraphNode`/`GraphEdge` so Explorer consumers stay untouched, and 3.5 as a Rust harness (fixture → legacy graph vs facts → projected graph, normalized comparison) reusing `capture_lsi_fixtures.py` goldens as the byte-stable legacy oracle. Add one additive `FactStore` read (`facts_in_snapshot`) to close the rebuild gap, and register the canonical `core:*` relation bootstrap set.

## Risks

- **Same-name collapse / engine nondeterminism**: legacy resolution keys by lowercase name (`analysis_service.rs:349`) with unsorted walk — KNOWN_UNSTABLE_SURFACES (3 quarantined surfaces). A fact-derived projection resolving by FQN may legitimately differ there; equivalence must exclude quarantined fixture/surface pairs or declare tolerance.
- **Perf gate**: full-build regression ≤10% gate (M2 exit, state.yaml) — keep the bridge additive/off-by-default; `symbol_index_build_1000_files` is already noise-prone (+14.18% fresh in e36 verify).
- **Feature-gate drift**: reusing `evidence-kernel` keeps one flag; adding explorer wiring may tempt a second flag — keep runtime wiring a gated no-op like e36.
- **EntityId instability**: ad-hoc hashing of FQNs→u64 would break golden stability; pin the convention in the harness and round-trip tests.
- **tree-sitter version**: brief said 0.20 — stale; workspace pins `tree-sitter = "0.24"`, grammars 0.23 (workspace `Cargo.toml:42-53`). No version work in e37; do not bump.
- **e36/e37 working-tree entanglement**: both changes uncommitted on `main`; e37 files must stay disjoint from `evidence_kernel/**` + M0 harness except the additive `FactStore` port method (coordinate commit ownership).

## Ready for Proposal

Yes — recommend sdd-propose with: scope = tasks 3.1–3.6; approach A; batch rebuild; `evidence-kernel` feature; additive `facts_in_snapshot` port method + `core:*` schema bootstrap called out as explicit deltas; equivalence tolerance + quarantine policy declared up front.
