# ADR-TH-010 — AAT ejecutado: qué se mata, qué se declara y qué falta

**Estado:** ACCEPTED. **Fecha:** 2026-10-15. **Contexto:** cierre del ciclo
núcleo → paridad → supervisor (ADR-TH-007..009). **Autoridad del gate:** no cambia.

## Contexto

El AAT llevaba desde el primer día declarado como especificación: 30 filas de
mutación con «debe producir», sin un solo test que inyectara la mutación. Un
documento así se lee como cobertura y no lo es. Además, tres filas pedían piezas
que el harness no tenía: suelo de obligaciones críticas (AAT-04, AAT-06), identidad
de repositorio en la reutilización (AAT-16) y declaración de inestabilidad
(AAT-27).

## Decisión

1. **La tabla «Ejecución» de `aat/AAT.md` es la que manda.** Cada fila tiene
   estado (`KILLED`, `PARTIAL`, `NOT_RUN`), y las `KILLED` nombran el test que la
   mata. El contrato del gate comprueba que ese test existe de verdad, que los
   estados son los tres admitidos y que el resumen coincide con la tabla. Un AAT
   que dice «esto se mata» sin test es fe, no evidencia.

2. **Se ejecuta lo que el harness puede matar hoy**: 21 filas `KILLED`, de las
   cuales 12 con test propio en `tests/test_aat.py` y 9 apoyadas en tests que ya
   existían (selección, fingerprints, ledger, vaciedad). Dos `PARTIAL` (exclusión
   probada, pero la fila pide sandbox o servicio externo) y siete `NOT_RUN` con
   motivo: campaña de sandbox, adaptador nextest, artefactos de release y
   `known_failures.yaml`.

3. **Suelo de obligaciones críticas** (`test-harness/obligations.json`). El AAT
   pedía que borrar `cargo clippy` o la obligación de doctests del gate no pasara
   desapercibido. La selección por impacto no puede cubrir eso (si la obligación no
   está en el catálogo, no hay nada que seleccionar), así que existe un mínimo
   versionado por lane: si una obligación crítica desaparece del catálogo, el plan
   queda `UNKNOWN_INPUT` con el motivo y el veredicto no puede ser `SATISFIED`.
   No es un inventario paralelo: el catálogo sigue leyéndose de las pipelines y
   este fichero solo dice qué no puede faltar.

4. **Identidad de repositorio en la reutilización** (AAT-16). Un recibo de otro
   repositorio no se reutiliza aunque el nombre del directorio coincida. La
   equivalencia entre worktrees del mismo repositorio se declara por el
   identificador (remoto `origin`), que es la equivalencia explícita que el AAT
   admite.

5. **Inestabilidad declarada** (AAT-27). Si una suite pasó y falló con la **misma**
   huella de entradas, `certify` no la certifica como estable: añade el hueco con
   el motivo en vez de presentar el último PASS como reproducibilidad. No se
   bloquea la reutilización (la política no cambia): se bloquea la afirmación de
   estabilidad.

6. **Aislamiento del estado** (AAT-20). Un id de suite hostil no puede escapar del
   directorio de logs (se sanea y se comprueba) y un symlink plantado donde el
   harness escribe no redirige la escritura: la escritura atómica reemplaza el
   enlace en vez de seguirlo.

## Consecuencias

- El AAT deja de ser prosa: 21 de 30 filas se matan con test, y las 9 restantes
  están declaradas con motivo. El gate lo comprueba en cada merge.
- Perder una obligación crítica ya no es silencioso: el plan queda indeterminado.
- Un PASS perdido y recuperado bajo la misma huella no se puede certificar como
  estable.
- 12 tests nuevos (`tests/test_aat.py`) y una comprobación más en el contrato del
  gate.

## Lo que este ADR no decide

- Ejecutar las filas que dependen de sandbox, nextest o release (W5 y lanes de
  release). Cuando existan, cambian de estado en la misma tabla y el contrato
  exige el test.
- El waiver explícito de AAT-30: hoy la vacuidad siempre falla; un mecanismo de
  excepción aprobada sería otro ADR con su revisión.
- El alcance de `known_failures.yaml` dentro del harness (hoy vive en `test-fast`).
