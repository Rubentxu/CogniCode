# Design: M5 — Program Analysis Core

> Change: `m5-program-analysis-core` | Cycle: `p-c1fac1fea05615c6/m5-program-analysis-core` | Phase: design
> Approach: in-house tree-sitter + evidence-kernel integration; 5–6 WUs; chained PRs

## Technical Approach

M5 builds CFG, DFG, slicing, per-function dominators, interprocedural summaries, and taint v1 entirely on top of the tree-sitter AST integration introduced in M0–M3. New pure-function algorithms land in `cognicode-graph-algos` (WASM-clean by construction, mirroring the `dominators`/`articulation_points` precedent). The evidence kernel (M1) consumes M5 outputs as typed `Fact` rows; replay determinism falls out of the existing snapshot+fact provenance machinery. M5 does NOT introduce new I/O, ports, or schema variants — it adds new algorithm IDs to the analytics registry (`AlgorithmId::from_static("cfg_per_function")`, etc.) and reuses `PlanLimits`, `AlgorithmDescriptor`, `DeterminismKind`, and the existing fixture pattern.

Architecture impact: **local** (additive; no boundary or deployable change).

## Architecture Decisions

### Decision: D1 — Algorithms live in `cognicode-graph-algos`, free functions over flat slices

**Choice**: New algorithms (`cfg`, `dfg`, `slicer_forward`, `slicer_backward`, `dominators_cfg`, `interproc_summary`, `taint_flow`) are added as free functions in `crates/cognicode-graph-algos/src/algorithms/`, mirroring existing `dominators.rs`.
**Alternatives**: (a) Pure-Rust AST visitors in `cognicode-core` (would couple to domain types and break WASM); (b) external IR (SCIP/LSIF — explicitly out of scope per M4 final proposal).
**Rationale**: `cognicode-graph-algos` is WASM-clean by construction (`petgraph-adapter` feature-gated). Mirroring existing pattern (`dominators.rs`) keeps the cycle reviewable by precedent.

### Decision: D2 — Algorithm descriptors live in `crates/cognicode-core/src/domain/analytics/`

