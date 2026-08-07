# Exploration: mcp-graph-primitives

> **Change**: `mcp-graph-primitives`  
> **Project**: cognicode  
> **Date**: 2026-06-09  
> **Mode**: hybrid (Engram + OpenSpec)

## Current State

### What exists today (14 MCP tools total)
- **8 legacy explorer tools**: `explorer_open_workspace`, `explorer_spotter_search`, `explorer_inspect_object`, `explorer_get_views`, `explorer_get_view`, `explorer_get_lenses`, `explorer_apply_lens`, `explorer_query_moldql`
- **6 impact tools**: `impact_radius` (incoming BFS), `impact_forward_radius` (outgoing BFS), `impact_has_path`, `impact_shortest_path` (Dijkstra), `impact_detect_cycles` (SCC≥2), `impact_component` (undirected component of node)

### CallGraphProjection (9 algorithm methods + 2 accessors + 2 BFS methods)
| Method | Returns | Maps to |
|--------|---------|---------|
| `find_impact_radius(root, max_depth)` | `Vec<SymbolId>` | Half of subgraph — nodes only, no edges |
| `find_forward_reach(root, max_depth)` | `Vec<SymbolId>` | Half of subgraph — nodes only, no edges |
| `strongly_connected_components()` | `Vec<Vec<SymbolId>>` | Half of cluster — Tarjan SCC, all sizes |
| `connected_components()` | `Vec<Vec<SymbolId>>` | Half of cluster — undirected BFS |
| `has_path(from, to)` | `bool` | Half of explain — existence only |
| `dijkstra(from, to)` | `Option<(Vec<SymbolId>, f64)>` | Half of explain — path + cost, no edge metadata |
| `topological_sort()` | `Result<Vec<SymbolId>, ProjectionError>` | Not relevant |
| `detect_cycles()` | `bool` | Not relevant (superseded by SCC) |

### ImpactAnalysisService (7 public methods)
All delegate to `CallGraphProjection` — stateless, creates fresh projection per call: `impact_radius`, `forward_radius`, `has_path`, `shortest_path` (returns `PathResultDto`), `detect_cycles` (returns `Vec<Vec<SymbolId>>`, filtered to size≥2), `containing_component`.

### MCP dispatch pattern
- Tool constant → arg struct (`#[derive(Deserialize)]`) → dispatch arm → `require_graph()` guard → service call → `ok_direct()` or `ok()` serialization
- All 6 impact tools use `require_graph()` returning `Err(CallToolResult)` when graph is `None`

---

## Q1: Gap Analysis — What maps and what's missing?

### `subgraph` → extract neighborhood around a node
**Already exists**: `find_impact_radius` (incoming) + `find_forward_reach` (outgoing) give node sets.
**Missing**:
- Returns `Vec<SymbolId>` — nodes only, **no edges**. A subgraph tool must return edges with `(DependencyType, confidence)`.
- No unified `direction` parameter (incoming/outgoing/both) — two separate methods.
- Root node is excluded in existing BFS methods (subgraph should include it).

**Gap**: Need a new projection method `extract_subgraph(root, direction, max_depth)` returning **both nodes AND edges with metadata**.

### `cluster` → detect clusters/communities
**Already exists**: `strongly_connected_components()` (Tarjan, all SCCs) + `connected_components()` (undirected BFS).
**Missing**:
- The existing `impact_detect_cycles` tool filters SCCs to size≥2. A `cluster` tool should expose ALL SCCs (including singletons) when method is "scc".
- `connected_components()` has no MCP tool yet.
- No cluster metadata (density, dominant edge types, internal edge count).

**Gap**: Thin wrapper — the projection already has the algorithms. Just needs service + MCP exposure.

### `explain` → explain why two nodes are connected
**Already exists**: `dijkstra(from, to)` returns path + cost. `has_path(from, to)` returns bool.
**Missing**:
- `dijkstra` returns `Vec<SymbolId>` path — but **no per-edge metadata** (DependencyType, confidence, provenance).
- No human-readable narrative ("A calls B via a Calls edge at confidence 0.95").
- Only returns one (cheapest) path — no multi-path support.

