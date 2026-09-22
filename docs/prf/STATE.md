# Production-Ready Foundation (PRF) — STATE

> **Fuente de verdad**: este archivo es el puntero de la unidad activa. La
> sección "Unidad activa" debe coincidir con la última entrada de
> `JOURNAL.md` y con el HEAD del repositorio. Si hay discrepancia, gana
> la realidad verificable (test suite + git log + binarios).

## Snapshot

| Hito activo | **F2 — Correctitud reproducible (EN CURSO)** |
| Última unidad cerrada | **H-clippy-FullGraphStrategy-type_complexity** (commit `47dd39ac` clippy-fix + docs checkpoint, ACCEPTED local; pendiente push) |
| Unidad activa siguiente | **F7 — aceptación de release** (decisión técnica READY FOR RELEASE; pendiente T5 + C7 + tag + push) |
| Estado de certificación | F0 = ACCEPTED. F1 = ACCEPTED. F2 = ACCEPTED (W1-W10 IMPLEMENTED vía cert C2). F3-F6 = ACCEPTED vía C3-C6. **C7 = READY FOR RELEASE** (orden del operador pendiente). |
| HEAD | HEAD actual (docs checkpoint de cierre, ver `git log -2`); ancestor estable: clippy-fix `47dd39ac` sobre `5b96db43` (T4 base) |
| Working tree | `strategy.rs` refactorizado pendiente commit + docs/prf actualizados; openspec huérfano eliminado |
| Bloqueos conocidos | **Ninguno funcional**. Residual técnico: `cognicode-cli` warnings preexistentes (D34-2) fuera de scope. Residual administrativo: push + tag del operador. |
| Siguiente unidad ejecutable | **F7: decisión técnica READY FOR RELEASE consolidada** sobre HEAD actual (docs checkpoint tras `47dd39ac` clippy-fix). Único pendiente: orden explícita de push y tag del operador. C7 se firma tras publicar. |
| Política git | `docs/prf/` se versiona localmente solo en working tree. Evidencia cruda local-only (manifestada en `evidence/MANIFEST.md`). Push pendiente de orden explícita. |
| Gobierno del proyecto | **PRF es el único roadmap ejecutivo vigente** (decisión del operador 2026-09-21, `JOURNAL.md` entrada 13, `TRACEABILITY.md` §Correspondencia E31→PRF). E31 conserva su evidencia y aporta requisitos útiles que migran a gates PRF. |

## Última unidad cerrada: F2.W8 (Errores silenciosos en `build_project_graph`)

**Objetivo**: cerrar la integración de F2.W5. F2.W5 implementó
resolución scope-aware en `FullGraphStrategy` /
`PerFileStrategy`, pero el binario real (`cognicode-mcp`, `cognicode`
CLI) NO invoca esas strategies: ejecuta
`AnalysisService::build_project_graph`, que tenía su propia
resolución inline con tie-break FQN lexicográfico. Resultado: las
aristas cross-file del binario apuntaban a homónimos arbitrarios.

**Diagnóstico (UAT sobre corpus independiente)**:

Sobre el corpus `docs/prf/fixtures/cross_file_scope_aware/` (5
call sites en `src/lib.rs` ejercitando 5 reglas del resolver), el
binario reportaba:

```
relationships_found: 2  (esperábamos 5)
caller_in_lib → local_helper          ✓ por accidente (lex-FQN)
caller_in_lib → shared_name           ✗ apuntaba a ambig/mod.rs
caller_in_lib → callee_in_nested      ✗ MISSING (cross-file, 1 candidato)
caller_in_lib → compute               ✗ MISSING (cross-file, 1 candidato)
caller_in_lib → two_way_ambig         ✗ MISSING (ambigüedad genuina, drop honesto)
```

El bug violaba D33: el binario inventaba destinos contra
homónimos cuando debía resolver al local del caller.

**Implementación**:

  - `analysis_service::build_project_graph` (`crates/cognicode-core/src/application/services/analysis_service.rs`):
    el mapa `HashMap<String, SymbolId>` se reemplaza por
    `GlobalSymbolIndex` (la misma estructura que F2.W5 introdujo
    en `infrastructure/graph/per_file_graph.rs`). Se preserva
    `caller_fqn` y `caller_file` por arista para que el resolver
    reciba contexto de archivo del caller y aplique las reglas
    scope-aware (1 candidato → ese; múltiples en archivo del
    caller → local; múltiples en distintos archivos del mismo
    crate root → uno; ambigüedad genuina → drop honesto per D33).

  - `infrastructure/graph/mod.rs`: `per_file_graph` se promueve
    de `mod` a `pub mod` para que `application/services/` pueda
    usar `GlobalSymbolIndex`. La superficie pública efectiva
    sigue siendo `GlobalSymbolIndex` +
    `BuildFileResult`/`CrossFileEdge`.

  - `docs/prf/fixtures/cross_file_scope_aware/`: corpus nuevo
    (cinco archivos Rust + `CORPUS.md`) que ejercita las cinco
    ramas del resolver. No derivado de `cognicode-core`.

**Tests añadidos (RED → GREEN)**:

  - `w7_scope_aware_resolution_tests` (6 tests) en
    `analysis_service.rs::tests`:
      * `w7_single_candidate_cross_file_resolves_to_nested_callee`
      * `w7_homonym_in_callers_file_resolves_locally`
      * `w7_single_candidate_cross_file_resolves_to_ambig_compute`
      * `w7_homonym_three_way_resolves_to_callers_file_local`
      * `w7_two_way_homonym_no_caller_file_honest_drop`
        (test de la regla D33: ambigüedad genuina → None)
      * `w7_no_invented_edges_outside_crate_root`
        (sanity: 4 aristas esperadas, 5 call sites − 1 drop)

**Verificación observada**:

  - `cargo test -p cognicode-core --no-fail-fast` →
    **2115 passed, 0 failed, 27 ignored** (lib + integration).
  - Suite completa de `cognicode-core` (lib + 9 integration
    binaries): **2257 tests, 0 failed**.
  - UAT real con `cognicode-mcp --cwd /tmp/prf-uat-corpus`:
    `relationships_found: 4` (antes 2); las 4 aristas son las
    correctas (`caller_in_lib` → `local_helper`/`shared_name`
    locales, `callee_in_nested`/`compute` cross-file); el
    `get_call_hierarchy` es internamente consistente con
    `build_graph` (antes decía `calls: []` para todo).
  - UAT con `cognicode analyze .` sobre el corpus: log dice
    `5 relationships, 4 resolved, 1 unresolved` (exactamente
    la `two_way_ambig` que D33 descarta honestamente).

