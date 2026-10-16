# Spec: ephemeral-acceptance — pruebas de campaña desechables, contratos permanentes

**Status:** PROPOSED. **Refs:** `AGENTS.md` estrategia fast/full, `docs/roadmap/production-ready/EXECUTION-PLAN.md` CR-08/09, `docs/roadmap/production-ready/METRICS-AND-ACCEPTANCE.md`, `docs/prf/specs/SPEC-CI.md` PRF-CI-01/03/07, `docs/adr/ADR-032-sandbox-validation-system.md`.

## Definiciones

- **UAT**: User Acceptance Test, oráculo del usuario/requisito contra producto.
- **E2E**: End-to-End, recorrido de componentes reales hasta el consumidor, no sólo MSW.
- **AAT**: Agent Acceptance Test de la feature (batería de pruebas/verificaciones que el agente utiliza para aceptar el cambio). El término no designa por sí mismo una prueba de seguridad permanente.
- **Efímero**: archivo de test/fixture específico de un ciclo que NO entra en `main`. **Duradero**: propiedad estable y barata que SI entra en `main` con su invocación en gate.

### VDH-TST-01 — Ciclo de vida declarado

**MUST** registrar por caso temporal: ID, hito, requisito, condición Given/When/Then, oráculo, fichero, comando real, SHA candidato, duración, resultado y enlace al contrato duradero o justificación de no promoción. La evidencia de ejecución **MUST** conservarse en recibo compacto append-only/release o sistema aprobado, sin depender de que el test temporal siga en `main`.

**Scenario A:** GIVEN `B1-AAT-01` ejecutado con RC=0 y mutante rechazado; WHEN termina B1; THEN el recibo durable referencia SHA y negativo observado, el fichero temporal está ausente y queda un contrato perpetuo que falla con la misma clase de bug.

### VDH-TST-02 — Eliminación antes del merge

**MUST** eliminar ficheros UAT/E2E/AAT **propios del ciclo** y fixtures temporales antes de merge. **MUST** fallar el merge si quedan rutas `__vdh_cycle_*` o `test-harness/.cycles/` trackeadas o presentes (salvo si la política excepcional aprobó retención). **MUST NOT** borrar automáticamente código ajeno o directorios fuera de rutas propiedad del ciclo. **MUST** reejecutar tests duraderos y gates después de borrar tests efímeros.

**Scenario B:** GIVEN un test `crates/.../tests/__vdh_cycle_B2_smoke.rs`; WHEN se intenta cerrar B2; THEN el guard devuelve !=0. GIVEN se elimina y se ejecuta la regresión permanente; THEN se puede continuar al merge.

### VDH-TST-03 — Nunca sacrificar un gate necesario

**MUST** retener (o reemplazar y demostrar equivalencia) protección de inyección/pérdida de secretos, read-only, compatibilidad MCP, aislamiento, rollback, integridad/completitud de releases. **MUST** registrar motivo, owner, tiempo medido, reemplazo y aprobación antes de retirar tests ya trackeados. **MUST NOT** usar `#[ignore]`, `skip`, `|| true` o `0 tests` como PASS de propiedades obligatorias.

**Scenario C:** GIVEN `prf_h06_adversarial_e2e` retirado sin reemplazo; WHEN se valida alcance del gate; THEN no se autoriza el cierre. GIVEN una suite nueva contractualmente equivalente y negativo mutante probado; THEN se puede eliminar el duplicado antiguo tras revisión.

### VDH-TST-04 — Selección precisa, fallback seguro

**MUST** mapear cambio→capacidades→contratos ejecutados. Path desconocido y cambios en Cargo/orquestación/contratos MUST forzar fallback seguro o abortar en la fase adecuada. **MUST** ejecutar sin asumir que glob >0 implica suites con tests >0. **SHOULD** dejar suite rápida en desarrollo, core/contratos en merge y matriz + distribución real en candidato.

**Scenario D:** GIVEN un contrato nuevo en crate no conocido; WHEN el selector lo clasifica; THEN no dice noop: escoge fallback/FAIL documentado. GIVEN suite ancla desaparece; THEN un guard detecta que ya no se ejecuta.

### VDH-TST-05 — Inventario y retiro medible

**MUST** partir de `test-harness/inventory/test_inventory.csv` (106 candidatos, 19 adicionales, snapshot 2026-10-15), completar tiempo/última ejecución/capacidad/propiedad protegida y **no** interpretar naming como orden de borrado. **MUST** mantener estado `REVIEW` hasta prueba real de duplicidad/obsolescencia. **SHOULD** eliminar el harness Dashboard antiguo primero si se demuestra falta de consumidor.

**Scenario E:** GIVEN uno de los 106 candidatos sin prueba de sustitución; WHEN se propone delete; THEN `decision=REVIEW/RETAIN` y el PR no lo elimina. GIVEN sustitución probada; THEN la fila indica PR+SHA+gate con el cual cerró.
