# ADR-058 — Lo que se rechaza no deja rastro, y las ediciones se aplican al original

**Estado**: ACEPTADO (decidido el 2026-10-24; pendiente de ejecución)
**Fecha**: 2026-10-24
**Decisores**: Maintainer
**Contexto**: H28 · las operaciones sobre ficheros (`crates/cognicode-core/src/application/services/file_operations.rs`), candidato `C38` de la cola de la misión

---

## Contexto

### Lo que dice la reauditoría

La reauditoría del 2026-10-24, en su apartado 3, levanta dos defectos residuales sobre
las operaciones de fichero, y en su apartado 5 los pone primeros entre las acciones
recomendadas:

- **P1 — «Una escritura rechazada todavía puede dejar directorios».** `write_file`
  realiza `create_dir_all` antes de comprobar si el contenido supera 10 MB. «Si la ruta
  es válida, pero el contenido supera el límite, la operación devuelve un error después
  de haber creado los directorios». Corrección propuesta: «comprobar tamaño, parámetros y
  permisos antes de cualquier efecto sobre el filesystem».
- **P2 — «Las ediciones múltiples pueden interferirse».** «`edit_file` comprueba las
  coincidencias sobre el texto original, pero después aplica cada sustitución sobre el
  resultado de la anterior. Por ejemplo, sobre `A B`, las operaciones `A → B` y `B → C`
  terminan produciendo `C C`, no `B C`». Corrección propuesta: «establecer semántica de
  edición simultánea, calcular posiciones sobre el contenido original, rechazar
  solapamientos y aplicar los cambios desde el final».

