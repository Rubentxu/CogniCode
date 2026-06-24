# ADR-007: No WASM in Browser

**Fecha:** 2026-06-12
**Estado:** ACCEPTED (enmendado por [ADR-047](ADR-047-wasm-shared-compute-amendment.md) el 2026-06-24)
**Decisión:** No WebAssembly en el browser para duplicar lógica de backend
**Fuente:** grill-with-docs + gap analysis vs gtoolkit
**Confianza:** alta

---

## Context

Durante la sesión de gap analysis vs gtoolkit (feenkcom/gtoolkit), surgió la pregunta de si CogniCode debería usar WebAssembly para ejecutar lógica de visualización o procesamiento en el browser.

gtoolkit usa Bloc (un engine de visualización en Pharo) compilado a WASM para su frontend. Existe la posibilidad de portar este engine a Rust/WASM para复用 lógica de visualización.

## Decision

**Descartar WASM en el browser para duplicación.** La arquitectura de CogniCode será:

- **Backend**: Axum server + MCP tools — produce layouts determinísticos (tree)
- **Frontend**: React + Cytoscape/D3 — aplica layouts interactivos (force-directed)
- **Regla §1**: Nunca duplicar lógica de backend en el frontend (rendering, layout, business logic)
- **Regla §2** (enmendado por [ADR-047](ADR-047-wasm-shared-compute-amendment.md)):
  WASM compilado desde Rust compartido **SÍ está permitido** si cumple single-source rule:
  - Mismo código fuente compila a `native` (backend) + `wasm32-unknown-unknown` (browser)
  - Cero duplicación JS↔Rust
  - Feature-gated, opt-in, con fallback graceful al backend
  - Solo para compute capabilities que NO existen en el browser (PageRank, SCC, community detection)

Ver [ADR-047](ADR-047-wasm-shared-compute-amendment.md) para el rationale y las reglas operacionales completas del §2.

## Rationale

1. **Duplicación de lógica**: WASM en browser replicaría lógica que ya existe en el backend Axum. El backend ya tiene toda la información del grafo (call graph, data flow, C4 levels).

2. **Layout determinístico en backend**: El tree layout producido por el backend es útil para:
   - Consumidores MCP que no quieren rendering
   - APIs que necesitan layout predecible
   - SEO y indexado

3. **Layout interactivo en frontend**: El frontend decide dinámicamente cómo visualizar basándose en:
   - Interacciones del usuario
   - Tamaño del subgraph
   - Preferencias de visualización

4. **Complejidad de port**: Bloc/GtGraphLayout tienen ~10 años de evolución en Pharo. Portarlos a WASM es un proyecto completo por sí mismo.

## Alternatives Considered

- **Opción A — WASM como library compartida**: El backend usa Rust compiled to WASM, frontend carga el mismo binary. Descartado porque no hay justificación para la complejidad adicional.

- **Opción B — WASM-only frontend**: Todo el rendering en WASM, backend solo provee datos. Descartado porque duplica lógica de layout que el backend ya necesita para APIs/MCP.

- **Opción C — Sin WASM, todo en JS/TS**: El frontend implementa su propia lógica de layout. Aceptado como default.

## Consequences

- El frontend React recibe datos + tree layout del backend y aplica layouts interactivos localmente
- No hay presión para portar Bloc/GtGraphLayout a Rust
- El MCP API permanece como first-class consumer del tree layout
- La visualization en browser es purely presentational

## Validation

- [ ] Frontend recibe tree layout del backend y lo renderiza correctamente
- [ ] Force-directed layout se aplica localmente sin datos adicionales del server
- [ ] Consumidores MCP reciben tree layout válido sin rendering logic
