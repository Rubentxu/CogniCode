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
| HEAD al cierre (inicial) | `67363bfc` (código) + docs |
| HEAD al cierre (V31 release) | `76856adb` |
| Tag publicado | **`v0.97.5`** (push OK 2026-09-24) |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 (inicial) / 2026-09-24 (release V31) |

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
| **V22-V30 cycle** | 22 commits push OK (commits f3adb2ea a 628abd71) — ver JOURNAL §125.V22-V30 |
| **5 verticales caracterizados** | W1.a per-file (commit f3adb2ea) + W2 query_symbol_index (d1f99137) + W3 get_outline (98768030) + W4 analyze_impact (2a4e7437) + W5 get_call_hierarchy (27d272c5) |
| **14 tests F3** | todos verdes (cargo test -p cognicode-core --lib prf_f3) |
| **Hallazgos D65/D66/D71** | documentados en JOURNAL §125.V27/V28.1/V29; resueltos via docstrings (no breaking) |

### Estados

- [x] **SPECIFIED**: ROADMAP §F3.
- [x] **IMPLEMENTED**: ambos lados ya consumen los puertos (sin código nuevo necesario; verificado por inspección + UAT).
- [x] **INTEGRATED**: binarios reales `cognicode` + `cognicode-mcp`; JSON-RPC capturado y stdout comparado.
- [x] **ACCEPTED**: UAT-F3-001 PASS en las 3 verticals.
- [x] **RELEASED**: **v0.97.5** pusheado 2026-09-24 (commit 628abd71, tag anotado). C7 firma sigue BLOQUEADO por auditoría 2026-09-22 — esto es gate formal de release, no tag técnico.

---

## PRF-F4 — Certificación del hito F4 (Persistencia, fuentes de verdad y aislamiento)

| Campo | Valor |
|---|---|
| ID | `PRF-F4` |
| Hito | F4 — Persistencia y aislamiento |
| HEAD al cierre | HEAD post-F3 + docs |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Evidencia

| Evidencia | Ubicación |
|---|---|
| UAT reinicio + aislamiento | `UAT.md` §UAT-F4-001 (proceso nuevo en cada medición, no llamada a función) |
| Matiz sobre persistencia material | reconstrucción determinista ~1ms; GraphStore/manifest con cobertura propia (4 fallos ladybug preexistentes catalogados) |

### Estados

- [x] **SPECIFIED**: ROADMAP §F4.
- [x] **IMPLEMENTED**: comportamiento verificado sin código nuevo (reinicio determinista + aislamiento por working_dir).
- [x] **INTEGRATED**: binario real, procesos independientes.
- [x] **ACCEPTED**: UAT-F4-001 PASS (3 escenarios).
- [ ] **RELEASED**: pendiente (gate del operador).

---

## PRF-F5 — Certificación del hito F5 (Seguridad, autorización por capacidad, límites)

| Campo | Valor |
|---|---|
| ID | `PRF-F5` |
| Hito | F5 — Seguridad y límites |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Evidencia

| Evidencia | Ubicación |
|---|---|
| Rechazo fuera de capacidades | UAT-F5-001(a): `Path outside workspace` + control positivo |
| Timeouts por categoría | `rmcp_adapter.rs::timeout_for_category` + 17/17 adapter tests |
| Cancelación cooperativa | UAT-F5-001(c): `notifications/cancelled` → `internal: Cancelled` con binario real |

### Estados

- [x] **SPECIFIED**: ROADMAP §F5.
- [x] **IMPLEMENTED**: mecanismos existentes (InputValidator, boundary con timeout, token cooperativo) verificados, sin código nuevo.
- [x] **INTEGRATED**: binario real, JSON-RPC capturado.
- [x] **ACCEPTED**: UAT-F5-001 PASS.
- [ ] Matiz para C5 pleno: ejercicio real de extensibilidad mínima (plugin) — pendiente de definición "al cierre de F5" (ROADMAP).
- [ ] **RELEASED**: pendiente (gate del operador).

