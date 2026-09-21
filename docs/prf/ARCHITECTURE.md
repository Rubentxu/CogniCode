# Arquitectura de la base de producto

## Límite del producto estable

```
Cliente CLI                         Cliente MCP (stdio)
  | argv, stdout/stderr               | JSON-RPC, stdout puro
  +------------+----------------------+ 
               v
         application use case
  (workspace, identity/basis, analysis, coverage,
   typed outcomes, cancellation/limits, capabilities)
               |
      ports de dominio/aplicación
               |
    +----------+--------------+
    | parsers/LSP/providers   | storage/index/cache (ownership claro)
    +-------------------------+ 
```

`cogh` instala/verifica/actualiza; **no es el motor de análisis**. `explorer-mcp` y `explorer-api` no se eliminan sin matriz y migración de clientes. El runtime es composition root y único lugar de ensamblaje de adaptadores.

## Decisiones de estabilidad

- **Estado**: seleccionar explícitamente la fuente canónica por capacidad. Un index/grafo derivado puede reconstruirse y advertir cobertura; LSI aporta facts/identidad/evidencia cuando la operación realmente los usa. No fingir persistencia de hechos por existir un adaptador in-memory.
- **Basis**: para cada análisis que prometa reproducibilidad, identidad workspace canónica + config digest + source manifest/revisión + provider/graph revision; si faltan datos reportar `Incomplete`. `mtime` solo es optimización, no certificado de igualdad de bytes.
- **Concurrencia**: cada workspace/config dispone de locks, cachés y namespace; dos procesos sobre misma ubicación no deben pisar write locks, ni compartir resultados de otro root.
- **Ports**: los casos de uso no importan `interface::mcp` ni concreciones de infraestructura. Refactor con caracterización de outputs, sin gran-bang rewrite.
- **Contratos**: `CapabilityDescriptor` mínimo por feature (id, versión, lenguaje, permiso, estabilidad, budget), mapeo CLI/MCP; estabilidad del comportamiento, no idéntico layout de argumentos.
- **Seguridad**: repository input no fiable; canonicalización y enforcement en frontera de lectura/escritura; read/modify/execute/net permisos independientes; rechazar traversal/symlink fuera; sin escapes ni secrets en logs/respuestas.
- **Operación**: core sin collector, daemon o cloud. Telemetría opt-in, errores tipados, salida MCP solo datos del protocolo; budgets y cancelación por solicitud; garantía de cierre y limpieza.

## Estrategia de migración

Primero capturar contra binarios actuales el corpus/baseline y los resultados adversariales. Después adaptar una vertical real, comparar old/new, mantener wrappers para APIs publicadas, ensayar rollback y solo entonces eliminar duplicidad demostrada. RPC/Control Plane deberán **consumir** esta misma capa más adelante si un cliente real los justifica; no crear API gRPC ni backend nuevo durante PRF.
