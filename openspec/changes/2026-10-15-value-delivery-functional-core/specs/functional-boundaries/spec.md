# Spec: functional-boundaries — FP estilo Haskell, Rust idiomático

**Status:** PROPOSED. **Refs:** `docs/roadmap/production-ready/EXECUTION-PLAN.md` ST-01..05 y CR-06, `docs/roadmap/production-ready/METRICS-AND-ACCEPTANCE.md`, `docs/adr/ADR-028-ladybugdb-port-abstraction-architecture.md`, `docs/roadmap/adr/ADR-012-st04-cli-mcp-separation.md`. **Código:** `AnalysisService`, `WorkspaceSession`, `HandlerContext`, `RuntimePorts`.

## Requirements

### VDH-FP-01 — Pure core, effectful boundaries

**MUST** poner decisiones de negocio, cómputo de planes, normalización, categorización y proyecciones detrás de funciones deterministas sobre valores explícitos. **MUST NOT** introducir `std::fs`, red, reloj implícito, `global` mutable, `Arc<Mutex<_>>` o acceso DB dentro de un evaluador presentado como puro. Efectos **MUST** quedar en adaptadores/composition root. `Result/Option` se utilizan para fallo semántico, no como autorización ni como error omitido.

**Scenario A:** GIVEN idéntico `GraphSnapshot`, plan y policy; WHEN se evalúa dos veces con distintas construcciones de Runtime; THEN ambos resultados de dominio son iguales y no hay efectos.

### VDH-FP-02 — ADTs y estados imposibles

**MUST** representar estados mutuamente excluyentes con `enum` exhaustivos. **SHOULD** usar newtypes para WorkspaceId, SnapshotId/RevisionId si aún no existe tipo nominal; reutilizar tipos vigentes antes de crearlos. **MUST** preservar compatibilidad de serialización en DTOs externos.

**Scenario B:** GIVEN `Unavailable`; WHEN se aplica `map(f)`; THEN sigue siendo `Unavailable` y `f` no es llamado. GIVEN `Partial`; THEN `map(f)` conserva su cobertura/degradación.

### VDH-FP-03 — Seams legítimos, cohesionados

**MUST** seguir ST-03/04/05: GraphBuild, GraphQuery, Coverage y SymbolIndex con **un propietario semántico**. **MUST NOT** crear puertos si no sustituyen una dependencia real ni añadir builders/facades paralelos. **MUST** mantener `application` sin referencias a `interface`; `application→infrastructure` solo mediante excepción formal temporal, owner y expiry durante migración. **MUST** limitar capability construction a `runtime` o composición externa.

**Scenario C:** GIVEN dos consumidores (CLI/MCP) sobre el mismo workspace y versión; WHEN crean sesión y preguntan el grafo; THEN obtienen semántica equivalente y no se crean dos cachés ocultas.

### VDH-FP-04 — Legibilidad sin pérdida de behavior

**MUST** cortar módulos gigantes por responsabilidades coherentes (no únicamente LOC). **MUST** caracterizar resultados antes de mover; **MUST** demostrar que no cambia la API pública o documentar versionado.

**Scenario D:** GIVEN antes/después de extraer `AnalysisService`/`views`; WHEN se serializa el mismo caso estable; THEN snapshots contractuales son iguales salvo diferencias explicitadas/aceptadas.

## Aceptación de hito

B2-UAT-01, B2-E2E-01, B2-AAT-01 efímeras. Permanentes: leyes, fitness CR-06, wiring de runtime, regressions de aislamiento/revisión. **MUST** ejecutar el mismo conjunto de pruebas DURADERAS tras borrar las efímeras.
