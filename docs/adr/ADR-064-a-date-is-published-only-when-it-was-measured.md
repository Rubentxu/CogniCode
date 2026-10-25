# ADR-064 — Una fecha se publica solo si se midió

**Estado**: EJECUTADO (decidido el 2026-10-25; ejecutado el 2026-10-25)
**Fecha**: 2026-10-25
**Decisores**: Maintainer
**Contexto**: H5 · la base temporal de la que depende la incrementalidad, y la superficie publicada del protocolo (`crates/cognicode-core/src/application/dto/common.rs`)

---

## Contexto

### Lo que faltaba

El producto publica la marca de tiempo de un fichero. La publica como un
número de segundos desde la época, y ese número dice siempre algo.

Medido, en el árbol de `229991e8`, con un fichero cuya fecha es un minuto
antes de la época:

- `crates/cognicode-core/src/infrastructure/graph/source_files.rs:242` — la
  medida interna del inventario publicaba `mtime_millis = 0` cuando la
  fecha no se podía leer.
- `crates/cognicode-core/src/application/services/file_operations.rs:1173` y
  `:1898` — la superficie publicada (`read_file` y `list_files`) hacía lo
  mismo.
- `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs:998` — el
  grafo por fichero no publicaba cero: publicaba **la distancia a la época**
  en vez del instante. Su comentario decía «saturate to 0» y el código hacía
  otra cosa. Un fichero de 1960 se publicaba como 1980: una fecha plausible,
  creíble y equivocada.
- `crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs:1888` —
  el manejador de reparseo usaba `mtime == 0` como centinela de «borrado». Un
  fichero anterior a la época cumplía el centinela y se reportaba como
  **eliminado del grafo**, con sus símbolos fuera.
- `crates/cognicode-core/src/application/workspace_session.rs:396` y `:471`,
  `crates/cognicode-core/src/infrastructure/graph/file_manifest.rs:115`,
  `:169` y `:194`, y
  `crates/cognicode-core/src/infrastructure/git/git_history.rs:99` — el mismo
  defecto en el resto de familias.

Once sitios en siete ficheros, nueve de los cuales medían la fecha de un
fichero por su cuenta.

### Por qué era invisible a la suite entera

La suite entera probaba que la fecha era *un número*, no que fuera *la fecha
de ese fichero*. Las dos aserciones que existían —
`crates/cognicode/tests/file_ops_integration_tests.rs:888` y
`crates/cognicode-core/src/interface/mcp/file_ops_handlers.rs:1173` —
comprobaban `modified > 0`, que es exactamente la condición que el defecto
cumple: cero es «no es un número positivo», y la prueba leía eso como
«tiene fecha».

### Lo que no se rompía

La **caché nunca sirvió un grafo viejo**. La invalidación del manifiesto
compara también el hash del contenido
(`crates/cognicode-core/src/interface/mcp/handlers/mod.rs:1040`), de modo que
un `mtime` a cero no servía nada. El defecto era de verdad, no de
estabilidad: el defecto era que el producto **publicaba una fecha que no
había medido**.

---

## Decisión

### Tres hechos, no uno

El defecto de fondo era que tres hechos distintos se vestían del mismo
número:

1. el fichero se escribió en un instante **medido**, que puede ser anterior a
   la época;
2. el fichero se escribió **exactamente** en la época;
3. el sistema de ficheros **no publicó** ninguna fecha.

Los tres se publicaban como `0`. El segundo es una medición; el tercero no
lo es, y no puede vestirse de una: `0` dice «1 de enero de 1970», que es
una afirmación sobre un fichero que nadie midió así.

Por eso el tipo publicado pasa a ser **`Option<i64>`**:

- `Some(0)` — escrito en la época: una medición.
- `Some(-60)` — escrito un minuto antes de la época: otra medición.
- `None` — no hay fecha: la ausencia de una medición.

El signo no es decorativo. Un fichero puede traer una fecha anterior a la
época —un tar que conserva sus marcas de tiempo, una copia de seguridad
restaurada— y un contador sin signo no la puede representar. Publicar `0` en
ese caso no es "no tener fecha": es tener una fecha y equivocarse.

### Un dueño único de la medida

La medida vive en un módulo del dominio,
`crates/cognicode-core/src/domain/value_objects/file_time.rs`, con una API
pequeña y explícita:

| Función | Para qué |
|---|---|
| `epoch_millis` / `epoch_nanos` / `epoch_seconds` | Convertir un instante, con signo |
| `measured_millis` / `measured_nanos` / `measured_seconds` | La medida, o ninguna si no hay fecha que medir |
| `seconds_of_millis` | Los segundos de una medida ya hecha |

El corte es **hacia el suelo**: un instante que cae dentro del segundo
anterior a la época cuenta como `-1`, no como `0`, para que no se confunda
con la época. La regla del corte vive en un solo sitio, y las dos rutas que
llevan la medida —el reloj y la medida ya hecha— dicen lo mismo del mismo
instante.

