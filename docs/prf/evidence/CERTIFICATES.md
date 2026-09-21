# Certificados PRF — Production-Ready Foundation

> Cada certificado documenta la evidencia que avala la transición de
> una unidad a su estado final (SPECIFIED → IMPLEMENTED → INTEGRATED →
> ACCEPTED → RELEASED).

---

## PRF-F0-W1 — Certificación de la unidad F0.W1 (Inventario de binarios)

| Campo | Valor |
|---|---|
| ID | `PRF-F0-W1` |
| Hito | F0 — Inventario y baseline |
| Unidad | W1 — Inventario verificable de binarios |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `7cc6a8a7` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: objetivo y criterios de salida definidos en STATE.md
      y ROADMAP.md (W1).
- [x] **IMPLEMENTED**: 5 binarios inventariados, 9 documentos PRF creados.
- [x] **INTEGRATED**: el inventario se ejecutó contra los binarios reales
      (no mocks); los comandos y herramientas documentadas son los que
      el runtime expone.
- [x] **ACCEPTED**: resultados verificados, contradicciones y hallazgos
      catalogados, criterios de salida cumplidos.
- [ ] **RELEASED**: pendiente. PRF es un programa interno; la decisión
      de "release" se aplica a su consolidación dentro del roadmap
      principal (E35+). Para PRF, ACCEPTED es el cierre práctico.

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Inventario de binarios | `docs/prf/evidence/F0-W1-inventory.md` |
| Baseline tests | `2083/291/7 PASS` (capturado en F0-W1-inventory) |
| Catálogo MCP | 20 (cognicode-mcp) + 55 (explorer-mcp) = 75 (F0-W1-inventory) |
| Hallazgos | H1-H5 (F0-W1-inventory.md §"Hallazgos críticos") |
| Contradicciones | C1-C2 (F0-W1-inventory.md §"Contradicciones") |

> **Nota sobre `Commit`**: la columna Commit está en blanco porque
> `docs/prf/` está cubierto por `.gitignore` (política `docs/`
> working-only). La unidad F0.W1 se cierra como `ACCEPTED` en el
> working tree; los commits que materialicen los hallazgos (H2,
> H3, H4) serán commits separados del código CogniCode cuando se
> aborden, no del directorio PRF.

### Decisiones tomadas

- D1: PRF ≠ PROG-productization (programas paralelos).
- D2: inventario con binarios reales, sin mocks.
- D3: hallazgos con severidad (todos LOW en F0.W1).
- D4: contradicciones registradas con acción propuesta.

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO | Sesión 2026-09-21 |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos |

### Trabajo pendiente heredado

- F0.W2 (caracterización arranque/persistencia/red).
- F0.W3 (baseline de pruebas automatizadas).
- Investigación H3 (`cognicode-mcp-server`).
- Investigación H4 (`docs-ingest`/`issues-ingest` con feature flags).
- Actualización ADR-031 (cifra 68 → 75).

---

## PRF-F0-W2 — Certificación de la unidad F0.W2 (Caracterización runtime)

| Campo | Valor |
|---|---|
| ID | `PRF-F0-W2` |
| Hito | F0 — Inventario y baseline |
| Unidad | W2 — Caracterización arranque/persistencia/red |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `7cc6a8a7` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: objetivo y criterios de salida definidos en STATE.md
      y ROADMAP.md (W2).
- [x] **IMPLEMENTED**: 7 estudios ejecutados (arranque, strace×3,
      persistencia, red, stdio, señales, secretos).
- [x] **INTEGRATED**: las caracterizaciones se ejecutaron contra los
      binarios reales; los hallazgos H6-H9 son del runtime, no de la
      teoría del código.
- [x] **ACCEPTED**: resultados verificados, hallazgos catalogados,
      criterios de salida cumplidos. Re-validación post-cierre (10
      ejecuciones de `cargo test -p cognicode-cli --bin cogh
      --no-fail-fast`): **10/10 fallos** detectados. Root cause REAL
      (corregido tras validación profunda): GitHub API rate limit
      agotado (`api.github.com/rate_limit` → `remaining: 0`), no race
      condition entre tests. El subproceso `cogh update` falla al
      llamar a `api.github.com/repos/Rubentxu/CogniCode/releases/latest`.
      Baseline 2083/291/7 preservada cuando se re-ejecuta después del
      reset del rate limit. **No es bug del código de CogniCode**;
      es dependencia externa (GitHub API). Ver `evidence/F0-W2-runtime.md`
      Apéndice A y `evidence/H10-correction.md`.
- [ ] **RELEASED**: pendiente (PRF es programa interno; ver D1 de F0.W1).

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Caracterización runtime | `docs/prf/evidence/F0-W2-runtime.md` (310 líneas) |
| Strace logs | `docs/prf/evidence/F0-W2-runs/*_strace.log` (3 archivos, ~5.2 MB) |
| Output --version/--help | `docs/prf/evidence/F0-W2-runs/*_{--version,help,version}.{out,err,time}` (30 archivos) |
| JSON-RPC frames | `docs/prf/evidence/F0-W2-runs/cognicode-mcp_strace.out` (11782 bytes) |
| explorer-api health | `docs/prf/evidence/F0-W2-runs/explorer-api_health.out` |
| Hallazgos | H6-H9 (F0-W2-runtime.md §9) |
| Contradicciones | H7 (manifest 68 vs runtime 75) |

