---
title: "ADR — D1: retirada de la clase UAT del repositorio"
slug: "ADR-D1-006-uat-retirement"
status: accepted
date: 2026-10-17
deciders: Maintainer
related:
  - "[[ADR-CI-ORCHESTRATOR-CUTOVER]]"
  - "[[ADR-D1-005-lane-rebalance]]"
context:
  - "El repositorio mantenía 28 targets *_uat.rs en tres crates, 68 tests y 4 804 líneas con la misma ambición que los contratos del gate de merge"
  - "Sostenida contra el inventario de obligaciones, esa clase duplicaba cobertura que los contratos de lane ya pinean por otra vía (a009, a016, prf_h06, b6_w1, cli_exit_code_propagation)"
  - "Su coste se pagaba en cada corrida de CI y en el baseline de tests del preflight"
---

# ADR — D1: retirada de la clase UAT del repositorio

## Decision

La clase UAT deja de existir en este repositorio.

1. **Se eliminan los 28 targets `*_uat.rs`** de `cognicode-cli`, `cognicode-core` y `cognicode-mcp`, con sus referencias en los contratos que los citaban por nombre (`cli_gate_coverage_contract`, `core_gate_coverage_contract`, `a013_lifecycle_gate_contract`) y en los comentarios que apuntaban a ficheros que dejan de existir.
2. **`test-harness/` queda fuera del repo**: las decisiones de D1 se versionan en `docs/adr/ADR-D1-*` con autoridad propia, no como un paquete importado.
3. **El AAT (pruebas de mutación) se mantiene**: es una clase distinta, la que sostiene la evidencia adversarial, y no es lo que el operador retiró.
4. **El baseline de `preflight-clean-clone.sh` se re-baseliniza**: el delta es calculado y el primer clean-clone posterior es quien lo confirma; si el observado difiere más que la tolerancia, se re-baseliniza con el observado.

## Lo que esta decisión acepta explícitamente

Deja de haber reproducción ejecutable en CI de las garantías que esa clase cubría — diferenciación read/write, allowlist de directorios, telemetría opt-in, shutdown limpio, namespace por digest, snapshots durables, concurrencia, determinismo de CLI, paridad del gate de clippy y distribución desde bytes release. Las garantías siguen descritas como evidencia histórica en `docs/prf/` y las que tienen contrato superviviente siguen verificadas por él; las demás pasan a ser afirmaciones documentadas, no verificadas en cada merge. **Ese es el intercambio, y está escrito aquí para que no se descubra por sorpresa.**

## Aplicación en D1

- `49aa6b16 test(uat): retirar la clase UAT del repositorio` cherry-pickeado desde `worktree/test-harness` aplica esta decisión sobre `main`. El cherry-pick resolvió dos conflictos con el estado de `main` post-D0:
  - `core_gate_coverage_contract.rs`: el comentario de la cuenta de suites en HEAD listaba 81 suites por la entrada de `lightweight_index_repro`, `map_symbol_collision_repro` y `haskell_scanner_overflow` desde el bloque SIGABRT. La retirada del UAT baja esa cuenta a 80. El comentario y el `assert_eq!` se actualizaron en el mismo commit; cualquier WU futura que añada o retire un suite debe mover ese número en su propio commit (la regla del propio contrato).
  - `ANCHOR_SUITES`: el UAT retirado se elimina del ancla. Los 4 suites SIGABRT-characterization que `main` añadió al ancla se conservan.

## Consecuencias medidas

- `merge-gate.pipeline.kts` pasa de 73 a 58 declaraciones de stage (verificado en el diff).
- `release-candidate.pipeline.kts` pasa de 33 a 30 stages.
- Las suites de perfil release bajan de 5 a 3 (CLI) y de 1 a 0 (MCP) tras la retirada de la clase UAT.
- El baseline de tests del preflight se reduce proporcionalmente (delta calculado: 67 tests que pasaban + 1 ignorado).

## Coherencia con la gobernanza

`AGENTS.md` exige una decisión explícita para reducir cobertura y prohíbe convertir un fallo en un aviso o reducir cobertura para poner un gate verde. Esta decisión no busca un gate verde: los contratos que sobreviven se actualizan en el mismo commit, y este ADR enumera lo que deja de verificarse.

## Reversibilidad

La retirada es un commit; revertirlo devuelve los 28 ficheros y sus obligaciones. Lo que no se puede recuperar por revert es la decisión: reabrir la clase UAT exige una decisión nueva del operador.

## Lo que este ADR no decide

- Cambiar la política de seguridad o de certificación.
- Relajar `test-full.sh` en `AGENTS.md`.
- Adoptar un scheduler o framework alternativo para la verificación (esto es W6 del overlay y se decide por datos, no por este ADR).
