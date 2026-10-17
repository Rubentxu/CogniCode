# ADR-K003 — Resolver Kotlin/JVM con ADTs y aristas respaldadas

**Estado:** PROPOSED. **Aplica a:** K2. **No sustituye:** `docs/adr/ADR-029-callgraph-projection-port-seam.md`, `docs/adr/ADR-016-renderer-neutral-semantic-projections.md`.

## Contexto

`GlobalSymbolIndex` en `per_file_graph.rs` resuelve nombres candidatos y prefiere coincidencia por archivo/crate cuando hay homónimos; descarta algunos casos ambiguos. En Kotlin, los overloads, extension functions y JVM interop pueden dar relaciones falsas incluso cuando solo existe un nombre candidato aparente. Un diagrama no debería mezclar «callee textual» y «target semánticamente comprobado».

## Decisión propuesta

- Separar **hecho observado del callsite** de **resolución semántica**, con `TargetResolution = Resolved | Ambiguous | Unresolved | External`. No inventar `SymbolId` terminal para un target ambiguo.
- Normalizar identidad de símbolos Kotlin/Java con `package`, owner, tipo y firma cuando estén disponibles; el local resolver aporta solo lo que su precisión permite. Fallback S0/S1/S2 ya existente, no heurístico textual de agente.
- Toda decisión de resolución se implementa como **función pura**, que recibe hechos de AST/imports/tipos y produce un ADT + evidencia; sin side effects hasta persistir una decisión aceptada por la capa de aplicación.
- Proyecciones de grafo reciben aristas resolubles tipadas con evidencia y dejan visibles los gaps. Calls estáticos no demuestran dispatch runtime, secuencia, Flow o servicio despliegue.

## Alternativas y tradeoff

Resolver por simple name: barato, pero falso positivo semántico inaceptable. Esperar compilador completo: demasiado tardío para funcionalidad S0 de calidad. Soportar todas las construcciones Kotlin a la vez: alto riesgo de loops y tests; el alcance inicial se limita a JVM y relaciones cuyo oráculo puede verificarse.

## Aceptación

Caso de overload correcto y homónimo negativo; Kotlin→Java y Java→Kotlin; ninguna arista cuando el target no se pudo probar. Contrato preservado entre CLI/MCP y grafo persistido. K2 sólo cerrado tras release remota comprobada.