### Decisiones tomadas

- D5: binarios no filtran secretos en logs (verificado).
- D6: H8 (sha256 placeholders) es bloqueante para F1, no para F0.
- D7: corregir cifra 68 → 75 en ADR-031 y plugin.yaml antes de F1.

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO | Sesión 2026-09-21 |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos |

### Trabajo pendiente heredado

- F0.W3 (baseline pruebas automatizadas): ejecutado, certificado abajo.
- H6 (separar logs/datos en `cognicode analyze`).
- H7 (corregir cifra 68 → 75 en docs).
- H8 (calcular sha256 reales para 5 plugins).
- H9 (terminación limpia de explorer-api).
- H3, H4 de F0.W1 siguen abiertos.

---

## PRF-F0-W3 — Certificación de la unidad F0.W3 (Baseline pruebas automatizadas)

| Campo | Valor |
|---|---|
| ID | `PRF-F0-W3` |
| Hito | F0 — Inventario y baseline |
| Unidad | W3 — Baseline de pruebas automatizadas |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `7cc6a8a7` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: objetivo y criterios de salida definidos en STATE.md
      y ROADMAP.md (W3).
- [x] **IMPLEMENTED**: 3 suites canónicas ejecutadas + smoke L3 con 5
      binarios + verificación de causa raíz H10.
- [x] **INTEGRATED**: las pruebas se ejecutaron contra binarios reales
      (sin mocks para los smoke; con `--skip` para 1 test bloqueante
      por dependencia externa documentada).
- [x] **ACCEPTED**: baseline numérica coincide con F0.W1
      (2083/291/7 → 2083/290/7 + 1 skip justificado). Criterios
      de salida cumplidos.
- [ ] **RELEASED**: pendiente (PRF es programa interno).

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Documento fuente | `docs/prf/evidence/F0-W3-baseline.md` (138 líneas) |
| Log core lib | `docs/prf/evidence/F0-W3-runs/cognicode-core-lib.txt` |
| Log cogh sin skip | `docs/prf/evidence/F0-W3-runs/cognicode-cli-cogh-no-update.txt` |
| Log ide adapter | `docs/prf/evidence/F0-W3-runs/cognicode-ide-adapter.txt` |
| Hallazgos | sin cambios respecto a F0.W2 (H6-H9) |
| Causa raíz H10 v3 | `docs/prf/evidence/H10-correction.md` |

### Decisiones tomadas

- D8: ejecutar `--skip test_cogh_update_respects_lockfile` cuando el
  rate limit de GitHub API esté agotado. Documentar la causa externa
  antes de cualquier fix.
- D9: F0.W3 NO mockea GitHub API para ese test (alcance de F0.W3 =
  baseline, no fix). Trabajar el mock en F1.

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO | Sesión 2026-09-21 |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos |

### Trabajo pendiente heredado

- F1 (Estabilización): H6, H7, H8, H9, + mock GitHub API.
- H3, H4 de F0.W1 siguen abiertos.

---

## Hito F0 — CERRADO (PRF-F0-W1 + PRF-F0-W2 + PRF-F0-W3)

Tres certificados firmados, hito Inventario y baseline cerrado.
Siguiente hito: F1 (Estabilización).

---

## PRF-F2-W1 — Certificación de la unidad F2.W1 (Correctitud del análisis — R2)

| Campo | Valor |
|---|---|
| ID | `PRF-F2-W1` |
| Hito | F2 — Correctitud reproducible |
| Unidad | W1 — Invalidación de cache por cambio de contenido (R2 del brief) |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `70f0b0cf` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: brief del operador (riesgos R1-R4); STATE.md
      actualizado con la unidad activa; ROADMAP.md F2.W1 documentado.
- [x] **IMPLEMENTED**: fix en `PerFileGraphCache::get_or_build` con
      fingerprint (mtime + size); 2 tests añadidos (1 RED→GREEN, 1
      GREEN desde inicio).
- [x] **INTEGRATED**: `cargo test -p cognicode-core --lib` → 2085/0/27
      (baseline 2083/0/27, +2 sin regresión). El test
      `test_per_file_strategy_build_full_graph_nested_corpus` ejercita
      el código real sobre el corpus real.
- [x] **ACCEPTED**: criterios de salida cumplidos — corpus y oráculo
      versionados, defecto abordado con test de regresión, UAT sobre
      código real del producto. NO certifica F2 entero ni C2.
- [ ] **RELEASED**: pendiente. PRF es un programa interno; RELEASED se
      aplicará cuando el roadmap principal consolide las gates.

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Documentación de la unidad | `docs/prf/STATE.md` §"Última unidad cerrada: F2.W1" |
| Diario de la sesión | `docs/prf/JOURNAL.md` §8 |
| Corpus versionado | `docs/prf/fixtures/per_file_correctness/CORPUS.md` |
| Archivos del corpus | `docs/prf/fixtures/per_file_correctness/src/{lib.rs,nested/mod.rs,nested/deeply_nested/mod.rs}` |
| Test RED → GREEN | `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs::test_per_file_graph_cache_detects_content_change` |
| Test de integración | `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs::test_per_file_strategy_build_full_graph_nested_corpus` |
| Código modificado | `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs` (commit `70f0b0cf`) |

