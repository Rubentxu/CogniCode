# ADR-TH-014 — El gate de rustdoc vuelve a la autoridad, porque el coste era el target compartido

**Estado:** ACCEPTED. **Fecha:** 2026-10-16. **Enmienda a:** ADR-TH-011 (dueño único
del gate de rustdoc). **Autoridad del gate:** la obligación vuelve a la lane que
bloquea `main`; ninguna garantía queda fuera.

## Contexto

ADR-TH-011 movió `rustdoc-gate` de `merge-gate.pipeline.kts` a
`integration.pipeline.kts` el 2026-10-15 con dos afirmaciones:

1. **Medida y cierta**: el mismo target corría dos veces por cambio y la copia del
   merge costaba 667 s dentro de un merge de 1 630 s.
2. **Falsa**: que integración «es la lane que bloquea `main`». No lo es.
   `scripts/ci/pipeline_authority.py` declara
   `MERGE_AUTHORITY = "merge-gate.pipeline.kts"` y nada más, y `AGENTS.md` lo repite
   (`strict:true, contexts:[merge-gate]`).

El contrato se reescribió para exigir ese estado mientras su prosa afirmaba «in a
lane that blocks `main`»: prosa y aserción discrepantes, un contrato que certificaba
el agujero en vez de señalarlo.

## El recorrido, con los números que lo decidieron

1. **Intento de devolverla**. Se midió el precio en el gate: la etapa costaba
   **700,2 s** (corrida `e3465380`), contra un presupuesto de merge de 6–7 min. Con
   esa cifra, mover la etapa y mover el presupuesto eran la misma decisión.
2. **Se declaró la desviación** en vez de esconderla: `declared_deviations.json` con
   dueño, motivo, precio, fecha, corrida, ADR y el comando que restaura la garantía,
   y un contrato que exige esa declaración (y su ausencia cuando la autoridad sí la
   ejecuta).
3. **Se midió el motivo de los 700 s**, y no era el chequeo: era el **directorio de
   target compartido**. `cargo doc --workspace --no-deps` no reutiliza su caché
   cuando comparte directorio con `clippy --all-targets` y las suites. Con un
   directorio propio, medido en el mismo worktree:

   | Medición | Valor |
   |---|---|
   | Primera vez (frío, 12 crates) | **124 s** |
   | Repetición inmediata | **1 s** |
   | Tras un build de tests en el directorio compartido | **1 s** |
   | Tras tocar el `src` de `cognicode-core` | **~70 s** |

   El chequeo nunca fue caro por ser un chequeo: era caro por dónde compilaba.

## Decisión

1. **La stage nombrada `rustdoc-gate` vive en `merge-gate.pipeline.kts`**, y ese es
   su único dueño. Una garantía que no puede fallar un merge no es un gate.
2. **El target fija su propio directorio de target** para el `cargo doc` que ejecuta
   (`CARGO_TARGET_DIR=<target_directory>/doc-check`, resuelto con `cargo metadata`,
   es decir un subdirectorio del que la configuración del repositorio ya declara).
   Estable entre corridas, y aislado de las invocaciones mezcladas que lo invalidaban.
3. **La desviación declarada desaparece**: `declared_deviations.json` queda vacío. El
   mecanismo se queda, porque el problema que resuelve —que una garantía no pueda
   salir de la lane que bloquea `main` sin declararse, medirse y fecharse— sigue
   existiendo; hoy no hay ninguna desviación que declarar.
4. **El contrato exige las dos direcciones**: un dueño nombrado y único; declaración
   cuando el dueño no es la autoridad; y **ausencia** de declaración cuando sí lo es
   («stale declaration»).
5. **`integration.pipeline.kts` no tiene etapa nombrada**, aunque su
   `--all-targets` siga ejecutando el target como consecuencia de correr el workspace
   entero: eso no es una obligación nombrada ni un dueño.

## Consecuencias

- La lane que bloquea `main` recupera la comprobación de M0.11 sin pagar 700 s: en
  régimen estable cuesta segundos, y su coste real aparece cuando alguien toca las
  fuentes de las que dependen los documentos.
- El defecto que sí gravaba todo el trabajo en un worktree quedó corregido y
  vigilado por contrato: `crates/cognicode-cli/build.rs` declaraba
  `rerun-if-changed=../../.git/HEAD`, ruta que no existe cuando `.git` es un fichero,
  y cargo trata una entrada declarada y ausente como siempre sucia (`cargo doc`
  repetido: 163 s → 1 s; `-v` con 0 unidades `Dirty`).
- ADR-TH-011 no se reescribe: se enmienda a la vista. Su medición era correcta y su
  conclusión era falsa, y las dos cosas quedan escritas.
- **La decisión que este ADR no tuvo que escalar**: pagar 700 s por merge. Ya no hay
  nada que decidir sobre el presupuesto de este chequeo.

## Lo que este ADR no decide

- El presupuesto de merge en general ni la selección por impacto (W6).
- Que integración deje de ejecutar el target dentro de `--all-targets`.
