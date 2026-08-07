# Design: MCP Graph Primitives — Subgraph, Cluster, Explain

## Technical Approach

Three new MCP tools (`graph_subgraph`, `graph_cluster`, `graph_explain`) following the established **3-layer delegation** pattern: Projection → Service → MCP dispatch. Two new projection methods (`extract_subgraph`, `explain_path`) plus 4 projection-layer DTOs. Three thin service wrappers. Three MCP dispatch arms with arg structs and result DTOs. Cluster reuses existing projection methods (`strongly_connected_components`, `connected_components`) with zero new projection logic. `TOOL_NAMES` updates from 14→17.

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| `extract_subgraph` return type | `SubgraphView { nodes, edges }` | Single flat vec | Caller needs both node set and edge metadata (dep type + confidence). Returning only nodes would lose the edge information the spec requires. |
| `SubgraphDirection` enum | `Outgoing \| Incoming \| Both` in projection layer | String-based dispatch | Type-safe at projection; service validates string→enum once. Matches `petgraph::Direction` semantics. |
| `explain_path` return type | `Option<ExplanationView>` | `Result<ExplanationView, _>` | `None` for missing/unreachable mirrors `dijkstra()` convention. Service wraps to `Some(ExplainResultDto { found: false, .. })` so MCP tool returns structured payload with `is_error == false`. |
| Verb mapping for `DependencyType` | `match` on all 8 variants + wildcard fallback `"depends on"` | `Display` trait reuse | `Display` gives lowercase machine names (`"calls"`, `"imports"`); verb mapping gives agent-readable rationale (`"calls"`, `"imports"`, `"inherits from"`, etc.). Wildcard prevents panic on future variants. |
| Cluster: where logic lives | Service layer only (delegates to existing projection methods) | New projection method | `strongly_connected_components()` and `connected_components()` already exist and return `Vec<Vec<SymbolId>>`. Zero new projection code for cluster. |
| Default subgraph depth | `DEFAULT_SUBGRAPH_DEPTH = 3` (constant in mcp.rs) | Reuse `DEFAULT_IMPACT_RADIUS_DEPTH = 5` | 3 is tighter; subgraph returns edges too so it's heavier per hop. Spec explicitly mandates 3. |
| New DTOs location | Projection-layer DTOs in `call_graph_projection.rs`; MCP DTOs in `impact_dto.rs` | All in one file | Projection DTOs use `SymbolId` (generic, string-free). MCP DTOs use `String`. Keeps projection string-free per spec. |

## Data Flow

```
MCP Agent
  │
  ▼
dispatch() ── match tool_name ──▶ require_graph() guard
  │                                    │
  │  serde_json::from_value(args)      │
  │  validate required fields          │
  │  parse direction/method enum       │
  ▼                                    ▼
ImpactAnalysisService                Arc<CallGraph>
  │                                    │
  │  .subgraph(graph, root, dir, depth)│
  │  .cluster_components(graph, method)│
  │  .explain_path(graph, from, to)    │
  ▼                                    │
CallGraphProjection::from_call_graph(graph)
  │
  │  .extract_subgraph(root, dir, depth)  → SubgraphView
  │  .strongly_connected_components()     → Vec<Vec<SymbolId>>
  │  .connected_components()              → Vec<Vec<SymbolId>>
  │  .explain_path(from, to)              → Option<ExplanationView>
  ▼
Result DTOs (String-based) ── ok_direct() ──▶ CallToolResult
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | Modify | Add `extract_subgraph()`, `explain_path()`, `SubgraphDirection` enum, `SubgraphView`, `SubgraphEdge`, `ExplanationView`, `ExplanationHop` types |
| `crates/cognicode-core/src/application/services/impact_analysis.rs` | Modify | Add `subgraph()`, `cluster_components()`, `explain_path()` delegation methods |
| `crates/cognicode-core/src/application/dto/impact_dto.rs` | Modify | Add `SubgraphResultDto`, `SubgraphEdgeDto`, `ClusterResultDto`, `ClusterDto`, `ExplainResultDto`, `ExplainHopDto` |
| `crates/cognicode-explorer/src/mcp.rs` | Modify | Add 3 tool constants, 3 arg structs, 3 dispatch arms, 3 schemas; update `TOOL_NAMES` 14→17; add `DEFAULT_SUBGRAPH_DEPTH` |

## Interfaces / Contracts

### Projection Layer (call_graph_projection.rs)

```rust
/// Direction for subgraph extraction.
pub enum SubgraphDirection {
    Outgoing,
    Incoming,
    Both,
}

