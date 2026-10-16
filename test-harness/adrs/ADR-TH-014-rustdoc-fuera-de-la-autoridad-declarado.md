# ADR-TH-014 — La garantía de rustdoc queda fuera de la autoridad, declarada y con precio

**Estado:** ACCEPTED (con decisión pendiente del operador sobre el presupuesto).
**Fecha:** 2026-10-16. **Enmienda a:** ADR-TH-011 (dueño único del gate de
rustdoc). **Autoridad del gate:** sin cambios: la obligación tiene **un** dueño
nombrado, y ese dueño no es la autoridad de merge. Lo que cambia respecto a
ADR-TH-011 es que eso ya no se justifica con una frase falsa: se declara en un
fichero que el contrato comprueba, con precio medido.

## Contexto

ADR-TH-011 movió la stage `rustdoc-gate` de `merge-gate.pipeline.kts` a
`integration.pipeline.kts` el 2026-10-15, con dos afirmaciones:

1. **Medida y cierta**: el mismo target se ejecutaba dos veces por cambio (la
   stage nombrada del merge y el `cargo test --workspace --all-targets` de
   integración), y la copia del merge costaba 667 s dentro de un merge de 1 630 s.
2. **Falsa**: que integración «es la lane que bloquea `main`». No lo es.
   `scripts/ci/pipeline_authority.py` declara
   `MERGE_AUTHORITY = "merge-gate.pipeline.kts"` y nada más, y `AGENTS.md` lo
   repite (`strict:true, contexts:[merge-gate]`).

Con la segunda afirmación sosteniendo la primera, el resultado fue que `main`
podía recibir un cambio con avisos de rustdoc en las categorías de M0.11, y que el
contrato lo aceptaba: se reescribió para exigir
`runners == vec!["integration.pipeline.kts"]` mientras su prosa decía «in a lane
that blocks `main`». Prosa y aserción discrepantes: el contrato certificaba el
agujero en vez de señalarlo.

### Lo que se midió el 2026-10-16, intentando arreglarlo

El intento obvio era devolver la etapa al merge. Antes de eso se midió el precio, y
el precio no cabía:

| Medición (2026-10-16) | Valor |
|---|---|
| `rustdoc-gate` dentro del gate, run PipelineK `e3465380` | **700,2 s** |
| Resto de etapas del mismo run en ese punto | 18 etapas, 1 183,7 s acumulados |
| Presupuesto de merge declarado por el operador | 6–7 min (360–420 s) |
| `cargo doc --workspace --no-deps`, segunda pasada, worktree quieto | 300 s sin terminar (timeout) con 5 crates re-documentándose |

Es decir: **el coste es real y recurrente**, no un artefacto de caché. El target
documenta el workspace entero y su caché no sobrevive a las invocaciones mezcladas
con `clippy --all-targets` y el resto de suites del mismo directorio de target.

Durante el mismo trabajo se encontró y corrigió un defecto **independiente** que sí
gravaba cualquier compilación en un worktree: `crates/cognicode-cli/build.rs`
declaraba `cargo:rerun-if-changed=../../.git/HEAD`, ruta que no existe cuando `.git`
es un fichero, y cargo trata una entrada declarada y ausente como «siempre sucia»
(`cargo doc` repetido: 163 s → 1 s; `-v` con 0 unidades `Dirty` tras resolver la
ruta con `git rev-parse --absolute-git-dir`). Ese arreglo es correcto y se queda,
pero **no** elimina el coste del gate de rustdoc: los 700 s medidos en el gate se
deben al trabajo de documentación, no al defecto de la ruta.

## Decisión

1. **La obligación tiene un dueño nombrado y único: `integration.pipeline.kts`.**
   `merge-gate.pipeline.kts` no declara ninguna stage para ese target.
2. **La desviación está declarada por máquina, no por prosa.** El fichero
   `scripts/ci/declared_deviations.json` la lista con: obligación, pipeline dueña,
   motivo, segundos medidos, fecha, corrida donde se midió, ADR y la instrucción
   exacta para revertirla.
3. **El contrato la exige.** `the_rustdoc_gate_runs_in_exactly_one_authority`
   comprueba: (a) la obligación corre en exactamente un `*.pipeline.kts`, por
   nombre dentro de un cuerpo `sh(...)`; (b) si ese dueño **no** es la autoridad de
   merge, la desviación tiene que estar declarada con precio y ADR existente; y (c)
   si el dueño **es** la autoridad de merge, no puede haber una entrada que diga lo
   contrario. Un agujero ya no puede existir sin estar declarado, medido y fechado.
4. **El precio se declara aunque no se pague.** Quien devuelva la etapa al merge
   tiene la medición: 700,2 s, y la instrucción de revertir la declaración.

## Consecuencias

- La garantía de M0.11 se ejecuta en una lane que **no** bloquea un merge. Eso es
  una pérdida real de cobertura en el punto de bloqueo, no una equivalencia: queda
  dicho con esas palabras, en el ADR y en el fichero de desviaciones.
- El presupuesto del merge se respeta porque la etapa no está dentro. La primera
  versión de este ADR afirmaba que la cifra de 667 s de ADR-TH-011 «no se
  sostenía»; la medición del gate la refutó: la cifra era correcta y la decisión de
  B1b era cara pero cierta en su precio. Lo que estaba mal era su justificación.
- El defecto de `rerun-if-changed` queda corregido y vigilado por
  `scripts/ci/test_build_inputs_exist.py`: afectaba a **cualquier** compilación en
  el entorno del operador, no sólo a este gate.
- **Decisión del operador pendiente**: (A) pagar 700 s por merge y recuperar la
  garantía en el punto de bloqueo; (B) dejarla como está (merge ligero, M0.11 sin
  bloquear, declarado); (C) hacer la comprobación barata antes de decidir, con dos
  vías identificadas y sin construir: aislar el `cargo doc` en su propio target dir
  para que su caché sí se reutilice, o ejecutarlo sólo sobre los crates afectados
  por el cambio (selección por impacto, W6).

## Lo que este ADR no decide

- Cuál de las tres opciones se toma: es presupuesto del merge, y eso lo decide el
  operador con estas cifras delante.
- El orden de adopción por etapas (W6) ni el presupuesto de merge en sí.
- Que integración deje de ejecutar el target dentro de `--all-targets`: hoy la
  stage nombrada es el dueño declarado y esa corrida ancha es un efecto colateral.
