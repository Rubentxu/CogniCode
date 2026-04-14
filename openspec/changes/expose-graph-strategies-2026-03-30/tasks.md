# Tasks: expose-graph-strategies-2026-03-30

## Implementation Progress

### Phase 1: MCP Schemas
- [x] 1.1 `BuildIndexInput/Output` - for building lightweight index
- [x] 1.2 `QuerySymbolInput/Output` - for querying symbol locations
- [x] 1.3 `BuildSubgraphInput/Output` - for on-demand subgraph
- [x] 1.4 `GetPerFileGraphInput/Output` - for per-file graphs
- [x] 1.5 `MergeGraphsInput/Output` - for merging file graphs

### Phase 2: MCP Handlers
- [x] 2.1 `handle_build_lightweight_index` - builds index, returns stats
- [x] 2.2 `handle_query_symbol_index` - queries index for symbol locations
- [x] 2.3 `handle_build_call_subgraph` - builds on-demand subgraph
- [x] 2.4 `handle_get_per_file_graph` - gets graph for specific file
- [x] 2.5 `handle_merge_graphs` - merges multiple file graphs

### Phase 3: MCP Server Registration
- [x] 3.1 Register tools in `handle_tools_list`
- [x] 3.2 Add tool dispatch in `handle_tools_call`

### Phase 4: CLI Commands
- [x] 4.1 `cognicode index build` - Build lightweight index
- [x] 4.2 `cognicode index query <symbol>` - Query symbol locations
- [x] 4.3 `cognicode graph on-demand <symbol> --depth N --direction in|out|both`
- [x] 4.4 `cognicode graph per-file <file>` - Get per-file graph
- [x] 4.5 `cognicode graph full --rebuild` - Full project graph

### Phase 5: Tests
- [x] 5.1 Add tests for `handle_build_lightweight_index`
- [x] 5.2 Add tests for `handle_query_symbol_index`
- [x] 5.3 Add tests for `handle_build_call_subgraph`
- [x] 5.4 Add tests for `handle_get_per_file_graph`
- [x] 5.5 Add tests for `handle_merge_graphs`

## Status
- **Total Tasks**: 16
- **Completed**: 16
- **Remaining**: 0

## Files Changed
| File | Action |
|------|--------|
| `src/interface/mcp/schemas.rs` | Modified - Added new schema types |
| `src/interface/mcp/handlers.rs` | Modified - Added new handlers and tests |
| `src/interface/mcp/server.rs` | Modified - Registered new tools |
| `src/interface/cli/commands.rs` | Modified - Added new CLI commands |

## Tests Added
- `test_handle_build_lightweight_index_invalid_directory` - Tests error handling for invalid directory
- `test_handle_query_symbol_index_empty_symbol` - Tests query with empty symbol name
- `test_handle_build_call_subgraph_empty_symbol` - Tests subgraph building with empty symbol
- `test_handle_get_per_file_graph_nonexistent_file` - Tests error handling for nonexistent file
- `test_handle_merge_graphs_empty_list` - Tests merge with empty file list
- Schema serialization tests for all new types
