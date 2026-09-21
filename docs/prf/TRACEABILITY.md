# Production-Ready Foundation (PRF) — Traceability

> Matriz requisito ↔ implementación ↔ test ↔ commit.
> Esta es la tabla canónica que conecta cada requisito del programa PRF
> con su evidencia concreta. Se actualiza al cerrar cada unidad.

## Leyenda

- **REQ**: identificador del requisito (formato `PRF-<hito>-<req>`).
- **Descripción**: resumen de una línea.
- **Implementación**: archivos / binarios / funciones que cubren el req.
- **Test**: comando de prueba o suite que valida el req.
- **Evidencia**: ruta al archivo de evidencia (`docs/prf/evidence/...`).
- **Commit**: SHA del commit que cierra la unidad (cuando exista).

## Trazabilidad por requisito

### PRF-F0 — Inventario y baseline

| REQ | Descripción | Implementación | Test | Evidencia | Commit |
|---|---|---|---|---|---|
| PRF-F0-001 | Inventario verificable de binarios (`cogh`, `cognicode`, `cognicode-mcp`, `explorer-mcp`, `explorer-api`) | Los 5 binarios del scope | Comandos `--version`, `--help`, `tools/list`, `tools/call read_file` ejecutados | `docs/prf/evidence/F0-W1-inventory.md` | n/a (working tree) |
| PRF-F0-002 | Catálogo MCP runtime | JSON-RPC `tools/list` | `cognicode-mcp`: 20 tools; `explorer-mcp`: 55 tools | id. | n/a (working tree) |
| PRF-F0-003 | Baseline tests L1+L2 | `cargo test` core/cli/ide_adapter | 2083/291/7 PASS | id. | n/a (working tree) |
| PRF-F0-004 | Documentación de contradicciones | ADR-031 vs runtime | C1: 68 vs 75; C2: docs-ingest conditional | id. | n/a (working tree) |
| PRF-F0-005 | Documentación de hallazgos | Hallazgos H1-H5 | LOW severidad | id. | n/a (working tree) |

> **Nota sobre `Commit`**: `docs/prf/` es working-tree-only (ver
> `JOURNAL.md` §"Política git"). Los REQs de F0.W1 se cierran como
> `ACCEPTED` sin SHA. Los REQs que requieran cambio de código (H2, H3,
> H4) abrirán commits separados en sus respectivos crates cuando se
> aborden.

### Work in progress (sin requisitos cerrados)

| REQ provisional | Descripción | Estado | Notas |
|---|---|---|---|
| PRF-F0-W2-001 | Caracterización arranque/persistencia/red de 5 binarios | Pendiente | Próxima unidad (F0.W2) |
| PRF-F0-W2-002 | Captura strace/ltrace de al menos 2 binarios | Pendiente | id. |
| PRF-F0-W3-001 | Baseline de pruebas automatizadas | Pendiente | F0.W3 |

### PRF-F2 — Correctitud reproducible

| REQ | Descripción | Implementación | Test | Evidencia | Commit |
|---|---|---|---|---|---|
| PRF-F2-W1-001 | Invalidación de cache `PerFileGraphCache` por cambio de contenido | `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs` — `get_or_build` con fingerprint (mtime+size) | `test_per_file_graph_cache_detects_content_change` (RED→GREEN) | `evidence/CERTIFICATES.md` (PRF-F2-W1) | `70f0b0cf` |
| PRF-F2-W1-002 | Caracterización del recorrido anidado en `PerFileStrategy::build_full_graph` | `PerFileStrategy` con `WalkDir` recursivo | `test_per_file_strategy_build_full_graph_nested_corpus` (GREEN desde inicio) | `docs/prf/fixtures/per_file_correctness/CORPUS.md` | `70f0b0cf` |
| PRF-F2-W2-001 | Reporte explícito de archivos omitidos por error de I/O/parseo/sintaxis en `PerFileStrategy` | `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs` — tipos `SkipReason`, `SkippedFile`, `BuildStatus`, `BuildReport`; helper `classify_io_error`; métodos `merge_with_report` (nuevo) y `merge` (preservado) | `test_merge_with_report_surfaces_parse_error`, `test_classify_io_error_read_vs_parse`, `test_merge_with_report_surfaces_unreadable_file` (RED→GREEN los tres) | `docs/prf/fixtures/per_file_partial_corpus/CORPUS.md`; `evidence/CERTIFICATES.md` (PRF-F2-W2) | `be729275` |
| PRF-F2-W2-002 | Detección explícita de errores de sintaxis en `build_file_graph` vía `TreeSitterParser::has_error_nodes` | `crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs` (uso de `has_error_nodes`) + `crates/cognicode-core/src/infrastructure/graph/strategy.rs` (rechazo con `InvalidData`) | cubierto por PRF-F2-W2-001 (el corpus `broken_syntax.rs` ejercita este path) | id. | `be729275` |
| PRF-F2-W2-003 | Propagación del error del subcomando Graph al exit code del binario CLI | `crates/cognicode-core/src/interface/cli/commands.rs` — `CommandExecutor::execute` retorna `Err(e)` en el brazo `Graph` | `uat_cli_graph_per_file_broken_syntax_returns_error`, `uat_cli_graph_per_file_missing_file_returns_error` (RED→GREEN) | id. | (commit actual de docs) |
| PRF-F2-W3-001 | Caracterización de equivalencia `full` vs `per_file` | (caracterización sin corrección) | (por definir) | — | — |

## Hallazgos con trazabilidad