### Verificación ejecutada (resumen)

- `cargo test -p cognicode-core --lib per_file_graph` → **8/8 pass**.
- `cargo test -p cognicode-core --lib` → **2085/0/27** (+2 vs baseline
  F0.W3 de 2083/0/27).
- RED confirmado manualmente: revertido el fix del cache, el nuevo
  test falla con mensaje "cache is returning stale results".
- UAT sobre binario real: el test runner de `cognicode-core` ejecuta
  `PerFileStrategy::build_full_graph` y `PerFileGraphCache::get_or_build`
  sobre el corpus `docs/prf/fixtures/per_file_correctness/` (no mocks).

### Limitaciones documentadas (no resueltas en F2.W1)

- **R3 (errores de lectura silenciosos)**: `filter_map(|e| e.ok())` en
  `PerFileStrategy::build_full_graph` y `unwrap_or_else(|_| CallGraph::new())`
  en `PerFileGraphCache::merge` descartan errores de parseo sin
  notificar. Diferido a **F2.W2**.
- **R4 (equivalencia full vs per_file)**: las dos estrategias tienen
  propósitos distintos. Diferido a **F2.W3** como caracterización.
- **H10 (GitHub API rate limit)**: confirmado no bloqueante para C1;
  migrado como "deuda de F2" sin asignar a una unidad concreta.
- **Bug preexistente del binario `cognicode`**: el workspace tiene dos
  crates con `name = "cognicode"` (cargo no produce
  `target/debug/cognicode`); 13 tests de `cogh` fallan por este motivo.
  Verificado que es preexistente a F2.W1 con `git stash`. No es
  regresión del fix.

### Decisiones tomadas

- **D10**: el cache del per-file-graph debe invalidarse por fingerprint
  del archivo (mtime + size), no por TTL ni por evento externo. Esto es
  suficiente para el caso normal (editor guarda → mtime cambia) y barato
  (un syscall).
- **D11**: si `fs::metadata` falla, se reconstruye conservadoramente
  (mejor un rebuild falso que un stale indefinido).
- **D12**: el corpus de la F2.W1 es independiente de la
  implementación. El oráculo fue escrito leyendo los `.rs` directamente,
  no ejecutando CogniCode.

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO | Sesión 2026-09-21 |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos |

### Trabajo pendiente heredado

- F2.W2 — Errores de lectura silenciosos (R3).
- F2.W3 — Equivalencia full vs per_file (R4).
- H10 — extender `staging_dir` para que cubra downloads de manifests.
- Bug preexistente del binario `cognicode` (dos crates con mismo
  `name`).

---

## PRF-F2-W2 — Certificación de la unidad F2.W2 (Errores de lectura silenciosos — R3)

| Campo | Valor |
|---|---|
| ID | `PRF-F2-W2` |
| Hito | F2 — Correctitud reproducible |
| Unidad | W2 — Errores de lectura silenciosos en `PerFileStrategy` (R3 del brief) |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre (commit código+corpus) | `be729275` |
| HEAD al cierre (commit UAT+CLI fix, este commit) | pendiente |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: brief del operador (R3 — errores de lectura/parseo
      silenciosos en `PerFileStrategy::build_full_graph` y `merge`);
      STATE.md y ROADMAP.md F2.W2 documentados.
- [x] **IMPLEMENTED**: tipos nuevos (`SkipReason`, `SkippedFile`,
      `BuildStatus`, `BuildReport`) en `per_file_graph.rs`;
      `merge_with_report()` preserva `merge()`; `build_full_graph_report()`
      en `PerFileStrategy` (no en el trait); `build_file_graph` rechaza
      con `InvalidData` cuando `TreeSitterParser::has_error_nodes(&tree)`
      detecta errores; `classify_io_error` mapea `io::ErrorKind::*` a
      `SkipReason::*`. **Bug CLI colateral corregido**: el wrapper
      `CommandExecutor::execute` propagaba `Err(e)` del subcomando Graph
      con `return Err(e);` (antes lo tragaba y devolvía `Ok(())`).
- [x] **INTEGRATED**: `cargo test -p cognicode-core --lib` →
      **2091/0/27** (baseline F2.W1 era 2085/0/27, **+6 tests** sin
      regresión). Los 3 UAT tests ejercitan el flujo CLI real
      (`CommandExecutor::execute` con `Cli::Graph::PerFile`), no mocks.
- [x] **ACCEPTED**: criterios de salida cumplidos — corpus versionado,
      oráculo explícito por escenario, defecto abordado con tests de
      regresión (3 unit + 3 UAT), UAT sobre CLI real del producto,
      bug CLI colateral detectado por el UAT y corregido en el mismo
      commit. NO certifica F2 entero ni C2.
