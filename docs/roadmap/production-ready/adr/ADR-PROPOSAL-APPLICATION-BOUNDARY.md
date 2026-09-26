# ADR Proposal — Application boundary becomes an executable architecture rule

**Estado:** Proposed

## Contexto
ADRs previos declaran que domain/application no deben depender de infraestructura, pero las constraints canónicas actuales protegen principalmente `domain`. La auditoría encontró imports production de `application` hacia `infrastructure` y `interface::mcp`.

## Decisión propuesta
Añadir como reglas canónicas:

- `architecture.application_no_infrastructure`
- `architecture.application_no_interface`

El primer run puede producir deuda conocida. Las excepciones transitorias deben estar enumeradas, tener owner, rationale y expiry.

## Estrategia de remediación
1. FileOperationsService.
2. WorkspaceSession composition.
3. AnalysisService.
4. HandlerContext consumers.
5. graph-build ownership.

## Criterio de diseño
Los nuevos puertos se aceptan solo cuando ocultan complejidad concreta y permiten sustituir/testear una dependencia real. No crear traits como ejercicio de simetría.
