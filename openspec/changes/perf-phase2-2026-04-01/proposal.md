# Proposal: Performance Phase 2 — Deep Optimization

## Intent
Corregir los ~30 hallazgos de rendimiento identificados en la revisión post-fase-1, agrupados en 4 batches paralelos por dependencia de archivos.

## Scope
### Batch 1 — Independent files (6 agents)
- `domain/services/cycle_detector.rs` — Tarjan O(V*E) → O(V+E), eliminar FQN alloc
- `domain/aggregates/call_graph.rs` — BFS predecessor map, iteradores en callers/callees
- `infrastructure/parser/ast_scanner.rs` — eliminar alloc por nodo, scan dirigido
- `infrastructure/parser/tree_sitter_parser.rs` — pre-split lines O(1)
- `domain/events/graph_event.rs` + `domain/services/impact_analyzer.rs` — HashMap O(1)
- `infrastructure/graph/pet_graph_store.rs` — dead code, double alloc

### Batch 2 — Semi-dependent (4 agents)
- `domain/aggregates/symbol.rs` + `value_objects/location.rs` — cache FQN
- `infrastructure/graph/symbol_index.rs` + `semantic/symbol_code.rs` — Arc cache
- `infrastructure/semantic/outline.rs` + `application/services/analysis_service.rs` — clones, ref
- `infrastructure/refactor/*.rs` (5 archivos) — triple parse, pre-split lines

### Batch 3 — MCP handlers + misc (2 agents)
- `interface/mcp/handlers.rs` — lowercase index, dedup find_usages
- `application/services/lsp_proxy_service.rs` + `context_compressor.rs` + `vfs/virtual_file_system.rs`

## Success Criteria
- Zero O(N²) en hot paths
- Zero allocations innecesarias en loops
- Todos los tests pasan
- Sin regresiones funcionales
