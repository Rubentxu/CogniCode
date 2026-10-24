# ADR-054 — El efecto viene después de la promesa, y una validación que no ocurrió lo dice

**Estado**: ACEPTADO (decidido el 2026-10-24 al elegir la unidad H1 · Archivos seguros; pendiente de ejecución)
**Fecha**: 2026-10-24
**Decisores**: Maintainer
**Contexto**: H25 · operaciones sobre archivos (`crates/cognicode-core/src/application/services/file_operations.rs`), unidad `C33` de la cola de la misión

---

## Contexto

Dos operaciones del producto afirma una cosa y hacen otra. Ninguna de las dos
mentía por descuido: las dos nacieron de una restricción real que el código
resolvió por el camino corto, y el camino corto se quedó.

### 1. `write_file` produce el efecto antes de pedir la promesa

`file_operations.rs:1095-1146` crea los directorios padre **antes** de validar
la ruta, y lo hace sobre una ruta resuelta de una manera distinta de la que
valida:

```rust
// For new files with create_dirs=true, create parent directories BEFORE validation
// This is necessary because validate_path checks parent exists for new files
let absolute_path = if requested.is_absolute() {
    requested.to_path_buf()
} else {
    std::env::current_dir()?.join(requested)   // el CWD del proceso
};
if let Some(parent) = absolute_path.parent() && !parent.exists() {
    fs::create_dir_all(parent)?;               // el efecto, antes de la promesa
}
let validated_path = self.validate_path(&path_for_validation)?;  // el workspace
```

Son dos sitios distintos para la misma ruta. `validate_path`
(`file_operations.rs:268-320`) resuelve las rutas relativas contra
`workspace_root`; el `mkdir` las resuelve contra `std::env::current_dir()`.
La política de rutas (`interface/mcp/security.rs:326-443`) canonicaliza y
comprueba el límite del workspace — pero se le pregunta **después** de que el
directorio ya existe.

Las consecuencias, medidas sobre el código:

- **Una petición rechazada deja directorios fuera del workspace.** Con
  `create_dirs = true` y una ruta absoluta fuera del workspace, el árbol de
  directorios se crea y la petición se rechaza después, con
  `PathOutsideWorkspace`.
- **Una ruta relativa crea el árbol donde el proceso está, no donde el
  workspace está.** El servidor MCP se arranca con `--cwd`; el `mkdir` no lo
  mira. Con el CWD del proceso distinto del workspace, la escritura relativa
  crea directorios en un sitio y valida en otro, y acaba rechazada con
  `PathNotAccessible` — dejando el árbol de directorios en el CWD.

La restricción que empujó a este código es real: `validate_path` canonicaliza
el padre, y un padre que no existe no se puede canonicalizar
(`security.rs:398-412`). La respuesta correcta no es mkdir primero, es
**preguntar en dos fases**: la política decide sin el sistema de ficheros, y
la canonicalización ocurre cuando los padres ya existen.

### 2. `edit_file` declara una validación que no ocurrió

`file_operations.rs:1235-1248` tiene dos salidas que devuelven
`passed: true` sin que haya habido ninguna verificación:

```rust
Err(_) => EditValidation { passed: true, // Can't create parser, skip validation
Un lenguaje desconocido => EditValidation { passed: true, // Unknown language, skip validation
```

El consumidor lee `validation.passed = true` y entiende que el fichero se
validó. No se validó: no había parser, o no se conoce el lenguaje. Un
cliente que aprende a leer `passed` como «el producto comprobó tu fichero»
aprende a leer una comprobación que no ocurrió como una que sí.

El daño no es teórico: `passed` es lo que el sandbox lee para clasificar el
desenlace de un escenario (`crates/cognicode-sandbox/src/main.rs:1912` y
`:2065`), y lo que la métrica de `edit_file` usa para contar un rechazo
(`interface/mcp/file_ops_handlers.rs:188`). Un campo que significa dos cosas
no puede significar las dos bien.

La auditoría externa del 2026-10-24 marca estos dos hallazgos como P0 y P1.
Los dos están medidos sobre el árbol.

---

## Decisión

### 1. Una resolución, una autoridad

La ruta se resuelve **una sola vez**, contra `workspace_root`, y esa ruta
resuelta es la que se valida, la que se crea y la que se escribe. Ninguna
operación vuelve a mirar `std::env::current_dir()`.

### 2. El efecto, después de la promesa

`validate_path` se parte en dos fases, y el orden es el que la seguridad
exige:

1. **Fase léxica**: se resuelve la ruta contra el workspace y se la pasa a la
   política de rutas. No toca el sistema de ficheros.
2. **Efecto**: sólo si la política ha aceptado, se crean los directorios
   padre — **del padre de la ruta ya resuelta y aceptada**.
3. **Fase canónica**: se canonicaliza y se comprueba el límite del workspace
   una vez que los padres existen.

Una petición rechazada no deja directorios, ni dentro ni fuera del workspace.

### 3. Una validación que no ocurrió, lo dice

`EditValidation` gana un campo `verification` con tres estados, y `passed`
deja de ser ambiguo:

| `verification` | Qué pasó | `passed` | Se escribe |
|---|---|---|---|
| `verified` | un parser corrió y el contenido es válido | `true` | sí |
| `unverified` | no hubo parser, no se conoce el lenguaje, o no había contenido nuevo que comprobar | `false` | sí |
| `rejected` | un parser corrió y el contenido está roto | `false` | **no** |