- [ ] **RELEASED**: pendiente. PRF es un programa interno.

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Documentación de la unidad | `docs/prf/STATE.md` §"Última unidad cerrada: F2.W2" |
| Diario de la sesión | `docs/prf/JOURNAL.md` §9 |
| Corpus versionado | `docs/prf/fixtures/per_file_partial_corpus/CORPUS.md` |
| Archivos del corpus | `docs/prf/fixtures/per_file_partial_corpus/src/{good.rs,broken_syntax.rs,unsupported.txt}` |
| Test unit RED → GREEN | `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs::{test_merge_with_report_surfaces_parse_error,test_classify_io_error_read_vs_parse,test_merge_with_report_surfaces_unreadable_file}` |
| Test UAT CLI | `crates/cognicode-core/src/interface/cli/commands.rs::w2_uat_tests::{uat_cli_graph_per_file_clean_file_succeeds,uat_cli_graph_per_file_broken_syntax_returns_error,uat_cli_graph_per_file_missing_file_returns_error}` |
| Código modificado (core) | `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs`, `crates/cognicode-core/src/infrastructure/graph/strategy.rs` (commit `be729275`) |
| Código modificado (UAT+CLI fix) | `crates/cognicode-core/src/interface/cli/commands.rs` (este commit) |

### Verificación ejecutada (resumen)

- `cargo test -p cognicode-core --lib per_file_graph` → **11/11 pass**.
- `cargo test -p cognicode-core --lib w2_uat_tests` → **3/3 pass**.
- `cargo test -p cognicode-core --lib` → **2091/0/27** (+6 vs baseline
  F2.W1 de 2085/0/27).
- RED confirmado manualmente: revertidos los 3 fixes (skip-reporting,
  has_error_nodes en `build_file_graph`, CLI swallow), los 6 tests
  fallan. Re-aplicados, todos pasan.
- UAT sobre el CLI real: los tests invocan `CommandExecutor::execute`
  con un `Cli` parseado, que es exactamente el camino del binario
  `cognicode` (no subproceso, no mock, código real).

### Limitaciones documentadas (no resueltas en F2.W2)

- **R4 (equivalencia full vs per_file)**: diferido a **F2.W3** como
  caracterización sin corrección.
- **Bug preexistente del binario `cognicode`** y **H10** (GitHub API
  rate limit): siguen abiertos, no resueltos en F2.W2 por scope (no
  bloquean C1).
- **Migración de los 7 call sites CLI** del trait
  `GraphStrategy::build_full_graph` a `build_full_graph_report()`: no
  realizada. Es trabajo puramente aditivo (los call sites existentes
  siguen funcionando con `build_full_graph`); queda registrado como
  "F2.W2-followup" para una iteración posterior. Justificación:
  ningún consumidor actual depende del campo `skipped`, y modificar
  7 sitios sin un consumidor real sería trabajo ceremonial.

### Decisiones tomadas

- **D13**: `build_full_graph_report()` se añade como método directo de
  `PerFileStrategy`, **no al trait `GraphStrategy`**. Esto preserva
  la firma del trait (los 7 call sites existentes siguen compilando
  sin cambios) y permite migrar consumidores gradualmente.
- **D14**: tree-sitter es error-tolerant; `has_error_nodes(&tree)` es
  el ÚNICO mecanismo fiable para detectar sintaxis rota. Sin este
  check, un archivo con `pub fn broken_fn(` (sin `)`) es
  indistinguible de un archivo vacío para el parser.
- **D15**: el fix del CLI swallow se aplica SOLO al subcommand
  Graph. Analyze/Refactor/Index/Navigate mantienen su contrato actual
  (que traga errores) porque no son scope de F2.W2 y cambiarlos sin
  tests específicos sería expansión de scope.
- **D16**: el `SkippedFile::reason` se serializa en stderr en formato
  legible, no como JSON. La razón: el consumidor primario de este
  output es un humano ejecutando `cognicode graph per-file …`, no un
  parser automático. La estructura `BuildReport` queda disponible para
  consumidores que prefieran JSON (vía un wrapper futuro).

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO | Sesión 2026-09-21 |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos |

### Trabajo pendiente heredado

- F2.W3 — Equivalencia full vs per_file (R4).
- H10 — extender `staging_dir` para que cubra downloads de manifests.
- Bug preexistente del binario `cognicode` (dos crates con mismo
  `name`).
- F2.W2-followup — migrar los 7 call sites de `build_full_graph` a
  `build_full_graph_report` cuando haya un consumidor real que
  necesite los `SkippedFile`s.

---

## PRF-F2-W3 — Certificación de la unidad F2.W3 (Equivalencia full vs per_file — R4)

| Campo | Valor |
|---|---|
| ID | `PRF-F2-W3` |
| Hito | F2 — Correctitud reproducible |
| Unidad | W3 — Caracterización de equivalencia entre `FullGraphStrategy` y `PerFileStrategy` (R4 del brief) |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `d9aa09c0` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: brief del operador (R4 — caracterización sin
      corrección); STATE.md y ROADMAP.md F2.W3 documentados;
      decisión de diseño "no forzar equivalencia" registrada en
      STATE.md y en este certificado.
