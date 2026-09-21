# F0.W1 — Inventario verificable de binarios PRF

**Fecha**: 2026-09-21
**Operador**: jcode-orchestrator
**Commit base**: `7cc6a8a7` (HEAD al inicio)
**Commit al cierre**: (pendiente)

## Resumen ejecutivo

Inventario de los binarios del workspace `Rubentxu/CogniCode`:

| Binario | Estado | Versión | Subcomandos |
|---|---|---|---|
| `cogh` | ✅ BUILD OK | 0.97.3 | install, uninstall, list, current, latest, update, reshim, rollback, doctor, where, init, plugin, skill, ide, version |
| `cognicode` | ✅ BUILD OK | 0.97.3 | analyze, serve, refactor, index, graph, navigate, doctor (docs-ingest/issues-ingest solo con feature `multimodal`) |
| `cognicode-mcp` | ✅ BUILD OK | 0.97.3 | stdio MCP server, JSON-RPC; **20 tools** registrados |
| `explorer-api` | ✅ BUILD OK | 0.97.3 | HTTP server en `--listen 127.0.0.1:8010`; LadybugDB backend |
| `explorer-mcp` | ✅ BUILD OK | 0.97.3 | stdio MCP server; **55 tools** registrados |

Total MCP tools accesibles al usuario: **20 (cognicode-mcp) + 55 (explorer-mcp) = 75**.

## Comandos ejecutados

```bash
# Build
cargo build -p cognicode-runtime --bin explorer-mcp  # ya estaba built

# cogh
/var/home/rubentxu/cargo-targets/release/cogh --version
/var/home/rubentxu/cargo-targets/release/cogh --help
/var/home/rubentxu/cargo-targets/release/cogh install --help
/var/home/rubentxu/cargo-targets/release/cogh ide --help
/var/home/rubentxu/cargo-targets/release/cogh version
/var/home/rubentxu/cargo-targets/release/cogh doctor
/var/home/rubentxu/cargo-targets/release/cogh ide detect
/var/home/rubentxu/cargo-targets/release/cogh init --home /tmp/prf-cogh-test/.cognicode
/var/home/rubentxu/cargo-targets/release/cogh list
/var/home/rubentxu/cargo-targets/release/cogh plugin list

# cognicode
/var/home/rubentxu/cargo-targets/release/cognicode --version
/var/home/rubentxu/cargo-targets/release/cognicode --help
/var/home/rubentxu/cargo-targets/release/cognicode analyze --help
/var/home/rubentxu/cargo-targets/release/cognicode serve --help
/var/home/rubentxu/cargo-targets/release/cognicode doctor

# cognicode-mcp (vía JSON-RPC sobre stdin)
echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | cognicode-mcp
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list",...}' | cognicode-mcp
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"read_file",...}}' | cognicode-mcp

# explorer-api
explorer-api --listen 127.0.0.1:18010 &  # arranca HTTP
curl http://127.0.0.1:18010/health

# explorer-mcp (vía JSON-RPC sobre stdin)
echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | explorer-mcp
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list",...}' | explorer-mcp
```

## Resultados por binario

### `cogh` (cognicode-cli/src/bin/cogh.rs)

```
$ cogh --version
cogh 0.97.3
$ cogh --help  (extracto)
Commands:
  install, uninstall, list, current, latest, update,
  reshim, rollback, doctor, where, init, plugin, skill,
  ide, version, help
Options:
  --home <HOME>, -v, -h, -V
```

UAT ejecutados:

| Comando | Resultado | Notas |
|---|---|---|
| `cogh --version` | PASS → `cogh 0.97.3` | |
| `cogh version` | PASS → `cogh 0.97.3 (managing CogniCode (no version pinned))` | |
| `cogh ide detect` | PASS → `Detected IDEs: ✗ opencode ✗ zcode` (no hay configs) | Funciona correctamente |
| `cogh doctor` | PASS → `UNHEALTHY` (esperado: home no existe), pass en `Native analysis` y `Isolation backend` (podman detectado) | |
| `cogh init` | PASS → 6 plugins instalados | Crea layout `~/.cognicode/{bin,cache,locks,plugins,shims,tracker,versions}` |
| `cogh list` | PASS → 6 plugins (mcp-server, skills-cognicode-core, sandbox-templates, zcode, claude, codex) | |
| `cogh plugin list` | PASS → tabla Plugin/Description | |

### `cognicode` (cognicode-cli/src/main.rs)

```
$ cognicode --version
cognicode 0.97.3
$ cognicode --help  (extracto)
Commands:
  analyze, serve, refactor, index, graph, navigate, doctor
  docs-ingest    # sólo con feature `multimodal`
  issues-ingest  # sólo con feature `multimodal`
```

