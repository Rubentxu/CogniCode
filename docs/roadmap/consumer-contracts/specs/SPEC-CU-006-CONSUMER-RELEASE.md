# SPEC-CU-006 — Compatibilidad, economía de tests y release consumible

**Estado:** PROPOSED · **Bloque:** CU-4 · **ADR:** `ADR-CU-004`

## Contratos de publicación

- **CU-006.1** El catálogo `product/product-manifest.json` determina el set de productos, plataformas y canales: incluir `cognicode`, `cognicode-mcp`, `cogh` y otros si el manifiesto del release así los requiere. No hardcodear número 8; leer inventario y verificar conjunto completo esperado vs publicado.
- **CU-006.2** Las bibliotecas/sdk Rust se publican solo si el repositorio realmente adopta un canal y forma distribuible; si el core se consume por tag Git, fijar `rev` o tag inmutable y testear `cargo build`. No atribuirle Maven a Rust ni subir un asset ficticio.
- **CU-006.3** N−1 de wire/schema, semver, feature set y capabilities con **cliente externo real**, incluyendo CogniCode Guard. El version handshake debe distinguir `Compatible`, `Degraded(NonEmpty<Gap>)`, `Incompatible` y `Unavailable`.
- **CU-006.4** RC sobre SHA limpio, íntegro y reproducible. Publicar exclusivamente los bytes ya validados; comparar checksums tras descargarlos del remoto. `SHA256SUMS` correcto para 3/8 activos no demuestra completitud. Fuente GitHub autogenerada ≠ producto.
- **CU-006.5** La ejecución de `release.pipeline.kts` queda detrás de su preflight `release-candidate.pipeline.kts`, merge gate y autorización pertinente; no usar GitHub Actions como nuevo orquestador. Proteger `/release` contra recompilación posterior al checksum.
- **CU-006.6** Tests: inner-loop solo afectados + contratos; merge tests relevantes + fast gate; integración/full en frontera de PR/tag/RC **según AGENTS.md**; test instalado y prueba adversarial de assets en release. Medir coste y reducir duplicados con `delivery-first-testing`.
- **CU-006.7** Toda prueba efímera se elimina tras registrar versión, SHA, artefacto, escenario, outcome, logs mínimos y decisión de regresión. UAT/AAT críticos no desaparecen sin equivalente más barato: clasificar `PROMOTE|REPLACE|RETIRE|DEFER`.
- **CU-006.8** Los releases de CU deben incluir changelog consumible por Guard; si migración de Guard queda pendiente, registrar `BLOCKED_EXTERNAL` pero no afirmar end-to-end cross-repo completo.

## Cierre de cada hito

`MERGED_REMOTE + TEST_FULL_PASS + RC_ARTIFACT_VERIFIED + RELEASE_REMOTE + CONSUMER_SMOKE + EPHEMERAL_TEST_CLEANUP + JOURNAL_RECEIPT`.

Si una condición obligatoria falta: `PAUSED|BLOCKED`, nunca `DONE`; el tag/versión exacto se decide tras inspeccionar el workspace actual y política de SemVer.

**Referencias:** `release-candidate.pipeline.kts`, `release.pipeline.kts`, `merge-gate.pipeline.kts`, `integration.pipeline.kts`, `scripts/test-full.sh`, `docs/roadmap/production-ready/runbooks/C8-RECERTIFICATION-RUNBOOK.md`, `docs/adr/ADR-034-cognicode-distribution-package.md`, `product/product-manifest.json`.
