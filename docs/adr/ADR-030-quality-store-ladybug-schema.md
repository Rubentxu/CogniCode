# ADR-030 — QualityStore Schema: LadybugDB Backend

**Estado**: ACEPTADO (schema `QualityStore` lbug con tablas `:QualityIssue`/`:QualityBaseline`/`:QualityRule` y los 10 métodos implementados en `LadybugStore`; reconciliación namespace post-colisión ADR-027 cerrada en `debt-e29-3-1` PR #215 v0.80.1; 1721 tests verdes; 2026-08-03)
**Fecha**: 2026-08-03
**Decisores**: CogniCode Architecture Team
**Contexto**: e29-3-port-abstraction-audit, branch `feat/e29-3-port-abstraction-audit` @ `1a32063e`

---

## Resumen ejecutivo

`QualityStore` (10 métodos) se implementa completamente sobre `LadybugStore` usando tablas lbug `:Issue`, `:Baseline`, `:Rule`. Los métodos de lectura degradan gracefulmente en BD vacía (0 filas → `Vec` vacío / conteo cero, sin error). Los métodos de escritura exponen fallos de I/O como `QualityError::Store(...)`. El conflicto de clave natural en `insert_issues` devuelve `UpsertSummary { inserted: 0, updated: 1 }` (no duplicado).

**Warning W2**: Los nombres de tabla `:Issue`/`:Baseline`/`:Rule` colliden con el namespace de tablas de usuario en la estrategia de schema híbrido (ADR-027). La renormalización a `:QualityIssue`/`:QualityBaseline`/`:QualityRule` está trackeada como `refactor/debt-e29-3-1`.

---

## Contexto

e29-2-semantic-projection-kernel introdujo `LadybugStore` con stubs para los 10 métodos de `QualityStore`. e29-3 completa la implementación real.

El schema de calidad (ADR-028 §Ladybug Schema) define:

```
issues(id, workspace_id, rule_id, severity, category, file_path, line, message, status, created_at)
  — UNIQUE(workspace_id, rule_id, file_path, line)
baselines(workspace_id, rating, total_issues, blockers, criticals, debt_minutes, last_run)
  — PRIMARY KEY(workspace_id)
rules(rule_id, description, category)
  — PRIMARY KEY(rule_id)
```

Estos 3 tablas coexisten con las tablas de grafo call (`:GraphNode`, `:GraphEdge`, etc.) en la misma BD lbug bajo la estrategia de ADR-027 (hybrid schema — un archivo SQLite por dominio logical).

---

## Decisión

`LadybugStore` implementa los 10 métodos de `QualityStore`:

| # | Método | Comportamiento |
|---|--------|----------------|
| 1 | `issues_for_file(file)` | SELECT por `file_path = $1` |
| 2 | `issues_for_scope(scope_prefix)` | LIKE prefix, boundary-aware (`src` ≠ `src_extra`) |
| 3 | `issues_at_line(file, line)` | WHERE `file_path = $1 AND line = $2` |
| 4 | `issue_by_id(id)` | Retorna `Ok(None)` si absent |
| 5 | `rule_summary(rule_id)` | SELECT + aggregations |
| 6 | `quality_gate(workspace_id)` | Rating + issue counts |
| 7 | `open_issues_count(workspace_id)` | COUNT con filtro status ≠ 'closed' |
| 8 | `issues_for_workspace(ws_id, filter)` | Paginación + filter |
| 9 | `insert_issues(issues[])` | INSERT OR REPLACE → `UpsertSummary` |
| 10 | `delete_issue(...)` | DELETE + `Ok(true/false)` flag |

---

## Degradación graceful

Los 8 métodos de lectura (1–8) retornan `Ok(<vacío>)` en BD sin filas:
- `Vec::new()` para resultados
- `0` para counts
- `QualityGateSummary::default()` para gate

NO retornan `Err(...)` en estado vacío.

---

## W2: Schema Collision Risk

**Problema**: Los nombres `:Issue`/`:Baseline`/`:Rule` colliden con el namespace de datos de usuario en la estrategia de ADR-027.

**Opciones**:
1. Renombrar a `:QualityIssue`/`:QualityBaseline`/`:QualityRule` — limpio, no hay backwards compat issue (es schema nuevo)
2. Documentar que cada dominio logical usa su propio schema file — ya es el caso en ADR-027

**Decisión intermedia**: Documentar como W2 (DOCUMENT-FOR-FOLLOW-UP) con follow-up `refactor/debt-e29-3-1`. La prioridad es HIGH dado que la colisión puede ocurrir en testing o en multi-tenant scenarios.

---

## Referencias

- [ADR-027 ladybugdb-hybrid-schema-strategy](./ADR-027-ladybugdb-hybrid-schema-strategy.md)
- [ADR-028 port abstraction](./ADR-028-ladybugdb-port-abstraction-architecture.md)
- e29-3-port-abstraction-audit delta spec: `openspec/changes/e29-3-port-abstraction-audit/specs/quality-store-backend/spec.md`
- engram: `sddk/e29-3-port-abstraction-audit/warning-w2-schema-collision`
