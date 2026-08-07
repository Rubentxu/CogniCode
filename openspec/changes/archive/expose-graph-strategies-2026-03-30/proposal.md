# Proposal: Expose Graph Strategies via CLI and MCP

## Intent

Exponer todas las estrategias de grafo (lightweight index, on-demand, per-file, full) via CLI commands y MCP tools para que los agentes de IA puedan elegir la estrategia óptima según sus necesidades.

## Scope

### CLI Commands to Add
- `cognicode index build` - Build lightweight index
- `cognicode index query <symbol>` - Query symbol locations
- `cognicode graph on-demand <symbol> --depth N --direction in|out` - On-demand subgraph
- `cognicode graph per-file <file>` - Per-file graph
- `cognicode graph full --rebuild` - Full project graph

### MCP Tools to Add
- `build_lightweight_index` - Fast index build
- `query_symbol_index` - Find symbol locations
- `build_call_subgraph` - On-demand subgraph with direction
- `get_per_file_graph` - Get graph for specific file
- `merge_file_graphs` - Merge multiple file graphs

## Implementation Plan

1. Add CLI commands in `cli.rs` or new `commands/` module
2. Add MCP tools in `handlers.rs` and `schemas.rs`
3. Connect to existing `GraphStrategy` implementations
4. Add proper error handling and output formatting

## Success Criteria

- [ ] All 4 graph strategies accessible via CLI
- [ ] All 5 new MCP tools registered
- [ ] Proper JSON output for agent consumption
- [ ] Helptext available for all commands/tools
