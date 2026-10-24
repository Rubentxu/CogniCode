# SPEC-CU-010 — La lectura declara la identidad de lo que leyó

**Estado:** PROPOSED · **Bloques:** CU-3 · **Superficie:** control plane HTTP

## Intención

El control plane responde a una pregunta sobre la arquitectura de un
proyecto: qué reglas se violan, con qué coordenadas y sobre qué fuentes. La
respuesta es un **veredicto**, y un veredicto sin saber **sobre qué código**
se emitió no se puede usar: sirve igual para el código que hay ahora y
para el que había la semana pasada.

Un consumidor que quiere saber si lo que tiene delante es lo que se
analizó necesita comparar dos cosas: el código que tiene delante y el
código que el producto miró. El producto publicaba el segundo sin el
primero, así que la comparación no se podía hacer y el consumidor tenía
que adivinarlo.

Lo que faltaba no era un checksum: era **decir de qué fuentes sale el
veredicto**. El código de un proyecto es un conjunto de ficheros, y la
identidad de ese conjunto es lo que permite decir «esto es lo que se
leyó» sin ambigüedad.

## Contrato

- **CU-010.1** La lectura publica la **identidad de las fuentes que
  sostienen el veredicto**. Un consumidor puede distinguir un análisis del
  código que hay de un análisis de otro código.
- **CU-010.2** La identidad cubre **exactamente** los ficheros que el
  recorrido recogió: ni uno más ni uno menos. Se calcula sobre el mismo
  conjunto que el evaluador examina, no sobre una segunda lectura del
  disco. Es identidad por construcción.
- **CU-010.3** La identidad es **portable**: se calcula sobre la ruta
  **relativa a la raíz leída**, no sobre la ruta absoluta. El mismo código
  tiene la misma identidad donde esté el árbol, de modo que un consumidor
  en otra máquina puede comparar su código con el que el producto miró.
- **CU-010.4** La identidad es **determinista**: el recorrido sigue el
  sistema de ficheros, que no garantiza un orden, así que la forma
  canónica ordena las rutas antes de resumirlas.
- **CU-010.5** Sin ficheros leídos **no hay identidad que publicar**. El
  estado lo dice y el resumen no viaja: el resumen del conjunto vacío se
  parecería a «no hay nada» y a «no lo sé», y son hechos distintos.
- **CU-010.6** Las dos rutas del control plane —la completa y la mínima—
  publican **el mismo campo** con la misma identidad. El contrato no
  depende de qué binario lo sirva.

## Forma canónica de la identidad

La identidad es un resumen del conjunto de fuentes. La forma canónica de
ese conjunto, que un consumidor tiene que poder reproducir, es:

```
para cada (ruta_relativa, contenido), ordenado por ruta_relativa:
    u64_le(longitud de la ruta en bytes) || ruta_relativa en UTF-8
    u64_le(longitud del contenido en bytes) || contenido en UTF-8

identidad = "sha256:" || hex(SHA-256 de lo anterior)[0..16 bytes]
```

- **ruta_relativa** es la ruta del fichero respecto de la raíz leída, con
  `/` como separador y con su extensión: `src/domain/service.rs`.
- **longitud prefijada**: cada piece va precedida de su longitud, para que
  ninguna concatenación pueda confundirse con otra.
- **128 bits**: el resumen se trunca a los primeros 16 bytes del SHA-256.
  Es suficiente para distinguir dos conjuntos de fuentes y mantiene pequeño
  el campo publicado.

## Ejemplo del contrato de la lectura (esquema orientativo)

```json
{
  "workspace_ref": "ws-1",
  "status": "evaluated",
  "source_basis": {
    "files": 2,
    "digest": "sha256:2a90719362775a301507d0462214ab42",
    "status": "computed",
    "reason": null
  },
  "violations": []
}
```

Y cuando la lectura no recogió nada:

```json
{
  "source_basis": {
    "files": 0,
    "digest": null,
    "status": "empty",
    "reason": "la lectura no recogió ningún fichero de fuentes"
  }
}
```

## Garantías mínimas

- Un consumidor sabe qué código sostiene el veredicto que está leyendo.
- El mismo código tiene la misma identidad en cualquier máquina y en
  cualquier checkout.
- Un cambio en cualquier fichero leído cambia la identidad.
- Una lectura sin ficheros no publica un resumen que se pueda confundir
  con «no lo sé».

**Pruebas:** un test permanente que compruebe que el mismo código en dos
raíces distintas produce la misma identidad; un test que compruebe que
cambiar un fichero cambia la identidad y que repetir la lectura no; un
test que compruebe que un árbol vacío publica el estado sin resumen; un
test que compare la identidad que publican las dos rutas sobre el mismo
árbol. Referencias:
`crates/cognicode-core/src/application/architecture/control_query.rs`,
`crates/cognicode-explorer/src/api.rs`,
`crates/cognicode-explorer/tests/cp1_control_plane_endpoint.rs`.

## Lo que este contrato no dice

- **No dice que el resumen detecte un cambio por sí solo.** Dice que dos
  lecturas de contenidos distintos dan identidades distintas. Qué hacer
  con esa diferencia —invalidar una caché, marcar un análisis como
  viejo— es de otro contrato.
- **No dice nada de las rutas que el veredicto publica.** Las violaciones
  siguen llevando la ruta como la llevaba el recorrido. La identidad y la
  ruta de una violación son dos cosas, y esta unidad no las mezcla.
- **No promete una identidad para un árbol que el producto no sabe
  leer.** Si el recorrido no recogió ficheros, no hay identidad: hay un
  estado.
