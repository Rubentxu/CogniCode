# ADR-062 — El consumidor declara la versión sobre la que escribe

**Estado**: EJECUTADO (decidido el 2026-10-25; ejecutado el 2026-10-25)
**Fecha**: 2026-10-25
**Decisores**: Maintainer
**Contexto**: H29 · el borde del protocolo (`crates/cognicode-core/src/interface/mcp/`), el catálogo publicado (`product/tools.json`) y su captura (`product/tool-catalog-runtime.json`)

---

## Contexto

### Lo que faltaba

`ADR-061` dejó la escritura condicional en la capa de aplicación: el
llamante puede declarar la versión que leyó y la escritura se niega si el
destino ya no la tiene. Lo que **no** cruzaba era el borde.

Medido, en el árbol de `98edb89c`:

- `crates/cognicode-core/src/interface/mcp/dto_mapping.rs:69` y `:92` — el
  campo se fijaba a `None` a propósito: el protocolo lo recibía y lo
  perdía.
- `crates/cognicode-core/src/interface/mcp/schemas.rs` — `WriteFileInput` y
  `EditFileInput` no declaraban el campo, de modo que un cliente que lo
  enviara ni siquiera llega al servicio.
- `ReadFileOutput` no publicaba ningún digest: el consumidor **no tenía forma
  de nombrar la versión que leyó**. Podía escribir declarando una versión,
  pero no había ningún sitio del que sacarla.

Es decir: la capacidad existía y era inaccesible. Un agente que lee un
fichero y lo reescribe sigue pisando el trabajo de otro editor, porque el
único camino para saber qué versión leyó no existe.

### Lo que se rompe al cambiar el catálogo

El esquema que el consumidor ve no sale del tipo Rust: sale de
`product/tools.json`, que el runtime publica tal cual
(`crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs:577` lo toma del
descriptor). El documento es de **autoría manual**, y su captura
(`product/tool-catalog-runtime.json`) es lo que prueba qué publica el
runtime de verdad.

Hay una trampas en el camino, y está documentada en el propio repositorio:
`product/README.md` instruía un procedimiento que **regeneraba
`product/tools.json` con el generador**, y el generador no es dueño de nueve
campos (`limitations`, `required_args`, `prerequisites`, `follow_ups`,
`intent_tags`, `language_scope`, `feature`, `impact_direction_floor`,
`precision_floor`): no tienen origen en el `_meta.cognicode` del runtime. El
procedimiento, tal como estaba escrito, borraba los nueve. La decisión ya
tomada —el maestro es de autoría manual y no se regenera— está en el Journal,
pero el README seguía instruyendo lo contrario.

## Decisión

**El consumidor declara la versión, y el servicio la comprueba.**

1. **`read_file` publica el digest del fichero entero.** Sale de la **misma
   lectura que cuenta las líneas** (`count_lines_and_hash`): no hay una
   segunda pasada por el fichero, y el digest nombra exactamente los bytes que
   el fichero tenía cuando se contó. El orden es además el seguro: el digest
   se calcula *antes* de leer el contenido, así que si el fichero cambia entre
   medias, el digest sigue nombrando la versión más antigua y una escritura
   que lo declare **se niega** en vez de aceptarse por encima de contenido más
   nuevo.

2. **El digest nombra el fichero entero, no el trozo.** Es lo que lo hace
   comparable con lo que la escritura comprueba, que también es el fichero
   entero. Un digest del trozo sería incompatible: ninguna escritura podría
   declararlo.

3. **El MCP acepta `expected_content_hash` y lo lleva a la aplicación.** Ya
   no a `None`: la conversión lo transporta, en `write_file` y en `edit_file`.

4. **El documento publicado declara el campo**, en las dos tools que lo
   aceptan, y la captura se refresca desde un `tools/list` real.

5. **Un contrato vigila que el documento publicado y la captura digan lo
   mismo.** El gate anterior sólo comparaba el *conjunto de nombres*: un
   documento y una captura pueden coincidir en los 78 nombres y discrepar en
   la forma que el consumidor pasa, y nadie se entera. El contrato compara el
   **esquema de entrada** de cada tool, y cada detector se aplica a un texto
   que lo viola, para que un detector que no mira nada no pueda declararse
   verde.

6. **El README deja de instruir un procedimiento que destruye datos.** La
   captura se regenera; el maestro se escribe a mano; el generador produce una
   proyección para comparar, no un fichero que sobrescribir.

## Alternativas consideradas

- **Un digest del trozo devuelto.** Incompatible con la comprobación del
  destino, que mira el fichero entero: toda escritura condicional sería
  rechazada. Descartada.
- **Una lectura extra para el digest.** Coste de E/S en cada lectura, y una
  ventana en la que el fichero puede cambiar entre el digest y el contenido.
  Descartada: la lectura que ya se hacía trae los bytes.
- **Calcular el digest *después* de leer el contenido.** Orden inseguro: si el
  fichero cambia entre medias, el digest nombra la versión nueva y la
  escritura se acepta por encima de contenido que el llamante nunca vio.
  Descartada.
- **Una tool nueva, `file_version(path)`.** Más superficie publicada y una
  llamada más por cada lectura; el consumidor ya llama a `read_file`.
  Descartada.
- **Regenerar `product/tools.json` con el generador.** Borra nueve campos
  curados a mano. Descartada: contradice la decisión ya registrada.
- **Un hash nuevo.** El repositorio ya tiene `domain::findings::sha256_hex`,
  que es la identidad de contenido del producto. Descartada.

## Consecuencias

- **Un cliente puede leer, escribir declarando la versión leída, y ser
  rechazado cuando la versión ya no es la.** Con el binario real por stdio:
  `read_file` devuelve el digest, `write_file` declarándolo se aplica, y con
  el digest viejo se niega con el mensaje que dice las dos versiones.
- **El contrato publicado de `write_file` y `edit_file` crece con un campo
  opcional.** Compatible: el que no lo declara sigue escribiendo como antes.
- **`read_file` publica un campo más.** El digest del fichero entero.
- **La captura se refrescó de verdad.** Venía de `v0.101.17`; el runtime
  reportaba `0.101.31`. Al refrescarla cambian el `tool_version` de los 78
  tools y las descripciones del esquema de `build_graph` que habían cambiado
  por fuera. Ninguna de las dos cosas la veía el gate, porque el gate
  comparaba nombres.
- **Queda medido y encolado, no resuelto aquí:**
  - El `runtime_metadata.tool_version` del maestro dice `0.99.2` mientras el
    runtime reporta `0.101.31`, y `check_architecture` lleva dos claves de
    `runtime_metadata` que el runtime no publica. Es deriva de otro tipo: la
    del documento, no la del esquema de entrada.
  - Los handlers de ficheros aplanan todo error a `AppError::InternalError`
    cuando hay métricas instaladas (`file_ops_handlers.rs:139`), de modo que
    un rechazo deliberado llega al cliente como *"Internal error"*. El
    rechazo ocurre y su mensaje dice las dos versiones, así que la garantía
    se cumple; lo que falla es la taxonomía, que es otra unidad.

## Referencias

- Recibo de la unidad `C42` en `docs/roadmap/JOURNAL.md`.
- `ADR-061` — La escritura se publica entera, o no se publica: la capacidad
  que esta unidad hace alcanzable desde el protocolo.
- `ADR-060` — Un corte por byte no parte un carácter: la otra mitad de H29.
