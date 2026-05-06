# CogniCode — Blueprint de Implementación: Arnés de Inteligencia de Código

> Versión 1.0 — Basado en el análisis de arquitectura actual (Mayo 2026)

---

## 0. Estado Actual — Lo que YA tenemos

| Componente | Ubicación | Estado | Qué hace |
|-----------|----------|--------|----------|
| **AVC Contracts** | `cognicode-core/src/infrastructure/avc/` | ✅ v1.0 | Contratos de 3 capas (Syntax+Semantic+Safety) |
| **SubscriptionEngine** | `cognicode-axiom/src/rules/subscription_engine.rs` | ✅ v1.0 | Motor determinista SonarQube-style |
| **Semantic Search (BM25)** | `cognicode-core/src/infrastructure/semantic/` | ✅ Funcional | Indexación BM25 + filtrado por SymbolKind |
| **Tree-sitter Parser** | `cognicode-core/src/infrastructure/parser/` | ✅ Funcional | Parseo multi-lenguaje (Rust/Python/JS/Java/Go) |
| **Incremental Parsing** | `tree_sitter_parser.rs:601` | ⚠️ API existe | No expuesto vía MCP |
| **MCP Server** | `cognicode-mcp/` + `interface/mcp/` | ✅ Funcional | 13 herramientas AIX |
| **Quality Analysis** | `cognicode-quality/src/handler.rs` | ✅ Funcional | 854 reglas, SQLite persistence |
| **Dashboard** | `cognicode-dashboard/` | ✅ Funcional | Multi-proyecto, 61 tests e2e |
| **SQLite Persistence** | `cognicode-db/` | ✅ Funcional | analysis_runs, issues, baselines, file_states |
| **Sandbox Core** | `cognicode-core/src/sandbox_core/` | ⚠️ Parcial | Ground truth + scoring (no Rust compiler) |

---

## 1. Arquitectura Objetivo: El "Agent-First" Runtime

```
┌──────────────────────────────────────────────────────────────────┐
│                    AI Agent (Claude, Cursor, Copilot)             │
│                                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │ Generate │  │  Edit    │  │  Query   │  │  Refactor│        │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘        │
│       │             │             │             │                │
│       ▼             ▼             ▼             ▼                │
│  ┌──────────────────────────────────────────────────────┐       │
│  │              MCP Protocol (Agent Interface)           │       │
│  │  generate_contract | validate_contract                │       │
│  │  detect_drift     | suggest_context                   │       │
│  │  smart_overview   | ranked_symbols                    │       │
│  └───────────────────────┬──────────────────────────────┘       │
│                          │                                       │
└──────────────────────────┼───────────────────────────────────────┘
                           │
┌──────────────────────────┼───────────────────────────────────────┐
│                CogniCode Engine (Rust)                            │
│                          │                                        │
│  ┌───────────────────────┴───────────────────────────────────┐  │
│  │                 AVC (Agent-Verifiable Context)             │  │
│  │  Layer 1: Syntax (Tree-sitter)                            │  │
│  │  Layer 2: Semantic (BM25)                                 │  │
│  │  Layer 3: Safety (Rust types + invariants)                │  │
│  └───────────────────────┬───────────────────────────────────┘  │
│                          │                                        │
│  ┌───────────┐  ┌────────┴───────┐  ┌──────────────────┐       │
│  │ Tree-sitter│  │  BM25 Index    │  │  Rules Engine     │       │
│  │ (Increm.) │  │  (RocksDB)     │  │  (854 rules)      │       │
│  └───────────┘  └────────────────┘  └────────┬─────────┘       │
│                                              │                   │
│                          ┌───────────────────┴──────┐           │
│                          │  SQLite Persistence       │           │
│                          │  .cognicode/cognicode.db  │           │
│                          │  + analysis_runs          │           │
│                          │  + issues (with status)   │           │
│                          │  + baselines              │           │
│                          │  + avc_contracts (NEW)    │           │
│                          └──────────────────────────┘           │
└──────────────────────────────────────────────────────────────────┘
```

---

## 2. Separación: cognicode-mcp vs cognicode-quality

### cognicode-mcp (Agent Interface)
**Rol**: Servidor MCP de alta velocidad. Interface directa con el agente.