/// A single edge in the subgraph result.
pub struct SubgraphEdge {
    pub source: SymbolId,
    pub target: SymbolId,
    pub dependency_type: DependencyType,
    pub confidence: f64,
}

/// Result of neighborhood extraction around a root.
pub struct SubgraphView {
    pub nodes: Vec<SymbolId>,
    pub edges: Vec<SubgraphEdge>,
}

impl CallGraphProjection {
    /// BFS neighborhood around `root` within `max_depth` hops.
    /// Direction controls which edges to follow. Root always included.
    /// Cycle-safe via HashSet<NodeIndex>. Returns empty nodes/edges
    /// when root is unknown or depth == 0 (empty nodes/edges for depth 0;
    /// root-only nodes for depth > 0 with unknown root → empty).
    pub fn extract_subgraph(
        &self,
        root: &SymbolId,
        direction: SubgraphDirection,
        max_depth: usize,
    ) -> SubgraphView;

    /// Shortest-path explanation with per-hop edge metadata.
    /// Uses existing `dijkstra()` then walks adjacent pairs.
    /// Self-path (from == to) returns Some with 0 hops.
    /// Returns None when either endpoint is unknown or unreachable.
    pub fn explain_path(
        &self,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Option<ExplanationView>;
}

/// Per-hop metadata in a path explanation.
pub struct ExplanationHop {
    pub from: SymbolId,
    pub to: SymbolId,
    pub dependency_type: DependencyType,
    pub confidence: f64,
    /// Human-readable verb derived from DependencyType.
    pub rationale: String,
}

/// Result of path explanation.
pub struct ExplanationView {
    pub hops: Vec<ExplanationHop>,
    pub total_cost: f64,
}
```

### Verb Mapping (projection layer, private fn)

```rust
/// Map DependencyType to agent-readable verb. Covers all 8 variants.
/// Wildcard returns "depends on" (no panic on future variants).
fn verb_for(dep_type: DependencyType) -> &'static str {
    match dep_type {
        DependencyType::Calls => "calls",
        DependencyType::Imports => "imports",
        DependencyType::Inherits => "inherits from",
        DependencyType::UsesGeneric => "uses generic",
        DependencyType::References => "references",
        DependencyType::Defines => "defines",
        DependencyType::AnnotatedBy => "annotated by",
        DependencyType::Contains => "contains",
    }
}
```

### Service Layer (impact_analysis.rs)

```rust
impl ImpactAnalysisService {
    /// Thin wrapper: projection → SubgraphView → SubgraphResultDto.
    /// Unknown direction string should be validated at MCP layer;
    /// service receives parsed SubgraphDirection.
    pub fn subgraph(
        &self, graph: &CallGraph, root: &SymbolId,
        direction: SubgraphDirection, max_depth: usize,
    ) -> SubgraphResultDto;

    /// Delegates to existing SCC or connected_components.
    /// Method validated at MCP layer ("scc" | "connected").
    pub fn cluster_components(
        &self, graph: &CallGraph, method: &str,
    ) -> ClusterResultDto;

