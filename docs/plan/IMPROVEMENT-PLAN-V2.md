# CogniCode — Plan Maestro de Mejoras v2

> **Fecha**: Abril 2026
> **Estado**: Plan de implementación
> **Alcance**: Mejoras derivadas de la integración con RCode + mejoras arquitecturales + persistencia
> **Fuente**: Sesiones de trabajo de integración CogniCode ↔ RCode

---

## Resumen Ejecutivo

CogniCode ha evolucionado desde el plan original (Fases 0-5 en `IMPROVEMENT-PLAN.md`). Las Fases 0-4 están sustancialmente completas: el parser TreeSitter funciona, el CallGraph está implementado, 626 tests pasan, y la integración con RCode está operativa con 19 herramientas expuestas.

Este documento recoge **todas las mejoras pendientes** identificadas durante el proceso de integración con RCode, organizadas por prioridad y dependencias. No reemplaza el plan original — lo complementa con la siguiente evolución.

**Métricas actuales**: ~48K LOC | 626 tests pasando | 28+ métodos en WorkspaceSession | 19 tools en RCode | edition 2024 | thiserror 2.0

---

## Índice

1. [P0 — Arreglo de API Pública](#p0--arreglo-de-api-pública)
2. [P1 — Rendimiento y Eficiencia](#p1--rendimiento-y-eficiencia)
3. [P2 — Capa de Persistencia](#p2--capa-de-persistencia)
4. [P3 — Features Deseables](#p3--features-deseables)
5. [P4 — Arquitectura y Calidad](#p4--arquitectura-y-calidad)
6. [P5 — Migración MCP a rmcp SDK](#p5--migración-mcp-a-rmcp-sdk)
7. [Roadmap de Implementación](#roadmap-de-implementación)
8. [Dependencias a Añadir](#dependencias-a-añadir)

---

## P0 — Arreglo de API Pública

> Prioridad: **Crítica** — Sin esto, la integración con consumidores es friction-heavy
> Esfuerzo estimado: **2-3 días**

### P0.1 `get_graph_stats()` — Estadísticas sin re-build

**Problema**: No existe forma de obtener `symbol_count` y `edge_count` del grafo cacheado sin reconstruirlo. `build_lightweight_index()` siempre llama `build_project_graph()` internamente, incluso si el grafo ya está en memoria.

**Firma propuesta**:
```rust
impl WorkspaceSession {
    /// Returns graph statistics from the cached graph.
    /// Returns None if no graph has been built.
    pub fn get_graph_stats(&self) -> Option<GraphStats> { ... }
}

pub struct GraphStats {
    pub symbol_count: usize,
    pub edge_count: usize,
    pub file_count: usize,
    pub language_breakdown: Vec<(String, usize)>,  // (language, file_count)
    pub index_timestamp: Option<std::time::Instant>,
}
```

**Implementación**: Acceder a `self.analysis.get_project_graph()` (ya cached via `ArcSwap`), iterar `symbol_count()` + `edge_count()`, computar language_breakdown desde los file paths de los símbolos.

**Criterio de aceptación**:
- Llamar `get_graph_stats()` después de `build_graph("full")` retorna datos sin re-parsear
- Llamar `get_graph_stats()` sin haber hecho build retorna `None`

---

### P0.2 `get_all_symbols()` — Listar todos los símbolos indexados

**Problema**: No hay forma de obtener "todos los símbolos" sin usar `semantic_search("")` (que es fuzzy y no determinista). Para hot paths, reportes, y enumeración, se necesita acceso directo al índice completo.

**Firma propuesta**:
```rust
impl WorkspaceSession {
    /// Returns all indexed symbols with optional pagination.
    pub fn get_all_symbols(&self, limit: Option<usize>, offset: Option<usize>) -> Vec<SymbolDto> { ... }
}
```

**Implementación**: Leer `graph.symbols()` del cached graph, paginar con `skip/take`, mapear a `SymbolDto::from_symbol()`.

**Criterio de aceptación**:
- `get_all_symbols(None, None)` retorna todos los símbolos del grafo
- `get_all_symbols(Some(10), Some(20))` retorna 10 símbolos empezando desde el offset 20

---

### P0.3 `get_hot_paths()` nativo — Fan-in directo del grafo

**Problema**: El `CallGraph` tiene `reverse_edges` (fan-in O(1) por símbolo), pero no lo expone. Los consumidores tienen que hacer N llamadas a `get_call_hierarchy` para computar fan-in, que es O(N) en vez de O(1).

**Firma propuesta**:
```rust
impl WorkspaceSession {
    /// Returns the symbols with highest fan-in (most called by other symbols).
    /// Uses reverse_edges index — O(1) per symbol lookup, O(N log N) total sort.
    pub fn get_hot_paths(&self, limit: usize, min_fan_in: usize) -> Vec<HotPathEntry> { ... }
}

pub struct HotPathEntry {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub kind: String,
    pub fan_in: usize,  // number of callers
}
```

**Implementación**: Iterar `reverse_edges` del cached graph, contar entries por símbolo, sort por count descending, truncar.

**Criterio de aceptación**:
- `get_hot_paths(10, 2)` retorna máximo 10 símbolos con fan_in ≥ 2
- Los resultados están ordenados por fan_in descendente

---

### P0.4 `semantic_search` con filtro de `kinds`

**Problema**: El parámetro `kinds` ya existe en `SearchQuery` interno, pero `WorkspaceSession::semantic_search()` solo acepta `query` y `max_results`. Los consumidores no pueden filtrar por tipo de símbolo server-side.

**Firma propuesta** (extendida):
```rust
impl WorkspaceSession {
    /// Semantic search with optional kind filtering.
    pub fn semantic_search_with_kinds(
        &self,
        query: &str,
        max_results: usize,
        kinds: Vec<String>,  // e.g., ["function", "struct"]
    ) -> WorkspaceResult<Vec<SymbolDto>> { ... }
}
```

**Alternativa**: Añadir el parámetro `kinds` directamente a `semantic_search()` con un valor por defecto (breaking change menor).

**Implementación**: Pasar `kinds` al `SearchQuery` interno que ya soporta el campo.

**Criterio de aceptación**:
- `semantic_search_with_kinds("process", 20, vec!["function".into()])` solo retorna funciones
- `semantic_search_with_kinds("User", 20, vec!["struct".into()])` solo retorna structs

---

## P1 — Rendimiento y Eficiencia

> Prioridad: **Alta** — Impacto directo en experiencia de usuario
> Esfuerzo estimado: **3-4 días**

### P1.1 `build_lightweight_index` debe ser idempotente

**Problema**: `build_lightweight_index()` siempre llama `self.analysis.build_project_graph()`, que reconstruye el grafo desde cero. El `WorkspaceSession` ya tiene el grafo cached en `self.graph` (via `ensure_graph_built`), pero `build_lightweight_index` no lo usa.

**Implementación**: Modificar `build_lightweight_index` para que:
1. Checkee si ya hay un grafo cached (`self.analysis.get_project_graph()`)
2. Si existe y `symbol_count > 0`, usarlo directamente
3. Si no existe, construir con `build_project_graph()`

```rust
pub async fn build_lightweight_index(&self, strategy: &str) -> WorkspaceResult<BuildIndexResult> {
    // Use cached graph if available
    let graph = self.analysis.get_project_graph();
    let symbols = graph.symbol_count();
    let edges = graph.edge_count();

    if symbols == 0 {
        // No cached graph — build it
        self.analysis.build_project_graph(&self.workspace_root)?;
        let graph = self.analysis.get_project_graph();
        // ... update counts
    }

    Ok(BuildIndexResult {
        success: true,
        symbols_indexed: symbols,
        locations_indexed: symbols + edges,  // edges derived
        // ...
    })
}
```

**Criterio de aceptación**:
- Llamar `build_lightweight_index()` después de `build_graph("full")` NO reconstruye el grafo
- El resultado tiene los mismos counts que el grafo cacheado

---

### P1.2 Indexing incremental (delta) — Solo re-indexar archivos cambiados

**Problema**: `build_graph("full")` reconstruye TODO el proyecto en cada llamada. Para proyectos medianos (200+ archivos), esto puede tomar 3-15 segundos. Con consumidores como RCode disparando re-indexing en cada file change, el coste es inaceptable.

**Implementación**:

1. Añadir `FileManifest` al `CallGraph`:
```rust
pub struct CallGraph {
    symbols: HashMap<SymbolId, Symbol>,
    edges: HashMap<SymbolId, HashSet<(SymbolId, DependencyType)>>,
    reverse_edges: HashMap<SymbolId, HashSet<SymbolId>>,
    name_index: HashMap<String, Vec<SymbolId>>,
    // NUEVO:
    file_manifest: HashMap<PathBuf, FileManifestEntry>,
}

pub struct FileManifestEntry {
    pub mtime: std::time::SystemTime,
    pub content_hash: [u8; 32],  // blake3
    pub symbol_ids: HashSet<SymbolId>,
    pub language: Language,
}
```

2. Nueva estrategia `"incremental"` en `build_project_graph`:
```rust
pub fn build_project_graph_incremental(&self, project_dir: &Path, manifest: &FileManifest) -> AppResult<IncrementalResult> {
    let current_files = walk_source_files(project_dir);
    let mut events = Vec::new();

    // Detectar archivos nuevos, modificados, eliminados
    for file in current_files {
        let current_mtime = fs::metadata(&file)?.modified()?;
        match manifest.get(&file) {
            None => events.push(ParseFile(file)),                           // Nuevo
            Some(entry) if entry.mtime != current_mtime => {
                events.push(RemoveFileSymbols(file.clone(), entry.symbol_ids.clone()));
                events.push(ParseFile(file));                                 // Modificado
            }
            Some(_) => {}                                                    // Sin cambios
        }
    }

    // Archivos eliminados
    for file in manifest.keys() {
        if !current_files.contains(file) {
            events.push(RemoveFileSymbols(file.clone(), manifest[file].symbol_ids.clone()));
        }
    }

    // Aplicar solo los cambios
    graph.apply_events(&events)?;
    Ok(IncrementalResult { files_parsed: events.parsed_count(), files_removed: events.removed_count() })
}
```

**Criterio de aceptación**:
- Cambiar 1 archivo en un proyecto de 200: re-indexing < 100ms (vs 3-15s full rebuild)
- Cambiar 0 archivos: < 5ms (solo mtime check)
- El grafo resultado es idéntico al de un full rebuild

---

### P1.3 Cache de lenguajes detectados

**Problema**: Los consumidores computan el language breakdown recorriendo el filesystem. Esta información se puede obtener gratis durante el indexing.

**Implementación**: En `build_project_graph`, trackear extensiones de los archivos parseados. Exponer via `get_graph_stats().language_breakdown`.

**Criterio de aceptación**: `get_graph_stats()` retorna `(language, file_count)` derivado de los archivos parseados, sin filesystem walk adicional.

---

## P2 — Capa de Persistencia

> Prioridad: **Alta** — Transforma CogniCode de parser efímero a índice persistente
> Esfuerzo estimado: **7-10 días**

### P2.1 Visión general

**Estado actual**: CogniCode construye el CallGraph en memoria cada vez que el proceso arranca. No hay persistencia. Cada restart = rebuild completo.

**Objetivo**: Persistir el CallGraph + FileManifest + Metrics en disco. Arranque instantáneo. Indexing incremental real.

**Arquitectura propuesta**:
```
~/.local/share/cognicode/
├── projects/
│   ├── {project-hash}/              # blake3 hash del project path
│   │   ├── manifest.json            # Project metadata
│   │   ├── graph.bincode            # CallGraph serializado (bincode)
│   │   ├── file_index.bincode       # FileManifest: {path -> (mtime, hash, symbol_ids)}
│   │   ├── diagnostics_cache.bincode
│   │   └── metrics/
│   │       ├── latest.json
│   │       └── {timestamp}.json     # Snapshots históricos
│   └── ...
├── global_config.json
└── cache/                           # Parser cache compartido
```

### P2.2 Tecnología: bincode + redb

**Decisión**: Arquitectura híbrida de 2 capas.

| Capa de datos | Tecnología | Por qué |
|---------------|-----------|---------|
| **CallGraph** (1-20 MB blob) | bincode (serialize/deserialize) | El grafo es un monolito que se recorre en Rust. Cualquier DB añade overhead sin beneficio. bincode: 1-5ms para load/save. |
| **FileManifest** (50-500 KB) | redb (embedded KV, ACID) | Key-value puro: `path -> manifest_entry`. redb: pure Rust, zero unsafe, prefix scan, ACID transactions. |
| **Metrics** (creciente) | redb | Append-only time-series. Range scan: `range(from..=to)`. |
| **Project Registry** | redb | CRUD simple de proyectos indexados. |

**¿Por qué no SQLite?** SQLite es excelente y RCode ya lo tiene. Pero para CogniCode como librería independiente:
- redb es pure Rust (compila en 30s vs 2min de sqlite3-sys con C code)
- redb es ~200KB de binary overhead vs ~2MB de SQLite
- Sin build flags, cross-platform, sin C toolchain
- Interfaz `GraphStore` trait permite migrar a SQLite si se integra 100% en RCode

**¿Por qué no una Graph Database (Neo4j, SurrealDB)?**
- El CallGraph es pequeño (1-20MB, 2K-30K nodos)
- Los HashMaps en memoria son más rápidos que cualquier DB remota para BFS/DFS
- CogniCode es una **librería** — no puede depender de un server externo
- Los traversal (get_call_hierarchy, trace_path, analyze_impact) son O(V+E) en HashMaps

**Benchmarks estimados**:

| Operación | Sin persistencia | bincode + redb | SQLite | Neo4j |
|-----------|-----------------|----------------|--------|-------|
| Startup (load) | 3-15s (full parse) | **50-200ms** | 100-500ms | 2-10s |
| Re-index (1 file) | 3-15s | **50-100ms** | 200-500ms | N/A |
| Re-index (0 files) | 3-15s | **5ms** | 10-50ms | N/A |
| Save snapshot | N/A | **2-10ms** | 5-20ms | 50-200ms |
| Memory overhead | 1-20MB | +2-5MB | +5-15MB | +200MB+ |

### P2.3 `GraphStore` Trait — Abstracción de persistencia

```rust
/// Abstraction over persistence backends.
/// Implementations: FilesystemGraphStore (default), SqliteGraphStore (for RCode).
pub trait GraphStore: Send + Sync {
    /// Save the complete graph + file manifest to disk.
    fn save(&self, project_hash: &str, graph: &CallGraph, manifest: &FileManifest) -> Result<()>;

    /// Load graph + manifest from disk. Returns None if not found or corrupt.
    fn load(&self, project_hash: &str) -> Result<Option<(CallGraph, FileManifest)>>;

    /// Check if persisted data exists for this project.
    fn exists(&self, project_hash: &str) -> bool;

    /// Delete persisted data.
    fn delete(&self, project_hash: &str) -> Result<()>;

    /// Save project metrics snapshot.
    fn save_metrics(&self, project_hash: &str, metrics: &ProjectMetrics) -> Result<()>;

    /// Load metrics history in a time range.
    fn load_metrics_range(
        &self,
        project_hash: &str,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<ProjectMetrics>>;
}
```

### P2.4 Serialización del CallGraph

**Prerrequisito**: Añadir `Serialize, Deserialize` a los tipos del dominio:

| Tipo | Derives actuales | Necesita |
|------|-----------------|----------|
| `CallGraph` | `Debug, Clone, PartialEq, Eq` | + `Serialize, Deserialize` |
| `Symbol` | `Debug, Clone, PartialEq, Eq, Hash` | + `Serialize, Deserialize` |
| `SymbolId` | `Debug, Clone, PartialEq, Eq, Hash` | + `Serialize, Deserialize` |
| `FunctionSignature` | `Debug, Clone, PartialEq, Eq, Hash` | + `Serialize, Deserialize` |
| `Parameter` | `Debug, Clone, PartialEq, Eq, Hash` | + `Serialize, Deserialize` |
| `DependencyType` | Ya tiene | ✅ |
| `Location` | Ya tiene | ✅ |
| `SymbolKind` | Ya tiene | ✅ |
| `SourceRange` | Ya tiene | ✅ |

**NOTA**: Los value objects ya tienen `Serialize, Deserialize`. Solo los aggregates necesitan el add.

### P2.5 FileManifest — Invalidación incremental

```rust
/// Tracks which files have been indexed and their state at index time.
/// Used to detect changes without re-parsing everything.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifest {
    pub entries: HashMap<PathBuf, FileManifestEntry>,
    pub project_root: PathBuf,
    pub build_timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifestEntry {
    pub mtime: SystemTime,
    pub content_hash: [u8; 32],     // blake3 hash
    pub file_size: u64,
    pub symbol_ids: HashSet<SymbolId>,
    pub language: String,
    pub symbol_count: usize,
}

impl FileManifest {
    /// Compare current filesystem state against manifest.
    /// Returns (new_files, modified_files, deleted_files).
    pub fn detect_changes(&self) -> FileChanges { ... }

    /// Update manifest after re-indexing specific files.
    pub fn update_entries(&mut self, files: &[(PathBuf, FileManifestEntry)]) { ... }

    /// Remove entries for deleted files.
    pub fn remove_entries(&mut self, paths: &[PathBuf]) { ... }
}
```

### P2.6 ProjectMetrics — Historial de salud

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetrics {
    pub timestamp: String,                          // RFC 3339
    pub symbol_count: usize,
    pub edge_count: usize,
    pub file_count: usize,
    pub cycle_count: usize,
    pub violation_count: usize,
    pub avg_complexity: f64,
    pub max_complexity: MaxComplexityEntry,
    pub language_breakdown: Vec<(String, f32)>,
    pub hot_paths: Vec<HotPathMetric>,              // (symbol, fan_in)
    pub architecture_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPathMetric {
    pub symbol: String,
    pub fan_in: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaxComplexityEntry {
    pub function: String,
    pub cyclomatic: u32,
    pub file: String,
}
```

**Casos de uso habilitados**:
- "Tu complejidad promedio subió un 15% esta semana"
- "Se añadieron 3 nuevos ciclos desde el último indexing"
- "La función `process_data` pasó de fan-in 5 a fan-in 12 — refactor candidate"
- Gráficos de evolución en UI

### P2.7 Flujo de Startup con Persistencia

```
WorkspaceSession::new(project_path)
│
├── 1. Compute project_hash = blake3(project_path)
│
├── 2. graph_store.load(project_hash)?
│   ├── Found → load CallGraph + FileManifest from disk (~50-200ms)
│   │          → detect_changes()
│   │          → if changes: re-parse changed files only (incremental)
│   │          → if no changes: use cached graph directly
│   │
│   └── Not found → full build_project_graph (first time, 3-15s)
│                  → graph_store.save() for next time
│
├── 3. Populate WorkspaceSession with loaded/built graph
│
└── 4. Background: save updated manifest + metrics
```

### P2.8 Ventaja competitiva como MCP server

Ningún MCP server de code intelligence persiste el índice:
- `sourcegraph` (Cody) → siempre re-indexa
- `aider` → no tiene call graph
- `continue.dev` → no tiene persistencia de análisis
- LSP servers → índice efímero (en proceso)

CogniCode con persistencia sería **el único** que ofrece:
1. Arranque instantáneo con datos pre-calentados (~50ms vs 3-15s)
2. Indexing incremental real (no full rebuild)
3. Historial de métricas del proyecto
4. Diff de grafo entre puntos temporales
5. Múltiples clientes MCP compartiendo el mismo store

---

## P3 — Features Deseables

> Prioridad: **Media** — Valor añadido para consumidores
> Esfuerzo estimado: **5-7 días**

### P3.1 `get_diagnostics()` — Diagnósticos combinados en una sola llamada

**Problema**: Para poblar un snapshot de estado, los consumidores hacen 3+ llamadas separadas (`check_architecture` + `semantic_search` + `get_call_hierarchy`). Una sola llamada sería más eficiente.

**Firma propuesta**:
```rust
impl WorkspaceSession {
    /// Returns all project diagnostics in a single call.
    /// Combines: architecture check + complexity scan + hot paths.
    pub fn get_project_diagnostics(&self) -> WorkspaceResult<ProjectDiagnostics> { ... }
}

pub struct ProjectDiagnostics {
    pub stats: GraphStats,
    pub cycles: Vec<CycleDto>,
    pub violations: Vec<ViolationDto>,
    pub complexity_warnings: Vec<ComplexityWarning>,
    pub hot_paths: Vec<HotPathEntry>,
    pub architecture_score: f64,
}
```

**Criterio de aceptación**: Una sola llamada reemplaza 3+ llamadas separadas con los mismos datos.

---

### P3.2 `subscribe_to_changes()` — Streaming de eventos del grafo

**Problema**: Los consumidores necesitan reaccionar cuando el grafo cambia. Actualmente no hay notificación — hay que polling.

**Firma propuesta**:
```rust
impl WorkspaceSession {
    /// Subscribe to graph change events.
    /// Returns a receiver that emits events when the graph is updated.
    pub fn subscribe_to_changes(&self) -> tokio::sync::broadcast::Receiver<GraphChangeEvent> { ... }
}

pub enum GraphChangeEvent {
    GraphBuilt { symbol_count: usize, edge_count: usize },
    SymbolsAdded { count: usize, files: Vec<PathBuf> },
    SymbolsRemoved { count: usize, files: Vec<PathBuf> },
    DependenciesChanged { affected_symbols: Vec<SymbolId> },
    DiagnosticsUpdated { new_warnings: usize, resolved: usize },
}
```

**Implementación**: Añadir un `broadcast::Sender<GraphChangeEvent>` al `GraphCache` o `AnalysisService`. Emitir eventos en `apply_events`, `set`, y `clear`.

**Criterio de aceptación**:
- Un subscriber recibe `GraphBuilt` cuando se construye el grafo
- Un subscriber recibe `SymbolsAdded` cuando se re-indexa incrementalmente

---

### P3.3 Subgrafos por módulo/directorio

**Problema**: Para proyectos grandes o monorepos, no siempre necesitas el grafo completo.

**Firma propuesta**:
```rust
impl WorkspaceSession {
    /// Build a subgraph limited to specific directories.
    /// Useful for monorepos or focused analysis.
    pub fn build_subgraph(&self, paths: &[&Path]) -> WorkspaceResult<Arc<CallGraph>> { ... }
}
```

**Implementación**: Filtrar `walk_directory` por los paths dados. El CallGraph resultante solo contiene symbols/edges de esos archivos.

**Criterio de aceptación**: `build_subgraph(&["src/auth/"])` retorna un grafo solo con los símbolos del módulo auth.

---

### P3.4 Doc comments en SymbolDto

**Problema**: `SymbolDto.documentation` siempre es `None`. Los doc comments (`///`, `/** */`) se pierden durante parsing.

**Implementación**: Durante TreeSitter parsing, buscar el nodo hermano anterior de tipo `comment` o `doc_comment` antes de cada declaración de función/struct.

**Criterio de aceptación**:
- Una función con `/// Processes incoming orders` tiene `documentation: Some("Processes incoming orders")`
- Hover tool puede mostrar la documentación

---

### P3.5 Métricas de cobertura del grafo

**Problema**: No hay forma de saber si el grafo está completo o hay gaps.

**Firma propuesta**:
```rust
pub struct GraphStats {
    // ... campos existentes ...
    pub coverage: GraphCoverage,
}

pub struct GraphCoverage {
    pub parsed_files: usize,
    pub total_source_files: usize,
    pub coverage_percent: f64,
    pub parsed_languages: Vec<String>,
    pub unresolved_calls: usize,  // calls a símbolos no encontrados en el grafo
}
```

**Implementación**: Trackear archivos parseados vs archivos totales durante `build_project_graph`. Contar edges apuntando a SymbolIds inexistentes.

---

## P4 — Arquitectura y Calidad

> Prioridad: **Media** — Deuda técnica y evolución
> Esfuerzo estimado: **4-5 días**

### P4.1 Separar cache de AnalysisService

**Problema**: `AnalysisService` tiene un `GraphCache` interno (`self.graph_cache`) que causa que `build_lightweight_index` siempre reconstruya. El `WorkspaceSession` tiene su propio cache de grafo. Estos dos caches están desconectados.

**Implementación**:
1. `WorkspaceSession` pasa su grafo cached al `AnalysisService` al crearlo
2. O: `AnalysisService` acepta un grafo pre-existente via constructor
3. `build_lightweight_index` checkea el cache antes de reconstruir

---

### P4.2 DTOs como crate separado (`cognicode-dto`)

**Problema**: Los DTOs están en `cognicode-core` que arrastra tree-sitter, LSP, etc. Consumidores que solo necesitan los tipos (como RCode para type-safe integration) deben compilar todo el árbol.

**Implementación**:
1. Extraer `application/dto/` + `domain/value_objects/` a `cognicode-dto` crate
2. `cognicode-core` depende de `cognicode-dto`
3. Consumidores ligeros pueden depender solo de `cognicode-dto`

---

### P4.3 Progreso de indexing con callback

**Problema**: Para proyectos grandes, el indexing puede tomar >10s. Los consumidores no tienen feedback de progreso.

**Firma propuesta**:
```rust
pub struct IndexingProgress {
    pub current_file: PathBuf,
    pub files_processed: usize,
    pub total_files: usize,
    pub phase: IndexingPhase,
    pub elapsed: std::time::Duration,
    pub estimated_remaining: Option<std::time::Duration>,
}

pub enum IndexingPhase {
    Walking,
    Parsing,
    BuildingGraph,
    ComputingDiagnostics,
    SavingToDisk,
}

impl WorkspaceSession {
    pub fn build_graph_with_progress(
        &self,
        strategy: &str,
        on_progress: Box<dyn Fn(IndexingProgress)>,
    ) -> WorkspaceResult<()> { ... }
}
```

---

### P4.4 Resolve `WorkspaceSession::new` blocking

**Problema**: El constructor es `async` pero internamente bloquea el thread con `block_on` en algunos paths. Esto puede causar panics si se llama desde un contexto tokio.

**Implementación**: Asegurar que todos los paths internos usen `.await` correctamente, sin `block_on`. El constructor ya es `async` — los callers pueden usar `.await`.

---

## P5 — Migración MCP a rmcp SDK

> Prioridad: **Media** — Modernización del MCP server
> Esfuerzo estimado: **3-4 días**
> Estado: Spec completa (ver engram `sdd/migrate-to-rmcp-sdk/spec`)

### P5.1 Contexto

CogniCode tiene un MCP server hand-rolled (~1800 líneas en `server.rs`) que reimplementa:
- Protocolo JSON-RPC (parseo, dispatch, serialización)
- State machine de sesión MCP
- Transporte stdin/stdout
- Request routing

El crate `rmcp` v1.4.0 es el SDK oficial de Rust para MCP y maneja todo esto.

### P5.2 Plan (resumido)

| Spec | Descripción | Criterio de aceptación |
|------|-------------|----------------------|
| **S1** | Añadir `rmcp = "1.4"` con features server + transport-io | `cargo build` pasa |
| **S2** | Crear `RmcpAdapter` implementando `ServerHandler` | 30 tools despachan correctamente |
| **S3** | Migrar `mcp_server.rs` para usar `serve_server()` rmcp | Server arranca via rmcp, OTel activo |
| **S4** | Actualizar `mod.rs` exports | Public API estable, no dangling imports |
| **S5** | Eliminar `server.rs`, `state.rs`, `progress.rs` + tipos JSON-RPC duplicados | Handler logic intacto |
| **S6** | Verificar backward compatibility | `mcp-client` funciona, sandbox ≥ 93.1% pass |

### P5.3 Beneficios

- ~1800 líneas menos de código hand-rolled
- Protocol compliance automático (MCP spec updates via rmcp)
- Cancellation support nativo
- Transporte extensible (future: HTTP, WebSocket)

---

## Roadmap de Implementación

### Fase A: API + Rendimiento (1 semana)

```
Semana 1:
├── Día 1-2: P0.1 get_graph_stats + P0.2 get_all_symbols + P0.4 semantic_search kinds
├── Día 3: P0.3 get_hot_paths nativo
├── Día 4-5: P1.1 build_lightweight_index idempotente + P1.3 language cache
└── Día 5: P3.1 get_diagnostics combinado
```

**Gate**: Todos los cambios de API tienen tests. `WorkspaceSession` expone los nuevos métodos. Los consumidores (RCode) pueden simplificar su código.

### Fase B: Persistencia (2 semanas)

```
Semana 2-3:
├── Día 1-2: P4.2 DTOs crate separado + Serialize derives en domain types
├── Día 3-4: P2.4 bincode serialization + P2.3 GraphStore trait
├── Día 5-6: P2.5 FileManifest + P2.7 Startup flow con persistencia
├── Día 7-8: P1.2 Indexing incremental + P2.1 redb store para metadata
└── Día 9-10: P2.6 Metrics + P4.3 Progress callback + integración tests
```

**Gate**: Startup desde disco < 200ms. Re-indexing incremental de 1 archivo < 100ms. Persistencia ACID.

### Fase C: Features + MCP (1 semana)

```
Semana 4:
├── Día 1-2: P3.2 subscribe_to_changes + P3.3 subgrafos
├── Día 3: P3.4 Doc comments + P3.5 Coverage metrics
├── Día 4-5: P5.1-P5.6 Migración rmcp SDK
└── Día 5: P4.1 Separar cache + P4.4 blocking fix
```

**Gate**: rmcp migration pasa sandbox ≥ 93.1%. Graph events se emiten. Doc comments disponibles.

---

## Dependencias a Añadir

| Dep | Versión | Propósito | Pure Rust? | Binary overhead |
|-----|---------|-----------|------------|-----------------|
| `bincode` | 2.x | Serialización binaria del CallGraph | ✅ | ~50KB |
| `blake3` | 1.x | Hashing de archivos (2GB/s) | ✅ | ~100KB |
| `redb` | 2.x | Key-value store ACID para metadata | ✅ | ~200KB |
| `dirs` | 6.x | Resolver `~/.local/share/` | ✅ | ~10KB |
| `rmcp` | 1.4.x | MCP SDK oficial (solo para cognicode-mcp) | ✅ | ~300KB |

**Total overhead**: ~660KB. Todas pure Rust, sin build flags, sin C toolchain.

### NOTA sobre chrono

Si no se quiere añadir `chrono`, se puede usar `std::time::SystemTime` + RFC 3339 formatting manual para los timestamps de metrics. Menos elegante pero zero deps extra.

---

## Resumen de Impacto

| Mejora | Consumidor (RCode/lib) | MCP Server | Esfuerzo |
|--------|----------------------|------------|----------|
| P0.1-P0.4 API pública | Elimina workarounds, simplifica tools | Más herramientas expuestas | 2-3 días |
| P1.1 Idempotencia | Elimina doble indexing | Faster startup | 1 día |
| P1.2 Incremental | 200x faster re-indexing | Real-time updates | 3 días |
| P2 Persistencia | Startup instantáneo | Único MCP con persisted index | 7-10 días |
| P3.1 Diagnostics combinado | 3 llamadas → 1 | Mejor UX | 1 día |
| P3.2 Event streaming | Elimina FileWatcher externo | Real-time subscriptions | 2 días |
| P3.3 Subgrafos | Análisis parcial | Focused queries | 2 días |
| P4.2 DTOs separados | Type-safe sin heavy deps | N/A | 2 días |
| P5 rmcp migration | N/A | -1800 LOC, spec compliance | 3-4 días |

---

## P6 — Absorción de funcionalidad desde RCode

> Prioridad: **Alta** — Consolidación arquitectural, eliminación de duplicados
> Esfuerzo estimado: **3-4 días** (en CogniCode) + **2-3 días** (en RCode para integrar)
> Prerrequisitos: P0, P1.1, P3.1, P3.2 deben estar completados

### P6.1 Contexto

La integración actual CogniCode ↔ RCode duplica cierta lógica porque CogniCode no exponía APIs suficientes. RCode tuvo que implementar internamente cosas que CogniCode debería hacer: detección de lenguajes, computo de hot paths, population de diagnostics, manejo de snapshot, y parte del LSP.

Con las mejoras P0-P3 implementadas, CogniCode puede absorber esta funcionalidad y RCode la elimina. Resultado: menos código total, menos bugs, una sola fuente de verdad.

### P6.2 LSP — CogniCode como proveedor único de inteligencia LSP

**Estado actual — Dos implementaciones duplicadas**:

| Componente RCode | LOC | Responsabilidad |
|------------------|-----|-----------------|
| `rcode-lsp` — `LspClient` | ~400 LOC | Conexión a LSPs externos (stdin/stdout transport) |
| `rcode-lsp` — `LanguageServerRegistry` | ~350 LOC | Registry de LSPs por language ID, lifecycle management |
| `rcode-lsp` — `LspToolAdapter` | ~300 LOC | Adapter que convierte LSP ops en Tool trait de RCode |
| `rcode-lsp` — `Transport` (stdio) | ~250 LOC | Transporte stdin/stdout para comunicación LSP |
| `rcode-lsp` — Types (responses, requests) | ~489 LOC | Tipos LSP (TextDocumentIdentifier, Location, Hover, etc.) |
| **Total `rcode-lsp`** | **1789 LOC** | |

| Componente CogniCode | LOC | Responsabilidad |
|----------------------|-----|-----------------|
| `cognicode-core` — `infrastructure/lsp/LspClient` | ~500 LOC | Conexión a LSPs externos (stdin/stdout transport) |
| `cognicode-core` — `infrastructure/lsp/LspManager` | ~400 LOC | Gestión de múltiples LSPs, lifecycle, initialization |
| `cognicode-core` — `infrastructure/lsp/types` | ~300 LOC | Tipos LSP reutilizables |
| **Total CogniCode LSP** | **~1200 LOC** | |

**Análisis de duplicación**: Ambas implementaciones hacen lo mismo — conectan a LSPs externos (rust-analyzer, typescript-language-server, etc.) y exponen go_to_definition, hover, find_references. CogniCode ya tiene patrón **LSP-first con fallback a TreeSitter** (más robusto). `rcode-lsp` solo funciona si el LSP está corriendo.

**Consumidores de `rcode-lsp` en RCode**:

| Consumidor | Uso | Puede delegar a CogniCode? |
|------------|-----|---------------------------|
| Agente (via tools) | `GoToDefinitionTool`, `HoverTool`, `FindReferencesTool` | ✅ 100% — CogniCode ya expone estas ops via `WorkspaceSession` |
| Frontend web — outline | `GET /api/sessions/:id/outline` → `document_symbols` | ✅ TreeSitter suficiente (no necesita type info) |
| Frontend web — diagnostics | Potencial futuro | ✅ CogniCode P3.1 get_project_diagnostics |

**Plan de absorción en CogniCode**:

1. **Asegurar que CogniCode expone toda la funcionalidad LSP que RCode necesita**:
   ```rust
   impl WorkspaceSession {
       // Ya existen pero necesitan ser robustos:
       pub async fn go_to_definition(&self, file: &str, line: u32, col: u32) -> WorkspaceResult<Vec<SourceLocation>> { ... }
       pub async fn hover(&self, file: &str, line: u32, col: u32) -> WorkspaceResult<String> { ... }
       pub async fn find_references(&self, file: &str, line: u32, col: u32, include_decl: bool) -> WorkspaceResult<Vec<SourceLocation>> { ... }

       // NUEVO — para el frontend web (outline):
       pub async fn document_symbols(&self, file: &str) -> WorkspaceResult<Vec<DocumentSymbolDto>> { ... }
   }
   ```

2. **Añadir `document_symbols()` para el frontend web**:
   El frontend de RCode usa `rcode-lsp` para obtener el outline de archivos (ruta `GET /api/sessions/:id/outline`). CogniCode ya puede parsear símbolos con TreeSitter sin necesidad de un LSP externo. Para el outline, TreeSitter es suficiente (no necesita type information).

   ```rust
   /// Get document outline using TreeSitter (no LSP needed).
   /// Returns a hierarchical list of symbols in the file.
   pub async fn document_symbols(&self, file_path: &str) -> WorkspaceResult<Vec<DocumentSymbolDto>> {
       // Uses existing TreeSitter parser — no external LSP required
       self.analysis.get_file_symbols(Path::new(file_path))
   }
   ```

3. **LSP fallback**: CogniCode ya tiene un patrón LSP-first con fallback a TreeSitter. La navigation (go_to_definition, hover, find_references) intenta usar el LSP externo (rust-analyzer) y si no está disponible, usa TreeSitter como fallback. Esto es mejor que `rcode-lsp` que solo funciona si el LSP está corriendo.

**Qué cambia en RCode**:

| Componente `rcode-lsp` | Destino | LOC eliminado |
|------------------------|---------|---------------|
| `LspClient` | ❌ Eliminado — CogniCode gestiona sus propios LSPs | ~400 LOC |
| `LanguageServerRegistry` | ❌ Eliminado — CogniCode tiene `LspManager` | ~350 LOC |
| `LspToolAdapter` | ❌ Eliminado — RCode delega a `CogniCodeSession` | ~300 LOC |
| `Transport` (stdio) | ❌ Eliminado — CogniCode maneja transporte internamente | ~250 LOC |
| Types (LSP types) | ⚠️ Se mantiene un subconjunto mínimo si el server tiene rutas directas | ~489 → ~100 LOC |
| **Total eliminado** | | **~1489 LOC** |

- Las rutas del server (`/outline`) llaman a `cognicode_session.document_symbols()` en vez de `lsp_registry.get_server()`
- El crate `rcode-lsp` puede eliminarse completamente del workspace si las rutas del server migran a CogniCode
- Estimación: `rcode-lsp` pasa de 1789 LOC a ~0 LOC (eliminación completa) o ~100 LOC (types residuales)

**Criterio de aceptación**:
- `cognicode_go_to_definition`, `cognicode_hover`, `cognicode_find_references` funcionan sin `rcode-lsp`
- El outline del frontend web funciona via CogniCode TreeSitter (no requiere rust-analyzer)
- Si rust-analyzer está disponible, las tools de navegación usan LSP (más preciso)
- Si no está disponible, las tools usan TreeSitter (fallback, menos preciso pero funcional)
- `rcode-lsp` se elimina del workspace Cargo.toml

---

### P6.3 `to_xml()` + `build_workspace_context_sync()` — Inyección proactiva completa en CogniCode

**Estado actual**:

| Componente | Dónde | LOC | Qué hace |
|------------|-------|-----|----------|
| `IntelligenceSnapshot::to_xml()` | `rcode-cognicode/snapshot.rs` | ~60 LOC | Genera `<code-intelligence>` XML con stats, hot_paths, diagnostics, languages |
| `IntelligenceSnapshot` struct | `rcode-cognicode/snapshot.rs` | ~240 LOC | Struct completo con SharedSnapshot, population, defaults |
| `build_workspace_context_sync()` | `rcode-agent/executor.rs` | ~40 LOC | Construye `<env>` (cwd, OS, project path) + `<code-intelligence>` y lo prependea al system prompt |
| `with_intelligence_snapshot()` | `rcode-agent/executor.rs` | ~15 LOC | Setter para inyectar el SharedSnapshot en el executor |

**Análisis detallado de `build_workspace_context_sync()`**:

La función tiene dos responsabilidades diferenciadas:

| Parte | Qué genera | De quién es conocimiento | Destino |
|-------|-----------|-------------------------|---------|
| `<env>` block | cwd, OS, project path, git branch | Conocimiento genérico de RCode | **Se queda en RCode** |
| `<code-intelligence>` block | symbol_count, edge_count, hot_paths, diagnostics, languages | Conocimiento 100% de CogniCode | **Se mueve a CogniCode** |
| Inyección en system prompt | Prepend XML al primer mensaje del agente | Lógica del executor | **Se queda en RCode** (pero simplificada) |

Con `get_project_diagnostics()` (P3.1), RCode solo necesita:
```rust
let intelligence_xml = cognicode_session.get_project_diagnostics().await?.to_xml();
```
Ya no necesita `SharedSnapshot`, `IntelligenceSnapshot`, ni la lógica de population manual.

**Plan de absorción en CogniCode**:

1. **Mover `to_xml()` a `ProjectDiagnostics` en CogniCode**:
   ```rust
   // En cognicode-core
   impl ProjectDiagnostics {
       /// Format as XML for injection into LLM system prompt.
       /// The consumer (RCode, MCP server, etc.) just calls this
       /// and prepends it to the agent's system prompt.
       pub fn to_xml(&self) -> String { ... }
   }
   ```

2. **El formato XML evoluciona con CogniCode** — si añadimos coverage metrics, doc comments, etc., el XML se enriquece sin que el consumidor cambie una línea.

3. **En RCode, el executor simplifica a**:
   ```rust
   // Antes (RCode): build_workspace_context_sync maneja env + intelligence
   let intelligence_xml = self.intelligence_snapshot
       .as_ref()
       .map(|snap| snap.read().to_xml())
       .unwrap_or_default();
   let workspace_context = build_workspace_context_sync(ctx, &intelligence_xml);

   // Después (RCode): solo maneja env, intelligence viene de CogniCode directo
   let env_xml = build_env_context(ctx);  // cwd, OS, project path — genérico
   let intelligence_xml = cognicode_session
       .get_project_diagnostics()
       .await
       .map(|d| d.to_xml())
       .unwrap_or_default();
   let workspace_context = format!("{}\n{}", env_xml, intelligence_xml);
   ```

**Qué cambia en RCode**:

| Componente | Destino | LOC eliminado |
|------------|---------|---------------|
| `IntelligenceSnapshot` struct completo | ❌ Eliminado — reemplazado por `ProjectDiagnostics` de CogniCode | ~240 LOC |
| `SharedSnapshot` (Arc<RwLock<>>) | ❌ Eliminado — reemplazado por llamada directa a CogniCode | ~15 LOC |
| `with_intelligence_snapshot()` setter | ❌ Eliminado — no necesita inyectar snapshot si lo obtiene on-demand | ~15 LOC |
| `build_workspace_context_sync()` | ⚠️ Simplificado — solo genera `<env>`, intelligence lo da CogniCode | ~40 → ~20 LOC |
| `populate_hot_paths()` | ❌ Eliminado — `get_hot_paths()` nativo en CogniCode | ~35 LOC |
| `populate_diagnostics()` | ❌ Eliminado — `get_project_diagnostics()` en CogniCode | ~25 LOC |
| `populate_snapshot_stats()` | ❌ Eliminado — `get_graph_stats()` en CogniCode | ~90 LOC |
| **Total eliminado** | | **~440 LOC** |

**Criterio de aceptación**:
- `ProjectDiagnostics::to_xml()` genera el mismo formato que `IntelligenceSnapshot::to_xml()` actual
- El XML se enriquece automáticamente cuando CogniCode añade nuevos campos (coverage, complexity warnings)
- RCode no tiene lógica de formato de inteligencia — solo llama `to_xml()` y lo inyecta
- `build_workspace_context_sync()` solo genera `<env>` genérico, sin lógica CogniCode

---

### P6.4 Snapshot reactivo — Eliminación de polling manual y SharedSnapshot

**Estado actual**:

| Componente | Dónde | LOC | Qué hace |
|------------|-------|-----|----------|
| `CogniCodeService` con background indexing | `rcode-cognicode/service.rs` | ~415 LOC | Spawnea indexing en background, mantiene `SharedSnapshot`, usa `FileWatcher` |
| `SharedSnapshot` (Arc<RwLock<IntelligenceSnapshot>>) | `rcode-cognicode/snapshot.rs` | ~15 LOC | Los consumers leen el snapshot para inyección proactiva |
| `IntelligenceSnapshot` | `rcode-cognicode/snapshot.rs` | ~240 LOC | Struct con stats, hot_paths, diagnostics, languages + `to_xml()` |
| `FileWatcher` | `rcode-cognicode/watcher.rs` | ~167 LOC | Usa notify crate para detectar cambios y disparar re-indexing |
| `CogniCodeSession` wrapper | `rcode-cognicode/session.rs` | ~36 LOC | Wrapper sync sobre `WorkspaceSession` async |
| `count_languages_recursive()` | `rcode-cognicode/service.rs` | ~25 LOC | Walk recursivo del filesystem para detectar lenguajes |
| `extension_to_language()` | `rcode-cognicode/service.rs` | ~15 LOC | Mapeo de extensión → lenguaje |
| `populate_hot_paths()` | `rcode-cognicode/service.rs` | ~35 LOC | 30 llamadas a get_call_hierarchy para computar fan-in |
| `populate_diagnostics()` | `rcode-cognicode/service.rs` | ~25 LOC | check_architecture + complexity scan |
| `populate_snapshot_stats()` | `rcode-cognicode/service.rs` | ~90 LOC | Orquesta todo lo anterior |

**Análisis del problema**:

RCode mantiene un snapshot **duplicado** y **separado** del estado de CogniCode. El flujo actual es:

```
FileWatcher detecta cambio
    → CogniCodeService::do_index() (re-parsea todo con build_lightweight_index)
        → populate_snapshot_stats() (walk filesystem para languages)
        → populate_hot_paths() (30 llamadas get_call_hierarchy)
        → populate_diagnostics() (check_architecture + complexity)
        → Escribe en SharedSnapshot (Arc<RwLock<>>)
            → Executor lee SharedSnapshot en cada turno
                → to_xml() → inyecta en system prompt
```

Problemas:
1. **Doble indexación**: `build_lightweight_index` siempre re-construye el grafo (P1.1)
2. **Filesystem walk duplicado**: `count_languages_recursive()` recorre el FS cuando CogniCode ya parseó todos los archivos
3. **Hot paths ineficiente**: 30 llamadas individuales cuando CogniCode tiene `reverse_edges` (P0.3)
4. **Snapshot stale**: Si el indexing falla, el snapshot queda desactualizado sin notificación
5. **Polling-based**: El executor lee el snapshot en cada turno — no hay notificación de cambio

**Plan de absorción en CogniCode**:

1. **CogniCode tiene su propio FileWatcher interno** (P3.2 subscribe_to_changes):
   ```rust
   impl WorkspaceSession {
       /// Subscribe to graph change events.
       /// The consumer doesn't need its own file watcher.
       pub fn subscribe(&self) -> broadcast::Receiver<GraphChangeEvent> { ... }
   }
   ```

2. **RCode elimina su FileWatcher y SharedSnapshot**:
   ```rust
   // Antes (RCode): FileWatcher + SharedSnapshot + CogniCodeService
   let service = CogniCodeService::spawn(project_path).await?;
   let snapshot = service.shared_snapshot();
   // ... el executor lee snapshot.read().to_xml() en cada turno
   // ... populate_hot_paths() hace 30 llamadas
   // ... populate_diagnostics() hace 2+ llamadas
   // ... count_languages_recursive() hace filesystem walk

   // Después (RCode): directo a CogniCode session
   let session = CogniCodeSession::new(project_path).await?;
   
   // Para inyección proactiva en el executor:
   let diagnostics = session.get_project_diagnostics().await?;
   let xml = diagnostics.to_xml();  // to_xml() vive en CogniCode (P6.3)

   // Para reactividad (reemplaza FileWatcher + SharedSnapshot):
   let mut changes = session.subscribe().await;
   tokio::spawn(async move {
       while let Ok(event) = changes.recv().await {
           match event {
               GraphChangeEvent::SymbolsAdded { .. } => {
                   // Re-fetch diagnostics (una sola llamada) y actualizar executor
                   let diagnostics = session.get_project_diagnostics().await?;
                   executor.update_intelligence(diagnostics.to_xml());
               }
               _ => {}
           }
       }
   });
   ```

**Qué cambia en RCode**:

| Componente actual | Destino | LOC eliminado |
|-------------------|---------|---------------|
| `CogniCodeService` completo | ⚠️ Simplificado a ~80 LOC — solo spawn + subscribe | ~335 LOC |
| `FileWatcher` (notify crate) | ❌ Eliminado — CogniCode tiene su propio watcher (P3.2) | ~167 LOC |
| `SharedSnapshot` (Arc<RwLock<>>) | ❌ Eliminado — reemplazado por eventos de CogniCode | ~15 LOC |
| `IntelligenceSnapshot` | ❌ Eliminado — reemplazado por `ProjectDiagnostics` (P6.3) | ~240 LOC |
| `count_languages_recursive()` | ❌ Eliminado — `get_graph_stats().language_breakdown` (P0.1) | ~25 LOC |
| `extension_to_language()` | ❌ Eliminado — CogniCode ya detecta lenguajes internamente | ~15 LOC |
| `populate_hot_paths()` | ❌ Eliminado — `get_hot_paths()` nativo (P0.3) | ~35 LOC |
| `populate_diagnostics()` | ❌ Eliminado — `get_project_diagnostics()` (P3.1) | ~25 LOC |
| `populate_snapshot_stats()` | ❌ Eliminado — `get_project_diagnostics()` combina todo (P3.1) | ~90 LOC |
| `CogniCodeSession` wrapper | ⚠️ Se mantiene como adapter mínimo | ~36 LOC |
| **Total eliminado** | | **~947 LOC** |

**Net result en `rcode-cognicode`**:
- service.rs: 415 → ~80 LOC (-81%)
- watcher.rs: 167 → 0 LOC (-100%, eliminado)
- snapshot.rs: 240 → 0 LOC (-100%, eliminado)
- session.rs: 36 → ~36 LOC (se mantiene como adapter)
- tools/: ~1251 → ~1251 LOC (se mantiene — son tool implementations)
- **Total crate**: 2372 → ~1367 LOC (-42%)

**Criterio de aceptación**:
- RCode no tiene FileWatcher — CogniCode notifica cambios
- RCode no mantiene snapshot duplicado — obtiene datos de CogniCode on-demand
- El executor se actualiza reactivamente cuando CogniCode emite eventos

---

### P6.5 Resumen de migración RCode → CogniCode

| Funcionalidad RCode | LOC eliminados en RCode | Absorbida por CogniCode | Impacto |
|---------------------|------------------------|------------------------|---------|
| **rcode-lsp** — Client, Registry, Transport, Adapter | ~1489 LOC | P6.2 CogniCode LSP interno | LSP management centralizado en CogniCode |
| `IntelligenceSnapshot::to_xml()` | ~60 LOC | P6.3 `ProjectDiagnostics::to_xml()` | Formato vive en CogniCode |
| `SharedSnapshot` (Arc<RwLock<>>) | ~15 LOC | P6.4 Snapshot reactivo via eventos | Elimina cache duplicado |
| `IntelligenceSnapshot` struct completo | ~240 LOC | P6.3 `ProjectDiagnostics` de CogniCode | Struct eliminado, reemplazado por DTO |
| `build_workspace_context_sync()` simplificación | ~20 LOC (de 40) | P6.3 Separación env/intelligence | Solo genera `<env>`, intelligence viene de CogniCode |
| `count_languages_recursive` + `extension_to_language` | ~40 LOC | P0.1 `get_graph_stats().language_breakdown` | Elimina filesystem walk duplicado |
| `populate_hot_paths` (30 calls) | ~35 LOC | P0.3 `get_hot_paths()` nativo | 30 llamadas → 1 llamada |
| `populate_diagnostics` | ~25 LOC | P3.1 `get_project_diagnostics()` | 2+ llamadas → 1 llamada |
| `populate_snapshot_stats` completo | ~90 LOC | P3.1 `get_project_diagnostics()` | Toda la lógica de "poblar" desaparece |
| `FileWatcher` (notify) | ~167 LOC | P3.2 `subscribe_to_changes()` | Elimina watcher externo |
| `CogniCodeService` completo | ~335 LOC (de 415) | Simplificado a ~80 LOC | Solo spawn + subscribe |
| **Total eliminado en RCode** | **~2516 LOC** | | |

**Net result por crate**:

| Crate RCode | LOC antes | LOC después | Reducción |
|-------------|----------|-------------|-----------|
| `rcode-lsp` | 1789 | ~0 (eliminado) o ~100 (types residuales) | -94% a -100% |
| `rcode-cognicode` total | 2372 | ~1367 | -42% |
| `rcode-agent` (executor) | ~40 (workspace_context) | ~20 | -50% |
| **Total RCode** | ~4201 | ~1487 | **-65%** |

La lógica que queda en `rcode-cognicode` es pura adaptación (tool trait implementations + wiring mínimo + session adapter).

---

## Relación con el Plan Original

Las Fases 0-4 del `IMPROVEMENT-PLAN.md` original están sustancialmente completas:

| Fase original | Estado | Notas |
|---------------|--------|-------|
| Fase 0: Estabilización | ✅ Completada | Tests arreglados, traits unificados, warnings limpios |
| Fase 1: Walking Skeleton | ✅ Completada | get_file_symbols funciona end-to-end, MCP stdin/stdout operativo |
| Fase 2: Grafo Real | ✅ Completada | CallGraph con HashMaps, BFS/DFS, impact analysis, cycle detection |
| Fase 3: Refactorización | ✅ Parcialmente | rename funciona, extract/move/inline existen pero necesitan VFS validation |
| Fase 4: Madurez | ✅ Parcialmente | tree-sitter actualizado, find_usages y complexity implementados |
| Fase 5: Diferenciación | 🔄 En progreso | Incremental (P1.2), LSP Proxy existe parcialmente |

Este documento (v2) es la continuación natural — mejora lo que ya funciona y añade la siguiente capa de evolución.
