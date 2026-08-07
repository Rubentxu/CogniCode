# Proposal: MCP Graph Primitives — Subgraph, Cluster, Explain

## Intent

Complete Phase 2 call-graph primitives with three missing capabilities: neighborhood extraction (subgraph), structure discovery (cluster), and path narration (explain). Existing 14 MCP tools answer "what depends on X?" and "is there a path?" but can't answer "show me what's around X with edges", "how does the graph partition?", or "why are A and B connected?". These three tools make the impact analysis suite self-contained for agent-driven refactoring decisions.

## Scope

### In Scope
- **`graph_subgraph`** — BFS neighborhood around a root node, returning nodes + edges with `(DependencyType, confidence)` metadata. Direction: `incoming | outgoing | both`. Depth: default 3.
- **`graph_cluster`** — expose existing `strongly_connected_components` (Tarjan SCC) and `connected_components` (undirected BFS) via MCP. Method: `scc | connected`. Default: `scc`.
- **`graph_explain`** — enriched shortest path with per-hop `(DependencyType, confidence)`. Uses existing `dijkstra` then walks edges for metadata.
- **2 new projection methods**: `extract_subgraph(root, direction, max_depth) → SubgraphView`, `explain_path(from, to) → Option<ExplanationView>`
- **Service layer**: `subgraph()`, `cluster_components()`, `explain_path()` — thin delegation wrappers following existing `ImpactAnalysisService` pattern
- **MCP dispatch**: 3 tool constants, 3 arg structs, 3 dispatch arms, 3 schema entries; `TOOL_NAMES` updated 14→17
- **24 TDD tests**: 11 projection + 5 service + 8 MCP dispatch
- **RED gate**: compile-error at MCP dispatch level (E0425) — tool constant, arg struct, dispatch arm, and result DTO don't exist yet

### Out of Scope
- Provenance/EvidenceBlock enrichment on edges (projection stores only `(DependencyType, f64)`)
- `max_paths` multi-path support for `graph_explain`
- `max_nodes` response limit for `graph_subgraph`
- DB/UI/dependency changes/brain-session/ask-router
- Performance caching of projection results

## Capabilities

### New Capabilities
- `graph-subgraph`: extract neighborhood subgraph with nodes and edges around a root symbol
- `graph-cluster`: detect clusters via SCC (Tarjan) or connected components
- `graph-explain`: explain why two symbols are connected with per-hop edge metadata

### Modified Capabilities
- `impact-analysis`: adds `subgraph`, `cluster_components`, `explain_path` methods to ImpactAnalysisService

## Approach

**3-layer delegation (Projection → Service → MCP)**, same pattern as all 6 existing impact tools:

| Layer | File | What's added |
|-------|------|-------------|
| Projection | `call_graph_projection.rs` | `extract_subgraph()`, `explain_path()` + DTO types |
| Service | `impact_analysis.rs` | 3 thin delegation wrappers |
| MCP | `mcp.rs` | 3 tool constants, arg structs, dispatch arms, schemas, 8 tests |

`subgraph`: BFS with edge collection. Two-pass BFS for `direction: both`. Root included. ~70 LOC projection.  
`cluster`: delegates to existing `strongly_connected_components()` / `connected_components()`. Zero new projection logic. ~15 LOC projection.  
`explain`: runs existing `dijkstra`, then walks adjacent pairs looking up edges. ~40 LOC projection.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | Modified | +2 methods, +4 DTO types |
| `crates/cognicode-core/src/application/services/impact_analysis.rs` | Modified | +3 delegation methods |
| `crates/cognicode-explorer/src/mcp.rs` | Modified | +3 tools, constants, dispatch arms, schemas |
| `crates/cognicode-core/src/application/dto.rs` | Modified | +3 result DTOs |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Subgraph response too large on dense graphs | Med | Default `max_depth=3` (tighter than impact's 5); document sentinel behavior |
| `explain_path` O((V+E)logV) for large graphs | Low | ≤10K-node graphs acceptable; caching deferred |
| Provenance gap — no EvidenceBlock on edges | Low | Consistent with existing tools; deferred to future slice |
| Tool count inflation (14→17) | Low | Each tool answers distinct question; same thin-dispatcher pattern |

## Rollback Plan

Revert the 3 commits in reverse order: (1) MCP layer removal restores 14-tool set, (2) service layer removal is a no-op if no callers, (3) projection layer removal has no callers. All new code is additive — zero changes to existing method signatures. `git revert` clean.

## Dependencies

- None. No new crates, no DB schema changes, no external services.

## Success Criteria

- [ ] `graph_subgraph(A, outgoing, 2)` on A→B→C returns `{nodes: [A,B,C], edges: [(A→B, Calls, 1.0), (B→C, Calls, 1.0)]}`
- [ ] `graph_cluster("scc")` on mutual cycle A↔B returns one SCC `{A, B}`
- [ ] `graph_explain(A, C)` on A→B→C returns 2 hops with per-edge `(DependencyType, confidence)`
- [ ] All 24 TDD tests pass; RED gate at MCP dispatch level (compile error E0425) before any implementation
- [ ] `TOOL_NAMES` lists 17 tools; `build_tool_schemas()` includes all 3 new schemas
- [ ] Graph-unavailable error message consistent with existing 6 impact tools
- [ ] Zero changes to existing method signatures (pure extension, OCP-compliant)
