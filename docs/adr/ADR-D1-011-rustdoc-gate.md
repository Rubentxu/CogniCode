---
title: "ADR — D1: el gate de rustdoc se paga una vez (de merge a integración)"
slug: "ADR-D1-011-rustdoc-gate"
status: accepted
date: 2026-10-17
deciders: Maintainer
related:
  - "[[ADR-CI-ORCHESTRATOR-CUTOVER]]"
  - "[[ADR-D1-005-lane-rebalance]]"
context:
  - "Perfil medido del gate de merge en el ledger durable de PipelineK (run 1b37e37b: 58 etapas, 1630 s): las diez primeras etapas son el 94% del tiempo y rustdoc-gate sola es el 41% (667 s, 737 s medidos con /usr/bin/time)"
  - "Ese mismo target lo ejecutaba también la lane de integración dentro de cargo test --workspace --all-targets: cada cambio pagaba dos veces una comprobación que solo puede emitir un veredicto"
  - "No es una etapa lenta sin alternativa: es trabajo duplicado"
---

# ADR — D1: el gate de rustdoc se paga una vez (de merge a integración)

## Decision

1. **Dueño único: la lane de integración.** La stage se retira de `merge-gate.pipeline.kts` y se nombra explícitamente en `integration.pipeline.kts` (antes era implícita dentro de `--all-targets`, que la hace ejecutar sin que el contrato pueda verificar quién es el dueño).

2. **La garantía no se retira, cambia de momento.** El workspace sigue teniendo que mantener cero avisos de rustdoc en las categorías gateadas, verificado por el mismo target, en una lane que bloquea `main`. Lo que cambia es que un enlace roto deja de bloquear el PR y bloquea la integración posterior (fix-forward).

3. **El contrato pasa de «el merge lo ejecuta» a «exactamente una autoridad lo ejecuta, y no es el merge».** `core_gate_coverage_contract::the_rustdoc_gate_runs_in_exactly_one_authority` falla si el gate desaparece de todas las lanes («runs in no pipeline, so it gates nothing») y si vuelve a aparecer en el merge («must run in exactly one authority», con la lista de runners). La aserción anterior no detectaba la duplicación; esta detecta las dos direcciones del error.

## Aplicación en D1

- `c8ee61fe perf(gate): el gate de rustdoc se paga una vez, no dos (−737 s por merge)` cherry-pickeado desde `worktree/test-harness`.

## Consecuencias medidas

- 737 s por iteración de merge, ya no duplicados.
- El presupuesto del gate de merge se reduce: la etapa `m011_rustdoc_gate` se va de `merge-gate.pipeline.kts` y se declara explícita en `integration.pipeline.kts`.
- La garantía de cero avisos en categorías gateadas sigue activa: el target `m011_rustdoc_gate` no se relaja, solo cambia de lane.

## Riesgo aceptado

Un cambio que introduzca un aviso de rustdoc pasa el merge y se detecta en la integración posterior. Esto es coherente con el modelo de lanes: la integración ya bloquea `main` (`certify` rechaza sin candidata fresca), y un aviso nuevo de doc se corrige con fix-forward, no con un parche de emergencia en el PR.

## Lo que este ADR no decide

- Relajar las categorías gateadas de M0.11.
- Cambiar el dueño de ninguna otra stage.
- Eliminar la garantía: la cobertura sigue; solo se paga una vez y en el momento correcto.
