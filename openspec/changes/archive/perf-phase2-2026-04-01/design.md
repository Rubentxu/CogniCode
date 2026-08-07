# Design: Performance Phase 2

## Batch 1 — Independent Files

### D1: cycle_detector.rs
- Reemplazar `state.stack.contains(&successor)` con `state.in_stack: HashSet<SymbolId>` — O(1) membership
- Añadir método `CallGraph::symbol_ids()` que retorne iterador de `(&SymbolId, &Symbol)` para evitar `fully_qualified_name()` alloc
- Reusar el método en `find_minimal_feedback_set` y `find_cycles`

### D2: call_graph.rs
- `find_path()`: usar predecessor map `HashMap<SymbolId, SymbolId>` en vez de clonar path por cada expansión
- `dependents()`: reemplazar `collect::<Vec<_>>().into_iter()` con iterador directo (Box<dyn Iterator>)
- `callers()`/`callees()`: retornar slice references en vez de Vec colectado

### D3: ast_scanner.rs
- `ScannedNode.node_type`: cambiar de `String` a `&'tree str` con lifetime del Tree
- `scan_recursive()`: push directo al children del padre, sin Vec intermedio
- `find_nodes_by_type()`: filtrar durante el scan recursivo, no después

### D4: tree_sitter_parser.rs
- Pre-split source en `Vec<&str>` una vez, pasar slice a traversals
- `extract_context()`: aceptar `&[&str]` pre-split en vez de coleccionar lines

### D5: graph_event.rs + impact_analyzer.rs
- `calculate_diff()`: usar `HashMap<String, usize>` para O(1) lookup en vez de `iter().find()`
- `collect_affected_files()`: usar `HashSet<String>` en vez de `Vec<String>` para `files.contains()`

### D6: pet_graph_store.rs
- Eliminar líneas 164-178 (dead code — remove_node ya limpia edges)
- Doble alloc en líneas 58-59, 77-78: computar key una vez
- `get_all_symbols()`: retornar iterador en vez de Vec<Symbol>

## Batch 2 — Semi-dependent

### D7: symbol.rs + location.rs
- Añadir campo `fqn_cache: Option<String>` a Symbol, computar lazy en `fully_qualified_name()`
- Location: usar `impl Display` con `fmt::Write` en vez de `format!`

### D8: symbol_index.rs + symbol_code.rs
- Cache values como `Arc<CachedSymbolCode>` y `Arc<Vec<SymbolLocation>>`
- Parser pool por lenguaje en symbol_code.rs

### D9: outline.rs + analysis_service.rs
- Eliminar `name.clone()` redundante en outline.rs:138
- `get_project_graph()`: retornar `Arc<CallGraph>` en vez de clonar
- `.to_string_lossy().into_owned()` en analysis_service

### D10: refactor strategies (5 files)
- Shared parse: `validate()` y `prepare_edits()` deben compartir source+tree
- Pre-split lines una vez, pasar a todos los métodos
- `"source".to_string()` → `const SOURCE: &str = "source"` en Location
- `extract_context()`: aceptar `&[&str]` pre-split
- `FunctionDefinition.source`: usar `Arc<str>` en vez de `String`
- `find_free_variables` + `find_local_variables` → single pass

## Batch 3 — Handlers + Misc

### D11: handlers.rs
- `find_symbol_in_graph()`: build lowercase index once
- `handle_find_usages` + `handle_find_usages_with_context`: extraer helper compartido
- Eliminar `path_entries.clone()` innecesario
- Usar `graph.roots()` y `graph.leaves()` directamente

### D12: misc services
- `lsp_proxy_service.rs`: adquirir write lock una sola vez
- `context_compressor.rs`: single pass para filtrar símbolos por tipo
- `virtual_file_system.rs`: `get_content` retornar `Option<Arc<str>>`
