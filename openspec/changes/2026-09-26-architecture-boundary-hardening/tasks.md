# Tasks — Application boundary hardening

- [x] T1 Añadir `application_no_infrastructure`.
- [x] T2 Añadir `application_no_interface`.
- [x] T3 Ejecutar self-host y congelar inventario de deuda.
- [x] T4 Definir mecanismo temporal de excepción con owner+rationale+expiry.
- [x] T5 Extraer `PathPolicy` del acoplamiento FileOperations→MCP validator.
- [x] T6 Migrar FileOperations a ports neutrales.
- [x] T7 Plantar import prohibido y verificar FAIL del gate.
- [ ] T8 Cerrar excepciones del vertical migrado.

## Evidencia (ST-01, commit `ba031da5`)

| Tarea | Evidencia medida |
|---|---|
| T1 | `cr06_synthetic_drift_application_to_infrastructure_is_detected` PASS — la constraint dispara |
| T2 | `cr06_synthetic_drift_application_to_interface_is_detected` PASS — la constraint dispara |
| T3 | `architecture_self_host_e2e` 6/6; inventario congelado en 39 y pineado por `inventory_size_is_pinned_at_current_baseline` |
| T4 | `TemporaryException { owner, rationale, expiry }`; helper `ex()` con expiry duro `2026-12-31` |
| T5 | `trait PathPolicy` en `application/ports/mod.rs:41`; `impl` en `interface/mcp/security.rs` |
| T6 | `cargo grep`: `crate::interface::mcp` en `file_operations.rs` solo en la línea 1990, dentro de `mod tests` (1982) — 0 en producción |
| T7 | 3 tests `cr06_synthetic_drift_*` PASS: el gate sigue fallando ante `application→bin/infra/interface` |

### T8 sigue abierta, y no por descuido

Las tres entradas de `file_operations.rs` que sobreviven en el allowlist son
**guardas de regresión deliberadas**, no deuda: el módulo `#[cfg(test)]`
compone los adaptadores reales (tree-sitter, `RustVerifier`,
`InputValidator`) por diseño de test de comportamiento. "Cerrarlas" eliminaría
la señal que avisa de una regresión de ST-01. Su cierre real es sustituirlas
por dobles de test, que es trabajo de otra unidad.
