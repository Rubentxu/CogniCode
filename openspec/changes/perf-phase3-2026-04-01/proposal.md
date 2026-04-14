# Proposal: Performance Phase 3 — Remaining Items

## Intent
Completar las 6 optimizaciones pendientes de la revisión de rendimiento, descartando las 2 de mayor riesgo/bajo ROI (H8 triple parseo en strategies, M5 caching en find_usages).

## Scope
### Batch 1 — Low risk (3 agents)
- **M10**: `symbol_index.rs` — eviction order con `indexmap` (fix correctness bug)
- **H15**: `inline_strategy.rs` — `FunctionDefinition.source` → `Arc<str>`
- **M2**: `call_graph.rs` — `callers()`/`callees()` eliminar clone interno

### Batch 2 — Medium risk (3 agents)
- **M9**: `dependency_repository.rs` trait — `get_all_symbols()` retorna iterador + update implementers
- **M6**: `handlers.rs` — extraer helper compartido `find_usages` / `find_usages_with_context`
- **M4**: `handlers.rs` — build lowercase index once en handlers que iteran símbolos

## Success Criteria
- Todos los tests pasan
- Eviction en symbol_index ahora es LRU (por inserción)
- FunctionDefinition no clona source completo
- callers/callees evitan alloc interno
- get_all_symbols no fuerza clone del grafo
- find_usages lógica no duplicada