- [x] **IMPLEMENTED**: 5 tests de caracterización en
      `strategy.rs::w3_equivalence_tests`; corpus de 9 escenarios
      versionado en `docs/prf/fixtures/equivalence_full_vs_perfile/`.
- [x] **INTEGRATED**: `cargo test -p cognicode-core --lib` →
      **2096/0/27** (baseline F2.W2 era 2091/0/27, **+5 tests** sin
      regresión). Los tests ejercitan el código real de ambas
      estrategias sobre el corpus real.
- [x] **ACCEPTED**: criterios de salida cumplidos — corpus y
      oráculo versionados, RED verificado (sneaky symbol → falla →
      restaurar → GREEN), divergencias legítimas documentadas,
      hallazgo emergente H-R4-1 registrado para F2.W4.
- [ ] **RELEASED**: pendiente.

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Documentación | `docs/prf/STATE.md` §"Última unidad cerrada: F2.W3" |
| Diario | `docs/prf/JOURNAL.md` §10 |
| Corpus versionado | `docs/prf/fixtures/equivalence_full_vs_perfile/CORPUS.md` + 7 archivos `.rs` |
| Tests añadidos | `crates/cognicode-core/src/infrastructure/graph/strategy.rs::w3_equivalence_tests` (5 tests, 199 LOC) |
| Hallazgo H-R4-1 | `docs/prf/TRACEABILITY.md` (tabla "Hallazgos con trazabilidad") |

### Verificación ejecutada (resumen)

- `cargo test -p cognicode-core --lib w3_equivalence_tests` → **5/5 pass**.
- `cargo test -p cognicode-core --lib` → **2096/0/27** (+5 vs
  baseline F2.W2 de 2091/0/27).
- **RED verificado manualmente**:
  `w3_corpus_has_expected_symbol_inventory` falla cuando se añade
  un símbolo extra (`sneaky_extra_symbol_for_test`); pasa cuando se
  restaura. El test es un detector de regresión real, no una
  tautología.

### Hallazgos emergentes

- **H-R4-1** (severidad MEDIO funcional, estado OPEN): ambas
  estrategias devuelven **0 edges** sobre el corpus de
  caracterización pese a que `lib.rs::caller` invoca
  `crate::nested::callee()`. Sugiere que
  `TreeSitterParser::find_call_relationships` no captura esa
  relación. **No es bug certificado** (sin UAT adicional); queda
  registrado en TRACEABILITY.md y en JOURNAL.md §10 como scope de
  **F2.W4**. Antes de corregirlo se requiere:
  1. Reproducir con un UAT manual sobre el corpus (no test).
  2. Decidir si es bug del parser (necesita fix) o limitación
     documentada (cross-file via `crate::` no es soportado).
  3. Si es bug, escribir un test RED que lo demuestre antes de
     tocar el parser.

### Decisiones tomadas

- **D17**: no forzar equivalencia bit-a-bit entre las dos
  estrategias. Las dos indexan de forma distinta; forzar igualdad
  obligaría a reescribir código sin beneficio para el producto.
  Los tests pinerán el estado actual.
- **D18**: el assert de inventario usa **un assert de total** además
  de los asserts por nombre. Esto convierte el test en un detector
  de regresión real (no basta con que `hello=1`, `shared=2`, etc.
  individualmente; el total debe ser exactamente 10). RED
  verificado.
- **D19**: H-R4-1 se documenta en bitácora y TRACEABILITY pero NO
  en CERTIFICATES como bug certificado. La política de PRF es
  certificar solo lo verificado; H-R4-1 requiere UAT adicional
  antes de poder hablar de "bug".

### Limitaciones documentadas

- **H-R4-1** sigue OPEN; su investigación es scope de F2.W4.
- **Bug R3-style en `FullGraphStrategy`** (los `_ => continue`
  tragan errores) sigue sin arreglar; pinerado en
  `w3_full_strategy_silently_ignores_broken_syntax_today` para que
  no se introduzca una regresión silenciosa. Scope de F2.W4.
- **Migración de los 7 call sites CLI** a `build_full_graph_report`
  sigue pendiente. Scope de F2.W4 (sin consumidor real no es
  trabajo ceremonial).
- **Bug preexistente del binario `cognicode`** y **H10** siguen
  sin arreglar (no scope de F2.W3).

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO | Sesión 2026-09-21 |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos |

### Trabajo pendiente heredado

- F2.W4 — Cerrar huecos (5 frentes: H-R4-1, R3 en `full`, mtime-preserved
  content change, migración de call sites, UAT CLI/MCP real).
- H10 — extender `staging_dir` para que cubra downloads de manifests.
- Bug preexistente del binario `cognicode` (dos crates con mismo `name`).
- F2.W2-followup — migrar los 7 call sites a `build_full_graph_report`.

---

## PRF-F2-W4 — Certificación de la unidad F2.W4 (Cerrar huecos — H-R4-1 capa 1)

