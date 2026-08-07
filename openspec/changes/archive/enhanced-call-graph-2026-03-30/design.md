# Design: Enhanced Call Graph LSP Features

## Technical Approach

Extend the existing `CallGraph` aggregate with depth-limited recursive traversal, add a `CallGraphAnalyzer` domain service for advanced metrics (fan-in, fan-out, hot paths), and expose new MCP tools. The existing `find_path` BFS algorithm will be reused for `trace_execution_path`.

## Architecture Decisions

### Decision: Add traversal methods to CallGraph aggregate vs. separate service

**Choice**: Extend `CallGraph` aggregate with `traverse_callees` and `traverse_callers` methods
**Alternatives considered**: Create separate `CallGraphTraversal` service
**Rationale**: Follows existing aggregate pattern where domain logic lives with the aggregate. Keeps related traversal and metric methods together.

### Decision: Max depth limit of 10 for traversal

**Choice**: Hard limit of `MAX_DEPTH = 10` in traversal methods
**Alternatives considered**: Configurable depth per call, unlimited depth
**Rationale**: Prevents stack overflow while being deep enough for real codebases. Matches proposal risk mitigation.

### Decision: Mermaid template-based export

**Choice**: String template for Mermaid flowchart generation
**Alternatives considered**: Dedicated Mermaid crate, graphviz DOT format
**Rationale**: Minimal dependency, matches existing template patterns in codebase, sufficient for static call graphs.

## Data Flow

```
MCP Request
    │
    ▼
handlers.rs ──► AnalysisService ──► GraphCache ──► CallGraph
    │                                           │
    │                                           ├── traverse_callees/depth +1
    │                                           ├── traverse_callers (incoming)
    │                                           ├── fan_in() / fan_out()
    │                                           └── to_mermaid()
    │
    ▼
schemas.rs ──► MCP Response (JSON)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/domain/aggregates/call_graph.rs` | Modify | Add `traverse_callees`, `traverse_callers` (depth-limited), `fan_in`, `fan_out`, `to_mermaid` |
| `src/domain/services/call_graph_analyzer.rs` | Create | Hot path detection, complexity metrics, entry/leaf analysis |
| `src/application/services/analysis_service.rs` | Modify | Add `get_entry_points`, `get_leaf_functions`, `trace_path` |
| `src/interface/mcp/handlers.rs` | Modify | Add handlers: `handle_get_entry_points`, `handle_get_leaf_functions`, `handle_trace_path`, `handle_export_mermaid`, `handle_get_hot_paths` |
| `src/interface/mcp/schemas.rs` | Modify | Add schemas: `GetEntryPointsInput/Output`, `GetLeafFunctionsInput/Output`, `TracePathInput/Output`, `ExportMermaidInput/Output`, `GetHotPathsInput/Output` |

## Interfaces / Contracts

### New CallGraph methods
```rust
/// Traverse callees up to max_depth (outgoing direction)
pub fn traverse_callees(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry>

/// Traverse callers up to max_depth (incoming direction)
pub fn traverse_callers(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry>

/// Fan-in: number of unique callers (direct)
pub fn fan_in(&self, id: &SymbolId) -> usize

/// Fan-out: number of unique callees (direct)
pub fn fan_out(&self, id: &SymbolId) -> usize

/// Export as Mermaid flowchart
pub fn to_mermaid(&self, title: &str) -> String
```

### New MCP Tools
```rust
// get_entry_points: returns symbols with no incoming edges
// get_leaf_functions: returns symbols with no outgoing edges
// trace_execution_path: find_path between two symbols
// export_mermaid: generate Mermaid diagram from subgraph
// get_hot_paths: top N functions by fan-in
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | CallGraph traversal, fan-in/fan-out, Mermaid output | `call_graph.rs` tests |
| Unit | CallGraphAnalyzer hot path, entry/leaf detection | `call_graph_analyzer.rs` tests |
| Integration | MCP handlers with real graph | `handlers.rs` integration tests |
| E2E | Full tool chain with sample Rust project | `tests/e2e.rs` |

## Migration / Rollout

No migration required. New functionality is purely additive. Feature flags not needed since no existing behavior changes.

## Open Questions

- [ ] Should Mermaid export include edge labels (dependency type)?
- [ ] Hot path threshold: fixed (top 10) or configurable?