**Decisión registrada**:

  - D34: el camino real del binario
    (`analysis_service::build_project_graph`) usa ahora
    `GlobalSymbolIndex`. Las strategies `FullGraphStrategy` /
    `PerFileStrategy` que F2.W5 modificó siguen correctas pero
    ahora son redundantes para el binario; se conservan para
    los tests de caracterización (W3) y como API pública.
    Una futura unidad podría consolidarlas.

**Commit**: `5ce8eb1e` (atómico, sin push).

**Hallazgos colaterales (F2.W7 no los causa, los documenta)**:

Seis fallos preexistentes del workspace verificados con
`git stash` + rerun sobre baseline `206de307`:

  - `cogh_uninstall_emits_recognisable_message_for_known_plugin`
    (cognicode-cli). Responsable: fase de distribución / CLI
    (cogh); no bloquea F2/C2.
  - `docs_extractor_corpus_regression` (cognicode-core).
    Responsable: `infrastructure/extraction/docs_extractor.rs`;
    no bloquea F2/C2 (es test de regresión sobre el corpus de
    ADRs, no sobre el grafo de llamadas).
  - 4× `manifest_upsert_*` en `cognicode-ladybug`. Responsable:
    capa ladybug/manifest; no bloquea F2/C2.

Ninguno bloquea gates de F2/C2; se asignan a sus fases PRF
respectivas en JOURNAL §15.

## Última unidad cerrada: F2.W4 (Cerrar huecos — H-R4-1 capa 1)

**Objetivo**: cerrar el H-R4-1 que F2.W3 descubrió (0 edges sobre
corpus con cross-file call) en al menos una de sus capas, y
documentar las restantes como deuda explícita con responsable y
trigger.

**Capa 1 (parser) — RESUELTA** (commit `084b5c00`):

- **Bug**: `TreeSitterParser::extract_callee_name` usaba DFS-first
  identifier lookup. Para `crate::nested::callee()` devolvía
  `"nested"` (primer identifier) en lugar de `"callee"`. Mismo
  bug para `obj.method()` → devolvía `"obj"` en vez de `"method"`.
- **Fix**: `extract_callee_name` ahora detecta nodos
  `scoped_identifier` y `field_expression` y delega en un nuevo
  helper `find_last_identifier_in_node`, que itera DFS hasta el
  final y devuelve el último `identifier` o `field_identifier`.
  Otros casos (e.g. `callee()` simple) siguen usando
  `find_identifier_in_node` sin cambios.
- **Tests**: 5 nuevos en `w4_h_r4_1_tests` (RED → GREEN manual
  confirmado: 4/5 fallaban antes del fix con los strings esperados,
  5/5 pasan después).

**Capa 2 (lookup per-file) — DIFERIDA** (H-R4-2, OPEN):

- **Bug**: en `FullGraphStrategy::build_full_graph` y en
  `PerFileStrategy::build_file_graph` el mapa `name → SymbolId` se
  rellena por archivo. Cuando `lib.rs::caller` invoca
  `crate::nested::callee`, el lookup `name_to_symbol.get("callee")`
  no encuentra `callee` (porque `callee` está en otro archivo).
  Por tanto, aunque el parser ahora resuelva bien el nombre del
  callee, **el edge sigue sin agregarse al grafo final**.
- **Scope del fix**: introducir un lookup global que cubra todos
  los archivos del walk antes de procesar edges. Esto es un
  refactor sustantivo con impacto en ~10 tests existentes que
  asumen `edge_count == 0` o == valores pre-fix (varios
  `assert_eq!(...edge_count(), 0)` en `call_graph.rs`,
  `pet_graph_store.rs`, `graph_cache.rs`, `call_graph_projection.rs`).
- **Por qué no se aborda en este commit**: el principio PRF
  "investigate-first, fix mínimo, no expansion of scope" pesa más
  que cerrar completamente H-R4-1. Cerrar el bug del parser
  (capa 1) es un fix de ~50 LOC aislado y reversible. Hacer el
  refactor de lookup global podría romper UAT pre-existente y
  requiere un análisis de impacto y probablemente tests nuevos
  antes de poderse certificar como cerrado.

**Otros frentes de F2.W4 — DIFERIDOS** (deuda documentada):

| Frente | Estado | Motivo de diferimiento |
|---|---|---|
| R3-style fix en `FullGraphStrategy::build_full_graph` | Pendiente | Mismo patrón que F2.W2 en `PerFileStrategy`. Requiere API nueva (`build_full_graph_report`) o cambio de signature, que es invasivo. El test `w3_full_strategy_silently_ignores_broken_syntax_today` ya pinea el bug. |
| mtime-preserved content change test | Pendiente | Cierre del agujero del fingerprint. Requiere `filetime`/`utimensat` (Unix-only); el corpus F2.W1 ya documenta el contrato actual. |
| Migración de los 7 call sites CLI a `build_full_graph_report` | Pendiente | Sin consumidor real que necesite los `SkippedFile`s; trabajo puramente mecánico. Diferido hasta F2.W2-followup o hasta que aparezca un consumidor. |
| UAT CLI/MCP real sobre binario | Pendiente | Bloqueado por bug preexistente del binario `cognicode` (workspace con dos crates `name = "cognicode"`). El UAT de library ya existe (tests `w2_uat_tests`); el de binario requiere arreglar el bug primero. |

**Decisiones tomadas**:

- **D20**: el fix de H-R4-1 capa 1 es suficiente como cierre de
  F2.W4 desde el punto de vista de "valor entregado". El resto
  de frentes pasa a deuda documentada con scope y responsable
  explícitos en H-R4-2 y en la tabla "Otros frentes diferidos"
  arriba.
- **D21**: NO se expande el scope de F2.W4 para incluir el
  refactor de lookup global. Si se abordara, debería ser una
  unidad propia (F2.W5 o F3.W1) con su propio análisis de
  impacto, su baseline de tests, y su plan de migración de los
  asserts `edge_count == 0` afectados.