Todos los sitios de producción miden ahora por el dueño. Donde el tipo no
admitía la ausencia —el manifiesto binario, que se serializa— el cambio de
tipo cambia el formato, y por eso la versión del snapshot pasa de
`cognicode.graph.cache/v2` a `v3`: un snapshot de la versión anterior no
puede leerse con la forma nueva, y su versión lo dice, de modo que se
reconstruye en vez de aceptarse como evidencia.

### La caché sigue invalidando por contenido

Un fichero sin reloj no invalida el snapshot por su cuenta. En el manifiesto,
`entry.mtime == file.mtime_millis` compara dos ausencias y da «igual»; lo
que decide es el hash del contenido, que es la identidad de los bytes. Lo
mismo en la caché de ficheros del servicio de análisis y en el manifiesto
incremental del grafo. El motivo queda escrito en el sitio, que es donde se
decide.

### El hueco que queda, dicho

La familia de la ingestión
(`crates/cognicode-core/src/application/ingest/scan.rs`) mide en segundos con
coma flotante y persiste el resultado en la base de datos de Ladybug, en otro
crate. Su hueco es doble: una fecha anterior a la época se publica como `0`,
y un fichero cuya fecha no se puede leer **desaparece del escaneo** en vez de
declararse. Arreglarla cambia el esquema persistido y el adaptador de otro
crate, así que es una unidad propia, medida y encolada aparte.

El contrato de CI la registra como hueco conocido, con el motivo. Si el hueco
se cierra, el contrato avisa para que el registro se actualice: un registro
que ya no dice la verdad es peor que no tener registro.

---

## Alternativas consideradas

### (a) Mantener `u64` y usar un centinela para «no hay fecha»

Descartada. Un centinela es un número, y el producto publica números. El
cliente no puede distinguir «la época» de «no se midió», que es justo lo que
hay que distinguir. Además, `0` ya está ocupado por un significado real.

### (b) `Option<u64>`: la ausencia existe, pero la fecha anterior a la época se pierde

Descartada. Es la alternativa más cercana a la elegida y resuelve el caso
principal, pero degrada una fecha **conocida** a «no hay fecha». Un fichero
de 1960 pasaría de publicar su fecha a publicar `null`, y el consumidor
perdería un dato que el producto sí tenía. El tipo con signo no cuesta nada
y no pierde nada.

### (c) Acotar el arreglo a la superficie publicada

Descartada. La línea que la auditoría señalaba primero
(`source_files.rs:242`) es la medida **interna**, y es la base temporal de la
que depende la incrementalidad. Dejar el interior fabricando y arreglar solo
lo publicado deja dos verdades sobre la misma fecha, que es la forma
habitual de que un defecto vuelva.

### (d) `Option<i64>` en toda la superficie publicada

**Elegida.** Distingue los tres hechos, no pierde ninguna medición, y para un
fichero con fecha normal el JSON no cambia: sigue siendo un número. El único
valor nuevo en el hilo es `null`, que aparece cuando de verdad no hay fecha.

---

## Consecuencias

### Positivas

- El consumidor puede ver la diferencia entre una fecha medida y una fecha
  que no existe, que es lo que el contrato de la unidad pedía.
- Una fecha anterior a la época se publica como la fecha que es, con su
  signo, en vez de como la época o como una fecha plausible y equivocada.
- Un fichero anterior a la época deja de reportarse como eliminado del
  grafo.
- La medida tiene un dueño, y el contrato de CI deriva de los ficheros
  donde se mide, no de una lista que se puede quedar corta.

### Negativas

- **Cambio de contrato publicado.** `FileMetadata.modified` y
  `FileEntry.modified` pasan de `u64` a `Option<i64>`. Para un fichero con
  fecha normal el JSON no cambia; para un fichero sin fecha pasa de `0` a
  `null`. Un consumidor que deserialize en `u64` sin admitir `null` tiene
  que admitlo. El manifiesto de herramientas (`product/tools.json`) declara
  `output_schema: null` para `read_file` y `list_files`, así que no hay
  esquema publicado que corregir.
- **Cambio de formato en disco.** El manifiesto binario del grafo cambia de
  forma. Por eso el snapshot sube de versión y se reconstruye.
- El hueco de la ingestión sigue abierto, y registrado.

### Mitigaciones

- La prueba de frontera (`crates/cognicode-core/tests/a_date_is_published_only_when_it_was_measured.rs`)
  comprueba las dos caras: que un fichero anterior a la época publica su
  fecha, y que un fichero con fecha normal publica el número que se midió.
  La segunda es la contraprueba: sin ella, un producto que publicara siempre
  `null` pasaría.
- El contrato de CI (`scripts/ci/test_a_date_is_published_only_when_it_was_measured.py`)
  vigila el tipo publicado, la existencia del dueño, la ausencia de
  conversiones propias en producción y la verdad del registro de huecos.

---

## Referencias

- Contrato de la unidad: `SPEC-CU-005-SNAPSHOT-INCREMENTAL.md` (CU-005.1:
  identidad sin suposiciones de hora como autoridad).
- ADR-063 — Un rechazo llega al cliente como un rechazo: el mismo criterio,
  en el borde del protocolo: lo que el cliente no puede distinguir, el
  producto no lo puede publicar.
- Unit Candidate C45 en `goal-compass.json`.
