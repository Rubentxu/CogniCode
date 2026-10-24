# ADR-055 — Un lote se escribe entero o no se escribe, y una capacidad que no está conectada no se ofrece

**Estado**: ACEPTADO (decidido el 2026-10-24; pendiente de ejecución)
**Fecha**: 2026-10-24
**Decisores**: Maintainer
**Contexto**: H26 · persistencia de rutas (`crates/cognicode-explorer/src/ports/route_store.rs`), unidad `C36` de la cola de la misión

---

## Contexto

La auditoría externa del 2026-10-24 señaló dos hallazgos sobre `RouteStore`, el puerto
que persiste las rutas y aristas que se ingieren desde una especificación OpenAPI.
La lectura del informe es correcta en los hechos y equivocada en el alcance, y esa
diferencia es la que decide la forma de esta unidad.

### Lo que el informe dice, y lo que el árbol muestra

**MEDIDO 2026-10-24.** El informe dice que «la ingestión de rutas tiene un puerto sin
adaptador en el arranque normal» y que «el runtime construye Explorer con
`route_store: None`». Los dos hechos son ciertos, y hay uno más detrás:

- **No existe ninguna implementación de `RouteStore` en el árbol.** `grep -rn 'impl
  RouteStore for' crates/` no devuelve nada: ni un adaptador de producción, ni un doble
  de prueba. El puerto es una costura sin nada detrás.
- **Ninguna puerta ni binario publicado compila el código que la usa.** Las dos
  herramientas (`ingest_openapi`, `trace_route`) se registran tras
  `#[cfg(feature = "multimodal")]` de `cognicode-explorer`, y esa feature sólo la
  activa `cognicode-runtime/multimodal`, que **nadie** en el workspace habilita.
  `cognicode-mcp/multimodal` habilita `cognicode-core/multimodal`, que no es lo mismo:
  el binario MCP con `--features multimodal` no compila ni una sola herramienta
  multimodal del explorer.
- **El catálogo publicado y el runtime ya coinciden.** `product/tools.json` declara 78
  herramientas y ninguna es `ingest_openapi` ni `trace_route`. En el build publicado el
  servidor MCP tampoco las ofrece. No hay drift entre el catálogo y la superficie real.
- **El handler ya dice la verdad en caliente.** Con el puerto sin cablear,
  `ingest_openapi` y `trace_route` responden `feature_disabled` y
  «route_store port not wired». La capacidad **no está conectada y no está oculta**.

Consecuencia directa sobre el criterio de cierre que propone el informe —«fallo
inyectado en mitad de una escritura; ninguna persistencia parcial»—: **no es
medible tal como está escrito**. Sin almacén no hay escrituras que inyectar un fallo
en. Un criterio que no se puede medir no es un criterio.

### El defecto que sí es real

`RouteStore::save_many` promete en su documentación una garantía que su
implementación por defecto no da:

