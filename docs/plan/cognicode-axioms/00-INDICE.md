# Axiom — Research Index (Refactored)

## Visión General

**cognicode-axiom** es un crate de **análisis de calidad de código nativo en Rust** para CogniCode. Proporciona code smells, quality gates, technical debt, ratings y detección de duplicados — sin JVM, sin SonarQube, sin dependencias externas pesadas.

**Stack**: Rust + CogniCode (tree-sitter, CallGraph, ComplexityCalculator) + rmcp + `declare_rule!` macro + `inventory`

**Arquitectura**: Dos servidores MCP en el mismo workspace:
- `cognicode-mcp` (puerto 8000) → code intelligence
- `cognicode-quality` (puerto 8001) → quality analysis

---

## Documentos de Investigación

| # | Documento | Líneas | Descripción |
|---|----------|--------|-------------|
| 01 | [ESTADO-ARTE-ALINEAMIENTO.md](./01-ESTADO-ARTE-ALINEAMIENTO.md) | 1,681 | Estado del arte: CLAUDE.md, contract-first, gaps en CogniCode |
| 04 | [ARQUITECTURA-SERVIDOR-MCP.md](./04-ARQUITECTURA-SERVIDOR-MCP.md) | 782 | Arquitectura de 2 servidores MCP, cognicode-quality, cognicode-core compartido |
| 05 | [PLAN-IMPLEMENTACION.md](./05-PLAN-IMPLEMENTACION.md) | 1,818 | Plan por 6 fases (~8 semanas): macro → rules → smells → gates → MCP server |
| 06 | [ROADMAP.md](./06-ROADMAP.md) | 662 | Releases v0.1-v1.0, features por release |
| 07 | [SONARQUBE-ANALISIS.md](./07-SONARQUBE-ANALISIS.md) | 1,182 | SonarQube deep dive: arquitectura, features, API, estrategia híbrida |
| 08 | [RULE-ENGINE.md](./08-RULE-ENGINE.md) | 2,109 | **`declare_rule!` macro**, `inventory` auto-registro, RuleRegistry, helpers de contexto |
| 09 | [CALIDAD-NATIVA.md](./09-CALIDAD-NATIVA.md) | 2,682 | Code smells, duplicaciones, gates, debt SQALE, ratings A-E — implementaciones completas |
| 10 | [ARQUITECTURA-INTEGRAL.md](./10-ARQUITECTURA-INTEGRAL.md) | 633 | Arquitectura definitiva: 2 MCP servers, shared core, sin governance |
| 11 | [EXTRACTION-CATALOGO.md](./11-EXTRACTION-CATALOGO.md) | 1,617 | SonarQube API → Rust: extracción, porting Java→Rust, batch pipeline |
| 12 | [DECISION-FINAL.md](./12-DECISION-FINAL.md) | 217 | Decisión arquitectónica definitiva: qué se eliminó, qué se conserva, qué se construye |

---

## Decisión Clave: Sin Governance

| Componente | Decisión | Razón |
|-----------|----------|-------|
| Cedar Policy | ❌ ELIMINADO | 200+ deps transitivas (~45s compile), especulativo |
| Hooks | ❌ ELIMINADO | Claude Code enforcement de governance |
| Reflection | ❌ ELIMINADO | Self-correction loop para agentes |
| Audit (SQLite) | ❌ ELIMINADO | rusqlite añade ~30s compile |
| **Code Quality** | ✅ FOCO | Mercado probado (SonarQube: 500K+ orgs) |

**Lo que queda**: `cognicode-axiom` refactorizado como rule engine de calidad, sin Cedar, sin governance.

---

## Arquitectura en 2 Líneas

```
cognicode-mcp (puerto 8000)          cognicode-quality (puerto 8001)
├─ 32 tools code intelligence        ├─ ~15 tools quality analysis
└─ cognicode-core                    └─ cognicode-axiom (quality)
                                        └─ cognicode-core (tree-sitter, CallGraph...)
```

---

## El Nucleo de la Innovacion

### `declare_rule!` — Reglas sin boilerplate

```rust
declare_rule! {
    id: "S138",
    name: "Functions should not be too long",
    severity: Severity::Major,
    category: Category::CodeSmell,
    params: { max_lines: usize = 80 },

    check |ctx, params| {
        ctx.query_functions()
            .filter(|f| f.line_count() > params.max_lines)
            .map(|f| Issue::new(f, format!("{} líneas", f.line_count())))
            .collect()
    }
}
```

Auto-registro en tiempo de compilación via `inventory`. Añadir una regla = crear un archivo `.rs`. Compilar. Listo.

### Sin SonarQube, Sin JVM, Sin Latencia

| Métrica | SonarQube | cognicode-quality |
|---------|-----------|-----------------|
| Parse 10K líneas | ~2s (Java) | ~50ms (tree-sitter) |
| Análisis 50 reglas | ~5s | ~200ms (rayon) |
| Quality gate | ~1s (servidor) | ~1ms (YAML) |
| Escaneo completo | ~2min (CI) | ~3s (local, MCP) |

---

## Stack Tecnologico

```
CogniCode/
├── cognicode-core/           # Domain (tree-sitter, CallGraph, Complexity, Cycles)
├── cognicode-mcp/            # MCP server: code intelligence (puerto 8000)
├── cognicode-quality/        # MCP server: quality analysis (puerto 8001) ← NUEVO
└── cognicode-axiom/         # Lib: rule engine + quality logic (REFACTORIZADO)
    ├── rules/                # declare_rule! + inventory + RuleRegistry
    ├── quality/              # SOLID, connascence, smells, gates, debt, ratings
    └── linters/             # clippy, eslint, semgrep
```

**Dependencias eliminadas**: cedar-policy, rusqlite, uuid, notify
**Dependencias nuevas**: inventory (~0.5s compile)
**Dependencias reutilizadas**: cognicode-core, rayon, tokio, serde, rmcp, etc.

---

## Fases de Implementacion

| Fase | Semanas | Entregable |
|------|---------|------------|
| 1. Foundation | 1-2 | declare_rule! macro + inventory + RuleRegistry |
| 2. Rule Catalog | 3-4 | ~21 reglas Tier 1 (S138, S3776, S134, S107...) |
| 3. Code Smells + Quality Gates | 4-5 | 9 smells nativos + YAML gates + SQALE debt |
| 4. Ratings + Duplications | 5-6 | Ratings A-E + BLAKE3 duplication detection |
| 5. cognicode-quality MCP | 6-7 | Nuevo servidor MCP (puerto 8001) |
| 6. SonarQube Catalog | 7-8 | Pipeline de porting + reglas adicionales |

---

## Resumen Ejecutivo

**Qué**: Un crate Rust (`cognicode-axiom`) que añade análisis de calidad de código nativo a CogniCode — sin dependencias externas (sin JVM, sin PostgreSQL, sin SonarQube).

**Como**: `declare_rule!` macro para reglas sin boilerplate + `inventory` para auto-registro + tree-sitter para parsing + Redb para cache + YAML para gates/profiles.

**Arquitectura**: Dos servidores MCP aislados (code intelligence + quality) compartiendo `cognicode-core`. Calidad aislada de inteligencia para que un crash no afecte al otro.

**Valor**: Análisis de código en tiempo real (no CI), reglas type-safe en compilación, performance 40x mejor que SonarQube, zero dependencias externas.

---

*Total investigación: ~13,500 líneas en 11 documentos — 30 abr 2026*
