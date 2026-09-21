# Cross-file scope-aware corpus

Corpus independiente (no derivado de `cognicode-core`) usado por la
unidad **F2.W7** para verificar que el camino
`analysis_service::build_project_graph` (el que invoca el binario
real `cognicode-mcp`) respeta las reglas scope-aware de resolución
`name → SymbolId` que F2.W5 introdujo en `FullGraphStrategy` /
`PerFileStrategy`.

## Estructura

- `src/lib.rs` — caller `caller_in_lib` con cinco llamadas que
  ejercen las cinco ramas del resolver.
- `src/nested/mod.rs` — homónimos que viven sólo aquí o también en
  `ambig/`.
- `src/ambig/mod.rs` — tercer archivo con homónimos.

## Las cinco llamadas y la regla que ejercen

| # | Caller invoca | Candidates | Regla | Destino esperado |
|---|---|---|---|---|
| 1 | `nested::callee_in_nested()` | sólo `src/nested/mod.rs::callee_in_nested` | 1 candidato | `src/nested/mod.rs::callee_in_nested` |
| 2 | `local_helper()` | `src/lib.rs::local_helper` + `src/nested/mod.rs::local_helper` | múltiples en archivo del caller → local | `src/lib.rs::local_helper` |
| 3 | `ambig::compute(42)` | sólo `src/ambig/mod.rs::compute` | 1 candidato (cross-file) | `src/ambig/mod.rs::compute` |
| 4 | `shared_name()` | `src/lib.rs::shared_name` + `src/nested/mod.rs::shared_name` + `src/ambig/mod.rs::shared_name` | múltiples en archivo del caller → local | `src/lib.rs::shared_name` |
| 5 | `nested::two_way_ambig()` | `src/nested/mod.rs::two_way_ambig` + `src/ambig/mod.rs::two_way_ambig` | ninguno en archivo del caller, mismo crate root → uno de los dos | `src/nested/mod.rs::two_way_ambig` (determinista) |

## Por qué este corpus

El corpus `equivalence_full_vs_perfile/` (F2.W3) sólo verificaba que
`full` y `per_file` descubriesen el mismo conjunto de símbolos; no
verificaba **a qué `SymbolId` resolvía cada call site**. El corpus
`per_file_correctness/` (F2.W1) verificaba invalidación de cache. El
presente corpus ataca la pregunta concreta que F2.W5 dejó sin
responder en el camino real: ¿el resolver del binario **elige el
destino correcto** o elige el que tiene el FQN lexicográficamente
menor (que es lo que el código de
`analysis_service::build_project_graph` hacía antes de F2.W7)?

## Uso

El test que ejecuta este corpus vive en
`crates/cognicode-core/src/application/services/analysis_service.rs`
bajo `w7_scope_aware_resolution_tests` y construye el grafo con el
camino real (`AnalysisService::build_project_graph`), no con las
strategies de `infrastructure/graph/`.