El ciclo H25 (PR #459) ya había cerrado el defecto principal de esta área: la ruta se
juzga antes de que se cree nada, y una ruta relativa se resuelve contra el workspace que
el contexto declara. Lo que queda son los dos huecos de arriba.

### Lo que mide el árbol

**MEDIDO 2026-10-24**, sobre `c70438b2` (el `main` de esta unidad).

**El hueco del tamaño.** El orden real de `write_file`
(`crates/cognicode-core/src/application/services/file_operations.rs:1123`) es:

1. `resolve_for_creation` (`:1128`) — la política de rutas juzga, sin tocar el disco.
2. `create_dir_all` (`:1133-1140`) — **el efecto**.
3. `validate_resolved_path` (`:1142`) — la política se vuelve a juzgar y el camino se
   canonicaliza.
4. El límite de 10 MB (`:1144-1151`) — **el control que llega tarde**.

El efecto es real y el control es real: una escritura de más de 10 MB con
`create_dirs` deja el árbol de directorios en el disco y después se rechaza.

Lo que **no** se puede subir es el paso 3: `validate_resolved_path` canonicaliza el
camino, y canonicalizar un padre que todavía no existe falla. El padre tiene que
existir. Y la política de rutas no hace falta repetirla: `resolve_for_creation` y
`validate_path` aplican «las mismas reglas léxicas, en el mismo orden»
(`crates/cognicode-core/src/interface/mcp/security.rs:457`). El único control que
depende sólo de la petición y no del disco es el del tamaño, y es exactamente el que
llega tarde.

**El hueco de las ediciones.** `edit_file` (`:1185`) hace dos cosas distintas con dos
contenidos distintos:

- La **comprobación** (`:1193-1194`) busca cada `old_string` en el contenido
  **original**: cero coincidencias es `no_match`, más de una es `multiple_matches`.
- La **aplicación** (`:1230-1237`) es secuencial: `new_content.replace(...)` sobre el
  resultado de la edición anterior.

Medido, sobre el árbol de esta unidad:

| Contenido original | Ediciones | Resultado actual | Lo que dice el contrato |
|---|---|---|---|
| `A B` | `A→B`, `B→C` | `C C` | `B C` |
| `abc` | `ab→X`, `bc→Y` | `Xc` — la segunda edición desaparece en silencio | las dos, o ninguna |

El segundo caso es el peor: la segunda edición ya no encuentra su coincidencia porque la primera
se la llevó, `replace` no cambia nada, y la respuesta dice `applied: true` sin motivo.
Una petición se declara aplicada cuando la mitad de lo que pedía no se hizo.

Y hay un tercer detalle: `replace` sustituye **todas** las ocurrencias del contenido
acumulado, así que la garantía que el servicio publica —«Each edit must match exactly
one occurrence» (`:1183-1184`)— no se cumple: la unicidad se comprueba sobre el
original y la sustitución se hace sobre otro texto.

### Lo que la reauditoría propone y esta unidad no hace

La reauditoría lista además un P2 —«`bytes_changed` no mide los bytes realmente
cambiados»— y propone «cambiar el nombre a `size_delta_bytes` si esa es la métrica
deseada, o calcular realmente los bytes modificados».

**MEDIDO 2026-10-24: ese cambio no es un arreglo de bug, es un cambio de contrato.**
El campo está publicado en el esquema MCP con su significado escrito —
`/// Number of bytes changed (absolute difference)`
(`crates/cognicode-core/src/interface/mcp/schemas.rs:1544`)— y además alimenta la
métrica `record_bytes_written`
(`crates/cognicode-core/src/interface/mcp/file_ops_handlers.rs:193`). Renombrarlo rompe
a los consumidores que leen el campo; redefinirlo contradice el texto que el esquema
publica. Y no está entre las acciones recomendadas. Queda medido y fuera de esta
unidad; se cierra con una entrada propia cuando se decida qué métrica se quiere.

## Decisión

1. **El límite de tamaño se comprueba antes de cualquier efecto en el sistema de
   ficheros.** El orden queda: la política de rutas juzga primero —su error es el que
   informa de un intento de salir del workspace—, el tamaño va justo después, y
   `create_dir_all` ocurre cuando las dos peticiones han dicho que sí. Una escritura
   rechazada por su tamaño no deja directorios.

2. **Las ediciones se aplican a la vez sobre el contenido original.** Las posiciones se
   calculan sobre el original, los cambios se aplican desde el final —para que las
   posiciones anteriores sigan valiendo— y dos ediciones que se solapan se rechazan
   porque la petición es ambigua. Dos ediciones idénticas son la misma edición, y
   cuentan una. La regla que el servicio ya publica —cada edición encuentra
   exactamente una ocurrencia— pasa a ser lo que el código hace.

3. **`bytes_changed` no cambia.** El esquema publicado documenta su significado; esta
   unidad no redefine un contrato para tapar un nombre flojo.

## Alternativas consideradas

**Subir el límite de tamaño después de `validate_resolved_path` y antes de escribir.**
No arregla nada: el efecto —el directorio— ya está en el disco cuando el control
llega.

**Rechazar toda petición con más de una edición.** Evita el problema, pero rompe el uso
real de la herramienta (un consumidor que manda tres cambios en una llamada deja de
poder hacerlo) y la reauditoría no lo pide: pide semántica simultánea.

**Aplicar las ediciones en el orden pedido y documentarlo.** Sería un cambio de
contrato público —el esquema MCP y el `reason` passerían a describir una semántica
secuencial— y el comportamiento actual no cumple ni siquiera su propio contrato. La
corrección más pequeña que hace truth el contrato es la simultánea.

**Renombrar `bytes_changed` a `size_delta_bytes`.** Ver arriba: es un cambio de
contrato, no de bug, y la métrica que lo consume cambiaría de nombre también.

## Consecuencias

**Positivas.**

- Una escritura rechazada por su tamaño no deja el árbol de directorios que pidió.
- Dos ediciones que se pisan ya no se pisan: se aplican sobre el original.
- Un solapamiento se dice en la respuesta en vez de perder media edición en silencio.
- La garantía «cada edición encuentra exactamente una ocurrencia» deja de ser una
  frase del doc y pasa a ser lo que el código cumple.

**Negativas.**

- El conjunto documentado de motivos de no aplicación suma uno: `overlapping_edits`.
  Es una extensión compatible —el campo `reason` es una cadena y el valor por defecto
  de un consumidor para un motivo que no conoce es «no aplicado», que es el lado
  seguro— y el esquema se documenta con el valor nuevo.
- El cambio de semántica de las ediciones múltiples es observable para un consumidor
  que dependía del comportamiento secuencial. El comportamiento anterior no cumplía el
  contrato publicado, así que lo que se rompe es una dependencia de un defecto.

**Lo que esta unidad NO hace.**

- No toca `bytes_changed`.
- No cambia el comportamiento de una sola edición.
- No reescribe el esquema MCP más allá del motivo nuevo y del texto que ya dice
  «absolute difference».
- No toca la semántica de `write_file` más allá del orden del control de tamaño.

## Referencias

- `crates/cognicode-core/src/application/services/file_operations.rs` — `write_file`
  (`:1123`) y `edit_file` (`:1185`).
- `crates/cognicode-core/src/interface/mcp/security.rs:454` — `resolve_for_creation`, la
  política de rutas juzgada antes de cualquier efecto.
- `crates/cognicode-core/src/interface/mcp/schemas.rs:1544` — el significado publicado
  de `bytes_changed`.
- `crates/cognicode-core/tests/h25_file_operations_effect_order.rs` — el precedente
  inmediato: el ciclo H25 y sus cinco checks de caracterización.
- ADR-055 — la misma regla en otro sitio: el efecto viene después de la promesa.
