# Tasks: LSP & Analysis Performance Optimization

## Batch 1 — Independent (parallel)

### T1: CallGraph Name Index (P8)
- [ ] Añadir `name_index: HashMap<String, Vec<SymbolId>>` a `CallGraph` struct
- [ ] Actualizar `new()` para inicializar name_index vacío
- [ ] Actualizar `add_symbol()` para insertar en name_index (base_name lowercase)
- [ ] Actualizar `remove_symbol()` para limpiar name_index
- [ ] Refactor `dependents()`: usar name_index en fallback en vez de contains() scan
- [ ] Refactor `find_all_dependents()`: usar name_index en fallback
- [ ] Añadir método público `find_by_name(&self, name: &str) -> Vec<&Symbol>`
- [ ] Verificar: cargo test call_graph

### T2: Incremental build_project_graph (P3)
- [ ] Crear `FileCacheEntry` struct con mtime, symbols, relationships
- [ ] Añadir `file_cache: HashMap<String, FileCacheEntry>` a AnalysisService
- [ ] Crear parser pool `HashMap<Language, TreeSitterParser>` en build_project_graph
- [ ] Antes de parsear cada archivo: obtener mtime, checkear cache
- [ ] Si mtime coincide: reusar cached symbols/relationships
- [ ] Si mtime cambia: reparsear y actualizar cache
- [ ] Verificar: cargo test analysis_service

### T3: SemanticSearch Top-K (P6)
- [ ] Añadir campo `name_lower: String` a `IndexedSymbol`
- [ ] Calcular name_lower en `index_file()` al crear IndexedSymbol
- [ ] Cambiar `search()` para usar `indexed.name_lower` en vez de to_lowercase()
- [ ] Reemplazar `results.sort() + truncate` con `BinaryHeap<Reverse<SearchResult>>` top-k
- [ ] Mover Symbol::new() construction después del top-k filter
- [ ] Verificar: cargo test semantic_search

### T4: LightweightIndex Slices + File Index (P4)
- [ ] Añadir `file_index: HashMap<String, Vec<String>>` (file → symbol names)
- [ ] Actualizar build_index/build_from_sources/insert para mantener file_index
- [ ] Cambiar `find_symbol()` retorno a `&[SymbolLocation]`
- [ ] Actualizar `find_in_file()` para usar file_index (O(1) lookup)
- [ ] Auditar todos los callers de find_symbol() y adaptar a &[SymbolLocation]
- [ ] Reusar TreeSitterParser por lenguaje en build_index
- [ ] Verificar: cargo test lightweight_index

## Batch 2 — Depends on T4

### T5: Fallback con LightweightIndex (P2)
- [ ] Añadir campo `index: Option<Arc<LightweightIndex>>` a TreesitterFallbackProvider
- [ ] Actualizar `new()` y añadir `with_index()` constructor
- [ ] Refactor `get_definition()`: usar index.find_symbol() → filter by kind → read only candidate files
- [ ] Mantener walkdir fallback cuando index es None o símbolo no encontrado
- [ ] Actualizar CompositeProvider para construir y compartir Arc<LightweightIndex>
- [ ] Verificar: cargo test fallback + composite

### T6: OnDemandGraphBuilder Smart Fuzzy (P5)
- [ ] Refactor `find_related_files()`: buscar exact match primero en index
- [ ] Si no hay exact: buscar prefix con primeros 3 chars del query
- [ ] Levenshtein solo sobre subconjunto prefix (max 50 entries)
- [ ] Verificar: cargo test on_demand_graph

## Batch 3 — Depends on T1

### T7: GraphCache Batch Updates (P7)
- [ ] Añadir `pending_events: Mutex<Vec<GraphEvent>>` a GraphCache
- [ ] Añadir método `queue_event(&self, event: GraphEvent)`
- [ ] Añadir método `flush_events(&self) -> Result<(), CallGraphError>`
- [ ] Mantener `apply_events()` público como flush inmediato (alias de queue + flush)
- [ ] Verificar: cargo test graph_cache
