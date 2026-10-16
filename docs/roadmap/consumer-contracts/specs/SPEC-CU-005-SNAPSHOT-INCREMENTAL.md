# SPEC-CU-005 — Snapshot/identidad de caché e incrementalidad honesta

**Estado:** PROPOSED · **Bloque:** CU-4 · **ADR:** `ADR-CU-004`

## Problema verificable

Guard construye una clave de caché indicando `cognicode-core/v{CARGO_PKG_VERSION}` desde `guard-cli` y el literal `no-git-rev`: versiona el consumer, no el proveedor, y no ancla la revisión. CogniCode ya posee lógica de hash/incremental y bases de análisis. Un contrato de identidad correcto debe evitar resultados stale sin crear un almacenamiento paralelo.

## Invariantes

- **CU-005.1** `SnapshotKey` se obtiene con función pura de estructura canónica ordenada: `workspace_identity`, paths normalizados+hashes **con el path asociado**, content manifest completo, source revision (si existe), dirty digest, analyzer build commit/version reales, schema, enabled features, config/capability profile y scope. Sin suposiciones de hora/mtime como autoridad.
- **CU-005.2** Una edición con mismo tamaño y mtime distinto solo de contenido invalida; un path diferente con los mismos bytes debe invalidar si su resolución semántica importa. Hashes de archivo no se pasan como lista sin identidad/orden estable.
- **CU-005.3** Cache hit exige validez de clave + decode validado + schema + integridad. Corrupt/stale/missing => recompute o `Unknown/Failed` explícito; nunca `Complete` usando datos stale.
- **CU-005.4** Hacerlo primero reutilizando `incremental/content_hash.rs` y cache de sesión existente. Evitar introducir otra DB o daemon.
- **CU-005.5** **No llamar incremental** al algoritmo que solo salta todo el análisis por cache hit. Describir `cold full`, `warm reuse`, `incremental delta` solo cuando el cambio parcial realmente reduce recomputación medido.
- **CU-005.6** Medir cold/warm/edit p50/p95, asignaciones y bytes de output de 1 fixture representativa; no fijar tiempos inventados. Ningún test en merge debe exigir tiempos inestables; budget reproducible en integración/RC.

**Aceptación:** cold y warm equivalen semánticamente; cambio semántico invalida; corrupción falla cerrado; diferencia de compilación y feature evita stale; workspace A != B. **Permanente:** una property pequeña con mutaciones del manifest; **UAT/AAT temporal** del recorrido completo.

**Referencias:** `crates/cognicode-core/src/incremental/content_hash.rs`, `src/application/analysis_basis.rs`, `odd/tasks/b3-agent-code-understanding.md`, `odd/tasks/b6-agent-ready-runtime.md`; en Guard `guard-application/src/graph_cache.rs`, `guard-cli/src/main.rs`.
