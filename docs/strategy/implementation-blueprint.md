# CogniCode — Blueprint Final: Arnés de Inteligencia de Código

> Versión 2.0 — Consolidado (Junio 2026)

---

## 0. Decisión Arquitectónica: Estrategia Híbrida (NO solo Visitor)

**No migramos todo a visitor de SonarQube. Usamos 4 estrategias según el tipo de regla.**

```
Reglas CogniCode:
├── 3%  SubscriptionEngine (AST queries)    → SQL injection, unsafe, nesting
├── 12% LineScan + comment-skip             → TODOs, IPs, URLs, credenciales
├── 6%  Metrics pre-calculadas              → Complejidad, líneas, params
├── 80% Style/Lint sin cambios               → Tabs, espacios, longitud
└── <1% SemanticQuery (NUEVO)               → Intent-drift (S7000-S7003)
```

**Fundamento**: El 80% de las reglas (style) no se benefician de visitor. Solo el 3% (estructurales) sí. SonarQube mismo usa esta estrategia híbrida.

---

## 1. Pilares Tecnológicos

| Pilar | Implementación | Persistencia |
|-------|---------------|-------------|
| **Sintaxis (Tree-sitter)** | `cognicode-core/infrastructure/parser/` — parseo incremental multi-lenguaje | — |
| **Semántica (BM25)** | `cognicode-core/infrastructure/semantic/` → **migrar a SQLite FTS5** | `cognicode.db` (tabla FTS5 `symbol_index`) |
| **Orquestación (Rust)** | `cognicode-core/infrastructure/avc/` — contratos verificables | `cognicode.db` (tabla `avc_contracts`) |

### SQLite FTS5 > RocksDB

**Decisión**: Todo en `cognicode.db`. No añadir RocksDB. SQLite FTS5 tiene `bm25()` nativo, es suficiente para monorepos de millones de líneas, y mantiene un solo archivo.

---

## 2. Separación de Responsabilidades

### cognicode-mcp → Interface con el Agente

```
MCP Tools (15 → 17):
├── [EXISTENTES] smart_overview, ranked_symbols, suggest_onboarding_plan
├── [EXISTENTES] auto_diagnose, suggest_refactor_plan, nl_to_symbol
├── [EXISTENTES] ask_about_code, find_pattern_by_intent, compare_call_graphs
├── [EXISTENTES] detect_api_breaks, generate_system_prompt_context
├── [EXISTENTES] detect_god_functions, detect_long_parameter_lists
├── [EXISTENTES] evaluate_refactor_quality
├── [NUEVO F1] generate_contract → AVC desde código existente
├── [NUEVO F1] validate_contract → Validar código agente vs AVC
├── [NUEVO F2] detect_drift → S7000 via MCP
└── [NUEVO F3] reparse_on_edit → Parseo incremental via MCP
```

### cognicode-quality → Motor de Análisis

```
854 reglas → 858 reglas:
├── [EXISTENTES 15] Seguridad → 10 pendientes de comment-skip
├── [NUEVAS 4] S7000-S7003 → Intent-Drift + AVC Compliance
├── [MIGRACIÓN] symbols → SQLite FTS5 (persistente, no DashMap)
└── [INFRA] SubscriptionEngine para reglas estructurales
```

### cognicode-dashboard → Visualización

```
13 endpoints → 16 endpoints:
├── [EXISTENTES] /api/analysis, /api/issues, /api/metrics...
├── [NUEVO F2] /api/drift → Eventos de drift en el tiempo
├── [NUEVO F2] /api/contracts → Estado de AVC contracts
└── [NUEVO F3] /api/agent-stats → Estadísticas de uso por agentes
```

---

## 3. SQLite Schema — Estabilizado

### Tablas existentes (sin cambios)

| Tabla | Datos |
|-------|-------|
| `analysis_runs` | Historial de análisis |
| `issues` | Issues con tracking (open/fixed) |
| `baselines` | Líneas base |
| `file_states` | Hashes BLAKE3 |
| `call_graphs` | Grafos serializados |
| `call_edges` | Edges |
| `file_imports` | Dependencias |

### Tablas modificadas

| Tabla | Cambio |
|-------|--------|
| `symbols` | **Migrar a FTS5** → `symbol_index` (virtual table con BM25 nativo) |
| `analysis_runs` | Añadir columna `drift_score` (REAL, default 0) |

### Tablas nuevas

```sql
-- BM25 Index persistente (reemplaza DashMap en memoria)
CREATE VIRTUAL TABLE IF NOT EXISTS symbol_index USING fts5(
    symbol_name, symbol_kind, file_path, docstring, body_tokens,
    tokenize='porter unicode61'
);

-- Contratos AVC generados
CREATE TABLE IF NOT EXISTS avc_contracts (
    id TEXT PRIMARY KEY,
    source_file TEXT NOT NULL,
    function_name TEXT NOT NULL,
    contract_json TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    compliance_score REAL DEFAULT 1.0
);

-- Eventos de deriva semántica
CREATE TABLE IF NOT EXISTS drift_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    file_path TEXT NOT NULL,
    function_name TEXT NOT NULL,
    drift_score REAL NOT NULL,
    intent TEXT,
    severity TEXT DEFAULT 'warning'
);

-- Tracking de interacciones de agentes
CREATE TABLE IF NOT EXISTS agent_interactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    tool_used TEXT NOT NULL,
    contract_id TEXT,
    result TEXT,
    duration_ms INTEGER
);
```

