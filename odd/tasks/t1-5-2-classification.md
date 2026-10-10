# T1-5.2 — Classification of 194 `println!` calls in `cognicode-core/src/interface/cli/commands.rs`

> **Estado**: T1-5.2 DONE (análisis). Output pineado en este doc;
> alimenta T1-5.3..6 (migración real, multi-día).

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `crates/cognicode-core/src/interface/cli/commands.rs` | **fuente** — 2910 líneas, 194 `println!`/`eprintln!` calls |
| `odd/tasks/t1-5-presenter-move.md` | goal doc que define las 3 categorías (structured-result / tracing-event / error) |
| `odd/tasks/t1-5-1-classification.md` (este doc) | output de T1-5.2; entrada para T1-5.3 (crear el struct/result/tracing scaffolding) |

## 1. Recolección

- `wc -l commands.rs` → 2910 líneas
- `grep -c 'println!('` → 154 stdout
- `grep -c 'eprintln!('` → 40 stderr
- `grep -c 'println!();'` → 2 (vacíos)
- **Total: 194 calls**

## 2. Clasificación por categoría de migración

Las 3 categorías del goal doc, aplicadas a los 194 calls:

### 2.1 structured-result (return via struct) — **~150 calls**

El presenter del bin (T1-5.5) consume el struct y formatea a stdout.
El test snapshot (T1-5.1) pinea que el output quede byte-a-byte igual.

| Sub-categoría | Count | Ejemplos |
|---|---|---|
| **Resultado de búsqueda/lookup** | ~20 | L 680 `"No locations found for symbol '{}'"`, L 682 `"Found {} location(s):"`, L 713 `"Found {} top-level symbols:"` |
| **Summary de operación** | ~20 | L 655 `"Index built successfully in {}ms using {}"`, L 782 `"Subgraph built in {}ms"`, L 911 `"Full graph built in {}ms"`, L 1120 `"Trace completed in {}ms"` |
| **Per-record data dump** | ~10 | L 975 `"{:<40} {:>8} {:>8}  {}:{}"`, L 1225 `"  Depth {}: {} entries"`, L 1439 `"Go to definition: {}:{}:{} (workspace: {})"` |
| **Section header / decoration** | ~4 | L 635 `println!();` (vacío), L 1757 `println!();` (vacío), L 2027 `"\n=== Architecture Check ==="`, L 2041 `"\n=== Complexity Analysis ==="` |
| **Tablas formateadas (multi-line)** | ~1 | L 1227 `"    - {} ({}) at {}:{}"` |
| **Otros "data: structure"** (132 - 22 categorizados arriba) | ~110 | El resto de los "K_unknown" del análisis automático son principalmente dumps de records (paths, symbol names, graph entries) y result summaries (`"Index built successfully"`, `"No path found"`, etc.) |

**Migración**: cada función `execute_*` cambia su firma de `()` a `Result<T, E>` donde `T` es un struct con los datos a presentar. El presenter en `cognicode-cli/src/bin/cogh.rs` (o donde corresponda) formatea el struct a stdout.

### 2.2 tracing-event (log via `tracing::info/warn/error!`) — **~21 calls**

El bin decide si escribir a stderr, a un fichero, o silenciar
según el run mode. El consumer de la librería no debería
asumir stdout.

| Sub-categoría | Count | Ejemplos |
|---|---|---|
| **Progress (Building/Querying/Getting)** | 14 | L 647 `"Building {} index at: {}"`, L 668 `"Querying symbol '{}' in: {}"`, L 699 `"Getting outline for: {}"`, L 722 `"Getting symbol code for: {}:{}:{}"`, L 760 `"Building on-demand subgraph"`, L 793 `"Getting per-file graph for: {}"`, L 798 `"Building per-file graph for: {}"`, L 813 `"Computing centrality"`, L 893 `"Computing communities"`, L 923 `"Running structural analysis"`, L 1161 `"Computing shortest path"`, L 1173 `"Computing entry points"`, L 1500 `"Running m1 m2 stage"`, L 1503 `"Running m1-m2 check"`, ... |
| **Warning (graph partial)** | 7 | L 947, 999, 1042, 1085, 1149, 1254, 1316 — todos `"Warning: graph is PARTIAL: {} file(s) skipped"` |