**Gap**: Need a new projection method that walks the dijkstra path and collects `(DependencyType, confidence)` per edge along the path.

---

## Q2: What projection methods need to be added?

### 1. `extract_subgraph(root, direction, max_depth) -> SubgraphView`
```rust
pub fn extract_subgraph(
    &self,
    root: &SymbolId,
    direction: SubgraphDirection,  // Incoming | Outgoing | Both
    max_depth: usize,
) -> SubgraphView { ... }

struct SubgraphView {
    root: SymbolId,
    nodes: Vec<SymbolId>,
    edges: Vec<SubgraphEdge>,
}

struct SubgraphEdge {
    source: SymbolId,
    target: SymbolId,
    dep_type: DependencyType,
    confidence: f64,
}
```
**Implementation**: BFS over `Direction` ∈ {Incoming, Outgoing}, or two-pass BFS for Both. Collects `Vec<SymbolId>` nodes AND pushes `(source, target, dep_type, confidence)` tuples when traversing edges. Root included. Visited-set prevents cycles. Same pattern as existing `find_impact_radius`/`find_forward_reach` but also collecting edges.

### 2. `explain_path(from, to) -> Option<ExplanationView>`
```rust
pub fn explain_path(&self, from: &SymbolId, to: &SymbolId) -> Option<ExplanationView> { ... }

struct ExplanationView {
    path: Vec<(SymbolId, Option<(DependencyType, f64)>)>,  // (node, edge FROM this node)
    total_cost: f64,
    found: bool,
}
```
**Implementation**: Uses existing `dijkstra` internally to get the path `Vec<SymbolId>`, then walks adjacent pairs `(path[i], path[i+1])` and looks up the edge between them in the `StableGraph`. For each hop, finds `(DependencyType, confidence)` from the edge weight. Returns enriched path.

**Alternative**: Instead of running dijkstra first then edge-walk, do a single BFS that collects edges as it goes. But dijkstra already handles cost-weighted shortest path — the edge-walk post-pass is simpler and reuses tested code.

### 3. Cluster — NO new projection method needed
`strongly_connected_components()` and `connected_components()` already exist. Just need:
- Service method: `cluster_components(method)` → delegates to existing projection methods
- MCP tool: wraps service

---

## Q3: Tool design — 3 tools or fewer?

**Recommendation**: 3 tools. Each answers a fundamentally different question.

| Tool | Question | Relation to existing |
|------|----------|---------------------|
| `graph_subgraph` | "Show me the neighborhood around X" | New — supersedes `impact_radius` + `impact_forward_radius` in expressiveness |
| `graph_cluster` | "How does this graph cluster/partition?" | Thin wrapper — delegates to existing SCC/connected_components |
| `graph_explain` | "Why/how are A and B connected?" | Enhances `impact_shortest_path` with edge metadata |

---

## Q4: Arguments per tool

### `graph_subgraph`
| Arg | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `root` | string | ✅ yes | — | Symbol ID of center node |
| `direction` | string | ❌ no | `"both"` | `"incoming"` \| `"outgoing"` \| `"both"` |
| `max_depth` | integer | ❌ no | `3` | Hop limit (0 = root only, `usize::MAX` = full reach) |

Note: `max_depth=3` is tighter than the existing `DEFAULT_IMPACT_RADIUS_DEPTH=5` because subgraph extraction with edges is heavier output.

### `graph_cluster`
| Arg | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `method` | string | ❌ no | `"scc"` | `"scc"` (Tarjan, directed) \| `"connected"` (undirected BFS) |

No required args — calling with no args returns all SCCs.

### `graph_explain`
| Arg | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `from` | string | ✅ yes | — | Source symbol ID |
| `to` | string | ✅ yes | — | Target symbol ID |

Minimal surface — two required endpoints. `max_paths` extension deferred to future.

