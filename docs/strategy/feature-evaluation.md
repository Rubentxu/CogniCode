# Evaluación: SQLite FTS5 vs RocksDB para BM25 + Mapa de Features Existentes

## 1. BM25: Estado Actual

**El índice BM25 actual es VOLÁTIL.** Se almacena en memoria (`DashMap`) y se pierde al reiniciar el servidor.

```rust
// cognicode-core/src/infrastructure/semantic/semantic_search.rs
pub struct SearchIndex {
    symbols_by_file: DashMap<String, Vec<IndexedSymbol>>,  // ← memoria
    all_symbols: DashMap<String, IndexedSymbol>,            // ← memoria
}
```

Cada vez que el MCP server arranca, hay que re-indexar todos los símbolos. No hay persistencia.

## 2. Recomendación: SQLite FTS5 (NO RocksDB)

### Por qué SQLite FTS5 > RocksDB para este caso

| Criterio | SQLite FTS5 | RocksDB |
|----------|-------------|---------|
| **BBDD existente** | ✅ Ya tenemos `cognicode.db` | ❌ Nuevo archivo `.cognicode/rocksdb/` |
| **Ranking BM25** | ✅ `bm25()` nativo en FTS5 | ❌ Hay que implementarlo |
| **Consultas SQL + texto** | ✅ `SELECT * FROM fts WHERE match('auth') AND kind='function'` | ❌ Solo key-value |
| **Transacciones** | ✅ ACID con el resto de tablas | ❌ Separado |
| **Despliegue** | ✅ Un solo archivo | ❌ Dos archivos |
| **Tamaño de índice** | ✅ Hasta ~10M docs (suficiente para monorepo) | ✅ Ilimitado |
| **Incremental** | ✅ `INSERT` / `DELETE` en FTS5 | ✅ Put/Delete |
| **Recencia de Git** | ✅ Columna `last_modified` en la tabla | ❌ Manual |

**Conclusión: SQLite FTS5 es la opción correcta.** Usar RocksDB sería duplicar infraestructura sin beneficio real.

### Implementación propuesta

```sql
-- Nueva tabla FTS5 en cognicode.db (idempotente)
CREATE VIRTUAL TABLE IF NOT EXISTS symbol_index USING fts5(
    symbol_name,
    symbol_kind,
    file_path,
    docstring,
    body_tokens,
    tokenize='porter unicode61'
);

-- Consulta con ranking BM25 nativo:
SELECT *, bm25(symbol_index, 1.0, 0.75) AS score
FROM symbol_index
WHERE symbol_index MATCH 'auth AND kind:function'
ORDER BY score
LIMIT 10;
```

### Migración desde DashMap a SQLite FTS5

```rust
// Antes (volátil):
let results = search_index.search(&query);  // DashMap en memoria

// Después (persistente):
let results = db.search_fts5("auth", Some("function"), 10);  // SQLite FTS5
```

---

## 3. Features Existentes — Mapa Completo

### cognicode-mcp (13 herramientas YA FUNCIONANDO)

| Tool | Archivo | Qué hace | Afectado por AVC |
|------|---------|----------|-----------------|
| `smart_overview` | aix_handlers.rs:15 | Vista general del proyecto | 🟡 Mejora: incluir AVC contracts |
| `ranked_symbols` | aix_handlers.rs:86 | Símbolos rankeados por relevancia | 🟡 Mejora: usar FTS5 en vez de DashMap |
| `suggest_onboarding_plan` | aix_handlers.rs:167 | Plan de onboarding | 🟢 Sin cambios |
| `auto_diagnose` | aix_handlers.rs:197 | Diagnóstico automático | 🟡 Mejora: añadir drift detection |
| `suggest_refactor_plan` | aix_handlers.rs:325 | Plan de refactorización | 🟡 Mejora: usar AVC para validar |
| `nl_to_symbol` | aix_handlers.rs:411 | Búsqueda NL → símbolo | 🟡 Mejora: FTS5 con BM25 nativo |
| `ask_about_code` | aix_handlers.rs:486 | Preguntas sobre código | 🟢 Sin cambios |
| `find_pattern_by_intent` | aix_handlers.rs:543 | Patrones por intención | 🔴 NUEVA: integración con S7000 |
| `compare_call_graphs` | aix_handlers.rs:603 | Comparar grafos | 🟢 Sin cambios |
| `detect_api_breaks` | aix_handlers.rs:685 | Detectar roturas de API | 🟡 Mejora: usar AVC contracts |
| `generate_system_prompt_context` | aix_handlers.rs:748 | Contexto para system prompt | 🔴 NUEVA: Zero-Query RAG |
| `detect_god_functions` | aix_handlers.rs:827 | Detectar god functions | 🟢 Sin cambios |
| `detect_long_parameter_lists` | aix_handlers.rs:889 | Listas de parámetros largas | 🟢 Sin cambios |
| `evaluate_refactor_quality` | aix_handlers.rs:933 | Evaluar calidad de refactor | 🔴 NUEVA: validar contra AVC |

