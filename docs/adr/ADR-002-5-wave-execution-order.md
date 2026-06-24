# ADR-002: 5-Wave Candidate Execution Order

**Fecha:** 2026-06-11  
**Estado:** SUPERSEDED  
**Decisión original:** Ejecución en 5 waves con gating CI automatizado por wave  
**Fuente:** auto-grill-loop Q007-P1, Q013-P2  
**Superseded by:** v0.5.0–v0.12.7 (jun–jul 2026), cycles ejecutados en orden distinto al plan original

---

## Revisión 2026-06-24 (v0.12.7)

**Estado actual:** SUPERSEDED.

**Por qué:** Los 5 waves originales se ejecutaron en un orden distinto al planificado, con gates CI parciales (no se construyó el sistema de CI gating automatizado por wave). El plan quedó obsoleto a medida que el trabajo real reveló dependencias no anticipadas:

- **C3 (WalkFilter)** se ejecutó dentro de v0.5.0 (C1-C6 cycle)
- **C5 (ReadMode)** se ejecutó dentro de v0.5.0
- **C6 (Mock crate)** se ejecutó parcialmente — el crate `cognicode-core-mock` separado NO se creó; los mocks permanecieron inline (ver ADR-006)
- **C2 (Builder)** se ejecutó dentro de v0.7.0 (deprecation + migration)
- **C4 (Unification)** se ejecutó parcialmente en v0.12.3 (Type divergence) — solo para ViewDescriptor, no para los 10 re-exports de ADR-015
- **C1 (Tool Registry)** se ejecutó dentro de v0.5.0

**Conclusión:** El plan de 5 waves fue una guía útil al inicio pero perdió relevancia. El trabajo real se priorizó por valor y dependencias, no por wave. La disciplina de CI gating por wave no se materializó — los tests de round-trip MCP son el firewall real (ver ADR-015).

**Estado final:** SUPERSEDED. No se reabre.  

---

## Context

Se identificaron seis candidatos de profundización arquitectónica en el codebase de CogniCode. Comparten archivos (`handlers/mod.rs`, `dto/file_ops.rs`, `rmcp_adapter.rs`), creando riesgo de merge conflicts si se ejecutan en paralelo sin orden.

## Decision

Ejecutar en 5 waves con CI gating automatizado por wave:

| Wave | Candidato | Archivos | ΔLines | Gate CI |
|------|-----------|----------|--------|---------|
| **1** | C3 (WalkFilter) | 5 | ~150 | Test suite + bench <5% regresión |
| **1** | C5 (ReadMode) | 2 | ~100 | Test suite |
| **1** | C6 (Mock crate) | 15 | ~250 | Test suite + mock crate compiles |
| **2** | C2 Builder | 1 | ~150 | Coexistence tests + `#[deprecated]` count=0 |
| **3** | C4 Unification | 5+ | ~500 | Trybuild tests + DTO migration test |
| **4** | C2 Deletion | 1 | ~50 | Dead-code lint clean |
| **5** | C1 Tool Registry | 2+ | ~200 | Integration suite + tool count match |

## Rationale

- **C4 gates C1:** Q001-P1 confirmó que C1 compila contra signatures de handlers que filtran tipos DTO; C4 debe limpiar la frontera primero
- **Wave 1 paralela:** C3, C5, C6 tocan archivos completamente disjuntos — riesgo de merge conflict cero
- **CI gates por wave:** Verificación automatizada previene bleed entre waves y permite rollback independiente

## Consequences

- C1 (candidato de mayor impacto) es Wave 5 — debe esperar a C4
- C4 se eleva de "vale la pena explorar" a dependencia del camino crítico
- Los checks CI por wave deben configurarse antes de cada wave

## Alternatives Considered

- **Big-bang todo junto:** rechazado — merge conflicts, PR no revisable, imposible hacer rollback
- **C1-first (saltar C4):** rechazado — Q001-P1 probó que la compilación de C1 depende de la limpieza de la frontera schema/DTO
- **Verificación manual por wave:** rechazado — propenso a errores; gates CI automatizados proveen criterios de completitud objetivos

## Validation

- [ ] CI gates de Wave 1 configurados y pasando
- [ ] Cada wave mergea independientemente (sin trabajo de wave N+1 en branch de wave N)
- [ ] Rollback testeado: revertir cualquier wave no rompe waves anteriores
