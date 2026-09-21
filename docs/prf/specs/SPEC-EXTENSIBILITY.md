# PRF-EXT — Extensibilidad sin reescribir el motor

- **PRF-EXT-01 MUST:** una capacidad estable se describe con id, versión, estabilidad, lenguajes, permiso read/write/execute/net y budgets cuando correspondan. Metadata es consistente entre documentación, `tools/list` y `cognicode` donde la capability exista.
- **PRF-EXT-02 MUST:** CLI y MCP usan el mismo servicio de aplicación y puertos neutrales de transporte, sin que dominio/aplicación importen tipos `rmcp`, HTTP/gRPC o `interface::mcp`.
- **PRF-EXT-03 MUST:** demostrar la incorporación de una capacidad sintética read-only y sus test contractuales sin modificar en varios lugares el dispatcher core ni romper versiones anteriores. No crear registry genérico si una tabla tipada sencilla satisface el caso.
- **PRF-EXT-04 MUST:** los adapters de backend no son fuente de verdad alternativa. Una extensión no gana autoridad por registro ni puede degradar la semántica de `Unknown`.
- **PRF-EXT-05 MUST:** un nuevo puerto/abstracción requiere test que reproduzca acoplamiento/duplicidad y mejora medible (reducción de cambio conjunto, testabilidad o tiempo); no introducir DI, plugin runtime o event bus por anticipación.
- **PRF-EXT-06 MUST:** compatibilidad old-client/new-binary y contract tests cubren schema/version/capabilities; rolling upgrade no sustituye una UAT de rollback.

**Certificación:** C3/C5; U08,U10,U26. gRPC, daemon, packs y UI se posponen; reutilizar contratos neutrales al surgir consumidor real.
