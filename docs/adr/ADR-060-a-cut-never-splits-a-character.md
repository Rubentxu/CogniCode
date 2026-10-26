# ADR-060 — Un corte por byte no parte un carácter

**Estado**: EJECUTADO (decidido el 2026-10-25; ejecutado el 2026-10-25)
**Fecha**: 2026-10-25
**Decisores**: Maintainer
**Contexto**: H29 · las operaciones sobre ficheros (`crates/cognicode-core/src/application/services/file_operations.rs`) y la presentación de la CLI, candidato `C40` de la cola de la misión

---

## Contexto

### Lo que dice la auditoría

Una auditoría externa de 2026-10-25 señaló, como P1, que el servicio de ficheros
recorta el texto por índice de byte, y que un índice que cae dentro de la
codificación multibyte de un carácter aborta el proceso en vez de devolver un
error controlado. La auditoría no habia ejecutado el código: leia.

### Lo que mide el árbol

**MEDIDO 2026-10-25**, con ejecución, sobre `2018587a` (la base de esta rama).

El token de continuación es un JSON codificado en base64 con el desplazamiento
que el llamante quiere. **No está firmado**, y su validador comprueba el
tamaño máximo del desplazamiento, el del trozo, que la ruta canónica coincida
y que la ruta esté dentro del workspace — pero no que el desplazamiento caiga
entre dos caracteres. Los seis sitios donde el texto se recortaba por índice:

| Sitio | Corte | Quién elige el índice |
|---|---|---|
| `file_operations.rs:986` | `&content[offset..]` | el token de continuación |
| `file_operations.rs:1003` | `remaining[read_size..]` | el `chunk_size` de la petición |
| `file_operations.rs:1376` | `&s[..max_len]` | el texto que se edita |
| `file_operations.rs:501`, `:526` | `content[..offset]` | el token de continuación |
| `cognicode-cli/src/presenter.rs:672` | `&diagram[..slice_end]` | la longitud del diagrama |
| `cognicode-cli/src/cmd/lifecycle_resolver.rs:445` | `&s[..max]` | la longitud del nombre de una release |

Los cuatro tests que acompañan a este ADR fallan **cuatro de cuatro** sobre el
código anterior, con el pánico exacto:

```
start byte index 1 is not a char boundary; it is inside 'é' (bytes 0..2 of string)
start byte index 2 is not a char boundary; it is inside 'é' (bytes 1..3 of string)
end byte index 50 is not a char boundary; it is inside '€' (bytes 48..51 of string)
```

### Por qué no es un detalle

El proceso que corta es el que atiende las peticiones de todos los agentes
conectados al servidor de protocolo. Un pánico ahí no falla una llamada: se
lleva por delante las de todos los demás. La clase de fallo no necesita un
adversario — basta con que un agente pida un trozo de 1 byte sobre un fichero
con una `é` acentuada.

## Decisión

**El corte del producto se pide con la forma que el tipo hace segura, y el
recorte que tiene que caber en un límite de bytes pasa por una función de
frontera.**

1. **Un módulo de fronteras de carácter.**
   `crates/cognicode-core/src/application/text_boundary.rs` expone cuatro
   operaciones puras: `floor_char_boundary` (el mayor índice que no supera el
   dado y cae entre dos caracteres), `is_char_boundary`, `truncate_at_boundary`
   (recorta a como mucho un número de bytes sin partir un carácter) y
   `slice_at_boundaries` (corta entre dos extremos dados, o dice que no).

2. **Los tramos se piden con `get`.** `str::get` devuelve `None` cuando el
   rango no cae en fronteras de carácter. El servicio pide así sus tramos, y
   el `None` se traduce en un error con su motivo. **La garantía la lleva el
   tipo**, no una validación que un cambio posterior pueda olvidar: si mañana
   alguien vuelve a cortar por índice, el compilador no le avisa, pero el
   contrato nuevo sí.

3. **El límite de bytes se ajusta al carácter entero.** `chunk_size` sigue
   siendo un límite de bytes, pero el trozo devuelto termina en el último
   carácter completo que cabe, así que puede medir menos de lo pedido y nunca
   parte un carácter. Un trozo más pequeño que el primer carácter se rechaza
   con su motivo, en vez de servir medio carácter.

4. **Un contrato vigila que no vuelva.** No lista los cortes prohibidos uno a
   uno —esa lista caduca—, sino que mira cualquier corte por índice sobre
   texto en los ficheros que recortan y exige que pase por `get` o por una
   frontera. Lleva su control anti-vacuidad: el detector ve un corte cuando
   hay uno, y no marca como inseguro uno que usa `get`.

## Alternativas consideradas

- **Validar el desplazamiento y seguir cortando.** Más rápido, pero la
  garantía depende de que nadie olvide la validación: es exactamente el
  patrón que ya falló, porque la validación existente comprobaba el tamaño
  máximo y no la frontera. Descartada.
- **Rechazar el texto que no sea ASCII.** Rompería el producto con el texto
  que tiene que leer. Descartada.
- **Dejar el recorte a cargo de cada llamante.** El defecto ya estaba en siete
  sitios; repartir la responsabilidad lo multiplica. Descartada.
- **Recortar por caracteres en vez de por bytes.** Cambiaría el contrato de
  `chunk_size`, que es un límite de bytes, y rompería el ensamblado byte a
  byte de los trozos. Descartada.

## Consecuencias

- **El proceso no aborta.** Un desplazamiento o un tamaño que caen dentro de
  un carácter son un error con su motivo, y el servidor sigue atendiendo.
- **El ensamblado no pierde contenido.** Los trozos siguen reensamblando el
  fichero byte a byte cuando el tamaño cae dentro de un carácter: hay un test
  que lo demuestra.
- **Un `chunk_size` más pequeño que el primer carácter es ahora un error.**
  Antes devolvía medio carácter. El consumidor que dependía de eso estaba
  recibiendo un fragmento inválido.
- **El contrato de `chunk_size` no cambia:** sigue siendo un límite de bytes,
  y el trozo devuelto nunca mide más de lo pedido.
- **No cambian los contratos publicados** de `cognicode`, `cognicode-mcp` ni
  `cogh`: el servicio acepta las mismas peticiones y publica los mismos
  resultados.
- **La escritura sigue sin ser transaccional.** El temporal se abre sin
  creación exclusiva, si la escritura falla el temporal se queda en disco, y
  no hay comprobación de versión que impida pisar un cambio ajeno. Eso es
  otra unidad (`C41`), medida y en la cola, y no se mezcla aquí.

## Referencias

- Recibo de la unidad `C40` en `docs/roadmap/JOURNAL.md`.
- `ADR-058` — Lo que se rechaza no deja rastro, y las ediciones se aplican al
  original: el mismo linaje. Una entrada inválida se rechaza con su motivo; no
  aborta el proceso que la atiende.
- La auditoría externa de 2026-10-25, apartado 2, P1.