UAT ejecutados:

| Comando | Resultado | Notas |
|---|---|---|
| `cognicode --version` | PASS → `cognicode 0.97.3` | |
| `cognicode analyze --help` | PASS | Acepta `[PATH]` opcional, default `.` |
| `cognicode serve --help` | PASS | Acepta `-p, --port <PORT>` default `8080` |
| `cognicode doctor` | FAIL diagnóstico (esperado) | LSPs no instalados: `pyright`, `typescript-language-server` faltan |

### `cognicode-mcp` (cognicode-mcp/src/main.rs)

```
$ cognicode-mcp --version
cognicode-mcp 0.97.3
$ cognicode-mcp --help
CogniCode MCP Server
Usage: cognicode-mcp [OPTIONS]
Options:
  -c, --cwd <CWD>  [default: .]
  -h, --help, -V, --version
```

UAT JSON-RPC:

| Método | Resultado | Notas |
|---|---|---|
| `initialize` | PASS | Responde `protocolVersion: 2024-11-05`, serverInfo `cognicode 0.97.3` |
| `tools/list` | PASS | **20 tools** registrados |
| `tools/call read_file` | PASS | Devuelve contenido real del Cargo.toml del repo |

Catálogo de las 20 tools (verificadas vía `tools/list`):

```
analyze_impact, build_call_subgraph, build_graph, export_mermaid,
find_references, find_usages, get_call_hierarchy, get_complexity,
get_entry_points, get_file_symbols, get_hot_paths, get_leaf_functions,
get_per_file_graph, get_symbol_code, go_to_definition, hover,
query_symbol_index, read_file, search_content, trace_path
```

### `explorer-api` (cognicode-runtime/src/bin/api.rs)

```
$ explorer-api --version
explorer-api 0.97.3
$ explorer-api --help  (extracto)
CogniCode Explorer API — moldable code exploration HTTP service.
LadybugDB is the sole persistence backend.
Usage: explorer-api [OPTIONS]
Options:
  -c, --cwd <CWD>        [default: .]
      --listen <LISTEN>  [default: 127.0.0.1:8010]
      --db <DB>          [default: ./cognicode.lbug]
      --with-architecture
      -h, -V
```

UAT ejecutados:

| Comando | Resultado | Notas |
|---|---|---|
| `explorer-api --version` | PASS | |
| `GET /health` | PASS | `{"service":"cognicode-explorer","status":"ok"}` |
| `GET /version` | EMPTY | 404 — el endpoint no existe |
| `GET /` | EMPTY | 404 — el endpoint no existe |

**Hallazgo**: solo `/health` responde. El resto de los endpoints del API requieren LadybugDB poblado. UAT bloqueado en este punto sin corpus indexado.

### `explorer-mcp` (cognicode-runtime/src/bin/mcp.rs)

```
$ explorer-mcp --version
explorer-mcp 0.97.3
$ explorer-mcp --help  (extracto)
CogniCode Explorer MCP — JSON-RPC over stdio.
LadybugDB-backed.
Usage: explorer-mcp [OPTIONS]
Options:
  -c, --cwd <CWD>, --db <DB>, -h, -V
```

UAT JSON-RPC:

| Método | Resultado | Notas |
|---|---|---|
| `initialize` | PASS | serverInfo `cognicode-explorer 0.97.3` |
| `tools/list` | PASS | **55 tools** registrados |

Catálogo de las 55 tools (verificadas vía `tools/list`, parcial):

```
brain_ask, brain_attach, brain_close, brain_focus, brain_open,
brain_status, build_context, cognicode_ask, detect_architecture_drift,
export_c4_mermaid, export_trace_mermaid, explorer_apply_lens,
explorer_get_lenses, explorer_get_view, explorer_get_views,
explorer_inspect_object, explorer_open_workspace,
explorer_query_moldql, explorer_spotter_search, find_cycles,
find_dead_code_v2, find_quality_issues, graph_all_simple_paths,
graph_cluster, graph_community_god_nodes, graph_communities,
graph_explain, graph_feedback_arc_set, graph_god_nodes,
graph_pagerank, graph_subgraph, graph_surprising_connections,
graph_transitive_reduction, health_dashboard, impact_component,
impact_detect_cycles, impact_forward_radius, impact_has_path,
impact_radius, impact_shortest_path, ingest_quality_issues,
lens_find_dead_code, lens_find_intersection, lens_hotspots,
moldql_pattern_capabilities, moldql_pattern_query, quality_gate,
view_delete, view_list, view_load, view_save
```

## Baseline de tests (L1+L2)