**Choice**: One descriptor file per algorithm kind: `cfg_descriptor.rs`, `dfg_descriptor.rs`, `slicing_descriptor.rs`, `interproc_summaries_descriptor.rs`, `taint_descriptor.rs`. Each implements `AlgorithmDescriptor` + `AlgorithmParams` + fixture harness.
**Alternatives**: (a) Single mega-descriptor (would exceed per-file budget; harder to test); (b) descriptors in `application/` (would break the registry's domain-layer location convention).
**Rationale**: Matches existing pattern (`pagerank_descriptor.rs`, `dominators_descriptor.rs`). One descriptor per file keeps each WU reviewable under the 400-line budget.

### Decision: D3 — Function scope comes from the existing parser symbol table

**Choice**: Per-function scope is defined by `Symbol` entries with `SymbolKind::Function` from the existing `domain::aggregates::Symbol` model; no new symbol kinds introduced.
**Alternatives**: (a) New `FunctionScope` aggregate (overlaps with existing `Symbol`); (b) tree-sitter top-level `function_item`/`function_declaration` traversal independent of symbols (loses identity).
**Rationale**: `Symbol` already carries FQN, location, kind. Reusing it gives M5 the same identity as M1–M4 (SubjectIndex, continuity). New `SymbolKind` would ripple bincode — out of scope.

### Decision: D4 — Output determinism via canonical serialization + digest pin

**Choice**: Every M5 output is serialized through the existing canonical encoder (sorted node ids, sorted outgoing edges, stable entry node, no timestamps) and pinned to a digest. Pins live in `crates/cognicode-core/tests/program_analysis_conformance.rs` alongside the harness, named `PINNED_CFG_DIGEST`, `PINNED_DFG_DIGEST`, `PINNED_SLICE_DIGEST`, `PINNED_DOMINATORS_DIGEST`, `PINNED_SUMMARY_DIGEST`, `PINNED_TAINT_DIGEST`. Re-pin is a dedicated commit.
**Alternatives**: (a) Golden fixtures per WU (M0 pattern; less granular); (b) snapshot-based equality (M1 pattern; brittle to unrelated changes).
**Rationale**: Digest pins are the proven pattern from M2/M3 (`PINNED_IDENTITY_DIGEST`, `PINNED_MATCHER_DIGEST`). One pin per algorithm kind isolates drift to the algorithm, not the test corpus.

### Decision: D5 — `PlanLimits` enforcement per algorithm

**Choice**: Each descriptor validates a `PlanLimits` value (max nodes, max edges, max wall-clock, max memory). Exceeding returns `AnalyticsError::LimitExceeded{nodes, edges, wall_clock_ms, memory_bytes}` — no partial facts emitted.
**Alternatives**: (a) Soft limits with truncation (silently degrades correctness); (b) per-algorithm limits outside the registry (breaks registry contract).
**Rationale**: The registry already owns `PlanLimits`. Reusing it gives every M5 algorithm the same resource contract as the existing 9 algorithms.

### Decision: D6 — Taint v1 sources/sinks declared per language, not auto-discovered

**Choice**: Sources and sinks are declared as static identifier-or-call patterns per language in `crates/cognicode-core/src/domain/analytics/taint_descriptor.rs::TaintPatterns`. A pattern declares: language, kind (source/sink/untaint), match shape (identifier name regex or call callee), optional LSP-resolved symbol identity.
**Alternatives**: (a) Full auto-discovery via dataflow (defeats the v1 suffix; expensive); (b) LSP-only (degrades serverless; contradicts M4 tier-aware design).
**Rationale**: Declarations are auditable, testable, and tier-aware. M4's tier system classifies paths: declared with LSP symbol identity → Extracted, declared with local resolver → Inferred, declared with only tree-sitter heuristics → Ambiguous.

### Decision: D7 — Interprocedural summaries are bottom-up with a fixed-point marker

**Choice**: Summaries compute callee first (depth-first, memoized). Recursion is detected via a per-snapshot call-graph SCC computation; recursive cycles emit a `FixedPoint` summary marker instead of expanding.
**Alternatives**: (a) Top-down inline (explodes stack on deep call chains); (b) worklist until convergence (correct but unbounded cost).
**Rationale**: Bottom-up + SCC detection matches classic interprocedural analysis (Sharir-Pnueli family). SCC computation reuses existing `cognicode-graph-algos::condensation`. Cost is bounded by `PlanLimits::max_summary_depth`.

### Decision: D8 — WASM exposure scope: CFG, dominators, slicing only

**Choice**: WASM-exposed algorithms (no feature flag): CFG, dominators-per-function, slicing (forward/backward). NOT WASM-exposed (gated behind `program-analysis-server` feature): DFG (depends on type info), interprocedural summaries (depends on call-graph SCC over a snapshot), taint v1 (depends on tier-aware pattern matching that pulls LSP).
**Alternatives**: (a) All WASM-exposed (would require stubbing LSP at runtime); (b) None WASM-exposed (defeats browser parity).
**Rationale**: Browser consumers get the deterministic pure-function algorithms; server consumers get the LSP-aware variants. Feature flag mirrors the existing `evidence-kernel`/`multimodal` feature gating pattern.

## Data Flow

```
        tree-sitter AST (existing)
                  │
                  ▼
       ┌─────────────────────────┐
       │  parser::Symbol table   │ (existing, reused for function scope)
       └────────────┬────────────┘
                    │
                    ▼
       ┌─────────────────────────┐
       │  AlgorithmDescriptor    │ (new per D2)
       │  (cfg/dfg/slicer/...)   │
       └────────────┬────────────┘
                    │
        ┌───────────┼───────────┐
        ▼           ▼           ▼
   algorithm    algorithm    algorithm
   (graph-algos, free fns, WASM-clean per D1)
        │           │           │
        └───────────┼───────────┘
                    ▼
       ┌─────────────────────────┐
       │  canonical serialization│ (deterministic, sorted, no timestamps)
       └────────────┬────────────┘
                    ▼
       ┌─────────────────────────┐
       │  digest pin harness     │ (per D4)
       │  PINNED_*_DIGEST        │
       └────────────┬────────────┘
                    │
                    ▼
       ┌─────────────────────────┐
       │  MCP handlers           │ (new in interface/mcp/handlers/)
       │  graph_handlers or new  │
       └─────────────────────────┘
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-graph-algos/src/algorithms/cfg.rs` | Create | Per-function CFG extraction from tree-sitter AST (per D1, D3) |
| `crates/cognicode-graph-algos/src/algorithms/dfg.rs` | Create | DFG from CFG (per D1) |
| `crates/cognicode-graph-algos/src/algorithms/slicer.rs` | Create | Forward + backward slicer over CFG (per D1) |
| `crates/cognicode-graph-algos/src/algorithms/dominators_cfg.rs` | Create | Per-function dominators (separate from call-graph `dominators.rs`) (per D1) |
| `crates/cognicode-graph-algos/src/algorithms/interproc_summary.rs` | Create | Bottom-up summary with SCC recursion marker (per D7) |
| `crates/cognicode-graph-algos/src/algorithms/taint_flow.rs` | Create | Forward flow-sensitive taint with declared patterns (per D1, D6) |
| `crates/cognicode-core/src/domain/analytics/cfg_descriptor.rs` | Create | AlgorithmDescriptor + PlanLimits + fixture harness (per D2) |
| `crates/cognicode-core/src/domain/analytics/dfg_descriptor.rs` | Create | AlgorithmDescriptor |
| `crates/cognicode-core/src/domain/analytics/slicing_descriptor.rs` | Create | AlgorithmDescriptor |
| `crates/cognicode-core/src/domain/analytics/interproc_summaries_descriptor.rs` | Create | AlgorithmDescriptor |
| `crates/cognicode-core/src/domain/analytics/taint_descriptor.rs` | Create | AlgorithmDescriptor + TaintPatterns per language (per D6) |
| `crates/cognicode-core/src/domain/analytics/lineage.rs` | Modify | Register 6 new `AlgorithmId` values (`cfg_per_function`, `dfg`, `slice_forward`, `slice_backward`, `dominators_cfg`, `interproc_summary`, `taint_flow`) |
| `crates/cognicode-core/src/domain/analytics/mod.rs` | Modify | Re-export new descriptors |
| `crates/cognicode-core/src/application/program_analysis_service.rs` | Create | Service facade: orchestrates algorithm runs, applies PlanLimits, surfaces errors |
| `crates/cognicode-core/src/interface/mcp/handlers/program_analysis_handlers.rs` | Create | MCP handlers: `get_cfg`, `get_dfg`, `slice_backward`, `slice_forward`, `dominators_per_function`, `summary_call`, `taint_flow` |
| `crates/cognicode-core/src/interface/mcp/handlers/mod.rs` | Modify | Register new handlers |
| `crates/cognicode-core/tests/program_analysis_conformance.rs` | Create | Conformance suite: 6 digest pins, replay-guard, perf envelope (per D4, D5) |
| `crates/cognicode-core/Cargo.toml` | Modify | Feature flag `program-analysis-server` (per D8); no new external deps |
| `crates/cognicode-graph-algos/Cargo.toml` | Modify | No new deps (tree-sitter is in cognicode-core; graph-algos stays pure) |
| `sandbox/fixtures/lsi-program-analysis/{rust,ts,java}/` | Create | Per-language canonical fixtures: 1 source + 1 expected digest per algorithm (per conformance spec) |
| `sandbox/scripts/run_lsi_program_analysis.py` | Create | Recipe runner mirroring `lsi-providers` pattern |
| `justfile` | Modify | Add `lsi-program-analysis` recipe + `program-analysis-conformance` recipe |
| `perf-budget.toml` | Modify | Add per-algorithm entries for the 6 new algorithms |

## Interfaces / Contracts

```rust
// New in crates/cognicode-core/src/domain/analytics/cfg_descriptor.rs
pub const CFG_PER_FUNCTION: AlgorithmId = AlgorithmId::from_static("cfg_per_function");
pub const DFG: AlgorithmId = AlgorithmId::from_static("dfg");
pub const SLICE_FORWARD: AlgorithmId = AlgorithmId::from_static("slice_forward");
pub const SLICE_BACKWARD: AlgorithmId = AlgorithmId::from_static("slice_backward");
pub const DOMINATORS_CFG: AlgorithmId = AlgorithmId::from_static("dominators_cfg");
pub const INTERPROC_SUMMARY: AlgorithmId = AlgorithmId::from_static("interproc_summary");
pub const TAINT_FLOW: AlgorithmId = AlgorithmId::from_static("taint_flow");

// New in crates/cognicode-core/src/application/program_analysis_service.rs
pub struct ProgramAnalysisService {
    registry: Arc<AnalyticsRegistry>,
    parser: Arc<dyn SymbolRepository>,
}

impl ProgramAnalysisService {
    pub async fn run(
        &self,
        algorithm: AlgorithmId,
        snapshot: SnapshotDescriptor,
        params: serde_json::Value,
        limits: PlanLimits,
    ) -> Result<RunOutput, AnalyticsError>;
}
```

```rust
// New in crates/cognicode-graph-algos/src/algorithms/cfg.rs
pub fn cfg_from_function(
    symbol: &Symbol,
    ast: &tree_sitter::Tree,
    source: &[u8],
) -> Cfg;

pub struct Cfg {
    pub entry: CfgNodeId,
    pub nodes: Vec<CfgNode>,
    pub edges: Vec<(CfgNodeId, CfgNodeId)>,
}

impl Cfg {
    pub fn canonical_bytes(&self) -> Vec<u8>; // sorted, deterministic
    pub fn digest(&self) -> u64; // fnv1a64
}
```

## Architecture Model

- **Impact**: `local` (additive; no new ports, no new schema variants, no boundary changes)
- **Observed baseline**: existing `cognicode-core` domain already owns analytics registry, evidence kernel, plan limits, parser integration, fact bridge
- **Planned intent**: extend analytics registry with 6 new algorithm IDs; new descriptors in `domain/analytics/`; new service in `application/`; new handlers in `interface/mcp/handlers/`. No C4 view changes (no new containers or deployables).
- **Render**: not applicable (impact: local)

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|---------|
| Unit (graph-algos) | CFG/DFG/slicer/dominators/taint correctness per language | `cargo test -p cognicode-graph-algos cfg dfg slicer dominators_cfg taint_flow` |
| Unit (analytics registry) | Each descriptor: param validation, fixture execution, output schema, determinism class | `cargo test -p cognicode-core --lib program_analysis` |
| Integration | Conformance suite: 6 digest pins, replay-guard, perf envelope | `cargo test -p cognicode-core --test program_analysis_conformance` |
| Server (gated) | MCP handler round-trip, PlanLimits enforcement | `cargo test -p cognicode-core --lib mcp_handlers --features program-analysis-server` |
| Sandbox | Per-language fixtures: rust + ts + java serverless + java LSP-gated | `just lsi-program-analysis` |

## Migration / Rollout

No migration required. All changes are additive. New MCP tools appear in tool catalogs on next build. Feature flag `program-analysis-server` is OFF by default; existing deployments see no change. WASM builds compile only the WASM-exposed algorithms (per D8).

## Open Questions

- [ ] Language coverage order: rust first (full), then ts (serverless), then java (gated on jdtls) — to confirm in Propose question round
- [ ] Should taint v1 include explicit "sensitive" patterns (e.g., `std::env::var`, `request.headers.get`) or only generic I/O? — design decision needed
- [ ] MCP tool naming: snake_case (`get_cfg`) or camelCase (`getCfg`) for backward compat? — naming decision needed (current MCP convention is snake_case)
- [ ] Interprocedural summary cache lifetime: per-snapshot only (rebuilt on snapshot change) or persistent across snapshots? — cache eviction strategy
- [ ] Slicing criterion vocabulary: just (variable, definition-site) or also (expression, statement-site)? — expressiveness vs simplicity

## ADR Candidates

- **D1** (algorithms in graph-algos, WASM-clean): hard to reverse (touches every WASM build) + surprising (why not in core?) + trade-off (extra trait plumbing) → ADR candidate
- **D4** (digest pin protocol): hard to reverse (re-pin is operational ceremony) + surprising (why not golden fixtures?) + trade-off (granularity vs maintenance) → ADR candidate
- **D7** (bottom-up summaries with SCC fixed-point): hard to reverse (changes complexity class) + surprising (why not worklist?) + trade-off (cost vs correctness on recursion) → ADR candidate
- **D8** (WASM scope split): hard to reverse (locks browser parity decision) + surprising (why split?) + trade-off (browser parity vs completeness) → ADR candidate