# ADR-PRF-004 — Autorización y límites por capacidad
**Status:** PROPOSED · **Fecha:** 2026-09-21.

**Contexto.** CLI/MCP manejan rutas y código no fiable; refactor y ejecución implican riesgos mayores que lectura; algunas validaciones dependen del handler.

**Decisión propuesta.** Dominio/application reciben un contexto de autorización comprobable y budgets; lectura/escritura/ejecución/red nunca se infieren unas de otras. Canonicalizar/rastrear rutas en cada operación de IO, no solo al parsear el request. Fail-closed si no hay cobertura, autoridad o recursos. Observabilidad opt-in y sin secretos.

**Alternativas.** Permiso global al workspace o un bool `trusted`; cada nuevo handler impone reglas diferentes.

**Validación:** matriz de operaciones y threat model, fuzz/path traversal/symlink/permission, secreto señuelo, cancelación y subprocess sandbox. C5, U14/U19/U25. **Rollback:** deshabilitar capability riesgosa y mantener lectura segura.
