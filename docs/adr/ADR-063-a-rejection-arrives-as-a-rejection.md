# ADR-063 — Un rechazo llega al cliente como un rechazo

**Estado**: EJECUTADO (decidido el 2026-10-25; ejecutado el 2026-10-25)
**Fecha**: 2026-10-25
**Decisores**: Maintainer
**Contexto**: H29 · el borde del protocolo (`crates/cognicode-core/src/interface/mcp/`), la telemetría (`crates/cognicode-core/src/infrastructure/telemetry/`) y el error de la aplicación (`crates/cognicode-core/src/application/error.rs`)

---

## Contexto

### Lo que faltaba

El servicio de ficheros rechaza peticiones: un camino fuera del workspace, un
patrón que no es una expresión regular, un fichero que no existe. Esos
rechazos son **decisiones del producto**, no caídas, y un cliente tiene que
poder distinguirlos.

Medido, en el árbol de `d0607abb`:

- `crates/cognicode-core/src/interface/mcp/file_ops_handlers.rs:88`, `:139`,
  `:197`, `:253` y `:306` — los cinco handlers que pasan por el contador
  reconstroían el error como `AppError::InternalError(e.message)`.
- El camino **sin** colector de métricas (`:64`, `:119`, `:172`, `:232`,
  `:286`) conservaba la categoría: `map_err(HandlerError::App)`.

La consecuencia, medida con el binario real: un rechazo deliberado llegaba al
cliente como

```
application: Internal error: Invalid parameter: Path traversal attempt detected: ../fuera
```

en vez de

```
application: Invalid parameter: Path traversal attempt detected: ../fuera
```

El cliente recibía "el producto se rompió" donde en realidad el producto
había dicho "no". Y no podía distinguir los dos casos, que son de naturalezas
opuestas: uno se puede reintentar cambiando la petición, el otro no.

### Por qué era invisible a la suite entera

`init_global_metrics()` lo llaman el servidor MCP
(`crates/cognicode-mcp/src/server.rs:266` y `main.rs:70`), **nunca los
tests**. `TOOL_METRICS` es un `OnceLock`: en un binario de test no está
inicializado, `get_global_metrics()` devuelve `None`, y los tests toman siempre
el camino que ya funcionaba.

Es decir: la suite entera pasaba por el camino bueno, y el camino malo —el
que usa el producto de verdad— no lo probaba nadie. No era un defecto
escondido por descuido; era un defecto **fuera del alcance de lo que se
probaba**.

### La causa no era el aplanado

El aplanado era el síntoma. La causa estaba un nivel más abajo:

```rust
pub async fn instrument_tool<F, T>(
    metrics: &ToolMetrics,
    tool_name: &str,
    f: F,
) -> Result<T, ToolError>
where
    F: Future<Output = Result<T, ToolError>>,
```

El contador de métricas —que vive en `infrastructure`— exigía su **propio
tipo de error**. El llamante no podía devolver su error; tenía que
convertirlo a `ToolError` para que pasara por el contador, y después
reconstruirlo. Y esa reconstrucción era la que perdía la categoría: el
`ToolError` solo llevaba un `kind` constante (`"AppError"`) y un mensaje, así
que lo único que se podía reconstruir era `InternalError`.

El contador de métricas necesita **un nombre** para el error, no el error. La
firma estaba pidiendo más de lo que la telemetría sabe.

## Decisión

**El contador devuelve el error de la operación, intacto, y pide una
etiqueta para la métrica.**

1. **`instrument_tool` es genérico sobre el error.** Pasa a ser
   `instrument_tool<F, T, E>(...) -> Result<T, E>`, y el error de la
   operación vuelve tal cual. La firma ya no menciona `ToolError`: el error
   de la aplicación viaja por la telemetría sin ser de la telemetría.
2. **El llamante lender una etiqueta, no el error.** La firma pide
   `label: impl Fn(&E) -> ToolError`, y la métrica registra `label(&e).kind`.
   Es lo único que la telemetría necesita saber del error: cómo se llama.
3. **`AppError::category()` nombra la variante**, con un `match` exhaustivo.
   Exhaustive a propósito: una variante nueva que no se nombre ahí no
   compila, de modo que la lista no puede quedarse vieja en silencio. No es
   una segunda fuente de verdad de la taxonomía —es una derivación, como lo
   es ya el `Display` que genera `thiserror`— sino el punto donde el
   compilador obliga a mantenerla al día.
