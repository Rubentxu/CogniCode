# ADR-PRF-005 — No construir RPC/daemon ni Control Plane para certificar la base
**Status:** PROPOSED · **Fecha:** 2026-09-21.

**Decisión propuesta.** Congelar nuevas piezas RPC (protocol/daemon, UDS/multiworkspace), Control Plane/Backstage, packs, agentes AI autónomos y federación durante F0–F7. Preservar su documentación y código existente. Integraciones futuras usan casos de uso CLI/MCP certificados y no constituyen condición para C7.

**Justificación.** Estos programas son consumidores/extensiones posibles, no sustitutos de garantías de correctitud/persistencia del núcleo.

**Reapertura:** problema real no resoluble en la superficie actual, cliente identificado, medición de coste/beneficio, ADR que enumere pruebas/seguridad/compatibilidad y entrada en [línea temporal posterior](../POST-PRF-EVOLUTION.md). **Rollback:** mantener cliente previo y no promover nuevo servicio a stable hasta UAT independiente.