> Batch upsert — convenience over `upsert_route` + `save_edge` in a single
> transaction. All-or-nothing: if any row fails, the whole batch is rolled back.
>
> Default impl loops over individual calls without a transaction (atomicity is the
> caller's responsibility).

El puerto afirma dos cosas incompatibles: que el lote es todo-o-nada, y que la
implementación por defecto no lo es. El comentario del propio código lo confiesa en
la segunda línea. Un adaptador nuevo que no sobrescriba el método hereda un lote que
escribe fila a fila: si la tercera falla, las dos primeras quedan escritas. El
informe lo detecta y lo formula bien: *«el riesgo está en que nuevos adaptadores
hereden un contrato falso»*.

### Y un defecto que sólo se ve con la feature puesta

Con `multimodal` activa, el registro ofrece las dos herramientas **sin condición**:
no consulta si hay un almacén detrás. Un cliente MCP ve `ingest_openapi` en su lista
de herramientas y la llama, y la respuesta es siempre «feature_disabled». La
herramienta que se ofrece no se puede usar, y el catálogo —que sí es la autoridad
—ni la menciona. Es el mismo patrón que la casa ya corrigió en `PathPolicy`
(ADR-054): un puerto o una superficie que ofrece algo que no puede hacer.

## Decisión

### 1. El lote heredado falla cerrado

La implementación por defecto de `save_many` **deja de escribir**. Devuelve un error
que nombra la obligación en vez de dejar a medias un lote:

- Si un adaptador no proporciona un lote transaccional, el lote **no se escribe**.
  El error dice qué falta, para que quien lo lea sepa qué implementar.
- La alternativa —bajar la promesa a la documentación— se descarta: un contrato
  que dice «esto no es atómico» deja al siguiente adaptador con permiso para
  escribir a medias. La casa no deja contratos falsos en el tipo; los quita del
  tipo (ADR-054, `resolve_for_creation`).

El puerto sigue ofreciendo el método con la misma firma, y por tanto el código que
lo usa no cambia: lo que cambia es lo que ocurre cuando nadie lo implementa bien.

### 2. La capacidad que no está conectada no se ofrece

Las dos herramientas OpenAPI se registran **sólo cuando hay un `RouteStore`
conectado**. El registro se construye después del contexto, así que la condición
está disponible en el mismo sitio donde se registra.

- Sin almacén: el servidor no ofrece `ingest_openapi` ni `trace_route`. Un cliente
  MCP no ve una herramienta que sólo puede responder «feature_disabled».
- Con almacén: se ofrecen, y funcionan. La capacidad no se esconde: se ofrece
  cuando se puede.

### 3. El runtime dice lo que es verdad

El cableado `route_store: None` sigue siendo `None` —no hay adaptador que
cablear— pero el comentario deja de describir un adaptador de Postgres retirado
como si quedara un cableado pendiente, y dice lo que hay: que no hay adaptador, y
que por eso la capacidad no se ofrece.

### 4. El build multimodal entra en la puerta

Ninguna puerta construye hoy con `--features multimodal`, así que un arreglo que
sólo se ve en ese build no lo vigilaría nadie. El precedent existe: `merge-gate`
corre `cargo test -p cognicode-core --features evidence-kernel`. El build
multimodal del explorer entra en la misma forma, y —por la regla de que un stage
nuevo de `merge-gate` pertenece también a `full`— también en `test-full.sh`.

## Alternativas consideradas

**Cablear un adaptador transaccional operativo** (la primera opción del informe). Un
`RouteStore` en memoria con `save_many` realmente atómico haría la capacidad
existente de verdad, y el criterio de cierre del informe sería medible. Se descarta
**para esta unidad**, no para siempre: es una decisión de producto —semántica de
persistencia, qué entra en el catálogo, si el binario publicado habilita
`multimodal`—, y la casa no construye adaptadores para casos hipotéticos. Si la
capacidad debe existir, es una unidad con su propio ADR, con su bump de versión y
su gate de candidato. Esta unidad deja el puerto honesto para que ese día llegue
sin sorpresas.

**Borrar el puerto y las dos herramientas.** Es la forma más radical de no prometer
nada, y su limpieza es real. Se descarta porque el puerto es la costura
documentada de la capacidad (ciclo e15.5) y borrarlo es también una decisión de
producto, además de destruir un camino que el producto puede querer usar.

**Bajar la promesa a la documentación** («el lote por defecto no es atómico»). Se
descarta por la razón de arriba: deja el contrato falso en el sitio donde un
adaptador nuevo lo va a leer.

## Consecuencias

**Positivas.**

- Ningún adaptador puede heredar un lote que escribe a medias: o lo implementa
  transaccional, o el lote no se escribe.
- El servidor MCP no anuncia herramientas que no puede usar, y el catálogo —que
  no las declara— vuelve a ser la descripción exacta de la superficie.
- El arreglo queda vigilado: el build multimodal entra en la puerta.

**Negativas.**

- `save_many` deja de funcionar «de serie». Cualquier implementación futura tiene
  que escribir el lote ella. Es el coste de no tener un puerto que miente.
- La puerta tarda más: una compilación con otro conjunto de features.
- La capacidad de ingesta de OpenAPI **sigue sin existir en el producto publicado**.
  Eso no es un defecto de esta unidad: es la verdad, y ahora la dice el código en
  lugar de un comentario.

**Mitigaciones.**

- La puerta del arreglo tiene dos guardas: un almacén que implementa el lote
  transaccional sigue funcionando, y un almacén conectado vuelve a ofrecer las
  herramientas. Ninguna de las dos puede pasar por alto el arreglo.

## Referencias

- `crates/cognicode-explorer/src/ports/route_store.rs` — el puerto y su
  implementación por defecto.
- `crates/cognicode-explorer/src/mcp/handler/ingest_openapi.rs` — las dos
  herramientas y su respuesta `feature_disabled`.
- `crates/cognicode-explorer/src/mcp/handler/ingest.rs` — el registro, gated por
  `multimodal`.
- `crates/cognicode-explorer/src/mcp/explorer.rs` — el contexto se construye antes
  que el registro; por eso la condición del cableado está disponible.
- `crates/cognicode-runtime/src/lib.rs` — el cableado `route_store: None`.
- `crates/cognicode-core/tests/b2_w2_catalog_parity.rs` — la paridad entre el
  catálogo y la superficie del MCP.
- ADR-054 — el mismo criterio aplicado al puerto de rutas de fichero: una promesa
  falsa se quita del tipo, no de la prosa.
