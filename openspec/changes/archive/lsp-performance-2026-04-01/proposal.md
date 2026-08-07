# Proposal: LSP & Analysis Performance Optimization

## Intent
Eliminar los 8 cuellos de botella de rendimiento identificados en el análisis de rendimiento de CogniCode. El objetivo es reducir latencia en operaciones interactivas (hover, definition, search, graph traversal).

## Scope

### Archivos afectados
- `src/infrastructure/lsp/providers/fallback.rs` — Punto 2
- `src/application/services/analysis_service.rs` — Punto 3
- `src/infrastructure/graph/lightweight_index.rs` — Punto 4
- `src/infrastructure/graph/on_demand_graph.rs` — Punto 5
- `src/infrastructure/semantic/semantic_search.rs` — Punto 6
- `src/infrastructure/graph/graph_cache.rs` — Punto 7
- `src/domain/aggregates/call_graph.rs` — Punto 8

### No afectados
- LSP providers (ya corregidos en sesión anterior)
- VFS, MCP handlers, CLI commands

## Approach

### Fase 1 (Alta prioridad) — Paralelizable
- **P8**: CallGraph: índice auxiliar base_name→SymbolId[], eliminar fallback lineal por contains()
- **P2**: Fallback: inyectar Arc<LightweightIndex>, usar find_symbol() en vez de walkdir+read_to_string
- **P3**: build_project_graph: parser pool por lenguaje, caché por archivo (mtime hash), unificar pasadas AST

### Fase 2 (Media prioridad) — Paralelizable
- **P6**: SemanticSearch: precomputar name_lower en IndexedSymbol, BinaryHeap top-k en vez de sort+truncate
- **P5**: OnDemandGraphBuilder: buscar exact/prefix primero, fuzzy solo sobre subconjunto candidato
- **P4**: LightweightIndex: find_symbol() devuelve &[SymbolLocation], índice secundario por archivo

### Fase 3
- **P7**: GraphCache: evaluar RwLock<CallGraph> vs ArcSwap+clone, o separar caches granulares

## Success Criteria
- Todas las operaciones interactivas < 100ms (excluyendo LSP warmup)
- Zero allocations innecesarias en hot paths
- Todos los tests unitarios existentes pasan
- Sin regresiones funcionales