---

## Q5: Response shapes

### `graph_subgraph` response
```json
{
  "root": "test.rs:main:1",
  "direction": "outgoing",
  "max_depth": 3,
  "node_count": 5,
  "edge_count": 6,
  "nodes": ["test.rs:main:1", "test.rs:init:5", "..."],
  "edges": [
    {
      "source": "test.rs:main:1",
      "target": "test.rs:init:5",
      "dependency_type": "Calls",
      "confidence": 1.0
    }
  ]
}
```

**Provenance/confidence**: Edge confidence comes from `ProjectionEdgeWeight = (DependencyType, f64)`. Provenance (EvidenceBlock) is NOT stored in the projection — it's in `CallGraph::edges_with_metadata()`. To include provenance, we'd need to either store it in the projection (adds memory) or do a separate lookup. **Decision**: Return `(DependencyType, confidence)` only — matches projection storage. EvidenceBlock enrichment is a future slice.

### `graph_cluster` response
```json
{
  "method": "scc",
  "cluster_count": 3,
  "clusters": [
    {
      "members": ["test.rs:A:1", "test.rs:B:1"],
      "size": 2
    }
  ]
}
```

Reuses `SccDto { members: Vec<String>, size: usize }` for SCC. For connected components, same shape.

### `graph_explain` response
```json
{
  "from": "test.rs:main:1",
  "to": "test.rs:render:42",
  "found": true,
  "path_length": 3,
  "total_cost": 0.5,
  "hops": [
    {
      "from": "test.rs:main:1",
      "to": "test.rs:init:5",
      "dependency_type": "Calls",
      "confidence": 1.0
    },
    {
      "from": "test.rs:init:5",
      "to": "test.rs:render:42",
      "dependency_type": "Calls",
      "confidence": 0.5
    }
  ]
}
```

When `found: false` (no path or missing endpoint), returns `null` (same convention as `impact_shortest_path`).

---

## Q6: Behavior-first TDD tests

### Projection layer (CallGraphProjection) — 11 new tests

**`extract_subgraph` (7 tests):**
1. `test_extract_subgraph_outgoing_direct_successor` — A→B, outgoing, depth 1 → nodes {A,B}, 1 edge (A→B, Calls, 1.0)
2. `test_extract_subgraph_incoming_predecessor` — B→A, incoming from A, depth 1 → nodes {A,B}, 1 edge (B→A)
3. `test_extract_subgraph_both_directions` — A→B, C→A, both from A, depth 1 → nodes {A,B,C}, 2 edges
4. `test_extract_subgraph_transitive_depth_two` — A→B→C, outgoing from A, depth 2 → nodes {A,B,C}, 2 edges
5. `test_extract_subgraph_zero_depth_root_only` — A→B, outgoing, depth 0 → nodes {A}, 0 edges
6. `test_extract_subgraph_missing_root` — root not in graph → empty nodes + edges
7. `test_extract_subgraph_cycle_terminates` — A→B→C→A, outgoing from A, depth MAX → nodes {A,B,C}, 3 edges, no duplicates

**`explain_path` (4 tests):**
8. `test_explain_path_direct_edge_metadata` — A→B (Calls, 1.0) → 1 hop with type + confidence
9. `test_explain_path_transitive_two_hops` — A→B→C → 2 hops, both with metadata
10. `test_explain_path_unreachable_returns_none` — A→B, C unreachable → None
11. `test_explain_path_missing_endpoint_returns_none` — missing from/to → None

### Service layer (ImpactAnalysisService) — 5 new tests
12. `test_subgraph_mirrors_extract_subgraph` — service.subgraph() == projection.extract_subgraph()
13. `test_cluster_scc_delegates_to_strongly_connected` — service.cluster_components("scc") == projection.strongly_connected_components()
14. `test_cluster_connected_delegates_to_connected_components` — same for "connected"
15. `test_explain_path_mirrors_explain_path_projection` — service.explain_path() == projection.explain_path()
16. `test_subgraph_empty_graph` — empty graph → empty result

