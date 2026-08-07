# Design: Impact Analysis Service

## Technical Approach

Stateless application service that constructs a `CallGraphProjection` per method call from an immutable `&CallGraph`, then delegates to projection algorithms. Follows `AnalysisService` pattern (struct + `new()`) but holds zero state — every method is `&self` + `&CallGraph`. No existing files modified except two 1-line additions (`mod.rs`, `dto/mod.rs`).

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| Struct shape | `pub struct ImpactAnalysisService;` (zero-sized) | Hold `CallGraphProjection` field | Spec R1: stateless. Projection is rebuilt per-call from `&CallGraph`. |
| Ownership model | Borrow `&CallGraph` per call | Own/wrap `CallGraphProjection` | Read-only consumption (R8). No interior mutability. Projection is ephemeral. |
| Predecessor naming | `impact_radius` = predecessors (reverse BFS) | `predecessor_radius`, `upstream_impact` | Matches existing `CallGraphProjection::find_impact_radius` API name. "What depends on X" semantics. |
| `usize::MAX` sentinel | Pass through to `find_impact_radius` unchanged | Custom `ImpactDepth::Unbounded` enum | `find_impact_radius` already handles this natively (bounded BFS with `usize::MAX` = effectively unbounded). No newtype needed. |
| Cycle-group semantics | SCCs with `len() >= 2` only; self-loops excluded | Include self-loops as size-1 SCCs | Spec R5: "non-trivial SCCs of size >= 2". Matches `CycleDetector` convention. |
| NaN confidence | Delegated to `sanitize_confidence` (NaN → 1.0 → cost 0.0) | Custom NaN handling in service | `CallGraphProjection` already normalizes at construction. Service sees only valid costs. |
| Missing symbol result | `vec![]` / `false` / `None` — never panic | `Result<..., Error>` | Projection methods already return these sentinel values. Wrapping in `Result` adds no information. |
| DTO String conversion | `SymbolId::as_str().to_string()` | `SymbolId::to_string()` (Display impl) | Existing DTOs use `as_str().to_string()` pattern (see `ImpactDto`, `CycleDto`). Both produce identical output but match convention. |

## Data Flow

```
Caller                     ImpactAnalysisService              CallGraphProjection
  │                              │                                    │
  ├── impact_radius(&graph, root, depth) ──────────────────────►│
  │                              ├── CallGraphProjection::          │
  │                              │   from_call_graph(&graph)        │
  │                              ├── .find_impact_radius(root, depth)
  │                              │                                    │
  │   ◄── Vec<SymbolId>         │◄── Vec<SymbolId>                  │
  │                              │                                    │
  ├── shortest_path(&graph, from, to) ─────────────────────────►│
  │                              ├── from_call_graph(&graph)         │
  │                              ├── .dijkstra(from, to)             │
  │                              ├── map → PathResultDto             │
  │   ◄── Option<PathResultDto> │◄── Option<(Vec<SymbolId>, f64)>  │
  │                              │                                    │
  ├── detect_cycles(&graph) ────────────────────────────────────►│
  │                              ├── from_call_graph(&graph)         │
  │                              ├── .strongly_connected_components()│
  │                              ├── filter |scc| scc.len() >= 2     │
  │   ◄── Vec<Vec<SymbolId>>    │◄── filtered SCCs                  │
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/application/services/impact_analysis.rs` | **Create** | `ImpactAnalysisService` struct, 5 public methods, `#[cfg(test)] mod tests` |
| `crates/cognicode-core/src/application/services/mod.rs` | **Modify** | +1 line: `pub mod impact_analysis;` |
| `crates/cognicode-core/src/application/dto/impact_dto.rs` | **Modify** | +`PathResultDto`, +`SccDto`, +`from` constructors |
| `crates/cognicode-core/src/application/dto/mod.rs` | **Modify** | +2 re-exports: `PathResultDto`, `SccDto` |
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | **None** | Consumed read-only |
| `crates/cognicode-core/src/domain/services/impact_analyzer.rs` | **None** | Unchanged |
| `crates/cognicode-core/src/domain/services/cycle_detector.rs` | **None** | Unchanged |
| `crates/cognicode-core/Cargo.toml` | **None** | Byte-identical (R9) |

## Interfaces / Contracts

### ImpactAnalysisService

