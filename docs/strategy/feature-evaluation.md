# CogniCode — Feature Map: Lo que YA existe vs Lo Nuevo

> Referencia para entender el impacto de cada cambio

---

## Mapa de Features Existentes

### cognicode-mcp: 13 tools ✅

| # | Tool | Archivo | Qué hace |
|---|------|---------|----------|
| 1 | `smart_overview` | aix_handlers.rs:15 | Vista general del proyecto |
| 2 | `ranked_symbols` | aix_handlers.rs:86 | Símbolos rankeados (usa BM25 DashMap) |
| 3 | `suggest_onboarding_plan` | aix_handlers.rs:167 | Plan de onboarding |
| 4 | `auto_diagnose` | aix_handlers.rs:197 | Diagnóstico automático |
| 5 | `suggest_refactor_plan` | aix_handlers.rs:325 | Plan de refactorización |
| 6 | `nl_to_symbol` | aix_handlers.rs:411 | NL → símbolo (usa BM25 DashMap) |
| 7 | `ask_about_code` | aix_handlers.rs:486 | Preguntas sobre código |
| 8 | `find_pattern_by_intent` | aix_handlers.rs:543 | Patrones por intención |
| 9 | `compare_call_graphs` | aix_handlers.rs:603 | Comparar grafos |
| 10 | `detect_api_breaks` | aix_handlers.rs:685 | Detectar roturas de API |
| 11 | `generate_system_prompt_context` | aix_handlers.rs:748 | Contexto para system prompt |
| 12 | `detect_god_functions` | aix_handlers.rs:827 | Detectar god functions |
| 13 | `detect_long_parameter_lists` | aix_handlers.rs:889 | Listas de parámetros largas |
| 14 | `evaluate_refactor_quality` | aix_handlers.rs:933 | Evaluar calidad de refactor |

### Dashboard: 13 endpoints ✅

| # | Endpoint | Método | Qué hace |
|---|----------|--------|----------|
| 1 | `/health` | GET | Health check |
| 2 | `/api/analysis` | POST | Análisis completo |
| 3 | `/api/issues` | POST | Issues con filtros + paginación |
| 4 | `/api/metrics` | POST | Métricas |
| 5 | `/api/quality-gate` | POST | Quality gate |
| 6 | `/api/ratings` | POST | Ratings A-E |
| 7 | `/api/validate-path` | POST | Validar ruta |
| 8 | `/api/fs/ls` | POST | File browser |
| 9 | `/api/projects` | GET | Lista proyectos |
| 10 | `/api/projects/register` | POST | Registrar proyecto |
| 11 | `/api/projects/:id/history` | GET | Historial |
| 12 | `/api/config` | GET/POST | Configuración |
| 13 | `/api/rule-profiles` | GET | Perfiles de reglas |

### cognicode-db: 9 tablas ✅

| # | Tabla | Datos |
|---|-------|-------|
| 1 | `analysis_runs` | Historial de análisis |
| 2 | `issues` | Issues con tracking (open/fixed) |
| 3 | `baselines` | Líneas base |
| 4 | `file_states` | Hashes BLAKE3 |
| 5 | `call_graphs` | Grafos serializados |
| 6 | `symbols` | Símbolos (→ migrar a FTS5) |
| 7 | `call_edges` | Edges |
| 8 | `file_imports` | Dependencias |
| 9 | `style_patterns` | (placeholder, no implementada aún) |

### cognicode-axiom: 854 reglas

| Estrategia | Count | Ejemplos |
|-----------|-------|----------|
| LineScan | ~785 | TODOs, IPs, URLs, naming, tabs, line length |
| AST/Visitor | ~20 | SQL injection, nesting, complexity |
| Metrics | ~50 | Complejidad, líneas, params |
| **NUEVO: SemanticQuery** | 4 | S7000-S7003 |

---

## Qué mejora con cada Fase

### Fase 1 — Fundación

| Feature | Antes | Después | Dónde |
|---------|-------|---------|-------|
| BM25 index | ❌ DashMap volátil | ✅ SQLite FTS5 persistente | `symbol_index` (FTS5) |
| AVC contracts | ❌ No existe | ✅ generate + validate via MCP | `cognicode-core/avc/` |
| Security rules FP | 5/15 con comment-skip | 15/15 con comment-skip | `catalog.rs` |
| SQLite tables | 9 | 12 (+avc_contracts, +drift_events, +symbol_index) | `schema.rs` |
| MCP tools | 13 | 15 (+generate_contract, +validate_contract) | `aix_handlers.rs` |

### Fase 2 — Intent-Drift

| Feature | Antes | Después |
|---------|-------|---------|
| Drift detection | ❌ | ✅ S7000-S7001 rules |
| Dashboard drift | ❌ | ✅ /api/drift endpoint + vista |
| MCP tools | 15 | 16 (+detect_drift) |
| SQLite tables | 12 | 12 (drift_events se puebla) |

### Fase 3 — Proactivo

| Feature | Antes | Después |
|---------|-------|---------|
| Zero-Query RAG | ❌ | ✅ suggest_context tool |
| Incremental parse MCP | ❌ | ✅ reparse_on_edit tool |
| Temporal rules | ❌ | ✅ S7002-S7003 rules |
| Agent analytics | ❌ | ✅ agent_interactions table |
| MCP tools | 16 | 18 (+suggest_context, +reparse_on_edit) |
| Dashboard | 13 endpoints | 15 (+/api/contracts, +/api/agent-stats) |

### Fase 4 — Evolutivo

| Feature | Antes | Después |
|---------|-------|---------|
| Git recency en BM25 | ❌ | ✅ Pesos temporales en FTS5 |
| Verifiable RAG | ❌ | ✅ rustc sandbox contracts |
| Multi-language drift | ❌ | ✅ Python, JS, Go, C++ |
| SQLite tables | 14 | 15 (+style_patterns) |

---

## Lo que NO cambia

| Feature | Por qué |
|---------|---------|
| 730 reglas Style/Lint | Por diseño necesitan escanear todo (tabs, longitudes) |
| Quality Gate | Ya evalúa correctamente |
| File browser (`/api/fs/ls`) | Ya funciona |
| call_graphs, call_edges | Ya usan SQLite |
| Ratings A-E | Ya calculan correctamente |
| Análisis incremental (changed_only) | Ya usa BLAKE3 + file_states |