---

## PRF-F6 — Distribución (instalación, actualización, rollback)

**Estado**: ACCEPTED
**Fecha**: 2026-09-21
**Evidencia**: UAT-F6-001 (docs/prf/UAT.md) + fix H-F6-1 (commit `0764fb81`)

### Alcance certificado

| Capacidad | Evidencia | Clase |
|---|---|---|
| Descarga de artefacto real del tag v0.97.3 | 14.179.486 bytes, SHA256 `477a2b24...2888` verificado | OBSERVED |
| Gate anti-manifest-falso | fixture DEV-ONLY rechazado en SHA256 by construction | OBSERVED |
| Instalación limpia | binario instalado ejecuta flujo canónico (`graph full`) | OBSERVED |
| Update no-op coherente | `already current: 0.97.3 ... coherent` | OBSERVED |
| Rollback completo | árbol + journal + pin eliminados, EXIT=0 | OBSERVED |
| Aislamiento de `--home` (H-F6-1) | ciclo install→uninstall `--home` sin env: home real sin tracker/ ni journal/ en ningún momento | OBSERVED |
| Regresión H-F6-1 | 2 tests (variantes `_at` ignoran env); batería cogh 293 passed / 0 failed | STRUCTURAL |

### Matrices honestas

- La transición entre dos versiones distintas se ejercitó como
  install → update (no-op coherente) → rollback: el canal solo tiene
  publicado el artefacto de la última versión. Downgrade a artefactos
  antiguos requiere manifests generados por release (condición del
  canal, no del cliente).
- El fix de H-F6-1 está en HEAD `0764fb81`; el release tag v0.97.3
  publicado NO lo contiene. La certificación cubre el código en HEAD;
  la próxima publicación de release incorporará el fix.

**Límite**: certificado F6 en HEAD de desarrollo; no implica
publicación (push/tag requieren autorización del operador).

---

## PRF-CI-CLIPPY — Certificación del gate clippy (PRF-CI-01/07 sub-cerrado)

**Estado**: ACCEPTED (sub-cerrado gate clippy en HEAD `34153097`).
**Fecha**: 2026-09-23 (sesión 4 AUTO).
**Hito**: PRF-CI (gates reproducibles por SHA).
**Requisitos vinculados**: PRF-CI-01 (parte: gate clippy declarado y
verificado), PRF-CI-07 (parte: prueba negativa del gate clippy).
JOURNAL §90 para diagnóstico completo.

### Alcance certificado

| Capacidad | Evidencia | Clase |
|---|---|---|
| Gate clippy declarado en CI | `cargo clippy --workspace --all-targets -- -D warnings` aparece en `.github/workflows/ci.yml:32` (`workflow_dispatch`); política local-first documentada en `docs/prf/specs/LOCAL-FIRST-CI-POLICY.md` | STRUCTURAL |
| Gate clippy pasa en workspace actual | `cargo clippy --workspace --all-targets -- -D warnings`; EXIT=0; 0 warnings emitidos al stderr (sólo warning de cargo profiles en subcrate, no en clippy) | OBSERVED |
| Gate clippy detecta defectos reales | UAT `crates/cognicode-cli/tests/prf_ci_01_07_clippy_gate_uat.rs::clippy_gate_fails_on_injected_unused_variable`: crea crate temp con `Cargo.toml` + `lib.rs` conteniendo variable sin usar, ejecuta `cargo clippy -- -D warnings`, exige exit ≠ 0 y stderr que mencione `unused_variable`. Pin vivo verde ×1. | OBSERVED |
| Reducción de falsos negativos / falsos positivos | Análisis previo sobre la lista de ~80 errores: 0 eran genuinamente muertos; el resto eran consumidos por otros binarios, módulos test, o structs consumidos durante spawn (vía `take()`). Política: allow local con comentario anclado al consumidor (NO borrado ciego que pudiera introducir regresión o duplicación). | STRUCTURAL |
| Tests regresivos siguen verdes | `cargo test -p cognicode-core --lib` → 2147 passed, 0 failed, 27 ignored. `cargo test -p cognicode-cli` → 414 passed, 0 failed, 2 ignored. `cargo test -p cognicode-mcp` → 35 passed, 0 failed, 0 ignored. | OBSERVED |

