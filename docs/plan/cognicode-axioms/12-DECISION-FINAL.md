# Decisión Final de Arquitectura

## Resumen Ejecutivo

Después de 11 documentos de investigación (~20,000 líneas) y análisis detallado del código existente, se tomó la siguiente decisión arquitectónica definitiva para el proyecto:

---

## La Decisión

| Eje | Decisión | Justificación |
|-----|----------|---------------|
| **Gobernanza** | ❌ ELIMINADA | Cedar añade 200+ deps transitivas (~45s compile). Mercado especulativo. |
| **Calidad** | ✅ FOCO ÚNICO | Mercado probado (SonarQube: 500K+ orgs). `declare_rule!` macro es la innovación real. |
| **Proyecto** | ✅ DENTRO de CogniCode | Reutiliza tree-sitter, CallGraph, ComplexityCalculator (79K líneas). Sin governance, solo añade `inventory` como dep nueva. |
| **MCP Servers** | ✅ DOS binarios | cognicode-mcp (code intel, puerto 8000) + cognicode-quality (quality, puerto 8001). Aislados, mismo workspace, mismo core. |
| **Crate axiom** | ✅ REFACTORIZADO | Se queda como `cognicode-axiom` pero enfocado en quality analysis. Sin Cedar, sin hooks, sin reflexión, sin audit. |

---

## Lo que se ELIMINA

| Componente | Líneas | Dependencias | Razón |
|-----------|--------|-------------|-------|
| `policy/` (Cedar) | ~787 | `cedar-policy` (200+ transitivas) | Governance pesado, especulativo |
| `hooks/` | ~364 | — | Claude Code hooks de enforcement |
| `reflection/` | ~1,707 | — | Self-correction loop para agentes |
| `audit/` | ~792 | `rusqlite` (SQLite bundled) | SQLite añade ~30s compile |

**Total eliminado**: ~3,650 líneas, ~75s de compile time.

---

## Lo que se CONSERVA

| Componente | Líneas | Estado |
|-----------|--------|--------|
| `quality/solid.rs` (SOLID) | 524 | ✅ Implementado |
| `quality/connascence.rs` | 551 | ✅ Implementado |
| `quality/boundary.rs` | 340 | ✅ Implementado |
| `quality/lcom.rs` | 435 | ⚠️ Esqueleto |
| `quality/delta.rs` | 416 | ⚠️ Esbozo SQALE |
| `rules/store.rs` | 270 | ✅ CRUD completo |
| `rules/adr_parser.rs` | 256 | ✅ ADR → Rules |
| `rules/validator.rs` | 210 | ✅ Validación |
| `linters/` (clippy, eslint, semgrep) | 933 | ✅ Wrappers |
| `mcp/tools.rs` | 1,049 | ⚠️ Necesita limpieza de governance |
| `error.rs` | 178 | ✅ |
| `lib.rs` | 39 | ⚠️ Necesita actualización |

**Total conservado**: ~5,201 líneas. Base sólida que necesita refinar.

---

## Lo que se CONSTRUYE (nuevo)

| Feature | Prioridad | Líneas estimadas | Dónde se documenta |
|---------|-----------|-----------------|-------------------|
| `declare_rule!` macro | 🔴 Crítica | ~400 | doc 08 |
| `inventory` auto-registro | 🔴 Crítica | ~100 | doc 08 |
| Code smells nativos (9) | 🔴 Crítica | ~1,200 | doc 09 |
| Quality gates YAML | 🟡 Alta | ~500 | doc 09 |
| Duplications (BLAKE3) | 🟡 Alta | ~400 | doc 09 |
| Technical debt (SQALE) | 🟡 Alta | ~300 | doc 09 |
| Ratings A-E | 🟢 Media | ~200 | doc 09 |
| `cognicode-quality` binario | 🔴 Crítica | ~200 | doc 12 |
| MCP tools (refactor) | 🔴 Crítica | ~300 | doc 04 |

---

## Arquitectura Final

```
CogniCode Workspace/
│
├── cognicode-core/            # LIB: shared infrastructure
│   ├── tree-sitter (6 langs)
│   ├── CallGraph (PetGraph + Redb)
│   ├── ComplexityCalculator
│   ├── ImpactAnalyzer
│   └── CycleDetector (Tarjan SCC)
│
├── cognicode-mcp/             # BIN: code intelligence (puerto 8000)
│   ├── 32 tools (analyze_*, get_*, find_*, refactor_*, export_*)
│   └── depende de: cognicode-core
│
├── cognicode-quality/         # BIN: quality analysis (puerto 8001) ← NUEVO
│   ├── ~15 tools (check_quality, smells, gates, debt, ratings, linters)
│   └── depende de: cognicode-core + cognicode-axiom
│
└── cognicode-axiom/           # LIB: rule engine + quality logic (REFACTORIZADO)
    ├── rules/                 # declare_rule! macro + inventory + RuleRegistry
    ├── quality/               # SOLID, connascence, LCOM, boundaries, delta, smells, gates, debt, ratings
    ├── linters/               # clippy, eslint, semgrep wrappers
    ├── mcp/                   # MCP tool definitions
    └── depende de: cognicode-core + inventory
```