| Campo | Valor |
|---|---|
| ID | `PRF-F2-W4` |
| Hito | F2 — Correctitud reproducible |
| Unidad | W4 — Cerrar huecos (H-R4-1 capa 1: parser; resto → deuda documentada) |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `084b5c00` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: brief del operador (cerrar H-R4-1 que F2.W3
      descubrió); STATE.md §F2.W4 documentado; decisión "capa 1
      sí, capa 2 no" registrada y justificada.
- [x] **IMPLEMENTED**: fix del parser en `extract_callee_name`
      (~50 LOC) con detección de `scoped_identifier` y
      `field_expression` + helper `find_last_identifier_in_node`.
- [x] **INTEGRATED**: `cargo test -p cognicode-core --lib` →
      **2101/0/27** (baseline F2.W3 = 2096/0/27, **+5 tests** sin
      regresión). Los 5 tests nuevos pinerán el fix.
- [x] **ACCEPTED-parcial**: capa 1 de H-R4-1 cerrada con test de
      regresión. Capa 2 (H-R4-2) registrada como OPEN con scope y
      responsable explícitos. Otros 4 frentes del brief original
      registrados como deuda documentada.
- [ ] **RELEASED**: pendiente.

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Documentación | `docs/prf/STATE.md` §"Última unidad cerrada: F2.W4" |
| Diario | `docs/prf/JOURNAL.md` §11 |
| Código modificado | `crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs` (commit `084b5c00`, +149 -3) |
| Tests añadidos | `crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs::w4_h_r4_1_tests` (5 tests) |
| Hallazgos | H-R4-1 (PARTIAL, capa 1 cerrada), H-R4-2 (OPEN), 4 frentes adicionales (deuda documentada) |

### Verificación ejecutada (resumen)

- `cargo test -p cognicode-core --lib w4_h_r4_1_tests` → **5/5 pass**.
- `cargo test -p cognicode-core --lib` → **2101/0/27** (+5 vs
  baseline F2.W3 de 2096/0/27).
- **RED verificado manualmente** antes del fix:
  `h_r4_1_module_qualified_call_resolves_to_leaf` esperaba
  `["callee"]` y obtuvo `["nested"]`; misma forma para
  `crate::nested::callee()` y `a::b::c::callee()`. Después del
  fix: los 4 tests pasan. El test de method call
  (`h_r4_1_method_call_on_receiver`) requirió una segunda
  iteración porque `field_identifier` no era reconocido por el
  helper; se añadió tras el primer intento fallido.

### Hallazgos diferidos (deuda documentada)

- **H-R4-2** (MEDIO funcional, OPEN): lookup `name → SymbolId`
  per-file impide que los edges cross-file lleguen al grafo
  final incluso con el parser corregido. Refactor requerido
  (lookup global pre-walk). Impacto estimado en ~10 tests
  existentes con `edge_count == 0` o valores pre-fix. Scope de
  una unidad futura (F2.W5 o F3.W1) con análisis de impacto
  propio.
- **R3-style fix en `FullGraphStrategy`**: pendiente. El test
  `w3_full_strategy_silently_ignores_broken_syntax_today`
  pine el bug.
- **mtime-preserved content change test**: pendiente. Sin
  consumidor inmediato que requiera cerrar ese agujero del
  fingerprint.
- **Migración de los 7 call sites CLI** a
  `build_full_graph_report`: pendiente. Sin consumidor real.
- **UAT CLI/MCP real sobre binario**: pendiente. Bloqueado por
  bug preexistente del binario `cognicode` (dos crates con mismo
  `name`).

### Decisiones tomadas

- **D20**: el fix de H-R4-1 capa 1 es suficiente como cierre
  de F2.W4 desde el punto de vista de "valor entregado".
- **D21**: NO se aborda H-R4-2 (lookup global) en este commit.
  Es un refactor sustantivo con impacto en tests existentes;
  requiere una unidad propia con análisis de impacto.
- **D22**: el test de pineo del bug silencioso en
  `FullGraphStrategy` sirve como detector de regresión.

### Limitaciones documentadas

- H-R4-2 sigue OPEN. Su investigación + fix es scope de una
  unidad futura.
- H10, bug preexistente del binario, R3 en `full`, mtime-preserved,
  migración de call sites, UAT binario: todos OPEN documentados.

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO (parcial) | Sesión 2026-09-21. Aprobación condicionada a la documentación honesta del alcance parcial. |
| Auto-revisión PRF | (programa PRF) | APROBADO (parcial) | Capa 1 cumplida; capa 2 registrada. |

### Cierre del hito F2

Con F2.W4-parcial, las 4 unidades del brief han sido procesadas.
El hito F2 se cierra **a nivel del programa** (no a nivel RELEASED
porque eso depende del roadmap principal):

| Unidad | Estado |
|---|---|
| F2.W1 (R2) | ACCEPTED |
| F2.W2 (R3) | ACCEPTED |
| F2.W3 (R4) | ACCEPTED |
| F2.W4 (cierre de huecos) | ACCEPTED-parcial (capa 1) + deuda documentada (resto) |

### Trabajo pendiente heredado

- C2 — Campaña de certificación del hito F2 sobre el estado
  actual. Certificado consolidado que cubre F2.W1-W4 con todos
  los findings y la deuda.