```rust
// crates/cognicode-core/src/application/services/impact_analysis.rs

use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::aggregates::CallGraph;
use crate::infrastructure::graph::CallGraphProjection;

/// Stateless application service for graph-aware impact analysis.
/// Constructs an ephemeral CallGraphProjection per method call.
pub struct ImpactAnalysisService;

impl ImpactAnalysisService {
    pub fn new() -> Self { Self }

    /// Predecessors of `root` within `max_depth` reverse hops.
    /// Returns vec![] for missing root, depth==0, or empty graph.
    pub fn impact_radius(
        &self, graph: &CallGraph, root: &SymbolId, max_depth: usize
    ) -> Vec<SymbolId>;

    /// true iff directed path exists. false for missing endpoints.
    /// Self-path (A,A) returns true when A is present.
    pub fn has_path(
        &self, graph: &CallGraph, from: &SymbolId, to: &SymbolId
    ) -> bool;

    /// Confidence-weighted shortest path (cost = 1.0 - confidence).
    /// None for missing endpoints or unreachable target.
    pub fn shortest_path(
        &self, graph: &CallGraph, from: &SymbolId, to: &SymbolId
    ) -> Option<PathResultDto>;

    /// Non-trivial SCCs (size >= 2). Self-loops excluded.
    /// vec![] for DAG or empty graph.
    pub fn detect_cycles(
        &self, graph: &CallGraph
    ) -> Vec<Vec<SymbolId>>;

    /// Undirected connected component containing `id`.
    /// None if `id` missing. Some(vec![id]) for isolated node.
    pub fn containing_component(
        &self, graph: &CallGraph, id: &SymbolId
    ) -> Option<Vec<SymbolId>>;
}
```

### PathResultDto

```rust
// In crates/cognicode-core/src/application/dto/impact_dto.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathResultDto {
    pub path: Vec<String>,      // SymbolId.as_str().to_string()
    pub total_cost: f64,
    pub found: bool,
}

impl PathResultDto {
    pub fn from_path(path: Vec<SymbolId>, cost: f64) -> Self {
        Self {
            path: path.iter().map(|s| s.as_str().to_string()).collect(),
            total_cost: cost,
            found: true,
        }
    }
}
```

### SccDto

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SccDto {
    pub members: Vec<String>,
    pub size: usize,
}

impl SccDto {
    pub fn from_scc(members: Vec<SymbolId>) -> Self {
        let strings: Vec<String> = members.iter().map(|s| s.as_str().to_string()).collect();
        let size = strings.len();
        Self { members: strings, size }
    }
}
```

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | `impact_radius` — chain/star/bounded/zero/max/empty/missing | In-memory `CallGraph::new()` + `add_symbol` + `add_dependency_with_provenance`. Assert returned `Vec<SymbolId>` contents (order-insensitive via sorted comparison). |
| Unit | `has_path` — direct/transitive/no-path/missing/self-path | Same in-memory construction. Assert bool. |
| Unit | `shortest_path` — confidence-weighted/unreachable/missing/self-path | Construct graph with known confidence (via `ExtractionContext::DirectExtraction` = 1.0). Assert `Option<PathResultDto>`. |
| Unit | `detect_cycles` — DAG/mutual/self-loop/multiple/empty | Filter `strongly_connected_components` for `len() >= 2`. |
| Unit | `containing_component` — member/missing/isolated | Assert `Option<Vec<SymbolId>>`. |
| Unit | DTO round-trip | JSON serialize/deserialize `PathResultDto`, `SccDto`. Assert field preservation. |
| Edge cases | E1–E10 from spec | Each edge case has >=1 dedicated test function. |

### Test-to-Scenario Mapping

| Test function | Spec scenarios covered |
|---------------|----------------------|
| `test_impact_radius_bounded_predecessors` | R2: Bounded predecessor traversal |
| `test_impact_radius_zero_depth` | R2: Zero depth → empty, E2 |
| `test_impact_radius_missing_root` | R2: Missing root → empty, E1 |
| `test_impact_radius_empty_graph` | R2: Empty graph → empty, E7 |
| `test_impact_radius_max_sentinel` | R2: `usize::MAX` sentinel, E6 |
| `test_has_path_direct_transitive_no_path` | R3: Direct/transitive/no path |
| `test_has_path_missing_endpoint` | R3: Missing → false, E1 |
| `test_has_path_self_path` | R3: Self-path, E9 |
| `test_shortest_path_confidence_weighted` | R4: Shortest confidence-weighted path |
| `test_shortest_path_unreachable` | R4: Unreachable → None, E5 |
| `test_shortest_path_missing_endpoint` | R4: Missing → None, E1 |
| `test_shortest_path_nan_confidence` | R4: NaN cost = 0.0, E8 |
| `test_shortest_path_self_path` | E10: shortest_path(A,A) |
| `test_detect_cycles_dag` | R5: DAG → empty |
| `test_detect_cycles_mutual` | R5: Mutual cycle as SCC |
| `test_detect_cycles_self_loop_excluded` | R5: Self-loop excluded, E9 |
| `test_detect_cycles_multiple` | R5: Multiple cycles |
| `test_detect_cycles_empty` | R5: Empty → empty, E7 |
| `test_containing_component_member` | R6: Member of component |
| `test_containing_component_missing` | R6: Missing → None, E1 |
| `test_containing_component_isolated` | R6: Isolated node |
| `test_path_result_dto_roundtrip` | R7: JSON round-trip |
| `test_scc_dto_size_matches` | R7: Size matches length |
| `test_stateless_non_mutating` | R8: Repeated calls don't mutate graph |
| `test_disconnected_graph_component` | E3: Component isolation |

## Migration / Rollout

No migration required. Pure extension — remove the new module and revert 2 one-line additions to rollback.

## Open Questions

None. All 10 design questions resolved by spec and codebase analysis.