---

## 4. Plan de Migración de Reglas (por estrategia)

### Bloque 1: Security LineScan (15 reglas) — comment-skip + word boundaries

| Regla | Estrategia | Estado |
|-------|-----------|--------|
| S2068, S4792, S5332, S1313, S1134 | LineScan + comment-skip | ✅ DONE |
| S2076, S2077, S2091, S2631, S3649 | LineScan + comment-skip | 🔲 TODO (2.5h) |
| S4423, S4426, S4507, S4834, S5042 | LineScan + comment-skip | 🔲 TODO (2.5h) |

### Bloque 2: Nuevas SemanticQuery (4 reglas)

| Regla | Descripción | Estrategia |
|-------|-------------|-----------|
| **S7000** | Intent-Drift: BM25(docstring) vs BM25(body) < threshold | SemanticQuery |
| **S7001** | AVC Contract Violation | AvcValidator |
| **S7002** | Obsolete Pattern (no usado en 30d) | SemanticQuery + Git |
| **S7003** | Forbidden Domain Term | SemanticQuery |

### Bloque 3: Estructurales SubscriptionEngine (20 reglas)

| Regla | Estrategia actual | Migrar a |
|-------|-----------------|----------|
| S5122 | ✅ Ya usa AST | — |
| S134, S138, S107 | ✅ Ya usa métricas | — |
| S3776 | ✅ Ya usa métricas | — |
| S1854, S1481, S3776... | LineScan → migrar | SubscriptionEngine |

### Bloque 4: Legacy LineScan (~85 reglas) — helper unificado

```rust
// Template batch:
for (idx, line) in ctx.non_comment_lines() {
    if re.is_match(line) { ... }
}
```

### Bloque 5: Style/Lint (~730 reglas) — sin cambios

---

## 5. Roadmap por Fases

### Fase 1 — Fundación (Semanas 1-2) 🔴

```
Estado: IN PROGRESS
├── ✅ SubscriptionEngine
├── ✅ AVC Contracts (generator + validator)
├── ✅ Comment-skip en 5 reglas (S2068, S4792, S5332, S1313, S1134)
├── 🔲 Comment-skip en 10 reglas restantes de seguridad
├── 🔲 Exponer generate_contract + validate_contract via MCP
├── 🔲 SQLite: crear tablas avc_contracts + drift_events
└── 🔲 SQLite: crear FTS5 symbol_index (migrar desde DashMap)

Métricas F1: 15/15 security con comment-skip | BM25 persistente | AVC via MCP
```

### Fase 2 — Intent-Drift (Semanas 3-4) 🟣

```
├── 🔲 S7000: Intent-Drift Rule (SemanticQuery)
├── 🔲 S7001: AVC Compliance Rule
├── 🔲 MCP tool: detect_drift
├── 🔲 Dashboard: /api/drift endpoint + vista
├── 🔲 SQLite: drift_events poblándose automáticamente
└── 🔲 FP tests para S7000-S7001

Métricas F2: 4 nuevas reglas | drift detection funcionando | dashboard drift view
```

### Fase 3 — Proactivo (Semanas 5-6) 🟡

```
├── 🔲 Zero-Query RAG: suggest_context via MCP
├── 🔲 Incremental parsing via MCP: reparse_on_edit
├── 🔲 S7002 + S7003 rules
├── 🔲 SQLite: agent_interactions tracking
├── 🔲 Dashboard: /api/contracts + /api/agent-stats
└── 🔲 style_patterns table (Git recency)

Métricas F3: MCP con parseo incremental | agent analytics | estilo temporal
```

### Fase 4 — Evolutivo (Semanas 7-8) 🟢

```
├── 🔲 Git recency weights en BM25 (temporal indexing)
├── 🔲 Verifiable RAG: rustc sandbox para contratos de compilación
├── 🔲 Multi-language drift detection (Python, JS, Go)
├── 🔲 Dashboard: analytics dashboard con gráficos de tendencia
└── 🔲 Optimización: índices para monorepos >1M archivos

Métricas F4: RAG verificable | multi-lenguaje | optimizado para escala
```

---

## 6. Métricas de Éxito

| Métrica | Ahora | F1 | F2 | F4 |
|---------|-------|----|----|-----|
| Reglas con comment-skip | 5/854 | 15/15 security | 15/15 | 100/854 |
| Reglas con FP tests | 12/854 | 30/854 | 50/854 | 150/854 |
| BM25 persistente | ❌ DashMap | ✅ FTS5 | ✅ | ✅ |
| AVC via MCP | ❌ | ✅ 2 tools | ✅ | ✅ |
| Drift detection | ❌ | ❌ | ✅ S7000 | ✅ multi-lang |
| Verifiable RAG | ❌ | ❌ | ❌ | ✅ |
| MCP tools | 13 | 15 | 17 | 19 |
| Dashboard endpoints | 13 | 13 | 15 | 16 |
| SQLite tables | 9 | 12 | 14 | 15 |
