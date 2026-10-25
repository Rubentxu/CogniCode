# ADR-061 — La escritura se publica entera, o no se publica

**Estado**: EJECUTADO (decidido el 2026-10-25; ejecutado el 2026-10-25)
**Fecha**: 2026-10-25
**Decisores**: Maintainer
**Contexto**: H29 · el servicio de ficheros (`crates/cognicode-core/src/application/services/file_operations.rs`), candidato `C41` de la cola de la misión

---

## Contexto

### Lo que dice la auditoría

Una auditoría externa de 2026-10-25 señaló, como P2, que la escritura del
servicio de ficheros todavía no era transaccional. Citaba tres defectos
concretos y una cuarta cuestión de contrato: `File::create` no exige creación
exclusiva; si la escritura falla, el temporal puede quedar en disco; no existe
una condición de versión que impida sobrescribir cambios hechos por otro
editor entre la lectura y la escritura; y un `rename` atómico no equivale por
sí solo a persistencia garantizada tras un fallo eléctrico.

### Lo que mide el árbol

**MEDIDO 2026-10-25**, con ejecución, sobre `01d196a1` (la base de esta rama).
El código anterior de `write_file` era este, entero:

```rust
// Atomic write: write to temp file then rename
let temp_path = unique_temp_path(&validated_path);

// Write content to temp file
{
    let mut file = File::create(&temp_path).map_err(|e| {
        AppError::InvalidParameter(format!("Failed to create temp file: {}", e))
    })?;
    file.write_all(input.content.as_bytes()).map_err(|e| {
        AppError::InvalidParameter(format!("Failed to write to temp file: {}", e))
    })?;
}

// Atomically rename temp file to target
fs::rename(&temp_path, &validated_path).map_err(|e| {
    // Clean up temp file on error
    let _ = fs::remove_file(&temp_path);
    AppError::InvalidParameter(format!("Failed to write file atomically: {}", e))
})?;
```

Los cuatro puntos donde el defecto era real:

| Sitio | Qué pasa | Consecuencia |
|---|---|---|
| `file_operations.rs:1215` | `File::create(&temp_path)` | no exige creación exclusiva: un temporal que ya estaba se abre y **se trunca** |
| `file_operations.rs:1218` | `write_all(...)?` | si la escritura falla, el `?` sale y **el temporal se queda en disco**: la limpieza sólo estaba en el error del `rename` |
| `file_operations.rs:1224` | `fs::rename` | el cambio de nombre es atómico, pero no hay `fsync`: un corte de luz puede dejar el destino con el contenido viejo, o con el nombre viejo |
| `file_operations.rs:1380`, `:1388` | lo mismo en `edit_file` | una edición **pisa el trabajo de otro editor** sin preguntar: no había ninguna comprobación de versión |

Lo cuarto no es un defecto de robustez: es una **pérdida de trabajo**. El
servicio lee un fichero, calcula el cambio, y lo publica. Si otra persona
—otro agente, otro editor, un formateador— escribió entre la lectura y la
publicación, ese trabajo disappears sin que nadie lo sepa. El agente cree que
ha guardado su cambio sobre la versión que leyó, y no es verdad: ha
sobrescrito una versión distinta de la que nunca llegó a ver.

La misma ruta tampoco comprobaba nada cuando el llamante **sí** quería
declarar con qué versión estaba trabajando: no había forma de decirlo.

### Por qué no es un detalle

El que escribe es el que atiende las peticiones de todos los agentes
conectados al servidor de protocolo. Un temporal que se trunca significa que
dos escrituras simultáneas —o un temporal de una ejecución anterior— pueden
producir un fichero con el contenido de una y el nombre de otra. Un temporal
abandonado es basura que el siguiente barrido no encuentra, porque no sabe
buscar temporales. Y una edición que pisa el trabajo ajeno hace que el agente
y la persona no se pongan de acuerdo sobre lo que hay en el disco, y el que
miente es el agente.

## Decisión

**La escritura se publica entera o no se publica, y publica sólo la versión
que el llamante dice que leyó.**

1. **Un adaptador único para publicar.** `crates/cognicode-core/src/application/services/atomic_file_writer.rs`
   (nuevo) es el único camino por el que el servicio escribe un fichero:
   - El temporal se **crea en exclusiva** (`create_new(true)`). Un nombre que ya
     existe no se abre ni se trunca: se busca el siguiente, hasta 16 intentos.
     El nombre lleva el identificador del proceso, así que dos procesos
     distintos no se pisan.
   - El contenido se **sincroniza** (`sync_all`) antes de que el destino exista,
     y el **directorio** se sincroniza después del cambio de nombre. Es lo que
     hace que la publicación sobreviva a un corte de luz: no basta con que el
     contenido esté en el disco; el nombre nuevo también tiene que estarlo.
   - La **limpieza está en `Drop`**, no en el camino de error. Lo que no se
     publica se borra porque el escritor se suelta, no porque alguien se
     acuerde de borrar.

2. **El llamante puede declarar la versión con la que está trabajando.**
   `WriteFileRequest` y `EditFileRequest` llevan un `expected_content_hash`
   opcional. Si lo declara, el servicio compara esa versión con la que hay en
   el destino y **se niega** si no son la misma. Si no lo declara, el
   comportamiento es el de siempre.