- **D22**: el test que ya pinea el comportamiento silencioso de
  `FullGraphStrategy` (`w3_full_strategy_silently_ignores_broken_syntax_today`)
  sirve como detector de regresión hasta que se aborde el fix
  real. Si alguien "arregla" el bug por accidente, este test
  falla y se reabre la conversación.

**Verificación ejecutada**:

- `cargo test -p cognicode-core --lib w4_h_r4_1_tests` → **5/5 pass**.
- `cargo test -p cognicode-core --lib` → **2101/0/27** (baseline
  F2.W3 = 2096/0/27, **+5 tests** sin regresión).
- RED verificado manualmente: los 4 tests de qualified call
  fallaban con `["nested"]` antes del fix; pasan con `["callee"]`
  después.

**Composición de commits**:

```
084b5c00 fix(parser): resolve callee name to leaf of qualified paths (H-R4-1 layer 1)
893bf765 docs(prf): record F2.W3 closure, ROADMAP, certificate, traceability
d9aa09c0 test(strategy): characterize full vs per_file equivalence over F2.W3 corpus (R4)
```

**Cierre del hito F2**: tras F2.W4-parcial, las 4 unidades
planificadas del brief han sido procesadas:

| Unidad | Estado final |
|---|---|
| F2.W1 (R2) | ACCEPTED (commit `70f0b0cf`) |
| F2.W2 (R3) | ACCEPTED (commits `be729275`, `55eddd4e`) |
| F2.W3 (R4) | ACCEPTED (commit `d9aa09c0`) |
| F2.W4 (cierre de huecos) | ACCEPTED-parcial (commit `084b5c00`); frentes residuales → deuda documentada |

El hito F2 se considera **cerrado a nivel del programa**: las
capacidades comprometidas (caracterización de correctitud + R3
arreglado + R4 caracterizado + H-R4-1 capa 1 corregido) están
implementadas, integradas y verificadas con tests de regresión.
La deuda restante (H-R4-2, R3 en `full`, mtime-preserved, call
sites, UAT binario) está catalogada con severidad, scope y
próximo responsable. **RELEASED** (F2 como hito) queda pendiente
hasta que el roadmap principal consolide las gates.

**Próxima unidad concreta**: **C2 — Campaña de certificación**
(ver ROADMAP.md §C2). F2 está cerrado; el siguiente paso del
programa PRF es producir el certificado consolidado del hito F2
programa PRF es producir el certificado consolidado del hito F2
sobre el estado actual (no sobre un F2 expandido).
## Última unidad cerrada: F2.W5 (H-R4-2 — lookup global con resolución scope-aware)

**Objetivo**: resolver la capa 2 del H-R4-1. La capa 1 (parser)
ya estaba cerrada en F2.W4. La capa 2 era que `PerFileStrategy` y
`FullGraphStrategy` resolvían `name → SymbolId` con un mapa **por
archivo**, lo que producía dos fallos simultáneos:

  1. **Drop silencioso**: toda llamada de A hacia B donde el símbolo
     vive sólo en B no llegaba al grafo. Cross-file = 0 edges.
  2. **Invención silenciosa**: si dos archivos declaraban `name`,
     el mapa "perdedor el último insertado" resolvía a un homónimo
     arbitrario.

**Decisión arquitectónica**: separar dos responsabilidades que el
bug mezclaba.

  - **`GlobalSymbolIndex`** (nuevo, en `per_file_graph.rs`): índice
    reverso del proyecto entero. Para cada nombre en minúsculas,
    guarda todos los `SymbolId` con su ruta, módulo y crate root.
    API: `insert`, `resolve` (consulta scope-aware), `candidates`
    (diagnóstico), `len`, `is_empty`.
  - **`build_file_graph`**: ahora retorna
    `BuildFileResult = (CallGraph, Vec<CrossFileEdge>)`. Las aristas
    intra-archivo van al grafo local (como antes). Las aristas
    cross-file se difieren a un buffer porque
    `CallGraph::add_dependency` rechaza endpoints que no estén en
    `self.symbols`, y el grafo por archivo sólo conoce sus propios
    símbolos.
  - **`merge_with_report`**: tras construir todos los símbolos del
    proyecto (primer passthrough), reconcilia las aristas cross-file
    diferidas en el grafo mergeado.
  - **`FullGraphStrategy::build_full_graph`**: rewrite a dos pasadas:
    (a) construir `GlobalSymbolIndex`, (b) poblar el petgraph y
    resolver aristas con el índice.

**Reglas de resolución (scope-aware)**:

  - 1 candidato                            → se usa.
  - Múltiples en el archivo del caller    → se usa el local.
  - Múltiples en archivos distintos del
    mismo crate root                       → se usa ese.
  - Otro caso (ambiguo)                   → `None`. La arista se
                                              descarta HONESTAMENTE;
                                              no se inventa.

**Tests añadidos** (RED → GREEN):

  - `w5_cross_file_call_edge_resolves_to_correct_symbol` (per_file)
  - `w5_cross_file_call_edge_also_present_in_full` (full)
  - `w5_intra_file_duplicate_does_not_invent_cross_file_edges`
  - `w5_compute_overload_no_call_site_yields_no_invented_edges`
  - `global_index_tests` × 4 (reglas del resolver a nivel unitario)

**Verificación observada**:

  - `cargo test -p cognicode-core --lib` →
    **2109 passed, 0 failed, 27 ignored** (baseline 2101/0/27 → +8
    tests, 4 w5 + 4 global_index_tests).
  - `cargo check --workspace --all-targets` → clean.
  - `cargo clippy -p cognicode-core --all-targets` → sólo warnings
    preexistentes; ninguno introducido por este cambio.
  - 4 fallos observados en el run de workspace son preexistentes
    (verificados con `git stash` + rerun; pertenecen a
    `cognicode-cli` y `cognicode-ladybug`, no a esta superficie).

**API/contrato**: las firmas públicas (`merge`, `merge_with_report`,
`build_full_graph`, `build_full_graph_report`, `get_or_build`,
`build_local_graph`) se mantienen. Los nuevos `BuildFileResult` y
`CrossFileEdge` son alias de tipo públicos. `GlobalSymbolIndex` es
`pub` para que herramientas de diagnóstico futuras (F3/C3
observabilidad) puedan auditar ambigüedad sin tocar el resolver.