La invariante es una: **`passed` es cierto si y sólo si `verification` es
`verified`**. Un análisis omitido nunca se presenta como una validación
satisfactoria.

Lo que **no** cambia: el fichero que se escribe. Hoy una edición en un
lenguaje sin parser se escribe (`passed` era `true` y `passed` gobernaba la
escritura en `file_operations.rs:1251`); mañana se sigue escribiendo. Lo que
cambia es lo que la respuesta **afirma**: antes afirmaba una verificación que
no había ocurrido.

### 4. El temporal es de una escritura, no de un proceso

El fichero temporal se nombra `{ruta}.tmp.{pid}` (`file_operations.rs:1150` y
`:1252`). Dos escrituras concurrentes al mismo camino dentro del mismo
proceso comparten el temporal y se pisan: el resultado puede ser un fichero
con el contenido de las dos escrituras entrelazado. El temporal pasa a
llevar un discriminante único por escritura.

### 5. Compatibilidad

- La forma JSON **conserva** `validation.passed` y `validation.syntaxIssues` y
  **gana** `validation.verification`. Es aditivo: un consumidor que ignora
  campos desconocidos sigue leyendo lo que leía.
- El **significado** de `passed` se estrecha: pasa de «la edición no fue
  rechazada» a «el contenido fue verificado». La otra pregunta —«¿se
  rechazó?»— ya tiene su propio sitio en la salida: `applied` y `reason`.
  Esto es un cambio de semántica sobre un campo publicado, y por eso lleva
  pruebas de consumidor (abajo) y no una deprecación silenciosa.
- `reason` gana un valor nuevo, `no_change`, para el caso en que la edición
  no cambia nada: hoy ese caso devuelve `applied: false` con `reason: None`,
  que contradice su propia documentación («Reason for rejection or
  non-application (if not applied)»).

---

## Alternativas consideradas

1. **Validar la ruta absoluta sin resolverla antes del `mkdir`.** No cambia
   nada: el efecto sigue siendo anterior a la promesa, y el `mkdir` seguiría
   resolviendo contra el CWD del proceso. Descartada.

2. **Cerrar en falso cuando no hay parser** (no escribir si no se puede
   verificar). Rompe editar ficheros de texto, Markdown o configuración, que
   es un uso legítimo y frecuente de `edit_file`. Descartada: el defecto no
   es que se escriba, es que se diga que se comprobó.

3. **Quitar `passed` y dejar sólo el enum.** Es la forma más limpia, pero
   borra un campo del contrato publicado de `cognicode-mcp` sin deprecación.
   Descartada: el campo se queda, y lo que cambia es su significado, que es
   un cambio que sí se puede medir y revertir.

4. **Dejar `passed` como estaba y añadir `verification` al lado.** Habría
   roto la invariante: `passed: true` con `verification: unverified` es
   exactamente la respuesta que se quiere eliminar. Descartada.

5. **Mover la política de rutas al puerto de aplicación.** Es el arreglo de
   fondo —que la política siga siendo la autoridad y no un efecto colateral
   del servicio—, pero es un refactor de una pieza que hoy cumple su papel.
   La decisión de hoy es que el orden sea correcto; dónde vive la política se
   decide aparte.

---

## Consecuencias

**Positivas**:

- Una petición rechazada no deja rastro fuera del workspace.
- La ruta se lee en un solo sitio: el mismo camino se valida, se crea y se
  escribe.
- La respuesta de `edit_file` dice qué pasó, en los tres casos, y no dice
  «validado» cuando no hubo validación.
- Dos escrituras concurrentes al mismo camino no se pisan.

**Negativas**:

- `passed` cambia de significado en tres casos (lenguaje desconocido, parser
  no disponible, la edición no cambia nada). Un consumidor que lo leía como
  «la edición no fue rechazada» recibe `false` donde antes recibía `true`.
  La mitigación es que la otra pregunta tiene su campo, y que el cambio se
  mide con las pruebas de consumidor de abajo.
- El sandbox clasifica el desenlace de un escenario con `!validation.passed`
  (`cognicode-sandbox/src/main.rs:1912` y `:2065`). Con el significado
  nuevo, un escenario en un lenguaje sin parser se clasificaría como fallo.
  El sandbox pasa a leer `verification`, y sus escenarios de `edit_file` se
  vuelven a verificar en la misma unidad.

**Mitigaciones**:

- Pruebas de consumidor anteriores, exigidas por el contrato: las de
  `crates/cognicode/tests/file_ops_integration_tests.rs` (cinco aserciones
  sobre `validation.passed`), las de
  `crates/cognicode-core/src/interface/mcp/file_ops_handlers.rs` y la
  clasificación del sandbox.
- El discriminante se ve al revés: revirtiendo sólo la Fase 2 (el `mkdir`
  antes de la promesa) o sólo la Decisión 3 (el `passed` sin verificación),
  los contratos correspondientes se ponen rojos.

---

## Referencias

- [ADR-029 — seam de proyección del grafo](./ADR-029-callgraph-projection-port-seam.md) — precedente de cómo se decide un cambio que toca un puerto
- [ADR-D1-006 — retirada de la clase UAT](./ADR-D1-006-uat-retirement.md) — precedente de cómo se declara lo que una decisión deja de comprobar
- `crates/cognicode-core/src/application/services/file_operations.rs` — `write_file` (`:1095`), `validate_path` (`:268`), `edit_file` (`:1150`)
- `crates/cognicode-core/src/interface/mcp/security.rs:326-443` — la política de rutas
- `docs/roadmap/JOURNAL.md` — el recibo de la unidad `C33`
