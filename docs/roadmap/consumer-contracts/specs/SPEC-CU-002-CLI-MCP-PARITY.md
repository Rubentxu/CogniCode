# SPEC-CU-002 — Equivalencia semántica, contratos públicos y affordances CLI/MCP

**Estado:** PROPOSED · **Bloques:** CU-1, CU-3 · **ADR:** `ADR-CU-002`

## Intención

El operador CLI, el agente MCP y el consumer SDK no deberían necesitar conocer motores o identificadores internos diferentes. La misma solicitud, sobre el mismo snapshot y perfil, tiene el mismo significado; solo difiere la presentación/transport.

## Contrato

- **CU-002.1** `Analyze`/servicio existente constituye la autoridad única de la ejecución. CLI `cognicode` y MCP `cognicode-mcp` son adaptadores. Respetar ADR-012: **no escindir binarios** ni rehacer `rmcp_adapter` salvo evidencia de acoplamiento concreto.
- **CU-002.2** `tools/list`, `cognicode capabilities --json` y `product/tools.json` derivan de catálogo existente. Añadir metadatos solo si cubren un uso concreto: `intent`, `required_args`, `preconditions`, `follow_ups`, `precision`, `authority`, `completeness`, `estimated_cost` (estimación no SLA) y `schema_version`.
- **CU-002.3** Output tipado con `schema_version`, `basis/snapshot`, `state`, `coverage`, `result`, `gaps/errors`, `provenance` y `_links` cuando apliquen. Reutilizar output schemas derivados del tipo Rust. Si una tool no posee schema honesto, declararlo explícitamente, no publicar un `{}` aproximado.
- **CU-002.4** No exigir igualdad byte a byte entre JSON de CLI y frames MCP: normalizar por un DTO dominio estable y comparar entidades, orden lógico, precision, cobertura, reason codes. Evitar timestamps o telemetry volátiles en el comparador.
- **CU-002.5** `read` != `write` != `execute` != `network`; un `follow_up` jamás concede permiso. Errors de permiso no se degradan a lista vacía ni se filtran del catálogo a escondidas.
- **CU-002.6** API de ayuda: siguiente acción opcional HATEOAS-like (`rel`, referencia a herramienta/intent existente, argumentos mínimos, reason, preconditions). Debe ser resoluble desde catálogo; herramienta no anunciada o feature desactivada = `Unavailable`, nunca enlace muerto.
- **CU-002.7** `stdout` de MCP contiene solo JSON-RPC, logs/traces por `stderr`; CLI JSON representa una sola respuesta bien formada.

## Ejemplo de contrato de navegación (esquema orientativo)

```json
{"state":"partial","coverage":{"requested":5,"produced":3},"gaps":[{"capability":"call_graph","reason":"backend_unavailable"}],"_links":[{"rel":"inspect-prerequisites","tool":"capabilities","required_args":{}}]}
```

## Garantías mínimas

- Un cliente obtiene la misma semántica vía CLI y MCP, incluso cuando el grafo falta.
- `tools/list` y catálogo filtrado por features coinciden.
- Un agente sin conocimiento previo descubre qué hacer después cuando una llamada es incompleta.
- Traces/cancel no contaminan stdout del servidor MCP.

**Pruebas:** 1 test permanente de mapping resultado→wire y uno catálogo/schema; smoke CLI+MCP temporal en CU-1/CU-3. Referencias: `product/tools.json`, `crates/cognicode-core/src/interface/{catalog.rs,mcp/output_contracts.rs,intent.rs}`, `odd/tasks/b3-agent-code-understanding.md`, `docs/roadmap/adr/ADR-012-st04-cli-mcp-separation.md`.