```
Responsabilidades:
├── AVC: generate_contract, validate_contract
├── Intent-Drift: detect_drift, suggest_alignment
├── Zero-Query RAG: suggest_context, push_best_practices
├── Smart Overview: smart_overview, ranked_symbols
├── NL to Symbol: nl_to_symbol, auto_diagnose
├── Incremental Parse: reparse_on_edit (NEW — exponer parse_incremental)
└── Temporal Index: get_recent_patterns, get_team_style (NEW)
```

### cognicode-quality (Code Analysis Engine)
**Rol**: Análisis estático + dinámico. Motor de reglas.

```
Responsabilidades:
├── Rule Engine: 854 rules → migrar a SubscriptionEngine
├── Intent-Drift Rules: S7000 (NEW — detecta drift semántico)
├── AVC Compliance Rules: S7001 (NEW — valida contratos AVC)
├── Temporal Rules: S7002 (NEW — patrones obsoletos vs recientes)
├── SQLite Persistence: guardar resultados + tendencias
└── Dashboard Data: proveer datos para visualización
```

---

## 3. Plan de Migración de Reglas por Bloques

### Bloque 1: Seguridad (15 reglas) — PRIORIDAD MÁXIMA
**Objetivo**: Cero falsos positivos en reglas de seguridad.

| Regla | Acción | Estado |
|-------|--------|--------|
| S2068 | ✅ Comment-skip + FP tests | DONE |
| S4792 | ✅ Word boundaries + FP tests | DONE |
| S5332 | ✅ Comment-skip | DONE |
| S1313 | ✅ Comment-skip | DONE |
| S1134 | ✅ Comment-skip | DONE |
| S2076 | 🔲 Comment-skip + FP tests | TODO |
| S2077 | 🔲 Migrar a SubscriptionEngine | TODO |
| S2091 | 🔲 Comment-skip + FP tests | TODO |
| S2631 | 🔲 Comment-skip + FP tests | TODO |
| S3649 | 🔲 Migrar a SubscriptionEngine | TODO |
| S4423 | 🔲 Comment-skip + FP tests | TODO |
| S4426 | 🔲 Comment-skip + FP tests | TODO |
| S4507 | 🔲 Comment-skip + FP tests | TODO |
| S4834 | 🔲 Comment-skip + FP tests | TODO |
| S5042 | 🔲 Comment-skip + FP tests | TODO |

### Bloque 2: Intent-Drift (NUEVAS reglas)
**Objetivo**: Detectar mentiras en el código.

| Regla | Descripción | Motor |
|-------|-------------|-------|
| **S7000** | Semantic Intent-Drift: BM25(docstring) vs BM25(body) < threshold | SubscriptionEngine |
| **S7001** | AVC Contract Violation: código no cumple contrato | AvcValidator |
| **S7002** | Obsolete Pattern: patrón no usado en últimos 30 días (Git history) | BM25 + Git |
| **S7003** | Forbidden Domain Term: término prohibido en contexto | BM25 |

### Bloque 3: Estructurales (20 reglas existentes con Tree-sitter)
**Objetivo**: Añadir tests FP a las que ya usan AST.

| Regla | Acción |
|-------|--------|
| S134 | 🔲 Añadir FP tests |
| S138 | 🔲 Añadir FP tests |
| S107 | 🔲 Añadir FP tests |
| S5122 | 🔲 Añadir FP tests |
| S3776 | 🔲 Añadir FP tests |
| ... 15 más | 🔲 Añadir FP tests |

### Bloque 4: Legacy Line-Scan (~85 reglas)
**Objetivo**: Añadir comment-skip usando el helper unificado.

```rust
// Template para migración batch:
for (idx, line) in ctx.non_comment_lines() {  // ← Reemplaza ctx.source.lines()
    if re.is_match(line) { ... }
}
```

### Bloque 5: Style/Lint (~734 reglas)
**Sin cambios** — por diseño escanean todo (tabs, line length, etc.)

---

## 4. Estabilización de Persistencia SQLite

