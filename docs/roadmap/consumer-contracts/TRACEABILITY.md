# Matriz de trazabilidad CU — contrato → código → valor → prueba

Esta tabla es planificación de requisitos, NO certificación. La comprobación final exige análisis del diff real y test ejecutado.

| Req / ADR | Capacidad de producto existente reutilizada | Dolor de Guard / consumidor | Gate permanente mínimo | Evidencia efímera | Release |
|---|---|---|---|---|---|
| CU-001 / ADR-CU-001 | `AnalysisBasis`, `Analyze`, `CallGraphProjectionPort` | Plan CLI/MCP divergente, dependencia directa de core v0.101.10 | ADT outcome + facade Rust | UAT-1, AAT-1, E2E-1 condicional | CU-1 |
| CU-002 / ADR-CU-002 | `product/tools.json`, `interface/catalog.rs`, `output_contracts.rs` | Herramienta real y catálogo dispares; Graph None en MCP Guard | paridad canonical DTO + schema | UAT-1/UAT-3; AAT-3 | CU-1, CU-3 |
| CU-003 / ADR-CU-003 | `FindingVerifier`, evidence kernel, grounding | Guardia confunde observación, fact y regla admitida | scope mismatch + signature/evidence verification | UAT-2, AAT-2 | CU-2 |
| CU-004 / ADR-CU-002 | `inspect`, `intent`, `impact` | respuestas grandes, navegación no accionable | cursor partition + `Unknown` semantics | UAT-3, AAT-3 | CU-3 |
| CU-005 / ADR-CU-004 | `incremental/content_hash.rs`, `analysis_basis.rs` | cache del consumer identifica wrong provider, `no-git-rev` | invalidación por manifest/versión | UAT-4, AAT-4 | CU-4 |
| CU-006 / ADR-CU-004 | Product manifests + PipelineK release factory | assets ausentes y consumer no instalable | completitud inventario vs manifest | E2E-4, AAT-4 | CU-4 |

## Decisiones/acciones externalizadas a Guard

1. Guard `guard-cognicode/Cargo.toml`: bump a tag real **solo tras** CU-1 y semver verificable; no usar ruta `main` móvil.
2. `guard-cli`/`guard-mcp`: un único pipeline `ScanPlan` que use el grafo cuando las reglas lo requieren. Esta corrección es necesaria para la paridad Guard, pero el nuevo SDK CogniCode no modifica el repositorio remoto Guard por sí solo.
3. `guard-application/metric_provider.rs`: distinguir `GraphMetric` y `SymbolMetric`, precomputar agregados; resolver símbolo por id exacta (no sufijos ambiguos), parsear fichero una sola vez.
4. `guard-domain`/`guard-policy`: admitir solo reglas con aprobaciones verificadas; `AdmittedFinding::is_strictly_admitted` no prueba integridad de firma ni grounding; usar verificador real y separar estados.
5. `guard-application/scan_runner.rs`: propagar la completitud y el rc real, no asignar `Complete`/exit0 por defecto.
6. `guard-application/graph_cache.rs`: derivar provider version de CogniCode SDK y key completa, no versión Guard.

**Prioridad:** estos cambios externos se empaquetan en branches/PR de Guard, con permiso, matriz N−1, sin bloquear releases CogniCode con pruebas que están fuera de su control. Si no se pueden ejecutar, marcar el contrato externo `BLOCKED_EXTERNAL` y dejar un smoke cliente listo, sin afirmar que Guard está integrado al 100%.

## Rutas de autoridad que NO se sustituyen

- `docs/roadmap/ROADMAP.md` frente a este plan (única agenda).
- `docs/roadmap/JOURNAL.md` para recibos existentes, sin ledger paralelo.
- `docs/prf/` histórico y `C8-R` human-gated.
- `product/tools.json` y el descriptor Rust de catálogo, sin `consumer-tools.json` paralelo.
- `docs/roadmap/adr/ADR-012-st04-cli-mcp-separation.md`: no refactorizar bins por estilo.
- `merge-gate.pipeline.kts`, `integration.pipeline.kts`, `release-candidate.pipeline.kts`, `release.pipeline.kts`: autoridad ejecutable.
