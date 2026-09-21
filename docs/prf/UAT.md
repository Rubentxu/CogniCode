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