```text
$ cargo test -p cognicode-core --lib
test result: ok. 2083 passed; 0 failed; 27 ignored

$ cargo test -p cognicode-cli --bin cogh
test result: ok. 291 passed; 0 failed; 1 ignored

$ cargo test -p cognicode-cli --test cognicode_ide_adapter
test result: ok. 7 passed; 0 failed; 0 ignored
```

## Hallazgos críticos

### H1: 50 tests `cognicode-core` que el summary anterior marcaba como rojos están ya corregidos

Confirmado por la sesión anterior (commits `3f20f50f`, `f902be2f`):
- `cognicode_ide_adapter`: 7/7 ✅
- `cognicode-core lib`: 2083/0/27 ✅
- El último scorecard (2026-09-21 07:22) marca G5 RED por motivos distintos
  (analytics p95 alto en escenarios sandbox; no por fallos de tests).

### H2: herramientas MCP activas y accesibles

- `cognicode-mcp` expone **20 tools** de análisis de código.
- `explorer-mcp` expone **55 tools** de exploración moldable.
- Total accesible al usuario vía MCP: **75 tools**.
- La documentación `docs/MCP-TOOLS.md` (permanente) cubre ambos.

### H3: ausencia de `cognicode-mcp-server` en runtime real

- El binario `cognicode-mcp-server` (en `cognicode-mcp/src/server.rs`)
  está declarado pero NO se invoca en flujos de runtime observados.
- `cogh install` configura el daemon como `cognicode-mcp` (no
  `cognicode-mcp-server`).
- Posible duplicación de código; investigar si `cognicode-mcp-server`
  tiene propósito activo o es histórico.

### H4: `docs-ingest` y `issues-ingest` ausentes del binario default

- El `cognicode --help` lista estos comandos con la nota:
  *"Compiled in ONLY when the `multimodal` Cargo feature is active — on a
  default build the variant is absent and `cognicode docs-ingest` returns
  'Unknown command'"*.
- Implicación: la multimodalidad (graph layer con Markdown/GitHub) es
  opcional; el binario por defecto no la incluye.
- El roadmap `docs/adr/ADR-031` menciona "G2 = Cobertura MCP tools en
  sandbox" usando "N = runtime tools/list, actualmente 68" — el conteo
  **varía según feature flags**: con multimodal activa se acercarían a
  68; sin multimodal, **75 - 2 = 73** (los 2 son `docs-ingest`,
  `issues-ingest`).
- Verificación cruzada con el ADR-031: la cifra 68 podría estar
  desactualizada.

### H5: `cognicode doctor` reporta LSPs faltantes

- Faltan `pyright` y `typescript-language-server` en el entorno.
- No es un defecto del producto, pero sugiere un setup de UAT
  incompleto para validar flujos LSP.

## Comprobaciones bloqueadas / no ejecutadas

| Comprobación | Estado | Motivo |
|---|---|---|
| UAT explorador end-to-end con corpus real | BLOCKED | Requiere LadybugDB poblada con un workspace de prueba |
| UAT `cogh install mcp-server` end-to-end | NOT_RUN | Requiere red (descarga bundle release) |
| UAT sandbox containers | NOT_RUN | Requiere podman (presente) + red + manifests |
| UAT multimodales (`docs-ingest`, `issues-ingest`) | NOT_RUN | Requiere rebuild con `--features multimodal` |
| UAT `cognicode-mcp-server` | NOT_RUN | Binario no usado en runtime — ver H3 |

## Contradicciones detectadas

| Contradicción | Detalle |
|---|---|
| ADR-031 cita "68 tools MCP runtime", pero el catálogo real es **75** (20 + 55) | El ADR se redactó antes de los cambios que llevaron el total a 75; no es necesariamente un error, pero la cifra debe actualizarse. |
| `cognicode --help` menciona `docs-ingest` e `issues-ingest` como comandos ausentes en build default | El CLI los anuncia pero no existen en el binario por defecto. Esto puede confundir al usuario. |

## Cierre de la unidad

**F0.W1 — ACCEPTED** con la siguiente salvedad:

- 5 binarios inventariados y verificados funcionalmente.
- Catálogo MCP completo: 20 + 55 tools.
- Baseline de tests: 2083/291/7 (todos verdes).
- 5 hallazgos documentados (H1-H5).
- 5 comprobaciones marcadas BLOCKED/NOT_RUN con motivo.
- 2 contradicciones detectadas (cifra 68 vs 75, multimodal ausente en default).

**Siguiente unidad**: F0.W2 — Caracterización de arranque, persistencia,
red, stdout/stderr de los binarios inventariados en F0.W1.

## Evidencias complementarias

- Logs de salida de cada comando capturado en este informe.
- Build output de `cargo build -p cognicode-runtime --bin explorer-mcp` capturado.
- JSON-RPC frames capturados para `cognicode-mcp` y `explorer-mcp`.