    /// Wraps projection.explain_path().
    /// None → Some(ExplainResultDto { found: false, .. }) so MCP
    /// tool returns structured payload (not error).
    pub fn explain_path(
        &self, graph: &CallGraph, from: &SymbolId, to: &SymbolId,
    ) -> Option<ExplainResultDto>;
}
```

### MCP Layer (mcp.rs) — New Constants & Arg Structs

```rust
pub const TOOL_GRAPH_SUBGRAPH: &str = "graph_subgraph";
pub const TOOL_GRAPH_CLUSTER: &str = "graph_cluster";
pub const TOOL_GRAPH_EXPLAIN: &str = "graph_explain";
pub const DEFAULT_SUBGRAPH_DEPTH: usize = 3;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphSubgraphArgs {
    root: Option<String>,
    direction: Option<String>,   // "outgoing" | "incoming" | "both", default "both"
    max_depth: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphClusterArgs {
    method: Option<String>,      // "scc" | "connected", default "scc"
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphExplainArgs {
    from: Option<String>,
    to: Option<String>,
}
```

### MCP Result DTOs (impact_dto.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphEdgeDto {
    pub source: String,
    pub target: String,
    pub dependency_type: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphResultDto {
    pub nodes: Vec<String>,
    pub edges: Vec<SubgraphEdgeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterDto {
    pub members: Vec<String>,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterResultDto(pub Vec<ClusterDto>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainHopDto {
    pub from: String,
    pub to: String,
    pub dependency_type: String,
    pub confidence: f64,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainResultDto {
    pub found: bool,
    pub hops: Vec<ExplainHopDto>,
    pub total_cost: f64,
    pub summary: String,
}
```

### TOOL_NAMES Update

```rust
pub const TOOL_NAMES: &[&str] = &[
    // ... existing 14 ...
    TOOL_GRAPH_SUBGRAPH,
    TOOL_GRAPH_CLUSTER,
    TOOL_GRAPH_EXPLAIN,
];
```

### Dispatch Arms (mcp.rs) — Pseudocode

```
TOOL_GRAPH_SUBGRAPH =>
  require_graph → parse GraphSubgraphArgs → validate root required
  → parse direction string (invalid → error)
  → service.subgraph(graph, &root_id, direction, max_depth)
  → ok_direct(&result)

TOOL_GRAPH_CLUSTER =>
  require_graph → parse GraphClusterArgs (empty {} valid)
  → validate method (invalid → error, default "scc")
  → service.cluster_components(graph, method)
  → ok_direct(&result)

TOOL_GRAPH_EXPLAIN =>
  require_graph → parse GraphExplainArgs → validate from + to required
  → service.explain_path(graph, &from_id, &to_id)
  → ok_direct(&result)  // Some(dto) or Some(found:false dto)
```

## Testing Strategy

| Layer | What to Test | Approach | Count |
|-------|-------------|----------|-------|
| Projection | `extract_subgraph`: outgoing, incoming, both, depth 0, unknown root, cycle, self-loop, usize::MAX, empty graph, dense multi-fanout, parallel edges | Unit: build graph → projection → assert SubgraphView fields | 6 |
| Projection | `explain_path`: direct edge, multi-hop, self-path, unreachable, missing endpoint, cycle, NaN confidence, unknown DependencyType | Unit: build graph → projection → assert ExplanationView hops | 5 |
| Service | `subgraph` mirrors projection; `cluster_components` SCC vs connected; `explain_path` None → found:false dto | Unit: service + CallGraph → assert DTO shapes | 5 |
| MCP Dispatch | 3 tools × (happy + error paths); schema count 17; TOOL_NAMES contains new tools; graph-unavailable guard for all 3 | Unit: dispatch(service, &graph/None, args) → assert is_error, JSON fields | 8 |
| **Total** | | | **24** |

### TDD RED Gate Sequence

1. **MCP compile error** — Write 3 tool constants, 3 arg structs, 3 dispatch arms referencing non-existent service methods. `cargo build -p cognicode-explorer` → E0425.
2. **Service compile error** — Write 3 service methods referencing non-existent projection methods/DTOs. `cargo build -p cognicode-core` → E0425.
3. **Projection compile error** — Write `extract_subgraph`, `explain_path`, 4 DTO types. All 24 tests written referencing new API. `cargo build` fails until each layer lands.
4. **Implementation order**: Projection DTOs → `extract_subgraph` → `explain_path` → Service methods → Service DTOs → MCP DTOs → MCP constants/args/dispatch/schemas → Tests pass.

## Migration / Rollout

No migration required. Pure additive change — zero modifications to existing method signatures, no DB schema changes, no new dependencies, no UI changes.

## Open Questions

- [ ] Should `SubgraphResultDto.nodes` be deduplicated and sorted for deterministic output, or preserve BFS discovery order? (Proposal: preserve discovery order, root first)