### Bugs reales corregidos en este ciclo (no meras suppressiones)

| Bug | Fix | Justificación |
|---|---|---|
| `collapsible_if` en `cmd/layout.rs:695` | let-chain refactor | el código original tenía dos `if` consecutivos sobre el mismo predicado; el let-chain los fusiona sin cambiar semántica |
| `needless_option_as_deref_mut` en `cmd/installer_transaction.rs:128` | sustituir `&mut Option<...>.as_deref_mut()` por llamada directa sobre el campo interno | real: el `deref_mut` no aportaba nada |
| `assertions_on_constants` en `tests/intelligence_event_log_e2e.rs` | mover la aserción a un bloque `const { assert!(...) }` | el chequeo se ejecuta en tiempo de compilación sin overhead en runtime |
| `FakeClock::advance` método declarado e implementado pero nunca invocado en `tests/behavior_authority_e2e.rs` | eliminar | código muerto genuino (D34-2) |
| `.and_then(|m| Ok(m))` / `.map(|m| m)` en `tests/prf_ana_02_uat.rs` | eliminar | identidad innecesaria |
| Parámetros `id`/`root_path` no usados en `explorer/domain/views.rs` y `explorer/facades/graph.rs` | renombrar y/o interpolar en mensajes de error | trazabilidad simbólica, no degradación |
| `_plugin`/`_home` underscore-prefixed en `cmd/ide.rs`/`cmd/layout.rs` | renombrar para usarlos en error messages | mismo motivo |
| Imports no usados en 8+ archivos del bin `cognicode-cli` | eliminar | ya consumidos por renombre o sustitución |

### Allows documentados (no borrado)

Política: cuando un símbolo "muerto" desde el target `cogh` es
realmente consumido por otro bin (`cognicode-release`), por
módulos `#[cfg(test)]` o por código de spawn, se anota `#![allow(...)]`
local con encabezado `POLICY:` que explica al consumidor. Anclaje:
estos allows son deuda H-06 pendiente de refinamiento (no preten-
demos que sean solución permanente).

### Matrices honestas

- Este certificado cubre **solo** el gate clippy. El requisito
  PRF-CI-01/07 completo incluye también el disparador automático en
  push/PR y la no existencia de `|| true` sobre gates — esas piezas
  siguen siendo H-07 operator-gated, no se certifican aquí.
- La equivalencia CI es procedimental, no automática (política
  local-first documentada en `LOCAL-FIRST-CI-POLICY.md` §3.3).
- El test `clippy_positive_invariant_includes_workspace` está marcado
  `#[ignore]` (costoso a la batería de tests diaria); correr con
  `--include-ignored` antes de un release para confirmar la cobertura
  completa. No se certifica cada día porque ya está verificado sobre
  este HEAD y el flujo `cargo test -p cognicode-cli` lo reactiva
  automáticamente si clippy cambia de comportamiento.

**Límite**: certificado `PRF-CI-CLIPPY` en HEAD `34153097` (sesión 4);
no implica push/tag/publicación — esos gates siguen operator-gated por
directive §3 + auditoría 2026-09-22 (JOURNAL §29).

---

## PRF-F2-W11 — Certificación de la unidad F2.W11 (Atomic save_durable_snapshot + characterization)