4. **La etiqueta de la métrica pasa a ser la categoría.** Antes era
   `"AppError"` en los cinco handlers, el único sitio del repositorio que
   nombraba el *tipo* del error en vez de su *categoría*: los propios tests de
   la telemetría usan `"PathTraversal"` y `"Validation"`. Medido: el literal
   `"AppError"` no aparece en ningún otro sitio del repositorio, así que
   ningún panel, contrato ni golden dependía de él.
5. **El prefijo duplicado del error de expresión regular desaparece.**
   `validate_regex_pattern` devolvía `"Invalid regex pattern: {motivo}"` y su
   llamante volvía a añadir `"Invalid regex pattern: "`, de modo que el
   cliente leía la misma frase dos veces. Ahora el validador devuelve el
   motivo y el llamante dice qué parámetro falló. Es la misma cadena que
   esta unidad entrega al cliente, y por eso va aquí y no en otra unidad.

## Alternativas descartadas

### Reconstruir el error a partir de su categoría y su mensaje

Guardar la categoría en el `ToolError` y reconstruir el `AppError` con un
`match` inverso.

**Descartada.** `AppError` tiene diecisiete variantes. La reconstrucción
sería una segunda lista de la taxonomía, y una variante nueva que no
apareciera en ella caería en `InternalError` —el defecto que se está
corrigiendo, reintroducido un nivel más abajo. El compilador no puede
vigilar dos listas; solo una.

### Atrapar el error en una variable local mientras el contador corre

Capturar el `AppError` en un `Option` que el `async` del contador rellena, y
devolverlo en vez del `ToolError` reconstruido.

**Descartada.** Funciona, y es el arreglo más pequeño en líneas. Pero
introduce un canal lateral: el mismo error vive en dos sitios a la vez —el
`ToolError` que cuenta la métrica y el `Option` que se devuelve— y el
compilador no dice nada si se desincronizan. Es la forma de tener dos
fuentes de verdad para el mismo error.

### Dejar la etiqueta de la métrica como estaba

Arreglar solo el camino del cliente y dejar `error_type = "AppError"`.

**Descartada.** Sería la mitad del arreglo de la misma confusión: el cliente
deja de ver "fallo interno", pero la métrica sigue sin poder distinguir un
rechazo de una caída. Además, `"AppError"` era el único sitio del repositorio
que nombraba el tipo del error donde la convención —la de los propios tests de
la telemetría— nombra la categoría. El cambio es observable en la
telemetría, y por eso queda escrito aquí y no en silencio.

## Consecuencias

- El cliente recibe `"application: Invalid parameter: …"`. La categoría y el
  detalle viajan en el mismo sitio, y el detalle ya no repite la frase.
- La métrica `error_type` de las herramientas de ficheros pasa de un único
  valor constante a la categoría del error. Es un cambio observable en la
  superficie de telemetría, y es lo que hace la métrica capaz de distinguir
  un rechazo de una caída.
- `instrument_tool` deja de ser un tipo de la telemetría en el camino del
  error: el error de la aplicación ya no se convierte en un error de la
  telemetría para poder pasar por el contador.
- Los handlers quedan **más cortos**: el `match` que convertía el error
  dentro del `async` desaparece, porque ya no hay nada que convertir.
- `handle_retrieve_and_verify` no pasa por el contador y ya conservaba la
  categoría (`map_err(HandlerError::App)`); medido, no le afectaba el
  defecto y no se ha tocado.

## Lo que queda abierto

- La ruta **sin** colector de métricas y la ruta **con** colector siguen
  siendo dos ramas del handler. Hoy coinciden —las dos conservan la
  categoría— y el contrato de CI vigila que ninguna reconstructions el error
  como fallo interno, pero siguen siendo dos caminos. Unificarlos es trabajo
  de otra unidad, y no es lo que esta unidad cierra.
- La categoría del error en la métrica es un nombre de variante. Si el
  producto necesita una taxonomía de rechazos más fina —"rechazo del
  cliente" frente a "rechazo del servidor"— eso es un modelo de dominio, no
  una etiqueta.

## Evidencia

- `crates/cognicode-core/tests/a_rejection_arrives_as_a_rejection.rs` (nuevo):
  dos tests de frontera, escritos sobre la cadena que el protocolo publica.
  **RED medido: 2 de 2 fallaban**, y el fallo nombraba los cinco handlers.
  El test inicializa el colector de métricas —sin él, probaría el camino que
  ya funcionaba— y lo comprueba, de modo que un colector ausente no puede
  dar un verde vacío.
- `scripts/ci/test_a_rejection_arrives_as_a_rejection.py` (nuevo): tres
  detectores y su control anti-vacuidad. **RED medido: 2 de 4 fallan**
  contra el código previo, y el detector nombra las cinco posiciones donde
  se aplanaba el error.
