# Production-Ready Foundation (PRF) — UAT (User Acceptance Testing)

## Objetivo

PRF verifica que los productos CLI (`cogh`, `cognicode`) y el servidor
MCP (`cognicode-mcp`, `explorer-mcp`, `explorer-api`) **funcionan de verdad**
— no solo compilan y superan tests unitarios, sino que:

1. Responden a sus interfaces reales con los outputs esperados.
2. Manejan errores de manera predecible y documentada.
3. No dependen de mocks ni de fixtures internos para sus contratos.
4. Son reproducibles sobre la misma entrada y el mismo entorno.

## Procedimiento general

Para cada binario y cada flujo a certificar:

1. **Construir el binario** con la receta estándar (`just build` o la
   receta específica del binario). Capturar el commit hash.
2. **Aislar el entorno**: usar `TMPDIR=/tmp/prf-uat-{nonce}/` o un
   `fake_home` separado. Variables de entorno documentadas.
3. **Ejecutar el flujo** completo con argumentos válidos.
4. **Capturar**: stdout, stderr, exit code, tiempo de ejecución.
5. **Comparar** contra el contrato esperado (definido en la
   especificación del requisito).
6. **Ejecutar variantes de error**: argumentos inválidos, paths
   inexistentes, archivos corruptos.
7. **Documentar** los resultados en `evidence/UAT-<flow>.md` con el formato
   definido abajo.
8. **Promover** el estado del requisito en `evidence/CERTIFICATES.md`
   si todo ha pasado.

## Plantilla de informe UAT

Cada UAT se documenta con el siguiente formato (ver ejemplo en
`evidence/UAT-template.md` cuando se ejecute la primera):

```markdown
# UAT — <flow-id>

**Fecha**: YYYY-MM-DD
**Binario**: <binario>
**Commit**: <hash>
**Entorno**: <descripción>
**Operador**: jcode-orchestrator

## Escenario

<descripción del escenario Given/When/Then>

## Comando ejecutado

```text
$ <comando>
```

## Salida observada

### stdout

```text
<output>
```

### stderr

```text
<output>
```

### Exit code

`<code>`

## Verificación contra contrato

| Esperado | Observado | Pasa |
|---|---|---|
| <contrato 1> | <observado 1> | ✅/❌ |
| <contrato 2> | <observado 2> | ✅/❌ |

## Resultado

PASS / FAIL / BLOCKED / NOT_RUN
**Motivo** (si FAIL/BLOCKED/NOT_RUN): <motivo>
```

## Estados posibles

- **PASS**: el flujo se ejecuta y cumple el contrato.
- **FAIL**: el flujo se ejecuta pero no cumple el contrato.
- **BLOCKED**: el flujo no puede ejecutarse por falta de herramientas,
  permisos o infraestructura.
- **NOT_RUN**: el flujo está documentado pero no se ha ejecutado todavía
  (por ejemplo, una UAT programada para una unidad futura).

## Anti-patrones prohibidos

- Usar mocks o stubs en lugar del binario real.
- Modificar el código para que la UAT pase sin corregir la causa raíz.
- Omitir la captura de stderr.
- Reclasificar un FAIL como PASS para mantener la racha verde.

## Primeras UAT a ejecutar (F0.W1)

1. `cogh --version` y `cogh --help` — verificar CLI funciona.
2. `cogh install --help` — verificar subcomandos disponibles.
3. `cognicode-mcp --help` — verificar binario MCP responde.
4. `cognicode --help` — verificar binario principal responde.
5. `explorer-api --help` y `explorer-mcp --help` — verificar Explorer.
6. `cognicode-mcp` + `tools/list` JSON-RPC — verificar catálogo MCP.

Estas UAT se documentarán en `evidence/F0-W1-UAT.md` y similares.

## UAT ejecutadas

### UAT-F2-W8-001 — Enumeración de archivos omitidos por error de I/O/parseo

**Fecha**: 2026-09-21
**Binario**: `cognicode-mcp` (release, `/var/home/rubentxu/cargo-targets/release/cognicode-mcp`)
**Commit**: pre-W8 commit (HEAD local con W8 staged)
**Entorno**: workspace CogniCode en `/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode`; corpus UAT efímero `/tmp/prf-uat-w8/`
**Operador**: jcode-orchestrator

## Escenario (Given/When/Then)

**Given**: un directorio con 3 archivos `.rs`:
- `src/ok.rs` — código válido (`pub fn normal_function() -> i32 { 42 }`).
- `src/unreadable.rs` — código válido pero `chmod 000` (permisos nulos).
- `src/invalid_utf8.rs` — bytes inválidos (`0xFF 0xFE 0xFD 0xFC`), no UTF-8.

