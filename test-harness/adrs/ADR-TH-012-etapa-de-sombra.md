# ADR-TH-012 — Etapa de sombra: observar el plan sin decidir nada

**Estado:** ACCEPTED. **Fecha:** 2026-10-15. **Contexto:** el README declaraba
«adopción en modo sombra: no implementada» y TH-S05 pide pasos de observación antes
de cambiar la semántica del gate. **Autoridad del gate:** no cambia.

## Contexto

El harness ya construye planes con motivo por decisión, mide paridad contra CR-08
(0 debilidades sin declarar en el corpus) y tiene suelo de obligaciones críticas.
Lo que no existe es **dato real**: qué seleccionaría en los cambios que entran de
verdad por el gate, y cuántas obligaciones de la lane quedarían fuera en cada uno.
Sin ese dato, pasar etapas a selección por impacto sería una apuesta.

ADR-TH-008 decidió no añadir la etapa mientras no existiera una razón para
consumir el plan. La razón llegó con el mandato del operador: el presupuesto de
6–7 minutos para merge no se alcanza moviendo una sola etapa, y el camino medido
es que merge ejecute lo afectado en vez de la batería completa.

## Decisión

1. **Una etapa advisory en `merge-gate.pipeline.kts`**: `harness-shadow-plan`
   ejecuta `harness.py plan --lane merge --base origin/main --oracle cr08 --shadow`
   e imprime el resumen. No ejecuta suites, no escribe en el repositorio y no
   decide nada.

2. **No puede fallar la lane, por construcción.** PipelineK no tiene
   `continue-on-error`, así que el comando termina en `|| echo 'ADVISORY: …'`. Un
   plan roto deja una línea en el log y el gate sigue como estaba. El contrato
   comprueba esa forma: sin el `||` no hay etapa de sombra válida.

3. **El resumen está escrito para un log de gate**, no para una persona que
   ejecuta el comando: dice en la primera línea que no decide nada, cuántas
   obligaciones tiene la lane, cuántas exige el cambio, cuántas quedan bloqueadas
   por estado externo y qué dijo el oráculo. Un log que se puede confundir con un
   veredicto es un log peligroso.

4. **Solo en la lane de merge.** Las demás lanes no observan: el dato que hace
   falta es el del gate que se quiere aligerar, y una observación duplicada donde
   no se va a decidir es coste sin decisión.

5. **El contrato lo comprueba en las dos direcciones**: la etapa existe en merge,
   es advisory y no aparece en las otras cinco pipelines. Quitar la etapa o
   quitarle el `||` hace caer la etapa de contratos (probado con dos mutaciones).

## Consecuencias

- Cada merge deja en su log un plan real: obligaciones de la lane, requeridas por
  impacto y bloqueadas. Es la evidencia con la que el bloque siguiente decide qué
  etapas pasan a selección.
- Coste medido: ~4 s por merge (el plan más la consulta al oráculo), sobre 579 s
  del gate. No se presenta como ahorro: es observación.
- El repositorio no recibe escrituras: el ledger de la observación vive donde vive
  el de cualquier corrida (`$XDG_STATE_HOME`).
- La adopción sigue sin decidirse. Este ADR solo compra el dato.

## Lo que este ADR no decide

- Qué etapas pasan a ejecutarse por impacto y con qué suelo (su propio ADR, con
  estos datos y la paridad delante).
- Reutilización de recibos en la lane de merge (sigue opt-in por ADR-TH-007).
- Caché o reanudación dentro de PipelineK, que el mandato excluye por ahora.