- F3 o F2.W5 (a planificar): H-R4-2 (lookup global), R3 en
  `full`, mtime-preserved content change, call sites, UAT binario.
- H10 — extender `staging_dir` para que cubra downloads de manifests.
- Bug preexistente del binario `cognicode` (dos crates con mismo `name`).

---

## Hito F2 — CERRADO A NIVEL DEL PROGRAMA (con deuda documentada)

Tres certificados firmados (PRF-F2-W1, PRF-F2-W2, PRF-F2-W3,
PRF-F2-W4) + este certificado de cierre. Las 4 unidades
planificadas del brief han sido procesadas. La deuda restante
está catalogada con severidad, scope y responsable. **RELEASED**
queda pendiente hasta que el roadmap principal consolide las
gates.

Siguiente paso del programa PRF: **C2 — Campaña de
certificación**.

---

## ⚠️ Rectificación — 2026-09-21 — Cierre administrativo de F2 revocado

El cierre "Hito F2 — CERRADO A NIVEL DEL PROGRAMA" registrado justo
arriba (a continuación del cert `PRF-F2-W4`) **no representa la
aceptación certificada del programa PRF** y queda revocado por
directiva del operador.

### Por qué se revoca

El modelo de certificación de PRF (`docs/prf/CERTIFICATION.md`)
define 5 estados: SPECIFIED → IMPLEMENTED → INTEGRATED → ACCEPTED →
RELEASED. Para que un hito se considere **ACEPTADO** debe tener:

1. Cada requisito con UAT ejecutada **sobre el binario real**
   (no sobre la library).
2. Captura de stdout/stderr/exit code.
3. Comparación contra el contrato esperado.

Las 4 unidades de F2 tienen:

- Implementación verificada en library tests.
- **NO** tienen UAT ejecutada sobre `cognicode` o `cognicode-mcp`
  como binarios reales. Los 3 UAT de CLI en F2.W2 son a nivel de
  `CommandExecutor::execute` (un wrapper sobre la library), no
  del binario distribuible.
- H-R4-2 (lookup global) sigue OPEN y la unidad F2.W4 lo
  reconoció como "ACCEPTED-parcial".

Con esos dos hechos, **F2 no satisface el estado ACCEPTED** y el
"cierre a nivel del programa" era una inferencia administrativa
que no se sostenía contra los requisitos del modelo.

### Estado vigente tras esta rectificación

- **F2 (hito)**: **EN CURSO**. Las unidades W1-W4 conservan su
  valor técnico (sus commits no se reescriben); lo que se invalida
  es la inferencia de cierre.
- **C2 (campaña de certificación)**: **NO CERTIFICADO**. No se
  emite hasta que:
  - F2.W5 cierre H-R4-2 (lookup global).
  - F2.W6 aplique R3-style fix a `FullGraphStrategy`.
  - F2.W7 cierre el agujero de fingerprint mtime-preserved.
  - F2.W8 produzca UAT real sobre binario (con workaround al bug
    preexistente de los dos crates `name = "cognicode"`).
- **Cierre válido de F2** exige que las 4 unidades W5-W8 estén
  cerradas y C2 firmado.

### Compromiso

Esta rectificación se documenta sin borrar la entrada anterior.
Los hechos canónicos son los commits y los tests; la inferencia
"cerrado" era del agente, no del código. Cuando el operador
emita nueva directiva, el modelo de certificación la respeta.

Detalle completo en `JOURNAL.md` §12 y `STATE.md` §Snapshot.

---

## PRF-C2 — Certificación consolidada del hito F2 (Correctitud reproducible)

| Campo | Valor |
|---|---|
| ID | `PRF-C2` |
| Hito | F2 — Correctitud reproducible (unidades W1-W10) |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `dc189d54` (W10) + docs; último commit de código `dc189d54` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Alcance certificado

Unidades F2.W1-W10: R2 (invalidación de cache por contenido), R4
(equivalencia full ↔ per_file), H-R4-1 (parser de qualified calls),
H-R4-2 (lookup global scope-aware), integración en el binario (W7),
R3 (errores silenciosos, W8), mtime preservado (W9), pineado de
equivalencia de aristas y reproducibilidad (W10).

### Criterio de salida del hito (ROADMAP §F2)

> Cada vertical de análisis dispone de un corpus determinista con un
> oráculo independiente, los defectos descubiertos están cerrados con
> tests de regresión y un UAT que ejecute el binario real.

| Criterio | Evidencia | Cumple |
|---|---|---|
| Corpus determinista + oráculo | `docs/prf/fixtures/equivalence_full_vs_perfile/` (7 archivos + CORPUS.md); inventario pineado por `w3_corpus_has_expected_symbol_inventory` | Sí |
| Defectos cerrados con tests de regresión | R2: `test_per_file_graph_cache_detects_content_change` (`70f0b0cf`). H-R4-1 capa 1: tests parser (`084b5c00`). H-R4-2: `w5_*` + `global_index_tests` (`3f27a31d`, `5ce8eb1e`). R3: `w8_silent_errors_tests` ×3. mtime: `w9_mtime_tests` (`2a121aec`). Equivalencia/reproducibilidad: `w10_equivalence_tests` ×3 (`dc189d54`) | Sí |
| UAT con binario real | UAT-F2-W7 (`relationships_found: 4`, get_call_hierarchy consistente), UAT-F2-W8-001 (`skipped_files[]` con chmod 000 + UTF-8 inválido), UAT-F2-W9-001 (mtime restaurado, cache invalidado) — todas en `docs/prf/UAT.md` con binario release real | Sí |