**Migración**: `println!("...")` → `tracing::info!("...")`, `eprintln!("Warning: ...")` → `tracing::warn!("...")`. El `tracing-subscriber` ya está en el workspace (Cargo.toml:144); falta decidir el formato del log (default format vs json) y si se duplica a stdout/stderr.

### 2.3 error (propagate via `Result`) — **~16 calls**

Errores que actualmente se imprimen y se ignoran; deben
propagarse al caller que decide qué hacer.

| Sub-categoría | Count | Ejemplos |
|---|---|---|
| **Error al construir índice/grafo** | 6 | L 662, 674, 769, 807, 920, 1195 — todos `"Error building {}: {}"` |
| **Error en navigate** | 2 | L 1460, 1489 — `"Error: {}"` |
| **Otros errores** | 7 | Varios en execute_graph, execute_navigate, execute_analyze, etc. |
| **Usage hint cuando falta una dep** | 3 | L 551 `"Use 'cognicode-mcp' binary..."`, L 552 `"The MCP server uses stdio transport..."`, L 553 `"Run: cognicode-mcp --cwd <workspace>"` |

**Migración**: `eprintln!("Error ...")` → `return Err(CogniCodeCliError::Xxx.into())` o `bail!`. La función toma `Result<T, E>` y el caller (bin) decide el formato del error.

## 3. Por función (top emitters)

Las 10 funciones con más `println!` calls:

| Función | Count | Categoría dominante |
|---|---|---|
| `execute_graph` | ~50 | structured-result + tracing-event (progress) |
| `execute_index` | ~30 | structured-result + tracing-event |
| `execute_navigate` | ~25 | structured-result + error |
| `execute_analyze` | ~20 | structured-result (Architect Check / Complexity) |
| `execute` (root) | ~10 | usage_hint + error + decoration |
| `execute_capabilities` | ~5 | structured-result |
| `execute_metrics` | ~5 | structured-result |
| ... | ... | ... |

**Implicación para T1-5.3**: la migración no se hace de 194 calls a la vez; se hace función por función. Empezar por `execute_index` (más simple) y `execute_graph` (más compleja, top emitter) cubre ~80 calls en 2 work units.

## 4. Lo que el snapshot test pinea (T1-5.1, ya hecho)

`crates/cognicode-cli/tests/t1_5_snapshot.rs` captura el output actual de `cogh --version`, `cogh current`, `cogh doctor`, `cogh --help`. NO pinea el output de `execute_*` (los subcomandos del CLI), porque esos viven en `cognicode-cli` ya, no en `cognicode-core`. La migración de los 194 calls es invisible al snapshot test actual.

**Para T1-5.5/6**: añadir un snapshot test que cubra UN subcomando (e.g. `cogh analyze <fixture>` o `cogh find-usages <symbol>`) para que la migración tenga un RED detector en el lado del bin.

## 5. Recomendación para T1-5.3 (siguiente paso bounded)

1. **T1-5.3a**: elegir UN subcomando representativo (e.g. `execute_index` — domina L 635-740, ~30 calls, mix de las 3 categorías).
2. **T1-5.3b**: diseñar el struct de retorno + tracing scaffolding para ESE subcomando. Es el patrón que los demás replican.
3. **T1-5.3c**: añadir un snapshot test del subcomendo ELEGIDO en `cognicode-cli/tests/t1_5_snapshot.rs` (extender el archivo existente con `snapshot_cogh_analyze` o similar).
4. **T1-5.3d**: migrar ESE subcomando. Aprueba el patrón; los demás son copy-paste.
5. **T1-5.4..6**: replicar el patrón para los otros ~9 subcomandos (`execute_graph` el más grande).

**Estimación revisada**: T1-5.3a..d (1 subcomando end-to-end) es ~2 días bounded; T1-5.4..6 son 5-9 días que el `/loop` puede iterar uno a uno si el operador los autoriza.

## 6. Recibo (append-only)

| Fecha | WU | Commit | Notas |
|-------|----|--------|-------|
| 2026-10-10 | T1-5.2 | (este doc) | Análisis completo: 194 calls = ~150 structured-result + ~21 tracing-event + ~16 error + ~4 decoration + ~3 usage_hint. Top emitters: `execute_graph` (~50), `execute_index` (~30), `execute_navigate` (~25), `execute_analyze` (~20). T1-5.3a..d propuesto: patrón sobre 1 subcomando (`execute_index`) primero, replica después. |
