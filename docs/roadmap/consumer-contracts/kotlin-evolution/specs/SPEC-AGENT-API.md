# SPEC-K-API — Experiencia agent-first sin sobreanalizar ni inventar arquitectura (K0–K4)

## Invariantes CLI/MCP

- **K-API-01 MUST:** descubrir operaciones reales mediante catálogo/`tools/list` y validar `input_schema`; no codificar nombres CLI no existentes. En el catálogo actual aparecen `get_file_symbols`, `query_symbol_index`, `get_symbol_code`, `find_references`, `build_call_subgraph`, `graph_explain`, `get_implementors`, etc. **No todas exigen lo mismo:** el catálogo de `product/tools.json` es referencia generada y la sesión debe inspeccionar runtime actual.
- **K-API-02 MUST:** requests `ask/inspect/query` referidos a símbolos devuelven `question`, `resolvedSymbols`, `observedRelations`, `evidenceRefs`, `coverage`, `provider/tier`, `gaps`, `nextActions` y `analysisBasis` cuando estén disponibles; no duplicar bytes de código que el consumidor no pidió.
- **K-API-03 MUST:** el costo para responder una pregunta se minimiza: símbolo si basta, referencias si pide colaboradores, subgrafo/path para interacción, comunidades solo si hace falta agrupar. `build_graph` no es prerequisito de pregunta cuya operación no lo necesita.
- **K-API-04 MUST:** un fallo de graph no autoriza `read_file(raw)` / `search_content` para **reconstruir manualmente** la arquitectura. La búsqueda textual puede señalar candidatos que requieren confirmación estructurada. Si no hay capacidad, `Unsupported/Partial/Unknown`, sin dibujos inventados.
- **K-API-05 MUST:** evidencias de relaciones exigen identificadores de extremos, relación concreta y fuente. Callgraph estático no demuestra runtime ni deployment. Proyecciones/Mermaid/C4 siguen ADR-016; el renderer no decide hechos.
- **K-API-06 MUST:** resultados consistentes en CLI y MCP para el mismo core/snapshot; stdout MCP solo JSON-RPC y logs a stderr. Autoridad read/write/execute/network se mantiene separada, perfil agent-safe por defecto.
- **K-API-07 MUST:** compatibilidad pública: ampliar DTO de forma aditiva o mantener versionado; ejecutar contrato de consumidor N-1 cuando cambien esquemas de herramientas.
- **K-API-08 MUST:** `Partial` incluye razones relevantes (archivos omitidos, símbolos no resueltos, índices stale) y posible siguiente operación estructurada; no transitar a `Complete` porque el número de nodos es mayor que cero.

## Ejemplo conceptual de respuesta (no inventar operación pública)

```json
{
  "question": "who calls kotlin Service.run?",
  "basis": {"state":"Current","ref":"<observed>"},
  "support": {"language":"kotlin", "operation":"references", "tier":"S2"},
  "symbols": ["observed-symbol-id"],
  "relations": [{"id":"edge-observed", "source":"observed-id", "target":"observed-id", "kind":"Calls", "evidence":"query-id"}],
  "gaps": [], "coverage":"Partial", "nextActions":[]
}
```

Los nombres son propuesta semántica y no reemplazan automáticamente schemas publicados. Los handlers existentes adaptan DTOs de aplicación, no vuelven a resolver AST ni dependen de `tree_sitter`.
