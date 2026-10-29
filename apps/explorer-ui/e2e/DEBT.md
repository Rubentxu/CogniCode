# Deuda de la suite E2E — los casos desactivados y quién los toma

Un caso desactivado sigue siendo un caso: cuesta tiempo, aparece en el
informe como *skipped* y, si no dice por qué, no le sirve de nada a quien
lo lea. Este fichero es el registro publicado de los dueños de esa deuda
en esta suite.

## La marca

Cada desactivación lleva una marca, con el motivo y el dueño en el mismo
corchete:

```
[DEBT: <razón> — owner: <id>]
```

El motivo y el dueño comparten corchete a propósito: un motivo sin dueño es
exactamente la deuda que este registro cierra, así que no pueden escribirse
por separado. La razón es el título del caso en `test.skip("…")` y el
segundo argumento en `test.skip(true, "…")`.

## Los dueños

| Dueño | Qué toma | Casos |
|---|---|---|
| `TASK-9` | Los mocks de MSW no exponen el endpoint, la bandera o el dato que el caso necesita | 11 |
| `TASK-10` | La aplicación no expone una señal observable del cambio que el caso quiere comprobar | 4 |
| `TASK-11` | El modal construido con `div` no atrapa el foco y `Escape` no llega al manejador | 1 |
| `TASK-12` | El layout de 320 px desborda en horizontal | 1 |

Once más cuatro más uno más uno son **17** casos desactivados en 14 ficheros
de spec.

La auditoría externa del 2026-10-28 contó **16 en 13 ficheros**: contaba
llamadas a `test.skip` y no vio el `test.fixme` condicional de
`responsive-full.spec.ts`, que desactiva el viewport de 320 px. El
recuento de este fichero es el medido, y el contrato fija el tope de cada
API por separado —16 `test.skip` y 1 `test.fixme`— para que ninguna crezca
tomando prestada la otra.

## Qué lo vigila

`scripts/ci/test_explorer_e2e_disabled_cases_have_owners.py` es el contrato
que lee este fichero. Comprueba que cada desactivación lleva su marca con
dueño, que todo dueño citado está publicado aquí, que todo dueño publicado
está citado por al menos un caso, que los recuentos de la tabla coinciden
con el árbol, y que el número de casos desactivados no crece.

Dos loopholes quedan cerrados por el mismo contrato:

- una desactivación escondida tras un alias (`const testFn = test.fixme`)
  no se puede contar, así que no se puede poseer: el contrato la reporta;
- un helper que desactiva y que una spec consume dejaría un caso
  desactivado sin marca propia. `utils/feature-flags.ts` contiene uno
  (`skipIfDebt`), hoy sin ningún consumidor: el contrato exige que siga
  sin consumirse, o que su desactivación se posea donde está escrita.

Las tareas proprietarias viven en la bandeja local de Backlog.md, fuera de
Git por la política de documentos privados; este fichero es la parte
publicada.
