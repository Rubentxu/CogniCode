# Fase 3 — Estratégicos

**Duración:** 18–27 días-persona.  
**Objetivo:** bajar el coste de cambio y evitar que el siguiente ciclo vuelva a producir drift estructural.

## ST-01 — FileOperations
Primer vertical porque contiene la violación más clara: application depende de MCP.

## ST-02 — WorkspaceSession
Convertirlo en façade profunda que recibe capacidades en vez de construir adapters.

## ST-03 — AnalysisService
Separar razones de cambio detrás de una fachada estable; evitar convertir cada submódulo en API pública.

## ST-04 — HandlerContext
Reducir service-locator surface; campos privados; handlers dependen de capacidades mínimas.

## ST-05 — Graph build semantics
Una sola autoridad semántica para full graph; conservar estrategias como políticas/implementaciones si siguen aportando valor, no como owners paralelos.

## Hito de salida F3

`ARCHITECTURE-DEPTH-1`:

- 0 dependencias prohibidas en la zona migrada;
- APIs externas más pequeñas que antes;
- semántica graph-build con un owner;
- mega-módulos reducidos por responsabilidad, no por line-count cosmético.