### Dashboard (13 endpoints YA FUNCIONANDO)

| Endpoint | Qué hace | Afectado |
|----------|----------|----------|
| `/api/analysis` | Análisis completo | 🟡 Añadir AVC contract generation |
| `/api/issues` | Issues con filtros | 🟡 Añadir S7000-S7003 issues |
| `/api/metrics` | Métricas | 🟡 Añadir drift score trend |
| `/api/quality-gate` | Quality gate | 🟢 Sin cambios |
| `/api/ratings` | Ratings A-E | 🟢 Sin cambios |
| `/api/validate-path` | Validar ruta | 🟢 Sin cambios |
| `/api/fs/ls` | File browser | 🟢 Sin cambios |
| `/api/projects` | Lista proyectos | 🟡 Añadir drift/AVC status |
| `/api/projects/register` | Registrar proyecto | 🟢 Sin cambios |
| `/api/projects/:id/history` | Historial | 🟡 Añadir drift_events |

### cognicode-db (9 tablas YA EXISTENTES)

| Tabla | Datos | Afectado |
|-------|-------|----------|
| `analysis_runs` | Historial de análisis | 🟡 Añadir drift_score |
| `issues` | Issues con tracking | 🟡 Añadir S7000-S7003 |
| `baselines` | Líneas base | 🟢 Sin cambios |
| `file_states` | Hashes BLAKE3 | 🟡 Usar para incremental BM25 |
| `call_graphs` | Grafos serializados | 🟢 Sin cambios |
| `symbols` | Símbolos indexados | 🔴 Migrar a FTS5 |
| `call_edges` | Edges del grafo | 🟢 Sin cambios |
| `file_imports` | Dependencias | 🟢 Sin cambios |

### cognicode-axiom (854 reglas)

| Tipo | Count | Afectado |
|------|-------|----------|
| Seguridad (comment-skip) | 5/15 done | 🔴 10 pendientes |
| Intent-Drift (NUEVAS) | 0/4 | 🔴 S7000-S7003 nuevas |
| SubscriptionEngine | 1 regla | 🟡 Migrar 10 más |
| Line-scan legacy | ~85 | 🟠 Helper unificado |
| Style/Lint | ~734 | 🟢 Sin cambios |

---

## 4. Resumen: Qué mejora y qué NO cambia

### ✅ MEJORA — con la Fase 1

| Feature | Antes | Después |
|---------|-------|---------|
| BM25 index | Volátil (DashMap, se pierde) | Persistente (SQLite FTS5) |
| AVC contracts | No existe | `generate_contract` + `validate_contract` via MCP |
| 5 reglas security | FP en comentarios | 0 FP en comentarios |
| BBDD | 1 archivo SQLite | 1 archivo SQLite (sin añadir RocksDB) |
| MCP tools | 13 tools | 15 tools (+generate_contract, +validate_contract) |

### 🟡 MEJORA FUTURA — con Fases 2-4

| Feature | Cómo mejora |
|---------|------------|
| S7000 Intent-Drift | Nueva regla detectando mentiras en el código |
| Zero-Query RAG | Contexto proactivo sin que el agente pregunte |
| Temporal Indexing | BM25 pondera por recencia de Git |
| Dashboard drift view | Visualización de drift events en el tiempo |

### 🟢 SIN CAMBIOS

| Feature | Por qué |
|---------|---------|
| 734 reglas Style/Lint | Por diseño escanean todo (tabs, longitudes) |
| Quality Gate | Ya funciona, no requiere cambios |
| File browser (`/api/fs/ls`) | Ya funciona |
| call_graphs, call_edges | Ya usan SQLite, sin cambios |