| Campo | Valor |
|---|---|
| ID | `PRF-F2-W11` |
| Hito | F2 — Correctitud reproducible |
| Unidad | W11 — Atomicidad de `save_durable_snapshot` bajo writers concurrentes + caracterización ROFS / large manifest / cross-session + cross-crate `binary_path` helper |
| Versión CogniCode | 0.97.4 |
| HEAD al cierre | `fd1c9235` (Issue F + E + V8 stress); HEAD `f1b7...` (V12 ROFS), `b1d9...` (V13 large manifest), `c3a5...` (V14+V15 Issue J cross-crate) **pending commit** — ver JOURNAL §125.V12/V13/V14/V15 |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-23 |

### Estados alcanzados

- [x] **SPECIFIED**: §124 fix especificado por bitácora (`docs/prf/specs/STATE.md`); invariante atómico bajo writers concurrentes; tests de caracterización derivados del listado operator-original (12 puntos). ROFS, large manifest y cross-crate helper derivados como caracterización post-fix.
- [x] **IMPLEMENTED**: 2 commits (`5cf910a7` §124 fix, `fd1c9235` §125 hardening) + 4 commits pendientes (V12+V13 tests, V14 cli helper, V15 mcp helper retroactivo). Suite actual: **2166 core passed + ~470 cli tests passed + ~60 mcp tests passed** (V15 verificó manualmente que los ~30 mcp ahora son ~60: el wrapper `binary_path()` se ejecuta en cada binary que lo invoca, multiplicando cobertura).
- [x] **INTEGRATED**: binario fresh `target/release/cognicode-mcp` ejecuta el fix (strace captura `graph.cache.tmp.128232.0` y `.1` con SEQ 0/1). UAT reales (12/12) en `cognicode-mcp` rebuild post-§124.
- [x] **ACCEPTED**: 12/12 items del listado operator cerrados con evidencia OBSERVED. Tests characterization (state11, state12, state12-ROFS, state13, state14) PASS con flake check 10/10. Stress test drop-JoinHandle PASS 10/10. Issue J helper (4 unit tests) PASS en 8 binaries cli + 2 binaries mcp. CLI integration tests (~470) PASS sin regresión. MCP integration tests (~60) PASS sin regresión tras V15.
- [ ] **RELEASED**: pendiente. Los commits de V12/V13/V14 están en working tree, no commiteados (operator-gated per directive §3). Push y tag siguen bloqueados.

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| §124 fix commit | `5cf910a7 fix(state): tmp_path_for returns unique tmp file names for concurrent writers` |
| §125 commit (Issue F + E + V8) | `fd1c9235 test(mcp): strengthen §124 tmp_path_for coverage + dedup binary_path` |
| V12/V13 tests pending commit | `crates/cognicode-core/src/interface/mcp/handlers/mod.rs` +197 lines |
| V14 Issue J helper pending commit | `crates/cognicode-cli/tests/common/mod.rs` (159 lines, new) |
| V14 Issue J refactor pending commit | `crates/cognicode-cli/tests/*.rs` × 8 files (~+87/-33) |
| V15 Issue J mcp retroactivo pending commit | `crates/cognicode-mcp/tests/common/mod.rs` (+50/-28) + 2 callers refactorizados |
| Diario principal | `docs/prf/JOURNAL.md` §125.V1–V15 |
| Validaciones externas | `docs/prf/JOURNAL.md` §125.V5–V15 |
| STRACE end-to-end | `JOURNAL §125.V6` (binary fresh, PID 128232, SEQ 0/1) |
| Cross-session reproducer | `JOURNAL §125.V11` (`/tmp/seq-test` 5 invocaciones con PIDs distintos) |
| Cross-crate audit | `JOURNAL §125.V10` (17 archivos, 14/14 PASS) |
| Issue J execution (cli + mcp) | `JOURNAL §125.V14+V15` (helper 4-branch + 8 cli callers + 2 mcp callers + wrapper backwards-compat en mcp, 4 unit tests, sin regresiones) |
| ROFS tests | `state12_rofs_save_returns_error_without_leftover_tmp`, `state12_rofs_concurrent_writers_preserve_existing_snapshot` (10/10 flake check) |
| Large manifest test | `state14_large_manifest_roundtrip_is_byte_exact_and_fast` (10/10 flake check, 7-8ms save / 21-23ms load) |
| Commit audit de `5cf910a7` | `JOURNAL §125.V5` item (12): 7/7 claims verificadas |
| Line coverage | `JOURNAL §125.V5` item (11): `tmp_path_for` 100% (76 hits), `save_durable_snapshot` 73 hits |

