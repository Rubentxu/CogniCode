# ADR-PRF-002 — Casos de uso neutrales de transporte
**Status:** PROPOSED · **Fecha:** 2026-09-21.

**Problema.** `WorkspaceSession` importa `interface::mcp::security::InputValidator` e infraestructura, y `AnalysisService` combina graph/cache/parser concretos. Exponerlos directamente como API estable prolongaría el acoplamiento.

**Decisión propuesta.** Elegir **una** operación CLI/MCP con contrato y oráculos; extraer solo los puertos/datos necesarios y situar adapter assembly en `cognicode-runtime`. Preservar semántica, fuente de verdad, permisos y versiones previas. No reescribir el dominio ni introducir interfaces sin segundo consumidor real.

**Alternativas.** Refactor completo previo (elevado blast radius), wrappers duplicados (dos semánticas), nueva API RPC (infraestructura sin consumidor).

**Criterio de adopción.** TDD caracterización old/new, diff semántico y test de aceptación de proceso real. Si la extracción aumenta la connascence o no mejora testabilidad, documentar resultado y mantener la solución mínima.

**Validación:** PRF-ANA/EXT, F3/C3, U08/U10/U11/U26. **Rollback:** conmutar a adapter histórico manteniendo contrato.
