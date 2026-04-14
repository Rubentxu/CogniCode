# Tasks: Performance Phase 2

## Batch 1 — Independent (6 agents parallel)

### T1: cycle_detector.rs (C1, H2)
- [ ] Añadir `in_stack: HashSet<SymbolId>` a TarjanState
- [ ] Insertar en `in_stack` cuando se push a stack, remover cuando se pop
- [ ] Reemplazar `state.stack.contains(&successor)` con `state.in_stack.contains(&successor)`
- [ ] Añadir `pub fn symbol_ids(&self) -> impl Iterator<Item = (&SymbolId, &Symbol)>` a CallGraph
- [ ] Usar `symbol_ids()` en cycle_detector en vez de `fully_qualified_name()`
- [ ] `cargo test cycle_detector`

### T2: call_graph.rs (C4, M1, M2)
- [ ] Refactor `find_path()`: usar predecessor HashMap, reconstruir path al final
- [ ] `dependents()`: eliminar `collect::<Vec<_>>().into_iter()`
- [ ] `callers()` y `callees()`: retornar slices o iteradores en vez de Vec
- [ ] `cargo test call_graph`

### T3: ast_scanner.rs (H4, H5, H6)
- [ ] Cambiar `ScannedNode.node_type` de `String` a `&'a str` con lifetime del tree
- [ ] `scan_recursive()`: aceptar `&mut Vec<ScannedNode>` del padre, push directo sin Vec intermedio
- [ ] `find_nodes_by_type()`: filtrar durante scan, no después
- [ ] Ajustar lifetimes en ScanResult y OutlineBuilder si es necesario
- [ ] `cargo test ast_scanner`

### T4: tree_sitter_parser.rs (H7)
- [ ] Pre-split source en `Vec<&str>` antes de traversal
- [ ] `extract_context()`: aceptar `&[&str]` en vez de coleccionar lines
- [ ] Pasa pre-split a métodos que llaman extract_context
- [ ] `cargo test tree_sitter_parser`

### T5: graph_event.rs + impact_analyzer.rs (C2, C3)
- [ ] `calculate_diff()`: indexar old/new symbols en HashMap para O(1) lookup
- [ ] `collect_affected_files()`: cambiar `files: Vec<String>` a `files: HashSet<String>`
- [ ] `cargo test graph_event impact_analyzer`

### T6: pet_graph_store.rs (H11, M8, M9)
- [ ] Eliminar líneas 164-178 (dead code post remove_node)
- [ ] Doble alloc en add_symbol/ensure_symbol: computar key una vez
- [ ] `get_all_symbols()`: retornar iterador en vez de Vec
- [ ] `cargo test pet_graph_store`

## Batch 2 — Semi-dependent (4 agents parallel)

### T7: symbol.rs + location.rs (H1)
- [ ] Añadir `fqn: String` campo a Symbol, computar en new()
- [ ] `fully_qualified_name()` retorna `&str` en vez de `String`
- [ ] Location: implementar `Display` con `fmt::Write`
- [ ] `cargo test symbol location`

### T8: symbol_index.rs + symbol_code.rs (H12, H13, M10, M11)
- [ ] Cache values como `Arc<Vec<SymbolLocation>>` y `Arc<CachedSymbolCode>`
- [ ] Parser pool por lenguaje en symbol_code.rs
- [ ] Usar IndexMap para eviction order (o documentar la limitación)
- [ ] `cargo test symbol_index symbol_code`

### T9: outline.rs + analysis_service.rs (H14, H3, M3)
- [ ] Eliminar `name.clone()` redundante en outline.rs
- [ ] `get_project_graph()`: retornar `Arc<CallGraph>` o referencia
- [ ] `.to_string_lossy().into_owned()`
- [ ] `cargo test outline analysis_service`

### T10: refactor strategies (H8, H9, H10, H15, H16)
- [ ] Añadir `const SOURCE: &str = "source"` — usar en Location
- [ ] Pre-split lines una vez en execute(), pasar a validate+prepare_edits
- [ ] `FunctionDefinition.source`: `Arc<str>` en vez de `String`
- [ ] `find_free_variables` + `find_local_variables` → single pass en extract_strategy
- [ ] `cargo test refactor`

## Batch 3 — Handlers + Misc (2 agents parallel)

### T11: handlers.rs (M4, M5, M6, M7)
- [ ] Build lowercase index once en handlers que iteran símbolos
- [ ] Extraer helper compartido entre find_usages y find_usages_with_context
- [ ] Eliminar `path_entries.clone()` innecesario
- [ ] Usar `graph.roots()` y `graph.leaves()` directamente
- [ ] `cargo test handlers`

### T12: misc services (M12, M13, M14)
- [ ] `lsp_proxy_service.rs`: adquirir write lock una sola vez en setup_default_servers
- [ ] `context_compressor.rs`: single pass para filtrar símbolos
- [ ] `virtual_file_system.rs`: `get_content` retornar `Option<Arc<str>>`
- [ ] `cargo test lsp_proxy context_compressor virtual_file_system`
