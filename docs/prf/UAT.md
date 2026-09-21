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