### Estados por unidad

| Unidad | IMPLEMENTED | INTEGRATED | ACCEPTED | Evidencia |
|---|---|---|---|---|
| F2.W1 (R2 cache) | Sí | Sí | Sí | cert PRF-F2-W1, commit `70f0b0cf` |
| F2.W3 (equivalencia) | Sí | Sí | Sí | cert PRF-F2-W3, commit `d9aa09c0` |
| F2.W4 (H-R4-1 capa 1) | Sí | Sí | Sí-parcial | cert PRF-F2-W4, commit `084b5c00` |
| F2.W5 (H-R4-2) | Sí | Sí (UAT W7) | Sí | commit `3f27a31d` |
| F2.W7 (binario) | Sí | Sí | Sí | commit `5ce8eb1e` + UAT-F2-W7 |
| F2.W8 (R3 errores) | Sí | Sí | Sí | commit (W8) + UAT-F2-W8-001 |
| F2.W9 (mtime) | Sí | Sí | Sí | commit `2a121aec` + UAT-F2-W9-001 |
| F2.W10 (equivalencia edges + reproducibilidad) | Sí | Sí | Sí | commit `dc189d54` (tests de pineo sobre corpus; sin superficie de binario que integrar) |

### Evidencia de suite al cierre

- `cargo test -p cognicode-core --lib` → `2122 passed; 0 failed; 27 ignored`.
- `cargo test --workspace --no-fail-fast` → 6 fallos, todos
  preexistentes y catalogados en JOURNAL §15 con responsable y
  trigger (`cogh_uninstall`, 4× `manifest_upsert` ladybug,
  `test_cogh_update_respects_lockfile`/rate-limit GitHub H10,
  `docs_extractor_corpus_regression`). Ninguno en crates tocados
  por F2; ninguno bloquea este gate.

### Deuda documentada (no bloqueante, honesta)

- Reescritura con mismo tamaño Y mismo mtime no invalida cache
  (requeriría hash de contenido). Documentada en W9 y UAT-F2-W9-001.
- H-R4-2 residual: call sites CLI legacy aún en
  `build_full_graph` legacy (7 consumidores, migración a
  `build_full_graph_report` registrada como follow-up).
- H10 (GitHub API rate limit): deuda externa, mock pendiente.

### Estados finales

- [x] **SPECIFIED**: ROADMAP §F2 con criterio de salida.
- [x] **IMPLEMENTED**: suite 2122/0/27, todos los defectos con RED→GREEN.
- [x] **INTEGRATED**: binario real `cognicode-mcp` ejecuta los caminos corregidos (UAT W7/W8/W9).
- [x] **ACCEPTED**: UAT reales ejecutadas y registradas en `UAT.md`.
- [ ] **RELEASED**: pendiente de consolidación con roadmap principal y tag. Requiere decisión del operador (gate de push/tag).

---

## PRF-F3 — Certificación del hito F3 (Vertical de análisis compartida CLI + MCP)

| Campo | Valor |
|---|---|
| ID | `PRF-F3` |
| Hito | F3 — Vertical de análisis compartida |
| HEAD al cierre | `67363bfc` (código) + docs |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Criterio de salida

Para cada vertical de F2 (`graph per-file`, `graph full`,
`graph hierarchy`): un UAT en CLI y un UAT en MCP con el mismo
resultado observable sobre el mismo corpus; tools MCP delegan al
puerto de análisis sin lógica de cálculo propia.

### Evidencia

| Evidencia | Ubicación / comando |
|---|---|
| UAT de equivalencia | `UAT.md` §UAT-F3-001 (binarios reales, corpus `/tmp/prf-uat-f3`) |
| Puerto compartido per-file | `PerFileStrategy::build_local_graph` en ambos lados (inspección `interface/cli/commands.rs` + `handlers/mod.rs`) |
| Puerto compartido full | `FullGraphStrategy` (CLI) / `AnalysisService::build_project_graph` + `GlobalSymbolIndex` (MCP), mismo resolver F2.W5/W7 |
| Deuda registrada | H-F3-1 (`find_usages` con walk+parser inline) en TRACEABILITY |

### Estados

- [x] **SPECIFIED**: ROADMAP §F3.
- [x] **IMPLEMENTED**: ambos lados ya consumen los puertos (sin código nuevo necesario; verificado por inspección + UAT).
- [x] **INTEGRATED**: binarios reales `cognicode` + `cognicode-mcp`; JSON-RPC capturado y stdout comparado.
- [x] **ACCEPTED**: UAT-F3-001 PASS en las 3 verticals.
- [ ] **RELEASED**: pendiente (gate del operador).