---

## Dependencias (antes vs después)

| Antes (con governance) | Después (solo quality) |
|----------------------|----------------------|
| cedar-policy (200+ transitivas) | ❌ |
| rusqlite (SQLite bundled) | ❌ |
| uuid | ❌ |
| notify (hot-reload) | ❌ |
| cognicode-core | ✅ |
| rayon | ✅ |
| tokio | ✅ |
| rmcp | ✅ |
| serde/serde_json | ✅ |
| thiserror/anyhow | ✅ |
| chrono | ✅ |
| tracing | ✅ |
| dashmap/parking_lot | ✅ |
| regex | ✅ |
| **inventory** | ✅ (NUEVO, ~0.5s compile) |

---

## Cache Compartido

```
~/.cognicode/
├── cache/
│   ├── graphs.redb      # Call graphs (accedido por ambos servidores)
│   ├── parses.redb      # Parse trees cacheados
│   └── complexity.redb  # Métricas cacheadas
├── quality/
│   ├── gates/           # YAML quality gates
│   │   ├── default.yaml
│   │   └── strict.yaml
│   └── profiles/        # YAML quality profiles
│       ├── default.yaml
│       ├── security.yaml
│       └── rust.yaml
└── rules/               # .cedar ADR → .yaml rules (convertidas)
    └── adr-derived.yaml
```

---

## Resumen de MCP Tools

### cognicode-mcp (existente, sin cambios)
32 tools de code intelligence: analyze_*, get_*, find_*, refactor_*, export_*

### cognicode-quality (nuevas)
| Tool | Descripción |
|------|-------------|
| `check_quality` | Análisis completo de calidad de un proyecto |
| `quality_delta` | Compara dos snapshots de calidad |
| `check_boundaries` | Verifica límites arquitectónicos |
| `detect_duplications` | Detecta código duplicado (BLAKE3) |
| `list_smells` | Lista code smells encontrados |
| `evaluate_gate` | Evalúa un quality gate YAML |
| `compute_debt` | Calcula technical debt (SQALE) |
| `rate_project` | Asigna rating A-E al proyecto |
| `list_rules` | Lista reglas disponibles |
| `test_rule` | Prueba una regla con fixtures |
| `check_lint` | Ejecuta linters externos |
| `load_adrs` | Convierte ADRs a reglas |
| `get_profile` | Obtiene un quality profile |
| `get_rule` | Detalle de una regla específica |

---

## Performance Esperada

| Operación | Latencia |
|-----------|---------|
| Parse 10K líneas | ~50ms (tree-sitter) |
| Análisis 50 reglas en paralelo | ~200ms (rayon) |
| Quality gate evaluation | ~1ms (YAML lookup) |
| Duplication detection (100K líneas) | ~500ms (BLAKE3 hashing) |
| Escaneo completo (100K líneas, 50 reglas) | ~2-4s |

40x más rápido que SonarQube (2min en CI).

---

## Principios de Diseño

1. **Zero JVM**: Todo Rust, sin Java, sin PostgreSQL
2. **Type-safe rules**: `declare_rule!` verifica en compilación
3. **Single parse tree**: tree-sitter parsea una vez, las reglas se ejecutan en paralelo
4. **Incremental**: solo re-analizar archivos modificados (hash de contenido)
5. **Aislado**: quality no puede tumbar code intelligence
6. **Shareable**: quality gates, profiles y reglas son archivos YAML versionables

---

## Lo que NO hacemos

- ❌ Gobernar agentes IA con Cedar
- ❌ Bloquear acciones en tiempo real
- ❌ Audit trail de compliance
- ❌ Self-correction loops
- ❌ Hot-reload de políticas

---

## Lo que SÍ hacemos

- ✅ Análisis de calidad nativo en Rust
- ✅ Reglas type-safe con `declare_rule!` macro
- ✅ Code smells, duplicaciones, deuda técnica
- ✅ Quality gates configurables (YAML)
- ✅ Ratings A-E (estilo SonarQube)
- ✅ Integración con linters externos
- ✅ dos servidores MCP aislados y combinables

---

*Decisión tomada: 30 abril 2026. Documento definitivo.*
