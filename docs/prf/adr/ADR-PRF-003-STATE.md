# ADR-PRF-003 — Un dueño por dato, persistencia por necesidad
**Status:** PROPOSED · **Fecha:** 2026-09-21.

**Contexto.** Conviven grafo/cache histórico, store LadybugDB y puertos LSI de facts/evidence en memoria. Un daemon o Control Plane nuevo podría multiplicar las fuentes de verdad.

**Decisión propuesta.** F0/F4 inventarían ownership, namespace, schema/version, invalidación y recuperación por operación. Un dato es canónico, derivado o efímero; una proyección se reconstruye del canónico y debe identificar su basis. Reutilizar LadybugDB mediante puerto pertinente solo si una operación estable requiere persistir Facts/Evidence; ningún nuevo almacén por anticipación.

**Alternativas.** Migrar todo LSI antes del CLI estable (coste sin UAT) o confiar solo en cachés sin revisión (stale/mezcla).

**Validación:** snapshots/manifest, 2 workspaces, 2 procesos, kill/restart/crash/rollback y cold/warm equivalence; C2/C4, U09/U17/U18/U21/U22. **Rollback:** snapshot original, migración reversible o rechazo seguro.