**Commit**: `3f27a31d` (atómico, sin push).

**Decisiones registradas** (D32-D33, JOURNAL §14):

  - D32: cross-file edges se difieren a buffer y se reconcilian
    **después** de poblar todos los símbolos del proyecto, en lugar
    de encolar contra el grafo parcial por archivo.
  - D33: cuando la resolución es genuinamente ambigua, la arista
    se descarta en vez de inventar el enlace. Política explícita
    del operador ("un enlace inventado hacia un símbolo homónimo es
    tan incorrecto como una relación perdida").

**Próxima unidad concreta**: **F2.W6** (subdivisión TBD). El
alcance natural siguiente es preparar el workaround **F2.W0-bis**
para desbloquear F2.W8 (UAT de binarios): el workspace tiene dos
crates `name = "cognicode"` y `cargo install`/`cargo run` se
quejan. Alternativamente, una unidad de cierre del propio F2 (W7:
auditoría de los 4 fallos preexistentes del workspace, con
responsable y trigger por cada uno). Decisión del operador al
llegar a F2.W6.

## Última unidad cerrada: F2.W3 (Equivalencia full vs per_file — R4)

**Objetivo**: caracterizar — sin forzar equivalencia — las
divergencias legítimas y los bugs reales entre
`FullGraphStrategy::build_full_graph` y
`PerFileStrategy::build_full_graph` / `build_full_graph_report`,
sobre un corpus que cubre 9 escenarios representativos.

**Decisión de diseño**: las dos estrategias tienen propósitos
distintos. Forzar equivalencia bit-a-bit (p. ej. mismo orden de
inserción) no aporta valor y obligaría a reescribir una de las dos.
F2.W3 marca el **estado actual** con tests de regresión que fallarían
si ese estado cambia silenciosamente.

**Corpus nuevo** (`docs/prf/fixtures/equivalence_full_vs_perfile/`,
versionado):

- `CORPUS.md` — declara el oráculo por escenario.
- `src/lib.rs` — `hello`, `shared`, `caller → crate::nested::callee()`, `compute(x: u32)`.
- `src/dup.rs` — `pub fn same_name` en root y dentro de `pub mod inner`.
- `src/empty.rs` — archivo vacío.
- `src/comments_only.rs` — solo comentarios, ningún símbolo.
- `src/broken.rs` — `pub fn oops(` sin cierre (sintaxis rota).
- `src/nested/mod.rs` — `shared`, `callee`, `compute(s: &str)`.
- `src/nested/deeply/deep.rs` — `deep_symbol` (2 niveles de profundidad).

**Tests** (5 nuevos, todos GREEN, RED confirmado):

| Test | Verifica |
|---|---|
| `w3_full_and_per_file_discover_same_symbol_set` | Mismo `HashSet` de `fully_qualified_name` en ambas estrategias |
| `w3_corpus_has_expected_symbol_inventory` | Conteos exactos por nombre + total = 10. **RED verificado**: añadir un símbolo sneaky → count=11 → falla → restaurar → GREEN |
| `w3_full_and_per_file_agree_on_per_name_counts` | Multiplicidad por nombre (e.g. `shared=2`, `compute=2`) coincide |
| `w3_per_file_report_marks_broken_syntax_as_skipped` | `broken.rs` aparece en `SkippedFile` con `SkipReason::Parse` |
| `w3_full_strategy_silently_ignores_broken_syntax_today` | Pinear el bug equivalente a R3 en `FullGraphStrategy` (scope F2.W4) |

**Hallazgo emergente** (no es bug a corregir aquí, queda en bitácora
como **H-R4-1**): ambas estrategias devuelven **0 edges** sobre el
corpus, pese a tener `caller → callee` y relaciones implícitas.
Esto sugiere que `find_call_relationships` (vía tree-sitter) no
está capturando las llamadas cross-file en este corpus concreto.
**No es scope de F2.W3** (que es caracterización de equivalencia,
no feature work); queda registrado para investigarse en una unidad
posterior. **No se documenta como bug certificado** sin UAT previo;
los 5 tests de caracterización pasan en el estado actual.

**Verificación ejecutada**:

- `cargo test -p cognicode-core --lib w3_equivalence_tests` → **5/5 pass**.
- `cargo test -p cognicode-core --lib` → **2096/0/27** (baseline F2.W2
  era 2091/0/27, **+5 tests** sin regresión).
- RED confirmado: el assert de total=10 falla con 11 símbolos; pasa
  con 10. Test real, no tautología.

**Composición de commits**:

```
d9aa09c0 test(strategy): characterize full vs per_file equivalence over F2.W3 corpus (R4)
55eddd4e test(cli): add UAT coverage for per-file-graph R3 fix and stop swallowing Graph errors
be729275 fix(per-file-graph): report skipped files instead of silent failures (R3)
```

**Certificación**: F2.W3 = **ACCEPTED** (caracterización; sin fix).

**Política respetada**:

- Sin cambio de código de producción (solo se añadieron tests + corpus).
- Sin modificar APIs públicas ni traits.
- Sin expandir scope: el H-R4-1 queda registrado, no se corrige aquí.

**Próxima unidad concreta**: **F2.W4 — Cerrar huecos**, que
abordará:

1. **H-R4-1**: investigar por qué `find_call_relationships` no
   captura edges cross-file sobre el corpus. Decidir si es bug o
   limitación del parser; si bug, arreglarlo con test de regresión.
2. **R3-style bug en `FullGraphStrategy`**: aplicar el mismo patrón
   de F2.W2 a `FullGraphStrategy::build_full_graph` (los `_ =>
   continue` en cada `match` tragan errores; tras F2.W4 deben
   reportarse).
3. **mtime-preserved content change test**: ampliar el corpus F2.W1
   con un test que reescribe un archivo preservando mtime (vía
   `utimensat`/`filetime`); verificar que el cache invalida
   igualmente. Sin esto, el fingerprint tiene un agujero: si una
   herramienta externa restaura mtime al contenido viejo, el cache
   devuelve stale.
4. **Migración de los 7 call sites CLI** de
   `build_full_graph` a `build_full_graph_report` (registrado en
   F2.W2 como followup). Pospuesto hasta tener un consumidor real
   que necesite los `SkippedFile`s, o hasta F2.W4 si el UAT CLI lo
   demanda.
5. **UAT CLI/MCP real**: ejecutar `cognicode graph per-file` y la
   tool `get_per_file_graph` sobre el corpus, capturar stderr y
   exit code. Trabajar alrededor del bug preexistente del binario
   `cognicode` (dos crates con mismo `name`).

## Última unidad cerrada: F2.W2 (Errores de lectura silenciosos en PerFileStrategy)

**Objetivo**: cerrar R3 — el `walkdir` silencioso y el `merge` que ignoraba
errores de parseo en `PerFileStrategy::build_full_graph` y `merge`,
haciendo que el análisis pareciera completo cuando en realidad había
fallos no reportados.

**Defectos diagnosticados** (commit `be729275`):

1. `PerFileStrategy::build_full_graph` (en
   `crates/cognicode-core/src/infrastructure/graph/strategy.rs`):
   `walkdir` filtraba con `filter_map(|e| e.ok())`, descartando errores
   de I/O (permisos, ENOENT transitorios).
2. `PerFileGraphCache::merge` (en
   `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs`):
   `unwrap_or_else(|_| CallGraph::new())` colapsaba fallos de read/parse
   en un grafo vacío, sumándolos al grafo principal sin advertencia.
3. El parser tree-sitter es **error-tolerant**: devuelve una lista de
   símbolos vacía ante sintaxis rota. Sin detección explícita de nodos
   `ERROR` con `TreeSitterParser::has_error_nodes`, un archivo con `)`
   faltante o un `pub fn broken_fn(` queda indistinguible de un archivo
   vacío.

**Cambios mínimos**:

- Tipos nuevos en `per_file_graph.rs`:
  - `SkipReason::{ Read(String), Parse(String), UnsupportedExtension(String), Other(String) }`
  - `SkippedFile { path, reason }`
  - `BuildStatus::{ Complete, Partial { skipped: Vec<SkippedFile> } }`
  - `BuildReport { graph: CallGraph, status: BuildStatus }`
- Helper privado `classify_io_error(err) -> SkipReason` mapea
  `io::ErrorKind::*` a las 4 categorías.
- `merge_with_report()` (nuevo, preserva `merge()` viejo para no romper
  consumidores).
- `PerFileStrategy::build_full_graph_report()` (nuevo, no toca el trait).
- `build_file_graph` parsea una vez y rechaza con `ErrorKind::InvalidData`
  si `has_error_nodes(&tree)`; `classify_io_error` lo traduce a
  `SkipReason::Parse`.

**Bug CLI colateral descubierto y corregido** (en este mismo commit
F2.W2): el wrapper `CommandExecutor::execute` (en
`crates/cognicode-core/src/interface/cli/commands.rs`) **tragaba el
`Err`** de `execute_graph` con `if let Err(e) = … { eprintln!(…) }` y
retornaba `Ok(())`. Si no se hubiese añadido el UAT de CLI (ver abajo),
este defecto habría llegado a v1.0: el binario imprimía "Error building
per-file graph: …" en stderr y exit code 0. Fix: en el brazo `Graph` del
match top-level, `return Err(e);` para que el código de salida refleje
el fallo.

**Tests** (6 nuevos, todos GREEN):

| Test | Verifica |
|---|---|
| `test_merge_with_report_surfaces_parse_error` | `merge_with_report` clasifica `InvalidData` como `SkipReason::Parse` y lo reporta en `BuildStatus::Partial` |
| `test_classify_io_error_read_vs_parse` | Helper `classify_io_error` mapea NotFound/InvalidData correctamente |
| `test_merge_with_report_surfaces_unreadable_file` | chmod 0o000 → `SkipReason::Read` (Unix-only con restore en finally-style manual, sin scopeguard) |
| `uat_cli_graph_per_file_clean_file_succeeds` | `CommandExecutor::execute(Cli{Graph{PerFile{good.rs}}})` retorna `Ok(())` |
| `uat_cli_graph_per_file_broken_syntax_returns_error` | mismo flujo con `broken_syntax.rs` retorna `Err` (parse propagado al exit code) |
| `uat_cli_graph_per_file_missing_file_returns_error` | mismo flujo con archivo inexistente retorna `Err` (ENOENT propagado) |

**Corpus nuevo** (`docs/prf/fixtures/per_file_partial_corpus/`, versionado):

- `CORPUS.md` — describe el oráculo y los 3 escenarios (good / broken_syntax / unsupported).
- `src/good.rs` — `pub fn good_fn() {}`, oráculo de "análisis completo".
- `src/broken_syntax.rs` — `pub fn broken_fn(` (falta `)`), oráculo de "debe saltar con Parse".
- `src/unsupported.txt` — extensión no soportada (no se usa en los tests, se mantiene como documentación del comportamiento).

**Verificaciones ejecutadas**:

- `cargo test -p cognicode-core --lib per_file_graph` → **11/11 pass**.
- `cargo test -p cognicode-core --lib w2_uat_tests` → **3/3 pass**.
- `cargo test -p cognicode-core --lib` → **2091/0/27** (baseline F2.W1
  era 2085/0/27, **+6 tests** sin regresión).
- Manual: revertidos los tres fixes (skip-reporting, CLI swallow,
  has_error_nodes), los 6 tests fallan (RED confirmado); re-aplicados,
  todos pasan (GREEN confirmado).
- API pública de `PerFileGraphCache` preservada: la firma vieja `merge`
  sigue existiendo y delega a `merge_with_report`. Los 7 call sites CLI
  existentes del trait siguen usando `build_full_graph` sin cambios.

**Composición de commits**:

```
be729275 fix(per-file-graph): report skipped files instead of silent failures (R3)
1c91fe10 docs(prf): record F2.W1 closure, ROADMAP, certificate, traceability
70f0b0cf fix(per-file-cache): invalidate entries on content change (R2 from F2.W1)
```

El commit de código+corpus (`be729275`) precede a este commit de docs.
Los UAT tests + fix de CLI swallow van en un commit separado a
continuación para mantener la atomicidad (un commit por concern).

**Certificación**: F2.W2 = **IMPLEMENTED + INTEGRATED + ACCEPTED**.
Detalle en `evidence/CERTIFICATES.md` (cert PRF-F2-W2).

**Política respetada**:

- API pública de `PerFileGraphCache` y `PerFileStrategy` preservada
  (`merge_with_report` se añade, `merge` se conserva).
- Trait `GraphStrategy` no modificado: `build_full_graph_report` es
  método directo de `PerFileStrategy`.
- Sin nuevos módulos, ports, event bus, ni representación alternativa.
- Sin expansión de scope: la corrección del bug CLI es aditiva a R3
  (mismo patrón "silent failure"), no una refactorización del wrapper.

**Próxima unidad concreta**: **F2.W3 — Equivalencia full vs per_file
(R4)**. Las dos estrategias tienen propósitos distintos (full = una sola
pasada sobre todo el árbol; per_file = cache incremental por archivo);
F2.W3 construirá una matriz de caracterización sin forzar equivalencia
perfecta. Las divergencias legítimas (por ejemplo, full cuenta
archivos ignorados que per_file no) se documentarán como
comportamiento esperado.

## Última unidad cerrada: F2.W1 (Correctitud del análisis — invalidación de cache)

**Objetivo**: caracterizar la correctitud de la vertical `PerFileStrategy`
(`cognicode graph per-file <file>`, `get_per_file_graph` MCP tool) y
cerrar el primer defecto reproducible: el cache no detecta cambios de
contenido entre llamadas.

**Vertical elegida**: `PerFileGraphCache::get_or_build` en
`crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs`.
Justificación: accesible desde el binario real (CLI y MCP), aislada,
con test de regresión natural.

**Riesgos catalogados** (de los 4 descritos en el brief):

| ID | Riesgo | Estado |
|---|---|---|
| R1 | Recorrido anidado (subdirectorios) | Caracterizado en F2.W1 — test `test_per_file_strategy_build_full_graph_nested_corpus` PASS (≥3 símbolos sobre corpus de 3 archivos anidados) |
| R2 | Detección de cambios de contenido | **CLOSED en F2.W1** — fix mínimo con fingerprint (mtime+size), 2 tests nuevos GREEN |
| R3 | Errores de lectura silenciosos | Diferido a **F2.W2** (defecto real: `PerFileStrategy::build_full_graph` usa `filter_map(\|e\| e.ok())` y `merge` usa `unwrap_or_else(\|_\| CallGraph::new())`; cobertura insuficiente se reporta como éxito) |
| R4 | Equivalencia `full` vs `per_file` | Diferido a **F2.W3** (caracterización sin corrección; las dos estrategias tienen propósitos distintos) |

**Causa raíz (R2)**: `get_or_build` solo consultaba `entry.valid`; nunca
preguntó a `fs::metadata` por el estado del archivo. Resultado: después de
editar un archivo, el cache devolvía el grafo viejo.

**Fix mínimo**:
- `FileGraphCacheEntry` gana dos campos: `mtime_secs: Option<u64>` y
  `size: Option<u64>`.
- Helpers privados `file_fingerprint(path)` y `system_time_to_secs(t)`.
- `get_or_build` ahora comprueba `valid && mtime == fp.mtime && size == fp.size`.
- Si `fs::metadata` falla (archivo borrado, etc.), se reconstruye
  conservadoramente.

**Tests**:
- `test_per_file_graph_cache_detects_content_change` (RED → GREEN):
  escribe 1 función, cachea, reescribe con 2 funciones tras 1.1s (para
  superar granularidad de mtime), re-pide. Sin fix: 1 símbolo (stale).
  Con fix: >1 símbolo (nuevo).
- `test_per_file_strategy_build_full_graph_nested_corpus` (GREEN desde
  inicio): ejercita `PerFileStrategy::build_full_graph` sobre el corpus
  `docs/prf/fixtures/per_file_correctness/` y verifica ≥3 símbolos en
  3 archivos anidados (2 niveles).

**Corpus nuevo** (versionado en este commit):

- `docs/prf/fixtures/per_file_correctness/CORPUS.md` — describe el
  oráculo (3 funciones, 2 edges) y por qué es independiente de la
  implementación.
- `docs/prf/fixtures/per_file_correctness/src/lib.rs` — `top_level`.
- `docs/prf/fixtures/per_file_correctness/src/nested/mod.rs` — `mid_level`.
- `docs/prf/fixtures/per_file_correctness/src/nested/deeply_nested/mod.rs` — `leaf`.

**Verificaciones ejecutadas**:

- `cargo test -p cognicode-core --lib per_file_graph` → **8/8 pass**.
- `cargo test -p cognicode-core --lib` → **2085/0/27** (baseline F0.W3
  era 2083/0/27, **+2 tests** sin regresión).
- `cargo test -p cognicode-cli --bin cogh` → 277/13/1 + 1 filtered. Los
  **13 fallos son preexistentes** (verificado con `git stash`): el
  workspace tiene dos crates con `name = "cognicode"` y cargo no
  produce `target/debug/cognicode`, lo que rompe los tests que lanzan
  ese binario como subproceso. **No es regresión de F2.W1**.
- Manual: revertido el fix del cache localmente, el nuevo test falla
  (RED confirmado); re-aplicado, pasa (GREEN confirmado).
- UAT sobre el código real: el test runner de `cognicode-core` ejecuta
  el código real de `PerFileStrategy::build_full_graph` y
  `PerFileGraphCache::get_or_build` sobre el corpus, no un mock.

**Composición del commit**:

```
70f0b0cf fix(per-file-cache): invalidate entries on content change (R2 from F2.W1)
05ba121e docs(prf): reconcile F1 closure and version the PRF program documents
9628b1d3 fix(prf-f1.w3): replace placeholder sha256 in bundled plugin manifests (H8)
```

**Certificación**: F2.W1 = **IMPLEMENTED + INTEGRATED + ACCEPTED**.
Detalle en `evidence/CERTIFICATES.md` (cert PRF-F2-W1).

**Política respetada**:

- No se cambió la API pública de `PerFileGraphCache` (mismas firmas).
- No se introdujeron nuevos módulos, ports, event bus ni representaciones
  alternativas.
- No se rebajó el oráculo: el test exige `new > original` con mensaje de
  error explícito.
- El fix es mínimo (~50 líneas de código nuevo) y se aplica solo donde
  es necesario.

**Próxima unidad concreta**: **F2.W2 — Errores de lectura silenciosos en
PerFileStrategy::build_full_graph (R3)**. Defecto a corregir: cuando un
archivo no se puede parsear, el merge continúa silenciosamente con un
grafo vacío para ese archivo, llevando a una conclusión falsa de
"análisis completo". Solución: cambiar el contrato de `build_full_graph`
para que devuelva un tipo que incluya tanto el grafo como la lista de
archivos omitidos, y documentar el comportamiento en UAT.

## Hito F2 (Correctitud reproducible) — EN CURSO (cierre admin previo revocado en §Reconciliación)

| Unidad | Estado |
|---|---|
| F2.W1 — Invalidación de cache por cambio de contenido (R2) | **IMPLEMENTED** (commit `70f0b0cf`); library test GREEN; UAT de binario real cerrado → F2.W8 |
| F2.W2 — Errores de lectura silenciosos (R3, capa `per_file_graph`) | **IMPLEMENTED** (commits `be729275`, `55eddd4e`); library test + 3 UAT CLI a nivel wrapper |
| F2.W3 — Caracterización equivalencia full vs per_file (R4) | **ACCEPTED** (commit `d9aa09c0`); sin fix (caracterización, no feature) |
| F2.W4 — Cerrar H-R4-1 capa 1 (parser) | **ACCEPTED-parcial** (commit `084b5c00`); capa 2 (H-R4-2) registrada como OPEN, **scope de F2.W5** |
| F2.W5 — Resolver H-R4-2 (lookup global) | **IMPLEMENTED** (commit `3f27a31d`); library test GREEN (4 w5 + 4 global_index_tests) |
| F2.W6 — Desbloqueo binario | **CERRADO-SIN-ACCION** (verificación: `cargo install --path crates/cognicode-cli --bin cognicode` + `cargo run --bin cognicode` + `cognicode-mcp --cwd <dir>` funcionan; el conflicto de `crates/cognicode/` no bloquea operativamente) |
| F2.W7 — Integrar F2.W5 en el camino real del binario | **IMPLEMENTED** (commit `5ce8eb1e`); `analysis_service::build_project_graph` ahora usa `GlobalSymbolIndex` con caller_file context; UAT real con `cognicode-mcp` muestra `relationships_found: 4` correcto |
| **F2.W8 — Errores silenciosos en `build_project_graph` (R3, capa `analysis_service`)** | **IMPLEMENTED** (commit próximo); 4 fuentes de error silencioso corregidas; `AnalysisService::get_last_build_report()` enumera archivos omitidos con razón clasificada; handler MCP `build_graph` los surface como `skipped_files[]`; UAT real con `chmod 000` + UTF-8 inválido confirma enumeración |
| F2.W9 — mtime preservado (cambio de bytes con mtime conservado invalida cache) | IMPLEMENTED (commit `2a121aec`, UAT real GREEN) |
| F2.W10 — Equivalencia y reproducibilidad (full ↔ per_file con misma entrada determinista) | IMPLEMENTED (commit `dc189d54`; 3 tests de pineo, sin cambio de producción) |
| **F2 (hito)** | EN CURSO. W1-W10 IMPLEMENTED. Pendiente: gate C2. **C2 = NO CERTIFICADO**. |

## Hito F1 (Estabilización) → CERRADO (referencia histórica)

| Unidad | Estado |
|---|---|
| F1.W1 — Stdout/signals (H6+H9) | ACCEPTED (commit 834aff67) |
| F1.W2 — Cifra 68 → 75 (H2+H7) | ACCEPTED (commit 4ff514a7) |
| F1.W3 — sha256 reales (H8) | ACCEPTED (commit 9628b1d3) |
| F1.W4 — Mock GitHub API (H10) | WIP → migrado a deuda de F2 |
| F1.W5 — Decisiones UX (H3+H4) | ACCEPTED (solo docs) |

**Hito F1 cerrado**. Detalle completo en `JOURNAL.md` §6.

## Hito F0 (Inventario y baseline) → CERRADO (referencia histórica)

| Unidad | Estado |
|---|---|
| F0.W1 — Inventario de binarios | ACCEPTED |
| F0.W2 — Caracterización arranque/persistencia/red | ACCEPTED |
| F0.W3 — Baseline de pruebas automatizadas | ACCEPTED |

**Hito F0 cerrado**. Detalle completo en `evidence/F0-W2-runtime.md` y
`evidence/F0-W3-baseline.md`.

## Estado de certificaciones

Ver `evidence/CERTIFICATES.md`.

## Próxima unidad a abrir (F2.W9 — mtime preservado)

**F2.W9 — mtime preservado.** Caracterización y corrección del
defecto: cuando un archivo del corpus conserva su `mtime` pero sus
bytes cambian, la invalidación del cache actual (que mira sólo
`mtime`) no dispara re-parseo. Esto es relevante porque editores y
herramientas de refactor suelen preservar `mtime` al escribir.

**Plan**:
1. Crear un corpus UAT con un archivo cuyo contenido cambie pero
   cuyo `mtime` se conserve (vía `utimes`/`filetime`).
2. Caracterizar RED con test de integración: build → mutate
   content → set mtime al valor original → build → esperar
   símbolos NUEVOS reflejados.
3. Diagnosticar si el camino real del binario
   (`AnalysisService::build_project_graph` → `WalkBuilder`) usa
   `mtime` o `hash` para invalidación.
4. Decidir entre (a) hashing de contenido + comparación con cache
   o (b) documentar el comportamiento y exigir mtime actualizado.
5. Implementar, RED → GREEN, UAT real con binario.

## Cierre de F2.W8 (Errores silenciosos en `build_project_graph`)

**Objetivo**: cerrar R3 en el camino real del binario. F2.W2 ya
había atacado errores silenciosos a nivel de `per_file_graph`,
pero la fuga silenciosa real estaba en
`analysis_service::build_project_graph` (el path que ejecuta el
binario). 4 fuentes de error silencioso corregidas:

1. `std::fs::read_to_string(&path).ok()?` → captura `io::Error`,
   clasifica por `ErrorKind` y emite `SkipReason::Read` o `Other`.
2. `TreeSitterParser::with_cache(language).ok()?` → captura
   `ParseError`, emite `SkipReason::Parse`.
3. `find_all_symbols_with_path(...).unwrap_or_default()` →
   `unwrap_or_default()` (mantiene el comportamiento de "símbolo
   vacío" pero no esconde el fallo; el archivo pasa al
   `BuildReport` si su mtime lo marca para re-parse).
4. `find_call_relationships(...).unwrap_or_default()` →
   análogamente.

**API pública añadida**: `AnalysisService::get_last_build_report() ->
Option<BuildReport>`. `BuildReport = { graph, status }` donde
`status` es `BuildStatus::Complete` (todo OK) o
`BuildStatus::Partial { skipped: Vec<SkippedFile> }`. Cada
`SkippedFile` lleva `path` + `SkipReason` (clasificado).

**Surface MCP**: `handle_build_graph` ahora retorna un campo
`skipped_files: Option<Vec<SkippedFileDto>>` que se omite del JSON
cuando el grafo viene del cache y se popula con la lista
clasificada cuando hay un walk real. Cada DTO lleva `path`,
`reason_kind ∈ {read, parse, unsupported_extension, other}` y
`reason` con el mensaje textual del error.

**Tests añadidos** (`w8_silent_errors_tests`):
- `w8_unreadable_file_is_silently_dropped` — corpus con 3 archivos
  `.rs`, uno con `chmod 000`, uno con bytes UTF-8 inválidos;
  `coverage.parsed_files == 1` (sólo `ok.rs` cuenta).
- `w8_invalid_utf8_file_is_silently_dropped` — comprueba que
  `coverage_percent < 100%` con 2 de 3 archivos corruptos.
- `w8_build_report_enumerates_skipped_files` — pinea la API
  pública y la semántica `Complete` con corpus válido.

**UAT real con binario fresh** (`/var/home/rubentxu/cargo-targets/
release/cognicode-mcp`, rebuilt tras commit):
```
{"jsonrpc":"2.0","id":2,...}
  → "skipped_files":[
       {"path":"/tmp/prf-uat-w8/src/unreadable.rs",
        "reason_kind":"read",
        "reason":"Permission denied (os error 13)"},
       {"path":"/tmp/prf-uat-w8/src/invalid_utf8.rs",
        "reason_kind":"parse",
        "reason":"stream did not contain valid UTF-8"}
     ]
```

**Resultado suite**: `cargo test -p cognicode-core --lib --no-fail-fast`
→ `2118 passed; 0 failed; 27 ignored` (3 tests nuevos w8_*;
sin regresiones).

**Resultado clippy**: 4 errores preexistentes del workspace
(auditados en D34), 0 nuevos.

**Resultado fmt**: mis 2 archivos (`analysis_service.rs` +
`handlers/mod.rs`) están fmt-clean; los 13 diffs preexistentes de
`fmt --check` en otros archivos NO fueron tocados.

**Decisión D35**: Errores de lectura/parseo se reportan como
datos del build (en `BuildReport`), no como excepciones.
Justificación: AGENTS.md §5 ("no nuevas abstracciones si los
mecanismos existentes pueden satisfacer el requisito") +
reutilización de `BuildReport`/`SkippedFile`/`SkipReason` que
ya existían en `infrastructure/graph/per_file_graph.rs` desde
F2.W2.

## Última unidad cerrada: F2.W10 (equivalencia de aristas y reproducibilidad)

F2.W3 pineó equivalencia de símbolos cuando ambas estrategias
devolvían 0 aristas (0=0 trivial). Desde F2.W5/W7 las aristas
cross-file existen y había que pinear el nuevo estado.

**Tests (3, commit `dc189d54`)** sobre el corpus determinista de
F2.W3 (`docs/prf/fixtures/equivalence_full_vs_perfile/`):
1. `w10_full_and_per_file_agree_on_edge_set` — full y per_file
   producen el mismo conjunto (caller fqn, callee fqn). Verificado
   con probe que el conjunto NO es vacío ni el test trivial: hay
   exactamente 1 arista (`lib.rs:caller:16 → nested/mod.rs:callee:11`)
   idéntica en ambas estrategias.
2. `w10_edge_set_is_non_empty_on_cross_file_corpus` — una regresión
   a 0 aristas reabriría H-R4-1.
3. `w10_repeated_builds_are_reproducible` — dos builds consecutivos
   por estrategia producen símbolos y aristas idénticas.

**Sin cambio de código de producción**: es caracterización pineada
(comportamiento actual correcto tras W5/W7).

**Evidencia**: `cargo test -p cognicode-core --lib` →
`2122 passed; 0 failed; 27 ignored`. Clippy sin errores nuevos.

## Cierre previo: F2.W9 (mtime preservado — invalidación de cache)

**Defecto**: `file_cache` de `AnalysisService` claveaba entradas solo por
mtime. Un editor que reescribe bytes preservando el mtime (común en
refactors) obtenía símbolos obsoletos del cache.

**Cambio**: la entrada de cache pasa a `(mtime, size, symbols,
relationships)`; el cache-hit exige coincidencia de mtime Y size. Se
actualizaron los tres caminos de construcción (`build_project_graph`,
`build_graph_per_file` y el async), emitiendo `size` desde el walk.

**Limitación documentada**: una reescritura con mismo tamaño Y mismo
mtime sigue sin detectarse. Detectarla requeriría hash de contenido;
queda como deuda explícita, no como silencio.

**Tests**: `w9_mtime_tests::w9_content_change_with_preserved_mtime_
invalidates_cache` RED→GREEN. Suite completa `2119 passed; 0 failed;
27 ignored`. Clippy: solo los errores preexistentes catalogados (D34),
0 nuevos.

**UAT real** (binario `/var/home/rubentxu/cargo-targets/release/
cognicode-mcp` rebuilt tras el cambio, corpus `/tmp/prf-uat-w9`):
build #1 ve `original_function`; se reescribe `lib.rs` renombrando a
`renamed_function` y restaurando el mtime vía `os.utime` (verificado:
mtime preservado True); build #2 (proceso nuevo) muestra
`renamed_function` y ya no muestra `original_function`.

## Notas sobre la bootstrap de PRF

Este programa PRF se inicializa en esta sesión. La estructura `docs/prf/`
no existía previamente. Los 9 documentos base (README, ROADMAP,
CERTIFICATION, UAT, TEST-PLAN, TRACEABILITY, STATE, JOURNAL,
evidence/CERTIFICATES) se crearon como primer paso del trabajo de F0.W1.

Justificación documentada en JOURNAL.md, primera entrada.