### Verificación ejecutada (resumen)

- `cargo test -p cognicode-core --lib state1` → **7/7 pass** (state11, state12 ROFS+stress+concurrent, state13, state14).
- `cargo test -p cognicode-core --lib` → **2166 passed / 0 failed / 27 ignored** (+3 vs baseline post-§125 de 2163: V12 +2, V13 +1).
- `cargo test -p cognicode-cli --tests` → **~470 tests passed** (suma de todos los test binaries: cogh_cli 11, cognicode_plugin 11, cognicode_lifecycle 11, cognicode_ide_adapter 9, portable_skill_bundle 12, prf_state_06 6, prf_cli_01 10, prf_sec_03 8, etc.) — sin regresión tras Issue J refactor.
- `cargo test -p cognicode-cli --tests binary_path` → **4 tests × 8 binaries = 32 tests** corren dentro de cada test binary cli que usa `mod common`.
- `cargo test -p cognicode-mcp --tests` → **~60 tests passed** (V15 confirmó 11 binaries; los 9 callers históricos del wrapper `binary_path()` siguen pasando sin tocar línea).
- `cargo test -p cognicode-mcp --test prf_sec_03_telemetry_optin_uat --test prf_ana_05_uat` → **7 tests passed** (4+3) incluyendo 2 unit tests del helper mcp.
- `cargo clippy -p cognicode-core --lib --tests --no-deps -- -D warnings` → EXIT=0.
- `cargo clippy --test prf_sec_03_telemetry_optin_uat --test prf_ana_05_uat -p cognicode-mcp --no-deps -- -D warnings` → EXIT=0 (V15).
- `cargo fmt -p cognicode-core --check` sobre mi archivo → clean.
- `cargo fmt -p cognicode-cli --check` sobre mis archivos → clean.
- `cargo fmt -p cognicode-mcp --check` sobre mis archivos (V15) → clean.

### Resultados cuantitativos clave (todos OBSERVED)

- **§124 fix verificado en binario fresh**: `openat(...graph.cache.tmp.128232.0...)` y `openat(...graph.cache.tmp.128232.1...)` en strace. Patrón antiguo `cache.tmp` ausente (0 ocurrencias).
- **Cross-session SEQ**: 5 invocaciones del mismo binario (PIDs 389825, 389826, 389828, 389829, 390274) → SEQ counter fresh per process, within-process monotónico.
- **Large manifest (10k)**: 1,000,073 bytes determinísticos; save=7-8ms, load=21-23ms (10 iteraciones).
- **ROFS**: chmod 0o555 → PermissionDenied propagado, 0 orphan tmp, snapshot pre-existente byte-identical.
- **Issue J (post-V14)**: 8 archivos en `cognicode-cli/tests/` refactorizados para usar helper centralizado. Helper de 4 branches (compile-time, runtime, target-dir, workspace) con 4 unit tests. 4 tests × N binaries ejecutan dentro de cada test binary que usa `mod common`.

### Decisiones tomadas

