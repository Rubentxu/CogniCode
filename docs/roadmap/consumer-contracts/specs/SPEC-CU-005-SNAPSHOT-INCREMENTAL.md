# SPEC-CU-005 — Snapshot/identidad de caché e incrementalidad honesta

**Estado:** PROPOSED · **Bloque:** CU-4 · **ADR:** `ADR-CU-004`

## Problema verificable

Guard construye una clave de caché indicando `cognicode-core/v{CARGO_PKG_VERSION}` desde `guard-cli` y el literal `no-git-rev`: versiona el consumer, no el proveedor, y no ancla la revisión. CogniCode ya posee lógica de hash/incremental y bases de análisis. Un contrato de identidad correcto debe evitar resultados stale sin crear un almacenamiento paralelo.

## Frontera verificada del contrato

La ruta durable vigente persiste una tupla formada por `SNAPSHOT_SCHEMA`,
`SnapshotKey`, `CallGraph` y `FileManifest` en
`application/graph_snapshot.rs`. `SnapshotKey` identifica el proveedor del
análisis, el workspace canónico y el schema; `FileManifest` identifica los
paths y bytes que el grafo leyó. `load_or_build_graph` valida primero el
proveedor y después la frescura del manifest, y trata un decode corrupto o un
schema incompatible como snapshot ausente.

La configuración publicada en `BasisDto.config_digest` es provenance de la
respuesta: actualmente resume `HandlerContext.log_level` y no se persiste en
la tupla durable. `AnalysisBasis.profile` y `AnalysisBasis.budget` tampoco se
pasan a `load_or_build_graph`; la caracterización de `TASK-172` confirma que
variarlos no cambia el `CallGraph`, el manifest, la clave durable ni el origen
publicado. `AnalysisScope` pertenece a la ejecución de findings y fija el par
workspace/snapshot; no es un parámetro del grafo durable. Por tanto, no se deben
añadir esos contextos al `SnapshotKey` mientras no cambien la semántica del
grafo. Solo una opción que cambie el grafo producido puede ser parte de la
identidad del proveedor.

La medición de H5/C11 fija además esta frontera operacional: cuando el
manifest es fresco, `load_or_build_graph` reutiliza el snapshot completo;
cuando deja de ser reutilizable, llama a `build_project_graph` y reconstruye
el grafo completo. `IncrementalScanner` y `FileManifest::compute_delta`
existen como infraestructura separada, pero no están conectados a esta ruta.

La medición reproducible del instrumento, con una muestra, observó `full
rebuild` en frío (36.981 ms), `warm reuse` sin cambios (33.788 ms) y `full
rebuild` después de editar manteniendo tamaño y mtime (35.971 ms). En la ruta
separada de `IncrementalScanner`, la primera pasada procesó 2 ficheros, la
pasada sin cambios procesó 0, la edición de igual tamaño clasificó 1 cambiado
y 1 sin cambios, y la fase de alta/baja clasificó 1 añadido y 1 eliminado.
Estos conteos caracterizan el scanner, no demuestran recomputación parcial en
`load_or_build_graph`; por tanto, la ruta real no publica `incremental delta`.

## Invariantes

- **CU-005.1** La identidad durable del grafo se obtiene con función pura de
  `workspace_identity`, proveedor real (versión, commit y features) y schema;
  el `FileManifest` asociado conserva paths normalizados y hashes de contenido.
  Configuración, perfil, presupuesto y scope solo entran en esta identidad si
  una prueba demuestra que cambian el grafo producido. Sin suposiciones de
  hora/mtime como autoridad.
- **CU-005.2** Una edición con mismo tamaño y mtime pero distinto contenido
  invalida; un path diferente con los mismos bytes invalida cuando cambia la
  resolución semántica. Los hashes no se pasan como lista sin identidad ni
  orden estable.
- **CU-005.3** Cache hit exige proveedor compatible, decode validado, schema e
  integridad/frescura del manifest. Corrupt/stale/missing implica recompute o
  `Unknown/Failed` explícito; nunca `Complete` con datos stale.
- **CU-005.4** Reutilizar `incremental/content_hash.rs`, `FileManifest`,
  `ManifestStore` y la validación de snapshots existente. No introducir otra
  DB, daemon ni autoridad paralela.
- **CU-005.5** **No llamar incremental** al algoritmo que solo salta todo el
  análisis por cache hit. Describir `cold full`, `warm reuse`, `full rebuild
  after edit` e `incremental delta` solo cuando el cambio parcial realmente
  reduzca recomputación medida.
- **CU-005.6** Medir cold/warm/edit p50/p95 y muestras reales mediante un
  instrumento explícito fuera de la suite rápida; no fijar tiempos inventados,
  asignaciones no observadas ni umbrales inestables en tests de merge.

**Aceptación:** TASK-172 separa grafo, manifest, clave y provenance al variar
configuración, perfil, presupuesto y scope; el edit invalida de forma honesta;
corrupción falla cerrado; la medición distingue reutilización completa de
reconstrucción completa y no afirma delta sin observarlo. **Permanente:** una
property pequeña con mutaciones del manifest; **UAT/AAT temporal** del recorrido
completo.

**Referencias:** `crates/cognicode-core/src/incremental/content_hash.rs`, `src/application/analysis_basis.rs`, `odd/tasks/b3-agent-code-understanding.md`, `odd/tasks/b6-agent-ready-runtime.md`; en Guard `guard-application/src/graph_cache.rs`, `guard-cli/src/main.rs`.
