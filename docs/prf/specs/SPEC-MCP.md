# PRF-MCP — Servidor MCP stdio de producción

**Producto:** `cognicode-mcp` publicado. `explorer-mcp` conserva compatibilidad mientras se inventaría cliente real; no desaparecerá ni se declarará equivalente sin prueba.

- **PRF-MCP-01 MUST:** `initialize`, `tools/list`, `tools/call`, errores y terminación cumplen el protocolo soportado declarado; ejecutar con cliente externo real y capturar mensajes.
- **PRF-MCP-02 MUST:** stdout exclusivamente JSON-RPC. Logs, traces, diagnósticos y errores de inicialización van a stderr. Ninguna salida extra ni pánico destruye framing. El cliente puede cerrar servidor sin huérfanos.
- **PRF-MCP-03 MUST:** operaciones core read-only arrancan sin red, OTLP, proceso Explorer, backend remoto ni Podman. Si la telemetría opt-in falla, el análisis sigue o informa degradación, sin bloquear inicio.
- **PRF-MCP-04 MUST:** cada herramienta estable posee esquema validado, permisos, versiones/estabilidad y límites; mensaje de error tipado ante argumentos inválidos, lenguaje no soportado, timeouts o cobertura parcial.
- **PRF-MCP-05 MUST:** una herramienta que escribe/ejecuta/red requiere autoridad diferenciada y nunca la obtiene por texto dentro del repo o petición del modelo. Herramientas previas se conservan hasta resolver compatibilidad en F0.
- **PRF-MCP-06 MUST:** cancelación/desconexión de trabajo costoso libera recursos según el contrato; no servir datos stale como nuevos ni respuestas de otro workspace.
- **PRF-MCP-07 MUST:** cualquier cambio a herramientas existentes prueba cliente anterior vs servidor nuevo o sigue deprecación aceptada, manteniendo identidad y esquema salvo transición explícita.

**Certificación:** C0/C1/C3/C5. U02,U04,U05,U08,U10,U19,U25,U26. Tests de biblioteca por sí solos no cierran la UAT.