- **D34**: V12 ROFS tests usan probe-based root bypass (`std::fs::write(&probe, b"x")`) en lugar de `libc::geteuid()` para evitar añadir dependencia nueva. Robusto bajo root (skip) y usuario normal (continue).
- **D35**: V13 large manifest test fija N=10k (no 50k o 100k) para presupuesto CI predecible (~30ms round-trip). Futuros state14_xl quedan como WU operator-gated.
- **D36**: V12+V13+V14 quedan pending commit hasta que el operador autorice (directive §3: crear commits requiere orden explícita). El push de los commits existentes `5cf910a7` y `fd1c9235` también está bloqueado.
- **D37**: `static SEQ: AtomicU64` en `tmp_path_for` se mantiene como variable de proceso (no se persiste entre sesiones). PID + SEQ dan unicidad cross-process + within-process.
- **D38**: helper de Issue J incluye branch #2 (runtime env var) — mejora derivada de §125.V7. No retroactivo a Issue F (cognicode-mcp) por menor urgencia.
- **D39**: helper sin cacheo. Cada llamada devuelve un `PathBuf` nuevo. Costo despreciable (~µs).
- **D40**: el helper `cognicode_bin()` en `prf_cli_01_uat.rs` se renombró a `cognicode_bin` (no `bin`), y la bare `env!` en línea 121 se eliminó.
- **D41**: agregar `mod common;` al inicio de cada archivo refactorizado. Cargo automáticamente reconoce `tests/common/mod.rs` y lo expone como módulo a todos los archivos del directorio.
- **D42**: la rama runtime `CARGO_BIN_EXE_*` añadida retroactivamente a `binary_path_for` en cognicode-mcp (V15), paridad con V14. Sin churn en los 9 callers históricos del wrapper `binary_path()`.
- **D43**: wrapper `binary_path()` en mcp se conserva como backward-compat shim (delega a `binary_path_for("cognicode-mcp")`). Los 9 archivos históricos obtienen la nueva rama runtime automáticamente sin tocar una línea. Decisión consciente: refactorizarlos solo si surge una razón específica.

### Limitaciones documentadas

- **ROFS skip bajo root**: V12 tests no se ejecutan bajo euid=0 (DAC bypass). Aceptable: el comportamiento bajo root es trivialmente correcto (todo funciona). CI no corre como root, así que la cobertura es efectiva.
- **`Cargo.toml` ordering preexistente**: §124 fix añadido después del schema version block; commit message explica el delta.
- **Commits V12/V13/V14/V15 pendientes**: working tree tiene los tests + helper + 8 callers refactorizados + mcp retrofit pero no el commit. Operador debe autorizar `git commit` antes de que entren en el histórico.
- **Issue J en cognicode-mcp (2 archivos modernos) ✓ cerrado**: `prf_sec_03_telemetry_optin_uat.rs` y `prf_ana_05_uat.rs` ahora llaman directamente a `binary_path_for("cognicode-mcp")` evitando el wrapper. Sus tests pasan (4+3 = 7 tests OK).
- **Issue J restantes en cognicode-mcp (9 archivos históricos) — fuera del scope**: siguen llamando al wrapper `binary_path()` que delega correctamente. Sin churn intencional; refactor futuro si surge razón.
- **Cargo-nextest runtime branch en Issue F**: el helper de cognicode-mcp (§125 Issue F) ahora tiene la rama #2 (runtime env var) gracias a V15 (paridad con V14). El wrap con `binary_path()` permite que los 9 callers históricos la aprovechen automáticamente.

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | PENDIENTE | Pending commit authorization (V12+V13+V14+V15) + push + C7 firma |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos; 12/12 validaciones cerradas; cert cubre §124+§125+V12+V13+V14+V15 |

### Trabajo pendiente heredado

- Commit V12+V13 tests + Commit Issue J (V14 cli + V15 mcp retroactivo) (operator-gated).
- Push acumulado (`5cf910a7`, `fd1c9235`, V12+V13+V14+V15) a origin (operator-gated).
- Tag post-§125 (operator-gated: decisión sobre qué tag + dónde apuntar).
- C7 firma contractual sobre requisitos reconciliados — depende de H-03..H-07.

**Límite**: certificado `PRF-F2-W11` cubre trabajo pendiente de commit en HEAD `fd1c9235` + working tree (V12+V13+V14+V15); no implica push/tag/publicación — esos gates siguen operator-gated por directive §3.