3. **La versión se comprueba dos veces.** Al principio —antes de crear nada,
   para que una petición rechazada no deje ni el fichero ni los directorios que
   pidió— y **justo antes de publicar**, que es donde se detecta el cambio
   que ocurrió mientras la petición se preparaba. `edit_file` compara además
   el contenido que leyó con el que hay ahora, **incluso sin hash declarado**:
   si el fichero cambió entre la lectura y la publicación, la edición se niega
   en vez de pisar.

4. **El digest es el del producto, no un digest nuevo.** Se usa
   `domain::findings::sha256_hex`, que ya es la identidad de contenido del
   resto del repositorio. Una implementación de hash nueva habría sido una
   tercera forma de decir lo mismo —el repositorio ya tenía el FNV-64
   normalizado de la evidencia portable y el `sha256:` prefijado del registro
   de inteligencia—, y el producto no necesita una cuarta.

5. **Un contrato vigila que no vuelva.** No lista los errores prohibidos uno a
   uno —esa lista caduca—, sino que mira el código de producción y exige que
   las garantías estén donde tienen que estar: el temporal en exclusiva, la
   sincronización antes del cambio de nombre, la limpieza en `Drop`, el
   servicio publicando por el adaptador, y la comprobación de la versión antes
   de cada publicación. Cada regla se puede aplicar a un texto, así que el
   propio contrato comprueba que **cada regla nota un texto que la viola**:
   un detector que no mira nada no puede declararse verde.

## Alternativas consideradas

- **Borrar el temporal en cada camino de error.** Es lo que había, y es
  exactamente la forma del defecto: la limpieza depende de que nadie añada un
  camino de error nuevo. Un `Drop` no se olvida. Descartada.
- **Un contador atómico para el nombre del temporal.** El repositorio ya tenía
  uno, y evita que dos escrituras *del mismo proceso* compartan nombre. No
  evita que un temporal preexistente se trunque, que es el defecto
  Measured. Descartada: la creación exclusiva resuelve el caso más fuerte.
- **`fsync` sólo en el fichero, no en el directorio.** Deja el contenido
  durable pero el nombre no: tras un corte de luz puede aparecer el fichero
  viejo con el contenido nuevo, o ninguno de los dos. Se synchronizan los
  dos. Descartada la variante más débil.
- **Un hash nuevo, BLAKE3, como el de la caché incremental.** Es más rápido,
  pero el valor viaja en una petición y tiene que poder calcularse en
  cualquier construcción del producto, con o sin las features opcionales. El
  digest de la caché está detrás de `persistence`; el de aquí no. Además, un
  digest nuevo habría sido una cuarta manera de nombrar un contenido.
  Descartada.
- **Publicar el campo por el MCP en la misma unidad.** El catálogo público de
  herramientas se **genera desde una captura real de `tools/list`**
  (`scripts/product/generate_tool_catalog.py`) y el gate lo compara: añadir un
  campo al esquema del MCP cambia el catálogo publicado, y ese catálogo no se
  puede editar a mano. La publicación por el MCP es otra unidad (`C42`),
  medida y en la cola. Descartada para esta unidad.

## Consecuencias

- **Un temporal preexistente ya no se trunca.** Si el nombre está ocupado, el
  escritor busca el siguiente y lo que estaba ahí conserva su contenido.
- **Un fallo de escritura no deja basura.** La limpieza está en `Drop`, así que
  vale para cualquier camino de error, incluidos los que se añadan después.
- **La publicación sobrevive a un corte de luz**, dentro de lo que el
  sistema de ficheros prometa: el contenido está sincronizado antes del
  cambio de nombre, y el directorio después.
- **Una edición ya no pisa el trabajo ajeno.** Con hash declarado, la
  petición se niega y el destino conserva lo que tenía. Sin hash declarado,
  `edit_file` sigue comparando el contenido que leyó con el que hay, y se
  niega si no coinciden.
- **El consumidor del MCP no ve nada nuevo.** El campo existe en las
  peticiones de aplicación y vale `None` en la frontera del MCP: los
  contratos publicados de `cognicode`, `cognicode-mcp` y `cogh` **no
  cambian**. La publicación por el MCP es `C42`.
- **El digest que el llamante declara es el del producto.** Un consumidor que
  ya calcula `sha256_hex` puede declarar la versión sin aprender nada nuevo.
- **Una escritura rechazada no deja directorios.** La comprobación de la
  versión va antes de crear nada, que es lo que el propio servicio prometía
  desde antes y no cumplía.

## Referencias

- Recibo de la unidad `C41` en `docs/roadmap/JOURNAL.md`.
- `ADR-060` — Un corte por byte no parte un carácter: la otra mitad de H29.
  Leer un fichero no puede abortar el proceso que lo lee ni partir un
  carácter; escribirlo no puede perder lo que escribe ni pisar lo ajeno.
- `ADR-058` — Lo que se rechaza no deja rastro: el mismo linaje. Una entrada
  inválida se rechaza con su motivo; no aborta el proceso que la atiende.
- La auditoría externa de 2026-10-25, apartado 2, P2.