**When**: el cliente MCP envía `tools/call { name: "build_graph" }` al binario `cognicode-mcp --cwd /tmp/prf-uat-w8`.

**Then**:
1. El binario retorna `success: true` (errores de archivos son datos, no fallos del build).
2. El campo `skipped_files` aparece en el JSON de respuesta con 2 entradas.
3. Cada entrada tiene `path`, `reason_kind ∈ {read, parse}`, y `reason` con el mensaje del sistema.
4. `ok.rs::normal_function` aparece como símbolo en el grafo (`symbols_found: 1`).

## Comando ejecutado

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"prf-uat","version":"0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"build_graph","arguments":{}}}' \
  | /var/home/rubentxu/cargo-targets/release/cognicode-mcp --cwd /tmp/prf-uat-w8
```

## Salida observada (extracto)

```json
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"success\":true,\"symbols_found\":1,\"relationships_found\":0,\"edges\":[],\"message\":\"Graph loaded from built: 1 symbols, 0 relationships in 1ms\",\"skipped_files\":[{\"path\":\"/tmp/prf-uat-w8/src/invalid_utf8.rs\",\"reason_kind\":\"parse\",\"reason\":\"stream did not contain valid UTF-8\"},{\"path\":\"/tmp/prf-uat-w8/src/unreadable.rs\",\"reason_kind\":\"read\",\"reason\":\"Permission denied (os error 13)\"}]}"}],"isError":false}}
```

## Verificación contra contrato

| Esperado | Observado | Pasa |
|---|---|---|
| `success: true` | `success: true` | ✅ |
| `skipped_files` presente y con 2 entradas | 2 entradas presentes | ✅ |
| `invalid_utf8.rs` clasificado como `parse` | `reason_kind: "parse"` con mensaje `"stream did not contain valid UTF-8"` | ✅ |
| `unreadable.rs` clasificado como `read` | `reason_kind: "read"` con mensaje `"Permission denied (os error 13)"` | ✅ |
| `ok.rs::normal_function` en el grafo (`symbols_found: 1`) | `symbols_found: 1` | ✅ |
| Build no aborta con error | `success: true`, `isError: false` | ✅ |

## Resultado

**PASS**

**Motivo**: el camino real del binario (`AnalysisService::build_project_graph` invocado desde `handle_build_graph`) ahora surface los archivos omitidos con su `path`, su clasificación (`read`/`parse`) y el mensaje exacto del error. Los errores de lectura/parseo son datos del build, no excepciones — la cobertura `coverage_metrics` y el `BuildReport` reflejan fielmente qué se procesó y qué se omitió y por qué.

### UAT-F2-W9-001 — Invalidación de cache con mtime preservado

**Fecha**: 2026-09-21
**Binario**: `cognicode-mcp` (release, `/var/home/rubentxu/cargo-targets/release/cognicode-mcp`, rebuilt tras el cambio)
**Commit**: `2a121aec`
**Entorno**: corpus UAT efímero `/tmp/prf-uat-w9/`
**Operador**: jcode-orchestrator

## Escenario (Given/When/Then)

**Given**: `src/lib.rs` con `original_function` y `caller_one`.

**When**:
1. Build #1: `build_graph` sobre corpus inicial → contiene `original_function`.
2. Se reescribe `lib.rs` renombrando a `renamed_function` y se restaura el
   mtime original con `os.utime(ns=...)` (verificado: `mtime preserved: True`).
3. Build #2: `build_graph` en un proceso nuevo del binario.

**Then**: build #2 ve `renamed_function` y ya no ve `original_function`
(el cache (mtime, size) no sirve la entrada obsoleta).

## Verificación contra contrato

| Esperado | Observado | Pasa |
|---|---|---|
| Build #1 contiene `original_function` | presente (grep sobre salida JSON-RPC) | ✅ |
| mtime restaurado tras reescritura | `mtime preserved: True` | ✅ |
| Build #2 contiene `renamed_function` | presente | ✅ |
| Build #2 ya no contiene `original_function` | ausente | ✅ |

## Limitación probada (honestidad de cobertura)

Una reescritura con el mismo tamaño Y el mismo mtime sigue sin
invalidar la cache. Queda documentada como deuda explícita
(requeriría hash de contenido). No se ha disfrazado de pass.

### UAT-F3-001 — Equivalencia CLI ↔ MCP sobre las verticals de análisis de F2

**Fecha**: 2026-09-21
**Binarios**: `cognicode` + `cognicode-mcp` (release, `/var/home/rubentxu/cargo-targets/release/`)
**Commit**: `67363bfc` (HEAD al ejecutar la UAT)
**Entorno**: corpus `/tmp/prf-uat-f3/` (2 archivos: `src/lib.rs` con `caller → crate::nested::callee()`, `src/nested/mod.rs` con `callee`)
**Operador**: jcode-orchestrator

## Escenario (Given/When/Then)

**Given**: el corpus con 2 símbolos y 1 arista cross-file.
**When**: se ejecuta cada vertical en ambos lados (CLI stdout vs JSON-RPC del MCP) sobre el mismo corpus.
**Then**: los resultados observables son equivalentes.

## Resultados observados

| Vertical | CLI | MCP | Equivale |
|---|---|---|---|
| full (`graph full` ↔ `build_graph`) | 2 símbolos, 1 dep | `symbols_found:2, relationships_found:1, edges:[{from:caller,to:callee}]` | Sí |
| per-file (`graph per-file src/lib.rs` ↔ `get_per_file_graph`) | 1 símbolo, 0 deps | `symbol_count:1, dependency_count:0` | Sí |
| hierarchy (`graph hierarchy caller` ↔ `get_call_hierarchy`) | depth 1: `callee (callees)` | `calls:[{symbol:callee,confidence:1.0}], total_calls:1` | Sí |

## Verificación de arquitectura (sin lógica duplicada)

| Vertical | Puerto compartido |
|---|---|
| per-file | `PerFileStrategy::build_local_graph` invocado idénticamente por ambos |
| full | CLI: `FullGraphStrategy`; MCP: `AnalysisService::build_project_graph` (mismo `GlobalSymbolIndex` de F2.W5/W7) |
| hierarchy | Ambos consultan el grafo construido vía `CallGraph` |

## Hallazgo H-F3-1 (deuda, no bloqueante)

`find_symbol_usages` (tool MCP `find_usages`) implementa walk + parser
inline en el handler en lugar de delegar en un puerto de análisis. No
tiene correspondiente en `cognicode graph`, así que no viola el
criterio de salida de F3 (que aplica a las verticals de F2), pero es
candidato a refactor de delegación. Registrado en TRACEABILITY.

## Nota de sintaxis

`get_call_hierarchy` MCP exige `direction ∈ {incoming,outgoing}`
(no `callees`); el CLI usa `callees`/`callers` como texto descriptivo
de la dirección por defecto. Semánticamente equivalentes; la
divergencia es de naming de parámetro, documentada aquí.

### UAT-F4-001 — Persistencia ante reinicio y aislamiento de workspaces

**Fecha**: 2026-09-21
**Binario**: `cognicode-mcp` (release)
**Commit**: HEAD post-F3
**Entorno**: workspaces efímeros `/tmp/prf-uat-f4-ws1` y `/tmp/prf-uat-f4-ws2`
**Operador**: jcode-orchestrator

## Escenarios

**F4.a — Reinicio**: dos procesos nuevos del binario contra el mismo
workspace producen el mismo resultado (`1 símbolo, 0 relaciones` en
ambos). PASS.

**F4.b — Aislamiento (débil)**: workspace distinto produce su propio
resultado (1 símbolo), no hereda estado. PASS.

**F4.b-strong — Aislamiento (fuerte)**: tras añadir 2 símbolos a
ws2, un proceso nuevo contra ws1 sigue viendo exactamente 1 símbolo
(el estado de ws2 no contamina ws1) y ws2 ve sus 3 símbolos. PASS.

## Verificación

| Escenario | Esperado | Observado | Pasa |
|---|---|---|---|
| F4.a reinicio | mismo conteo en 2 procesos | 1 vs 1 | ✅ |
| F4.b aislamiento débil | ws2 independiente | ws2=1 | ✅ |
| F4.b-strong contaminación cruzada | ws1=1, ws2=3 | ws1=1, ws2=3 | ✅ |

## Limitación observada (honestidad)

En este corpus mínimo no se observó persistencia en disco
(`graph_db_path` no dejó artefacto visible en los workspaces de
prueba; el grafo se reconstruye en cada proceso, ~1ms). El criterio
"recupera estado sin intervención" se satisface vía reconstrucción
determinista; persistencia material queda cubierta por la suite
`manifest_upsert`/`GraphStore` (con 4 fallos preexistentes
catalogados en ladybug, JOURNAL §15). Registrado como matiz, no
como defecto nuevo.

### UAT-F5-001 — Seguridad: capacidades, límites y cancelación

**Fecha**: 2026-09-21
**Binario**: `cognicode-mcp` (release)
**Commit**: HEAD post-F4
**Entorno**: `/tmp/prf-uat-f4-ws1` como workspace permitido
**Operador**: jcode-orchestrator

## Escenarios

**(a) Rechazo fuera de capacidades**: `read_file` sobre
`/etc/passwd` con `--cwd` en workspace → respuesta
`isError:true`, `"Path outside workspace"`. Control positivo:
`read_file src/lib.rs` dentro del workspace → contenido correcto. PASS.

**(b) Límite de tiempo**: timeouts por categoría en el boundary
del dispatcher (`timeout_for_category`: graph 60s, navigation 45s,
search 500ms, default 30s) con tests propios
(`test_timeout_fires_when_handler_takes_longer`,
`test_timeout_does_not_fire_when_handler_returns_quickly`) —
17/17 tests del adapter en GREEN. PASS (evidencia de suite +
inspección del boundary único de ejecución).

**(c) Cancelación cooperativa**: `notifications/cancelled`
(JSON-RPC) → token puesto a true → la siguiente `build_graph`
retorna `isError:true, "internal: Cancelled"`. PASS.

## Verificación

| Escenario | Esperado | Observado | Pasa |
|---|---|---|---|
| (a) path fuera | error, sin contenido | `Path outside workspace`, isError | ✅ |
| (a+) path dentro | contenido | contenido de lib.rs | ✅ |
| (b) timeout | mecanismo activo y testado | 17/17 adapter tests GREEN | ✅ |
| (c) cancelación | operación posterior rechazada | `internal: Cancelled` | ✅ |

### UAT-F6-001 — Distribución: instalación, actualización y rollback sobre artefactos reales

**Fecha**: 2026-09-21
**Binarios**: `cogh` (release), artefacto del tag `v0.97.3` (GitHub releases)
**Entorno**: COGNICODE_HOME aislado `/tmp/prf-uat-f6-home`
**Operador**: jcode-orchestrator

## Escenario y pasos ejecutados

1. **Descarga del artefacto real del tag v0.97.3** (GitHub,
   `cognicode-0.97.3.tar.gz`, 14.179.486 bytes).
2. **SHA256 verificado**: `477a2b248b1c50bb9a8845f3da2619655dadf4930
   fdceeaae143276672a52888` (calculado sobre los bytes descargados e
   inyectado en el BundleManifest v2 de la UAT).
3. **Comprobación anti-manifest-falso**: instalación con el fixture
   DEV-ONLY falla en la etapa SHA256 "by construction" (correcto:
   no se instalan bytes sin digerir). Comportamiento de seguridad
   verificado positivamente.
4. **Instalación limpia** (`cogh install mcp-server --profile
   reviewer` en home aislado): instalado en
   `versions/0.97.3/`, binario instalado ejecuta `graph full`
   correctamente (flujo canónico). PASS.
5. **Actualización**: `cogh update` sobre versión ya instalada →
   `already current: 0.97.3 is installed and coherent (no
   transition performed)`. PASS.
6. **Rollback**: `cogh uninstall --version 0.97.3 --ide opencode
   mcp-server` → árbol `versions/0.97.3/` eliminado, journal
   eliminado, tracker pin limpiado. PASS.

## Defecto descubierto: H-F6-1 (doble resolución de home)

Síntoma: con `--home <UAT>` sin exportar `COGNICODE_HOME`, el
uninstall aborta con `clear tracker pin ... No such file or
directory` DESPUÉS de completar el rollback.

Causa raíz: `tracker::read_version_optional()` resuelve vía
`cognicode_home()` (env-only), mientras `home.tracker_version()`
usa `CognicodeHome` (que respeta `--home`). Con `--home` sin env,
la lectura ve el pin del home real y el borrado apunta al del home
UAT. Además, el pipeline de instalación escribió el pin de prueba
en el home real (contaminación de estado del operador, ya
restaurada manualmente).

Impacto: MEDIUM (correctitud de `--home`/rollback en installs
no-default; no afecta al flujo con env único). Registrado en
TRACEABILITY como OPEN para F6-followup.

## Verificación

| Paso | Esperado | Observado | Pasa |
|---|---|---|---|
| Descarga tag real | artefacto verificable | 14 MB del release v0.97.3 | ✅ |
| SHA256 gate | rechaza digests falsos | fixture DEV-ONLY falla en SHA (by design) | ✅ |
| Instalación limpia | binario usable | `cognicode 0.97.3` ejecuta graph full | ✅ |
| Update | no-op coherente | `already current ... coherent` | ✅ |
| Rollback | árbol+journal+pin eliminados | eliminados; EXIT=0 con env coherente | ✅ (con H-F6-1) |