### MCP dispatch layer — 8 new tests (RED gates)
17. `test_graph_subgraph_returns_nodes_and_edges` — A→B, outgoing, depth 1 → 2 nodes + 1 edge in JSON
18. `test_graph_subgraph_graph_unavailable` — Graph=None → "impact analysis unavailable"
19. `test_graph_cluster_scc_returns_all_components` — mutual cycle → 1 SCC with {A,B}, size=2
20. `test_graph_cluster_dag_returns_singletons` — A→B→C DAG → 3 singleton SCCs
21. `test_graph_cluster_connected_returns_components` — two disjoint subgraphs → 2 components
22. `test_graph_explain_returns_path_with_metadata` — A→B at Calls,1.0 → 1 hop JSON
23. `test_graph_explain_unreachable_returns_null` — no path → JSON null
24. `test_graph_explain_graph_unavailable` — Graph=None → error

**Total: 24 new tests** (11 projection + 5 service + 8 MCP dispatch)

---

## Q7: RED gate test

The **strictest RED gate** is a compile-error gate at the **MCP dispatch level**, following the exact pattern from `mcp-impact-tool` and `forward-reach-impact`:

The test MUST fail to compile because the tool constant, arg struct, dispatch arm, and response DTO don't exist yet. This is the **three-layer RED gate**: MCP fails → which forces service implementation → which forces projection method.

---

## Q8: Core/service layer or direct projection call?

**Must follow existing architecture**: Projection → Service → MCP Dispatch