| Hallazgo | REQ afectado | Severidad | Acción propuesta | Estado |
|---|---|---|---|---|
| H1: 50 tests rojos anteriores ya corregidos | PRF-F0-003 | LOW | Documentado en F0-W1-inventory | Cerrado |
| H2: catálogo MCP runtime = 75 (no 68) | PRF-F0-002, ADR-031 | LOW | Actualizar ADR-031 cifra "68" → "75" | CLOSED (F1.W2) |
| H3: `cognicode-mcp-server` declarado pero no usado | (PRF-W4 a definir) | LOW | KEEP — es la variante HTTP/SSE para deployment containerizado; `cognicode-mcp` es la variante stdio para instalación local. No es duplicación. | CLOSED (F1.W5 — KEEP+DOCUMENT) |
| H4: `docs-ingest`/`issues-ingest` visibles en `--help` solo con feature `multimodal` | (PRF-W4 a definir) | LOW | KEEP+MARK — el comportamiento actual (marcado con nota "Compiled in ONLY when the `multimodal` Cargo feature is active") es honesto y útil. No se debe ocultar. | CLOSED (F1.W5 — KEEP) |
| H5: LSPs `pyright`/`typescript-language-server` faltantes en el entorno | (no es defecto del producto) | LOW | Setup de UAT | Sin acción de código |

## Contradicciones con trazabilidad

| ID | Contradicción | Acción propuesta |
|---|---|---|
| C1 | ADR-031 cita "68 tools MCP runtime"; runtime real = 75 | Actualizar ADR-031 con cifra correcta y referencia al inventario F0-W1 |
| C2 | `cognicode --help` lista `docs-ingest`/`issues-ingest` pero no existen en binario default | Decidir UX: si se quiere anunciar, marcar con `[requires: feature multimodal]` o documentar en `--help` |

## Resumen por unidad

| Unidad | REQs cerrados | Estado |
|---|---|---|
| F0.W1 (Bootstrap + Inventario) | PRF-F0-001, PRF-F0-002, PRF-F0-003, PRF-F0-004, PRF-F0-005 | **ACCEPTED** |
| F0.W2 (Caracterización runtime) | (por abrir) | Pendiente |
| F0.W3 (Baseline pruebas) | (por abrir) | Pendiente |

## Matriz de trazabilidad (consolidada F0.W1 + F0.W2)

### PRF-F0 — Inventario y baseline

| REQ | Descripción | Implementación | Test | Evidencia | Commit |
|---|---|---|---|---|---|
| PRF-F0-001 | Inventario verificable de binarios | 5 binarios | --version/--help/tools/list ejecutados | F0-W1-inventory.md | n/a |
| PRF-F0-002 | Catálogo MCP runtime | JSON-RPC tools/list | 20+55=75 tools | id. | n/a |
| PRF-F0-003 | Baseline tests L1+L2 | cargo test core/cli/ide_adapter | 2083/291/7 PASS | id. | n/a |
| PRF-F0-004 | Documentación de contradicciones | ADR-031 vs runtime | C1/C2 (H2, H7 nuevo) | F0-W1, F0-W2 | n/a |
| PRF-F0-005 | Documentación de hallazgos | H1-H9 (5+4) | LOW/MEDIUM | id. | n/a |
| PRF-F0-006 | Caracterización arranque | /usr/bin/time × 10 | max RSS, wall time | F0-W2 §1 | n/a |
| PRF-F0-007 | strace ≥ 2 binarios | 3 strace ejecutados | syscalls, threads, red | F0-W2 §2 | n/a |
| PRF-F0-008 | Caracterización persistencia | cogh init | 7 dirs + 6 plugins | F0-W2 §3 | n/a |
| PRF-F0-009 | Caracterización red | strace + ss | 0 outgoing, bind local | F0-W2 §4 | n/a |
| PRF-F0-010 | Caracterización stdio | separar stdout/stderr | H6 detectado | F0-W2 §5 | n/a |
| PRF-F0-011 | Caracterización señales | SIGTERM/INT/KILL | H9 detectado | F0-W2 §6 | n/a |
| PRF-F0-012 | Búsqueda secretos en logs | grep regex | 0 coincidencias reales | F0-W2 §7 | n/a |

### Hallazgos consolidados (H1-H9)

| Hallazgo | REQ | Severidad | Acción propuesta | Estado |
|---|---|---|---|---|
| H1: 50 tests rojos anteriores corregidos | F0-003 | LOW | Documentado | Cerrado |
| H2: MCP runtime = 75 (no 68) | F0-002, ADR-031 | LOW | Actualizar ADR-031 + plugin.yaml | CLOSED (F1.W2) |
| H3: `cognicode-mcp-server` declarado pero no usado | (W4+) | LOW | Decidir destino | CLOSED (F1.W5 — KEEP+DOCUMENT) |
| H4: `docs-ingest`/`issues-ingest` solo con feature multimodal | (W4+) | LOW | Decidir UX | CLOSED (F1.W5 — KEEP+MARK) |
| H5: LSPs faltantes en entorno | (no es defecto producto) | LOW | Setup UAT | Sin acción |
| H6: `cognicode` CLI mezcla logs y datos en stdout | F0-010 | MEDIUM | Refactor: stderr para logs | CLOSED (F1.W1) |
| H7: `plugin.yaml` mcp-server dice "68 tools" | F0-004 | MEDIUM | Actualizar a "75 tools" | CLOSED (F1.W2) |
| H8: 5/6 plugin manifests sha256 placeholder | F0-008 | MEDIUM | Calcular sha256 reales | CLOSED (F1.W3) |
| H9: `explorer-api` SIGTERM silencioso | F0-011 | LOW | Hook de shutdown con log | CLOSED (F1.W1) |
| H10: Test failure por GitHub API rate limit | F0-003 | LOW | Ninguna (dependencia externa; esperar reset o usar token) | OPEN (WIP) |
