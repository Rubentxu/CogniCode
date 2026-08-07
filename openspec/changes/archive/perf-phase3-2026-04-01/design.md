# Design: Performance Phase 3

## Batch 1

### D1: symbol_index.rs — IndexMap LRU eviction
- Añadir `indexmap = "2"` a Cargo.toml dependencies
- Cambiar `query_cache: HashMap` a `query_cache: IndexMap`
- `evict_expired_entries()` ahora elimina la entrada más vieja (primera inserción) — IndexMap mantiene orden
- Resto de la API no cambia

### D2: inline_strategy.rs — Arc<str> source
- `FunctionDefinition.source: String` → `FunctionDefinition.source: Arc<str>`
- Actualizar constructor para aceptar `Arc<str>`
- Actualizar `find_function_definition()` para pasar `Arc::from(source)` en vez de `source.to_string()`
- Actualizar callers de FunctionDefinition si acceden a .source

### D3: call_graph.rs — callers/callees sin clone interno
- `callers()`: cambiar de `.cloned().collect()` a retornar iterador de referencias
- `callees()`: mismo cambio
- NOTA: Esto cambia el tipo de retorno. Necesito auditar callers y decidir:
  - Si callers necesitan owned: cambiar `SymbolId` a Copy (si es posible) o dejar Vec
  - Si pueden usar referencias: cambiar a `impl Iterator<Item = &SymbolId>`
- Decisión: `SymbolId(String)` no puede ser Copy. Pero podemos hacer `SymbolId` wrapper con `Copy` si usamos `Arc<str>`. Más simple: dejar el retorno como está pero eliminar clones innecesarios internamente.

## Batch 2

### D4: dependency_repository trait — iterator
- `get_all_symbols()` → `get_all_symbols_iter()` que retorna `Box<dyn Iterator<Item = Symbol>>`
- Mantener `get_all_symbols()` como wrapper que colecciona (para backward compat)
- Actualizar `PetGraphStore` implementación
- Actualizar callers en handlers y analysis_service

### D5: handlers.rs — shared find_usages helper
- Extraer lógica compartida entre `handle_find_usages` y `handle_find_usages_with_context`
- Crear función privada `find_usages_in_project(source_dir, symbol_name) -> Vec<Usage>`
- Ambos handlers llaman al helper, uno con contexto extra

### D6: handlers.rs — lowercase index
- Crear helper `fn build_lowercase_symbol_index(graph: &CallGraph) -> HashMap<String, Vec<Symbol>>`
- Usar en handlers que iteran símbolos con `.to_lowercase()`: handle_get_call_hierarchy, handle_analyze_impact, find_symbol_in_graph