Reason: The 3-layer pattern is consistently enforced across all 6 impact tools. Breaking it for the new tools would:
- Create inconsistent code (some tools use service, some don't)
- Lose the testability layer (service tests mock/proxy projection, MCP tests mock/proxy service)
- Violate the SRP/OCP/DIP patterns already established

**What goes in each layer:**

| Layer | File | What's added |
|-------|------|-------------|
| **Projection** | `call_graph_projection.rs` | `extract_subgraph()`, `explain_path()` + DTO types `SubgraphView`, `SubgraphEdge`, `ExplanationView`, `HopDetail` |
| **Service** | `impact_analysis.rs` | `subgraph()`, `cluster_components()`, `explain_path()` — thin delegation wrappers |
| **MCP** | `mcp.rs` | 3 new tool constants, 3 arg structs, 3 dispatch arms, 3 schema entries, 8 tests; `TOOL_NAMES` updated (14→17); `build_tool_schemas()` updated |

---

## Approaches Comparison

| Approach | Pros | Cons | Effort |
|----------|------|------|--------|
| **A: 3 tools with new projection methods** (recommended) | Full expressiveness; each tool answers distinct question; follows existing architecture; rich edge metadata | ~300 LOC across 3 layers; 24 new tests | Medium |
| **B: Extend existing impact tools** (add `include_edges: bool` to `impact_radius`, add edge metadata to `impact_shortest_path`) | Less new tool surface; reuses existing tool names | Confuses tool semantics; existing tools have agents depending on them | Low (but dirty) |
| **C: Single `graph_query` tool with DSL** (like MoldQL) | One tool surface; extensible | Premature abstraction; agents need structured JSON, not text DSL; huge design risk | High |
| **D: Skip `subgraph`, only `cluster` + `explain`** | Smallest surface; `cluster` is mostly there; `explain` enriches existing dijkstra | `subgraph` is the most distinctive new capability — it's the one thing existing tools can't do | Low |

**Recommendation: Approach A**. The subgraph tool is the most valuable — it answers "show me what's around X" which no existing tool does. The cluster tool is thin reuse. The explain tool makes dijkstra output actionable.

---

## Risks

- **Risk 1 — Response size**: `graph_subgraph` with `direction: "both"` and `max_depth: usize::MAX` on a dense graph could produce massive JSON. Mitigation: lower default max_depth (3 vs 5 for impact tools), document the sentinel behavior, add `max_nodes` limit in future slice.
- **Risk 2 — Performance**: `explain_path` runs dijkstra O((V+E) log V) then walks edges O(len(path)). Acceptable for <10K-node graphs. Caching deferred to future performance SDD.
- **Risk 3 — Provenance gap**: Edge metadata in projection is `(DependencyType, f64)` — no `EvidenceBlock`. This is consistent with existing tools. Adding provenance lookup would require a second pass into `CallGraph::edges_with_metadata()` or storing provenance in projection (more memory). Deferred to future slice.
- **Risk 4 — Tool count inflation**: Going from 14→17 tools may feel like bloat. But each tool answers a genuinely different question and follows the same thin-dispatcher pattern. The MCP dispatch is already structured for this.
- **Risk 5 — TDD RED gate compile-only**: MCP-layer RED gates are compile errors (E0425), which means they only fail when you run `cargo test -p cognicode-explorer --lib`. Need to verify the test file compiles in CI before the RED gate assertion. This is the established pattern and works.

---

## Entropy Analysis (Connascence Landscape)

**Method**: Heuristic (CogniCode graph not built for this phase). Confidence: estimated.

| Component A | Component B | Connascence Type | I(bits) | Severity |
|---|---|---|---|---|
| `CallGraphProjection::extract_subgraph` | `ImpactAnalysisService` | Name | ~1.0 | ✅ OK |
| `CallGraphProjection::explain_path` | `ImpactAnalysisService` | Name | ~1.0 | ✅ OK |
| `ImpactAnalysisService::subgraph` | `mcp.rs dispatch` | Name | ~1.0 | ✅ OK |
| `ImpactAnalysisService::cluster_components` | `mcp.rs dispatch` | Name | ~1.0 | ✅ OK |
| `ImpactAnalysisService::explain_path` | `mcp.rs dispatch` | Name | ~1.0 | ✅ OK |
| `SubgraphResult` DTO | projection + service + MCP | Type | ~2.0 | ✅ OK (3 consumers) |
| `ExplanationResult` DTO | projection + service + MCP | Type | ~2.0 | ✅ OK (3 consumers) |
| `mcp.rs` tool constants + schemas | `build_tool_schemas()` | Name (internal) | ~1.0 | ✅ OK |

**Critical Pairs (I > 3.0 bits)**: None
**Hidden Connascence (Meaning/Timing)**: None detected
**SOLID-Entropy Violations**: None

| Metric | Estimate | Threshold | Status |
|--------|----------|-----------|--------|
| H(Δ_existing) | 0.0 bits | < 1.0 | ✅ Pure extension |
| H(Δ_new) | ~3.5 bits | > 0 | ✅ |
| New connascence pairs | 3 (Name: service↔MCP) + 2 (Type: shared DTOs) | < 5 | ✅ |
| OCP compliant | Yes | — | ✅ |

**Verdict**: Green — zero entropy introduced to existing components. New coupling mirrors established 3-layer delegation pattern.

---

## Design Quality Score (estimated)

| Metric | Score | Rating |
|--------|-------|--------|
| Coupling (1 - H_coupling) | ~0.87 | 🟢 |
| Cohesion (avg 1 - F/H) | ~0.90 | 🟢 |
| LSP violations | 0.0 | 🟢 |
| Connascence penalty | ~0.03 | 🟢 |
| **DQS** | **~0.78** | 🟢 EXCELLENT |

---

## Summary

The `CallGraphProjection` already provides **half** of what each tool needs. The gap is:
- **subgraph**: nodes exist, edges are missing — need `extract_subgraph()` returning both
- **cluster**: algorithms exist — just need service + MCP wiring
- **explain**: path exists, edge metadata is missing — need `explain_path()` enriching the dijkstra result

All three fit naturally into the existing 3-layer architecture (Projection → Service → MCP). Total scope: ~300 LOC across 3 files, 24 new tests, 3-layer TDD RED gate at MCP layer.

**Ready for proposal**: Yes — all questions answered, risks identified, implementation path clear.
