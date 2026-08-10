# ADR-033 — DiagramWorkbench: WASM para Computation Visual

**Estado**: ACEPTADO (promoted 2026-08-10)
**Fecha**: 2026-08-09
**Decisores**: CogniCode Architecture Team
**Contexto**: CONTEXT.md:84 declara "No WASM in browser"; `cognicode-graph-wasm` ya existe como shim analítico opt-in; se necesita un motor de rendering/edición visual para diagramas C4/UML/grafos inspirado en arrows.app

---

## Resumen ejecutivo

CONTEXT.md:84 ("No WASM in browser") se refiere a **no duplicar lógica de dominio en el frontend**. El WASM existente (`cognicode-graph-wasm`) y el nuevo `cognicode-diagram-wasm` no duplican dominio: ejecutan **computation visual** (geometría, layout, routing, hit-testing, rendering). Esta ADR clarifica la distinción y autoriza WASM para rendering visual bajo feature flag opt-in.

## Contexto

### La regla original

CONTEXT.md:84 dice:

> **No WASM in browser**: Never duplicate backend logic in the frontend

La intención original era evitar duplicar lógica de análisis de código (MoldQL, graph extraction, call graph analysis) en el navegador. El frontend debe ser un cliente que consume APIs, no un segundo backend.

### La realidad actual

`cognicode-graph-wasm` (106 KB raw / 46 KB gzip) ya ejecuta algoritmos en el navegador bajo `VITE_ENABLE_WASM=true` (opt-in, default off). Estos algoritmos son **analíticos** (PageRank, comunidades, SCC, transitive reduction), no de dominio. No duplican lógica de extracción ni consulta de grafos.

### El nuevo requisito

Se necesita un motor de diagramas interactivo que soporte C4, UML y grafos genéricos con edición visual, layouts, routing de flechas y export. `arrows.app` (Neo4j Labs, Apache-2.0) tiene buenos conceptos algorítmicos, pero su stack es React/Redux/JS. Reimplementar los conceptos en Rust/WASM da:

- Determinismo native/WASM (mismo digest).
- Layouts síncronos en Web Worker sin bloquear React.
- Estructuras SoA eficientes para 1000+ nodos.
- Reutilización del motor en CLI, tests y batch.
- Sin dependencias runtime (no D3, no Cytoscape, no Sigma).

## Decisión

### D1 — WASM para computation visual es aceptable

La regla "No WASM in browser" se interpreta como **"no duplicar lógica de dominio"**. WASM para:

- Geometría, routing, layouts, hit-testing.
- Rendering (Canvas2D display list, SVG export).
- Command log y undo/redo visual.
- Spatial indexing.

es **aceptable** porque no duplica consultas, extracción, scoring, MoldQL ni ninguna decisión de dominio.

### D2 — El nuevo crate NO se mezcla con graph-wasm

`cognicode-graph-wasm` es un shim **analítico**. `cognicode-diagram-wasm` es un shim **visual**. Son responsabilidades distintas con consumidores distintos. Mezclarlos crearía un módulo sin cohesión.

### D3 — El motor es puro y agnóstico

`cognicode-diagram` (motor puro, sin WASM):

- No conoce CogniCode (no importa `cognicode-core` ni `cognicode-explorer`).
- No conoce el navegador (no importa `wasm-bindgen` ni `web-sys`).
- Recibe una `DiagramProjection` (datos planos) y produce geometría/render.
- Se compila para native y WASM desde el mismo código.

### D4 — Feature flag opt-in, no default

El workbench se activa con `VITE_ENABLE_DIAGRAM_WORKBENCH=true` (default false). Con flag desactivada: cero cambios visibles, cero descargas WASM, cero impacto en bundle inicial.

### D5 — No reemplaza nada existente

Cytoscape/ELK (`InteractiveGraph`), `GraphView` SVG, Mermaid y todos los pipelines actuales permanecen intactos. El workbench es una superficie **paralela** que se adopta solo si supera el benchmark.

### D6 — Inspiración algorítmica, no port de código

Se toman conceptos de `arrows.app` (routing de flechas, layouts, attachments, hit-testing). No se porta su modelo React/Redux, su formato `.arrows`, ni su sistema de estilos. Si se copia código literal (Apache-2.0 lo permite), se atribuye.

## Alternativas consideradas

### A — Fork de arrows.ts como iframe embed
Rechazada. Acoplamiento upstream, bridge postMessage complejo, dos stacks React, semántica C4/UML no soportada nativamente.

### B — Reutilizar Cytoscape con plugin de edición
Rechazada. Cytoscape es un viewer, no un editor. Su modelo de layout no soporta edition visual con undo.

### C — Canvas puro en TypeScript sin WASM
Rechazada. Pierde determinismo native/WASM, pierde Web Worker síncrono, duplica lógica entre CLI y browser.

## Consecuencias

### Positivas

- Motor de diagramas determinista, testeable en native, eficiente en WASM.
- Separación clara: dominio (backend), análisis (graph-wasm), visual (diagram-wasm).
- Workbench aislado, desmontable, sin impacto en producción.
- Inspiración de arrows sin heredar su deuda técnica.

### Negativas

- Dos crates WASM para mantener (analítico + visual).
- Necesidad de golden tests para garantir paridad native/WASM.
- El workbench no soporta UML sequence ni C4 dynamic hasta que existan proyecciones semánticas richer.
- Mayor superficie de test (layouts, routing, hit-testing, SVG export).

### Mitigaciones

- Feature flag default off.
- Benchmark E7 como gate de promoción.
- Aborto limpio: borrar 2 directorios restaura el estado anterior.
- Golden tests de compacidad native/WASM.

## Referencias

- [ADR-003](./ADR-003-diagram-representations.md) — Mermaid como serialización canónica
- [ADR-010](./ADR-010-diagram-artifacts-as-persistent-views.md) — Diagramas como artifacts derivados
- `crates/cognicode-graph-wasm/` — Shim analítico WASM existente (modelo a seguir)
- `apps/explorer-ui/src/hooks/useGraphAlgorithms.ts` — Patrón opt-in WASM existente
- `apps/explorer-ui/src/bench/renderers/types.ts` — `RendererAdapter` seam para benchmark
- arrows.app: https://github.com/neo4j-labs/arrows.app (Apache-2.0)

## Implementation Log

- **2026-08-10 (E31-C)**: Diagram workbench WASM visual computation authorized under feature flag opt-in (VITE_ENABLE_WASM). Closes the CONTEXT.md:84 ambiguity by clarifying that WASM is for visual computation (geometry, layout, routing, hit-testing), not domain logic.
