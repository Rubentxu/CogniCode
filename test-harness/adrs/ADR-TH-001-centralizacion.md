# ADR-TH-001 — Subárbol único `test-harness/`

- **Estado:** PROPOSED (pendiente del maintainer).
- **Decisión:** todo código nuevo y documentos de gobierno de la infraestructura de pruebas bajo `test-harness/`. No mover tests de `crates/*/tests` ni la publicación en `release.pipeline.kts`. Durante transición mantener *shims* finos en rutas actuales para `scripts/test-fast.sh`, `scripts/test-full.sh`, `scripts/ci/select-suites.sh`, `scripts/ci/run-all-contracts.sh`, `sandbox/scripts/run_campaign.sh`. Los *shims* delegan sin duplicar política.
- **Contexto:** múltiples scripts y runners distribuidos, cobertura contractual anclada a rutas, sandbox propio; migración masiva rompería gate y colaboración paralela.
- **Rechazadas:** `scripts/harness` sin documentación en mismo subtree (sigue disperso); mover todos los tests Cargo a carpeta única (rompe discovery); crear repositorio nuevo (doble fuente de verdad); crear un pipeline nuevo independiente de PipelineK (doble CI).
- **Consecuencia:** directorio central visible; al final puede quedar un mínimo de compatibilidad en raíz; eso NO se contabiliza como un segundo harness. Contratos mantienen una sola autoridad.
- **Prueba de aceptación:** ninguna implementación de selección/validación duplicada fuera de `test-harness`, contratos root no se rompen y todos los targets Cargo siguen descubriéndose.
