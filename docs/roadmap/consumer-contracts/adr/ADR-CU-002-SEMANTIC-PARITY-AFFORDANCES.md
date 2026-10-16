# ADR-CU-002 — Una semántica de análisis, múltiples transportes y affordances reales

**Estado:** PROPOSED · **Bloques:** CU-1, CU-3

## Contexto

B2/B3 publicaron `AnalysisOutcome`, catálogo `product/tools.json`, `route_intent`, schemas derivados y navegaciones progresivas. Se evita añadir un registro nuevo o separaciones de binario que contradigan `docs/roadmap/adr/ADR-012-st04-cli-mcp-separation.md`.

## Decisión propuesta

- El caso de uso de aplicación produce un resultado tipado; CLI, MCP y SDK lo adaptan. Comparar paridad por modelo normalizado, no bytes arbitrarios, y preservar `Partial|Unknown|Unsupported|Failed`.
- Affordances HATEOAS-like derivadas del **catálogo de herramientas existente**, expresadas como lista de acciones posibles (`rel/tool/args/precondition/authority`). Una tool no disponible no aparece como invocable; la ausencia explica razón.
- Paginación y compactación solo cuando la carga lo justifique. Cursores anclados en snapshot, filtros, orden y schema; no mezclar sesiones ni workspaces.
- `MCP stdout` reservado a protocolo. Errores tipados proporcionan next actions utilizables. Permisos no se elevan por presencia de enlace.

## Alternativas

- Crear `guard-next-actions` interno de CogniCode: duplicidad de catálogo; rechazado.
- Una API REST extra: no hay necesidad actual probada; diferir.
- Paridad estrictamente textual CLI/MCP: falla por orden/presentación; usar igualdad semántica.

**Consecuencias:** mejora descubrimiento, costo de tokens y UX sin ampliar infraestructura. Aceptación por agente y test CLI/MCP real; revertir nuevas affordances de forma aditiva si causan impacto.
