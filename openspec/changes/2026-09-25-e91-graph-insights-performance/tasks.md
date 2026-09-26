# Tasks — e91 Graph Insights Performance (cycle)

> Estado del cycle e91 a 2026-09-26. W1-W6 cerrados. PR-PERF
> (CR-03/04/05) continúa el trabajo sobre PR-G1 ya cerrado.

## Estado por work unit

- [x] **W1 — Metadata honest en `graph_communities`** (CLOSED 2026-09-26, commit `6f40a08b` + `42a1ddcf`). Devuelve `iterations_used` y `converged` reales; antes hardcodeaba `min(max_iter, 100)` y `true`. 2 tests existentes pineaban el bug (actualizados); 2 nuevos pinean el contrato. JOURNAL entry 11.
- [x] **W2 — Caracterización PageRank warm** (CLOSED 2026-09-26, commits `8b4bbe85` + `6b2738f3`). Confirmado: PageRank warm NO es bottleneck (mediciones sobre fixtures reales). JOURNAL entry 12.
- [x] **W3 — Cache de PageRank** (CLOSED 2026-09-26, non-viability). Hit ratio <0.07% del budget total: no viable. Cierre derivado de W2. JOURNAL entry 17.
- [x] **W4 — Paralelizar `god_nodes`** (CLOSED 2026-09-26, non-viability). Estimación derivada 2×W2 worst-case = 0.16% del budget: no viable. Cierre derivado de W2. JOURNAL entry 19.
- [x] **W5 — Memoize `surprising_connections`** (CLOSED 2026-09-26, non-viability). Mismo argumento derivado que W4. JOURNAL entry 19.
- [x] **W6 — Metadata envelope en 7 sibling handlers** (CLOSED 2026-09-26, commits `c1618e84` + `df8002f5`). Aplica el patrón de W1 al resto de handlers MCP. JOURNAL entry 19.

## Reapertura justificada

W4 y W5 admiten reapertura SOLO si caracterización directa demuestra
>10% del budget. A 2026-09-26 no se ha ejecutado esa caracterización,
y los cierres derivados se mantienen.

## Hand-off al nuevo programa (PR-PERF)

El programa production-ready stabilization (PR-PERF, ver
`docs/roadmap/production-ready/ROADMAP-ADDENDUM.md`) retoma e91 con
3 acciones nuevas:

- **CR-03** — Perf profiling por etapa con fixture multi-repo
  (1-2 días, P0, prerequisite QW-02). Reabre el espacio de optimización
  con metodología rigurosa y fixture estable.
- **CR-04** — Optimización mínima basada en profiling (2-4 días, P0,
  prerequisite CR-03). El profiling decide el cambio.
- **CR-05** — Test budget + scorecard y streak válido (1-2 días, P0,
  prerequisite CR-04). Regression gate con fixture multi-repo.

El profiling decidirá el camino. NO se crea `InsightCache` por
defecto (lesson del propio paquete EXECUTION-PLAN §CR-03/04/05).

## Reglas de no-regresión

1. Cualquier optimización nueva DEBE medirse contra fixture
   multi-repo pineada en `sandbox/fixtures/perf-regression/`.
2. El scorecard debe mostrar G5 GREEN (p95 < 60s target) y
   mantener streak ≥3 ejecuciones consecutivas.
3. Las features del programa (ST-01..05) NO deben mover la
   hot-path de `graph_insights`/`graph_communities` sin
   re-ejecutar el gate de PR-PERF.
