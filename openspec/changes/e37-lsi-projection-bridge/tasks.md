# Tasks: E37 LSI Projection Bridge

> Auto-chain, stacked-to-main: WUs in-tree, revertable; commits/PRs await approval. e36 shares tree (e37: new files + 3 additive edits, design.md header); threat shell-limited; N/A omitted.

## Review Workload Forecast

|Field|Value|
|---|---|
|Estimated changed lines|~1200–1500 excl. goldens|
|400-line budget risk|High|
|Chained PRs recommended|Yes|
|Suggested split|kernel→bridge→projections→harness+perf→wiring (PR1–PR5)|
|Delivery strategy|auto-chain|
|Chain strategy|stacked-to-main|

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

|Unit|Goal|Likely PR|Focused test command|Runtime harness|Rollback boundary|
|---|---|---|---|---|---|
|1|Kernel|PR1|`cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel`|N/A (in-memory)|revert 3 e36 edits|
|2|Fact bridge|PR2|`cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel`|double-run|rm `application/fact_bridge/`|
|3|Projections|PR3|`cargo test -p cognicode-core --lib call_graph_projection --features evidence-kernel`|synthetic compare|rm new files + edit|
|4|Harness+perf|PR4|`cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel`|`just lsi-equivalence` + bench compare|rm test/bench + recipe|
|5|Wiring+records|PR5|`cargo check -p cognicode-core` ±feature|N/A (gated no-op)|revert runtime + docs|

## Phase 1: Kernel additions (WU-1)

- [x] 1.1 RED `in_memory.rs`: `facts_in_snapshot` returns only its snapshot's facts, commit order; compile fails.
- [x] 1.2 `facts_in_snapshot(ws,snap)->Result<Vec<Fact>,KernelError>` additive in `domain/evidence_kernel/ports.rs` (no default body); impl `infrastructure/evidence_kernel/in_memory.rs`.
- [x] 1.3 `domain/evidence_kernel/bootstrap.rs`: `CORE_RELATIONS` six `core:*`; idempotent `bootstrap_registry`; wire `evidence_kernel/mod.rs`.
- [x] 1.4 GREEN command (idempotency, rejection); feature-off `cargo check -p cognicode-core` unchanged.

## Phase 2: Fact bridge (WU-2)

- [x] 2.1 RED `FactBatchBuilder` double-run on `sandbox/fixtures/rust-hello`→identical facts ("Repeated extraction is identical"); compile fails.
- [x] 2.2 `application/fact_bridge/{mod,entity_table,batch_builder}.rs`: `EntityIdTable` snapshot-scoped sorted strings→`EntityId(1..N)`; `finish()` sorts by (subject,predicate,object) (D3); cfg `pub mod fact_bridge;` (`application/mod.rs`).
- [x] 2.3 `application/fact_bridge/tree_sitter_facts.rs`: `ExtractionResult`→contains/defines(`kind=<K>`)/calls/imports/references; FQN `"{file}:{name}:{line}"`.
- [x] 2.4 `application/fact_bridge/lsp_facts.rs`: `&dyn CodeIntelligenceProvider`; 5 `ReferenceKind`→(Call→calls, Import→imports, Read/Write/Type→references, `ref=<Kind>`); parents→inherits.
- [x] 2.5 GREEN command (determinism, mapping, LlmAgent rejection); scoped clippy.

## Phase 3: Projections, RED first (WU-3)

- [x] 3.1 RED: `from_facts` vs `from_call_graph` synthetic multisets equal, unresolved dropped+counted; "Facts map to nodes and edges", "Empty snapshot yields empty projection".
- [x] 3.2 `CallGraphProjection::from_facts(&[Fact])` in `infrastructure/graph/call_graph_projection.rs`: defines→`SymbolId(fqn)` nodes, calls-only edges, lowercase resolution, lexicographic tie-break.
- [x] 3.3 `domain/ports/generic_graph_projection.rs` + `infrastructure/graph/generic_graph_projection.rs` (dual gate `all(evidence-kernel,multimodal)`, D5): `GenericProjection{nodes,edges}`; adapter reads `facts_in_snapshot`, emits only `GraphNode`/`GraphEdge` ("Consumer needs only emitted types"); wire `domain/ports/mod.rs`+`infrastructure/graph/mod.rs`.
- [x] 3.4 GREEN command incl. "Clear and rebuild is equivalent"; ±feature `cargo check -p cognicode-core`.

## Phase 4: Harness + perf (WU-4)

- [x] 4.1 RED `cognicode-core/tests/equivalence_harness.rs` (kernel cfg): named report ("Fixture below threshold fails the run", "Unquarantined divergence fails").
- [x] 4.2 Oracle `AnalysisService::build_project_graph`→`from_call_graph` (`python-hello`,`rust-hello`); extract_stage→`FactBatchBuilder`→`bootstrap_registry`→commit→`facts_in_snapshot`→`from_facts`; sorted multiset compare, `EQUIVALENCE_THRESHOLD=0.99` (R1).
- [x] 4.3 Quarantine `multi-lang-types` KNOWN_UNSTABLE: excluded+reported ("Quarantined divergence is excluded and reported"); digest pin, change⇒re-pin ("Convention change requires explicit re-baseline"); green "All fixtures meet the threshold"; R2 rebuild equivalence.
- [x] 4.4 `lsi-equivalence` recipe (fixed argv); `just lsi-fixtures check` byte-stable ("Gate off serves the legacy path").
- [x] 4.5 `crates/cognicode-core/benches/fact_bridge_benchmarks.rs` advisory baseline (D8); `cargo bench --bench fact_bridge_benchmarks`; `just lsi-baseline compare threshold=10`.

## Phase 5: Wiring + records (WU-5)

- [x] 5.1 `cognicode-runtime/src/lib.rs` gated no-op (stores+bootstrap): zero default-path change ("Failing harness keeps legacy default").
  > Determination (honest): **no runtime edit needed or made**. Gating is at the `cognicode-core` crate level — the runtime's default dependency graph compiles zero e37 code (`cargo check -p cognicode-runtime` exit 0, feature off), so "gate off serves the legacy path" holds structurally and a failing harness cannot affect the default. Runtime wiring of stores+bootstrap stays M3 work (e36 in-memory adapters: "future feature-gated runtime wiring"); wiring now would be dead code (no consumer, cutover forbidden in M2) and out of this batch's allowed edit roots. Design file-table row `runtime/src/lib.rs Modify` resolved as "nothing to wire in M2".
- [x] 5.2 UAT-U10/U11 in `docs/CogniCode_Living_Software_Intelligence/docs/uat/UAT-MILESTONES.md` (honest): formal runs deferred; harness score + FactStore-ignorance as evidence.
- [x] 5.3 `.agent/TESTING-STATE.md` handoff (fresh/stale); sweep WU1–4 commands, `cargo check -p cognicode-core` ±feature, clippy; never full suite.
