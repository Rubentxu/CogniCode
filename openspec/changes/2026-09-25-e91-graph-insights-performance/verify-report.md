# Verify report — e91 Graph Insights Performance (cycle)

> Cierre técnico de e91.W1-W6 a 2026-09-26. PR-PERF retoma
> optimización bajo gates más estrictos.

## Resumen ejecutivo

* **Estado del cycle**: W1-W6 CLOSED. Trabajo sustantivo continúa
  bajo PR-PERF (CR-03/04/05) con nueva metodología.
* **Tests workspace**: 5565 passed / 0 failed / 37 ignored (verde).
  37 ignored son tests con motivos legítimos documentados (binarios
  externos, PRF-CI gate). 4 ignored pinean M0.6 (PHP/Swift rotos).
* **Clippy**: `cargo clippy --workspace --all-targets -- -D warnings`
  exit 0.
* **Binario**: `cognicode 0.99.1`.

## Evidencia por work unit

### W1 — Metadata honest en `graph_communities`

* Commits: `6f40a08b`, `42a1ddcf`.
* Test RED previo: `test_detect_reports_real_iterations_chain` y
  `test_detect_reports_non_convergence_on_oscillating_2cycle` (nuevos).
* Tests actualizados: `test_oscillating_two_cycle_*` que pineaban el bug.
* Cambio: `CommunitiesMeta` propagado desde
  `cognicode_graph_algos::communities::leiden_iteration` hasta
  `graph_handlers::graph_communities`.
* Verificación: `cargo test -p cognicode-core --lib graph::communities`
  → todos verdes; metadata correcta en tests con grafos que oscilan.

### W2 — Caracterización PageRank warm

* Commits: `8b4bbe85`, `6b2738f3`.
* Metodología: instrumentación de `graph_algos::pagerank` con timing
  per-iteration + breakdown por fase (build matrix / iterate / read).
* Resultado: warm PageRank consume <0.5% del budget total de
  `graph_insights` en fixture `sandbox/fixtures/large-graph/`.
* Verificación: `cargo bench -p cognicode-core --bench pagerank_warm`
  estable, sin varianza significativa entre runs.

### W3 — Cache de PageRank (non-viable)

* Hit ratio estimado: <0.07% del budget (1 hit esperado por cada
  ~1500 invocaciones sobre fixtures reales).
* Cierre: derivado de W2, sin código nuevo.
* Lección 73: no inventar caches sin hit ratio demostrable.

### W4 — Paralelizar `god_nodes` (non-viable)

* Estimación worst-case: 2× coste W2 = 0.16% del budget.
* Realistic speedup: 1.2× con overhead de rayon + sync barriers.
* Cierre: derivado de W2.

### W5 — Memoize `surprising_connections` (non-viable)

* Mismo argumento derivado que W4.
* Cierre: derivado de W2.

### W6 — Metadata envelope en 7 sibling handlers

* Commits: `c1618e84`, `df8002f5`.
* Cambio: los 7 handlers restantes que delegan en `graph_communities`/
  `graph_insights` ahora propagan `iterations_used`/`converged` (o
  sus equivalentes según el handler) en el envelope MCP.
* Verificación: 7 nuevos tests pinean el contrato de metadata.

## Scorecard previo (pre-PR-PERF)

* **G1 (correctness)**: GREEN — 5565/0/37, 0 regressions.
* **G2 (security)**: GREEN — sin advisories nuevos (RUSTSEC-2024-0437
  preexistente, gestionado por CR-07).
* **G3 (API stability)**: GREEN — sin cambios incompatibles.
* **G4 (release reproducibility)**: YELLOW — C8 firmada pendiente.
* **G5 (performance cold-cache)**: RED — p95=367s, hot path
  `graph_insights`/`graph_communities` no optimizado aún.
  Este gate es **el target** de PR-PERF.

## Próximo gate

PR-PERF (CR-03 → CR-04 → CR-05) debe llevar G5 a GREEN con
fixture multi-repo estable y regression budget documentado.
Resultado se medirá contra este baseline.
