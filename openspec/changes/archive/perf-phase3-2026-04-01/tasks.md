# Tasks: Performance Phase 3

## Batch 1 (3 agents parallel)

### T1: symbol_index IndexMap eviction
- [ ] Añadir `indexmap = "2"` a Cargo.toml [dependencies]
- [ ] Cambiar `query_cache` de HashMap a IndexMap en symbol_index.rs
- [ ] Verificar que eviction ahora elimina la entrada más vieja
- [ ] `cargo check`

### T2: inline_strategy Arc<str>
- [ ] `FunctionDefinition.source: String` → `Arc<str>`
- [ ] Actualizar constructor y find_function_definition
- [ ] Actualizar callers que acceden a .source
- [ ] `cargo check`

### T3: call_graph callers/callees
- [ ] Auditar TODOS los callers de callers() y callees()
- [ ] Si la mayoría necesita owned Vec, hacer SymbolId Copy o dejar como está
- [ ] Si se puede cambiar a iterator, hacerlo
- [ ] `cargo test call_graph`

## Batch 2 (3 agents parallel)

### T4: dependency_repository iterator
- [ ] Añadir `get_all_symbols_iter()` al trait con `Box<dyn Iterator<Item = Symbol>>`
- [ ] Implementar en PetGraphStore
- [ ] Actualizar callers para usar iterador cuando sea posible
- [ ] `cargo check`

### T5: shared find_usages helper
- [ ] Leer handle_find_usages y handle_find_usages_with_context
- [ ] Extraer lógica compartida en función helper privada
- [ ] Ambos handlers usan el helper
- [ ] `cargo check`

### T6: lowercase symbol index in handlers
- [ ] Crear helper build_lowercase_symbol_index
- [ ] Usar en handle_get_call_hierarchy, handle_analyze_impact, find_symbol_in_graph
- [ ] `cargo check`