### Schema actual
```
analysis_runs    ← (timestamp, total_issues, debt_minutes, rating, blockers, criticals, files_changed, new_issues, fixed_issues)
issues           ← (run_id, rule_id, severity, category, file_path, line, message, status, first_seen_run, fixed_in_run)
baselines        ← (timestamp, total_issues, debt_minutes, rating, blockers, criticals)
file_states      ← (path, hash, issues_count, last_analyzed)
call_graphs      ← (id, data blob)
symbols          ← (id, file_path, name, kind, line, column, complexity)
call_edges       ← (caller_id, callee_id, dependency_type)
file_imports     ← (source_file, imported_file)
```

### Nuevas tablas necesarias

```sql
-- AVC Contracts generated by the engine
CREATE TABLE IF NOT EXISTS avc_contracts (
    id TEXT PRIMARY KEY,              -- contract_id
    source_file TEXT NOT NULL,
    function_name TEXT NOT NULL,
    contract_json TEXT NOT NULL,      -- serialized AvcContract
    generated_at TEXT NOT NULL,
    last_validated_at TEXT,
    compliance_score REAL DEFAULT 1.0
);

-- Intent-Drift detections over time
CREATE TABLE IF NOT EXISTS drift_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    file_path TEXT NOT NULL,
    function_name TEXT NOT NULL,
    drift_score REAL NOT NULL,        -- BM25 similarity score
    intent TEXT,
    actual_tokens TEXT,               -- JSON array of detected tokens
    severity TEXT DEFAULT 'warning'
);

-- Agent interaction tracking
CREATE TABLE IF NOT EXISTS agent_interactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    agent_type TEXT,                   -- "claude", "cursor", "copilot"
    tool_used TEXT,                    -- "generate_contract", "validate_contract"
    contract_id TEXT,
    result TEXT,                       -- "pass", "fail", "drift_detected"
    duration_ms INTEGER
);

-- Temporal style index (Git-aware)
CREATE TABLE IF NOT EXISTS style_patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_type TEXT NOT NULL,        -- "function_signature", "error_handling", "import_style"
    pattern_hash TEXT NOT NULL,
    file_path TEXT NOT NULL,
    first_seen TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    usage_count INTEGER DEFAULT 1,
    is_active BOOLEAN DEFAULT 1       -- Still used in recent commits?
);
```

### Migración SQL

```sql
-- Run once to add new tables (idempotent via IF NOT EXISTS)
-- Add to cognicode-db/src/schema.rs
```

---

## 5. Roadmap de Implementación

### Fase 1: Fundación (Semanas 1-2)
```
✅ AVC Contracts (contract, generator, validator)
✅ SubscriptionEngine
✅ Comment-skip en 5 reglas de seguridad
🔲 Exponer AVC via MCP (generate_contract, validate_contract)
🔲 Nuevas tablas SQLite (avc_contracts, drift_events)
🔲 Migrar S2076, S2077, S2091, S2631, S3649
```

### Fase 2: Intent-Drift (Semanas 3-4)
```
🔲 S7000 (Intent-Drift Rule)
🔲 S7001 (AVC Compliance Rule)
🔲 MCP tool: detect_drift
🔲 Dashboard: drift visualization
🔲 Integrar BM25 con Git history (temporal weighting)
```

### Fase 3: Proactivo (Semanas 5-6)
```
🔲 Zero-Query RAG: suggest_context tool
🔲 MCP tool: push_best_practices
🔲 Incremental parsing via MCP (reparse_on_edit)
🔲 RocksDB indexes para monorepos
🔲 Dashboard: agent interaction analytics
```

### Fase 4: Evolutivo (Semanas 7-8)
```
🔲 Temporal Indexing (Git recency weights)
🔲 Style alignment detection
🔲 Verifiable RAG (rustc sandbox integration)
🔲 Multi-language drift detection
```

---

## 6. Métricas de Éxito

| Métrica | Actual | Objetivo Fase 1 | Objetivo Fase 2 |
|---------|--------|-----------------|-----------------|
| Reglas con FP tests | 5/854 (0.6%) | 30/854 (3.5%) | 100/854 (12%) |
| Falsos positivos en seguridad | ~10% estimado | <1% | 0% |
| Reglas con SubscriptionEngine | 1 | 10 | 25 |
| AVC contracts generados | 0 | >0 (via MCP) | >100 (batch) |
| Drift detections | 0 | >0 (manual) | Automático |
| Dashboard coverage | 6 páginas | + drift view | + agent analytics |
