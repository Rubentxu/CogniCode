# Tasks: Enhanced Semantic Analysis

## Phase 1: Infrastructure - Hierarchical Outline

- [x] 1.1 Create `src/infrastructure/semantic/outline.rs` with `OutlineNode` struct
- [x] 1.2 Implement `OutlineBuilder` that builds hierarchy from tree-sitter AST
- [x] 1.3 Ensure outline builds for a file in < 10ms target
- [x] 1.4 Add optional exclude tests/private symbols

## Phase 2: Symbol Code Retrieval

- [x] 2.1 Implement `get_symbol_code(file, line, col)` function
- [x] 2.2 Include docstrings above the definition
- [x] 2.3 Cache results in memory using DashMap

## Phase 3: Search with Filters

- [x] 3.1 Create `src/infrastructure/semantic/semantic_search.rs`
- [x] 3.2 Implement `SearchQuery` struct with query, kinds filter, max_results
- [x] 3.3 Implement `SymbolKind` filter: function, class, method, variable, trait, struct, enum
- [x] 3.4 Implement fuzzy matching for typos
- [x] 3.5 Order results by relevance (exact match > prefix > fuzzy)

## Phase 4: Context Lines Extension

- [x] 4.1 Add `context_lines` parameter to find_usages (default: 3)
- [x] 4.2 Return source lines around each reference
- [x] 4.3 Make configurable per query

## Phase 5: MCP Schemas

- [x] 5.1 Add `OutlineInput/Output` - file_path, include_private
- [x] 5.2 Add `SymbolCodeInput/Output` - file, line, col
- [x] 5.3 Add `SemanticSearchInput/Output` - query, kinds[], max_results
- [x] 5.4 Add `FindUsagesWithContextInput/Output` - symbol, context_lines

## Phase 6: MCP Handlers

- [x] 6.1 Implement `handle_get_outline` - hierarchical symbol tree
- [x] 6.2 Implement `handle_get_symbol_code` - full code of a symbol
- [x] 6.3 Implement `handle_semantic_search` - search with filters
- [x] 6.4 Implement `handle_find_usages_with_context` - references with context

## Phase 7: Tool Registration

- [x] 7.1 Register `get_outline` tool in server.rs
- [x] 7.2 Register `get_symbol_code` tool in server.rs
- [x] 7.3 Register `semantic_search` tool in server.rs
- [x] 7.4 Register `find_usages_with_context` tool in server.rs

## Phase 8: Tests and Benchmarks

- [x] 8.1 Test outline builds correct hierarchy
- [x] 8.2 Test symbol retrieval returns correct code
- [x] 8.3 Test search filtering works
- [x] 8.4 Benchmark outline speed target < 10ms
