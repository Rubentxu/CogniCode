# Design: LSP & Analysis Performance Optimization

## Architecture Decisions

### AD-1: CallGraph Name Index
- Añadir campo `name_index: HashMap<String, Vec<SymbolId>>` a `CallGraph`
- Mantener sincronizado en `add_symbol()`, `remove_symbol()`, y al reconstruir desde `apply_events()`
- `dependents()` y `find_all_dependents()` usan `name_index` para fallback en vez de contains() sobre todos los símbolos
- Método nuevo `find_by_name(&self, name: &str) -> Vec<&Symbol>` público

### AD-2: Fallback con LightweightIndex
- `TreesitterFallbackProvider` obtiene `Option<Arc<LightweightIndex>>` en el constructor
- `CompositeProvider::new()` construye un `LightweightIndex` y lo comparte via `Arc`
- `get_definition()` usa `index.find_symbol(identifier)` → filtra por kind (Function/Struct/Class) → lee solo esos archivos → verifica patrón fn/def/struct/class
- Sin índice: fallback a walkdir actual

### AD-3: build_project_graph Incremental
- Struct `FileCache` con `HashMap<String, (u64, Vec<Symbol>, Vec<CallRelationship>)>` donde la clave es `path:mtime`
- Parser pool: `HashMap<Language, TreeSitterParser>` creado una vez, consultado por archivo
- `build_project_graph()` primero checkea mtime de cada archivo, solo reparsea los cambiados
- Los símbolos/relaciones cacheados se reutilizan directamente

### AD-4: SemanticSearch Top-K
- `IndexedSymbol` incluye `name_lower: String` calculado en `index_file()`
- `search()` usa `BinaryHeap<Reverse<SearchResult>>` con capacity `max_results`
- Construir `Symbol` solo para los top-k resultados finales

### AD-5: OnDemandGraphBuilder Smart Fuzzy
- `find_related_files()` primero busca en `self.index` con lookup exacto
- Si no hay exact match, busca prefix con los primeros 3 chars del query
- Levenshtein solo sobre el subconjunto prefix (usualmente < 50 entries)
- Si el subconjunto prefix > 50, truncar a los 50 primeros

### AD-6: LightweightIndex File Secondary Index
- `find_symbol()` cambia retorno a `&[SymbolLocation]` — auditar callers y adaptar
- Añadir `file_index: HashMap<String, Vec<String>>` (file_path → list of symbol names)
- `find_in_file()` usa file_index para lookup O(1)

### AD-7: GraphCache Batch Updates
- Mantener ArcSwap (bueno para reads concurrentes)
- Añadir `pending_events: Mutex<Vec<GraphEvent>>` para batch
- `queue_event()` acumula eventos, `flush_events()` aplica todos de una vez
- `apply_events()` público sigue funcionando (flush inmediato)

## File Dependencies
```
P8 (call_graph.rs) ← independiente
P2 (fallback.rs + composite.rs) ← depende de P4 (LightweightIndex API)
P3 (analysis_service.rs) ← independiente  
P6 (semantic_search.rs) ← independiente
P5 (on_demand_graph.rs) ← depende de P4 (LightweightIndex API)
P4 (lightweight_index.rs) ← independiente, pero P2 y P5 dependen de su API
P7 (graph_cache.rs) ← depende de P8 (CallGraph clone cost)
```

## Parallelization Plan
- **Batch 1** (fully independent): P8, P3, P6, P4
- **Batch 2** (depends on P4): P2, P5
- **Batch 3** (depends on P8): P7
