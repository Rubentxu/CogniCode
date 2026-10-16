# Spec: real-user-vertical — Explorer con backend y datos reales

**Status:** PROPOSED. **Refs:** `docs/adr/ADR-012-ui-visible-capability-contract.md`, `docs/adr/ADR-018-evidence-gated-product-operability.md`, `docs/adr/ADR-026-ladybugdb-canonical-migration.md`, `docs/prf/specs/SPEC-STATE.md`, `docs/prf/specs/SPEC-DISTRIBUTION.md` PRF-DIST-07.

## Requirements

### VDH-REAL-01 — Smoke integrado nativo con LadybugDB

**MUST** levantar `explorer-api` contra base Ladybug temporal aislada, con workspace fixture propio, sin depender de `cognicode-postgres`, `DATABASE_URL` ni `ensure_pg`. **MUST** fallar si el backend no arranca, no se indexa, no hay revisión o el resultado no coincide con oráculo. **MUST** limpiar procesos/estado temporal incluso en error y no borrar datos de usuario.

**Scenario A:** GIVEN checkout limpio y un fixture A→B; WHEN se inicia Explorer→open→scan→landing→query; THEN la revisión está visible, A y B se recuperan con identidad correcta, la relación existe y el proceso acaba sin huérfanos.

### VDH-REAL-02 — Capacidad visible y utilizable

**MUST** satisfacer ADR-012: discoverable desde Spotter/navegación, inspectable, usable desde UI y validated por al menos un recorrido real. **MUST NOT** usar `VITE_USE_MOCKS=true` como prueba de integración con backend. **SHOULD** usar Playwright real en release y Vitest/MSW para interacciones de componentes.

**Scenario B:** GIVEN aplicación real con API real; WHEN el usuario busca A y abre el call graph; THEN visualiza B, revisa origen/revisión y maneja un error real sin render en blanco engañoso.

### VDH-REAL-03 — Estados de fallo y limitación

**MUST** demostrar `Unknown/Partial/Failed` visible, no cobertura total ficticia; datos procedentes del workspace correcto; timeout/cancelación o error se declaran honestamente. **MUST** ejecutar pruebas negativas contra backend caído/corrupto y workspace vacío.

**Scenario C:** GIVEN API caída tras abrir UI; WHEN se solicita otra vista; THEN error recuperable visible con referencia al request, no vista `clean` ni `0` engañoso.

## Ciclo efímero y propiedad permanente

B3-UAT-01 (flujo visible), B3-E2E-01 (navegador real), B3-AAT-01 (fallos). Guardar recibo SHA/bits y borrar los tres artefactos de ciclo antes de merge. Mantener regresión permanente API real y mínimo smoke del usuario apropiado al coste/risgo.
