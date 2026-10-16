# SPEC-CU-004 — Consultas progresivas, explicación e impacto para agentes

**Estado:** PROPOSED · **Bloque:** CU-3 · **ADR:** `ADR-CU-002`

## Casos de usuario

1. «¿Dónde se define y usa esta función?» → identidad desambiguada, evidencia navegable, límites.
2. «¿Qué puede romper esta modificación?» → alcance de impacto, dirección real de aristas, precisión y dependencias desconocidas.
3. «¿Por qué Guard observó este fallo?» → fuente, línea/rango, detector, snapshot, evidencia, gaps y siguiente paso sin inventar permisos.

## Contrato

- **CU-004.1** Investigar los métodos ya publicados (`inspect`, `codebase_map`, `smart_search`, `get_symbol_code`, `read_file`, `graph_explain`, `impact`): extenderlos o componerlos, **no duplicarlos**.
- **CU-004.2** Respuesta compacta por defecto con `max_items`, `max_bytes` o `token_budget` claros; datos truncados deben llevar `truncated:true`, `next_cursor` y `total` cuando sea computable.
- **CU-004.3** Cursores opacos asociados a `snapshot`, tipo de consulta, filtros/orden y versión del schema. Alteración, cursor obsoleto o cambio de scope devuelve error tipado. Páginas concatenadas equivalen a resultado completo dentro de límites declarados (sin duplicados ni omisiones).
- **CU-004.4** Un `Unknown` de subgrafo o dependencia no debe traducirse a «no hay dependencia». El impacto es evidencia, no veredicto. `precision_floor` y `source/provenance` viajan hasta el agente.
- **CU-004.5** Next actions derivadas del catálogo; si falta un prerequisito, señalar herramienta habilitada que lo resuelve. Evitar ciclos infinitos en affordances, promesas `available` incorrectas y cadenas `follow_up` sin terminal.
- **CU-004.6** Priorizar respuesta en una llamada ligera de consulta; no arranque de daemon/gRPC nuevo, no ejecución de refactor ni red sin consentimiento y alcance.

**Tests permanentes:** ley cursor-paginación + property de `Unknown` no equivale a NoImpact. **UAT temporal:** agente completa una de las 3 investigaciones por CLI y MCP sin acceso a docs privadas.

**Referencias:** `odd/tasks/b3-agent-code-understanding.md`, `odd/tasks/b4-impact-architecture-quality.md`, `crates/cognicode-core/src/interface/intent.rs`, `src/impact/composite.rs`, `product/tools.json`.
