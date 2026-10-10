# T1-5.4 — Decisión: "Warning: graph is PARTIAL" inline (no centralizar)

> **Estado**: Decisión tomada. NO se centraliza. El eprintln!
> repetido en 6 sub-commands (HotPaths, EntryPoints,
> LeafFunctions, TracePath, Mermaid, Complexity, Impact)
> sigue inline.

## 0. Contexto

Durante T1-5.4b (migración de `execute_graph` a
`Result<GraphResult, _>`) se planteó si centralizar la línea:

```rust
eprintln!("Warning: graph is PARTIAL: {} file(s) skipped ({})",
          skipped.len(),
          skipped.iter().map(|f| f.path.clone())
              .collect::<Vec<_>>().join(", "));
```

repetida en 6 de los 11 arms.

## 1. Alternativas consideradas

### A) Centralizar como side channel

- Añadir `GraphResult::PartialWarning { count, files }` variant.
- Cambiar la signature a `Result<(GraphResult, Vec<PartialWarning>), _>`.
- El caller en `execute()` emite cada warning a stderr antes de presentar el resultado principal.
- Snapshot tests capturarían el output completo (incluyendo los warnings emitidos por el caller).

**Pros**: dedupe real; data side lleva el status parcial (útil para JSON consumers); alineado con T1-5.5 JSON dispatch.

**Contras**:
- Re-cambio de signature (T1-5.4b acaba de estabilizarla).
- Wrapper type / tuple / Vec<GraphEvent> — varias formas, todas añaden complejidad.
- Los snapshot tests pasan de "function emite text" a "caller+function emiten text en orden" — pierde locality.
- El text output del bin tiene que interleavar warnings (caller) con result (T1-5.5 presenter) en el orden correcto.

### B) Inline (esta decisión)

- El eprintln! se queda en cada uno de los 6 arms.
- Snapshot tests por sub-command pinea el texto completo, warnings incluidos.
- `GraphResult::Full` ya tiene `status: "complete"|"partial"`; los otros 5 sub-commands no lo llevan, pero pueden añadirlo en T1-5.5 si la JSON dispatch lo necesita.

**Pros**:
- Cero cambios de signature adicionales.
- Snapshot tests directos: el eprintln! emite el texto, el snapshot lo pinea, sin indirección.
- El text output es self-contained dentro de cada sub-command.

**Contras**:
- 6 sites × ~8 líneas de texto repetido (≈50 líneas de duplicación).
- El "partial" status solo está en el data side de `Full`; los otros 5 sub-commands lo pierden en JSON.

## 2. Decisión: B

El costo de A (re-cambio de signature, wrapper, snapshot tests con
locality perdida) excede el beneficio de A (dedupe de 50 líneas,
status parcial en 5 sub-commands). T1-5.5 puede añadir un campo
`status: "complete"|"partial"` a los 5 variants restantes si la
JSON dispatch lo requiere — sin tocar la signature.

## 3. Lo que SÍ hacemos en T1-5.4c..m

Snapshot tests por sub-command. Cada test assserta el texto
completo del sub-command INCLUYENDO el "Warning: graph is
PARTIAL" si lo emite. Eso pinea:
  - La lista de files skipped (texto exacto, en orden).
  - El número `N` en "{} file(s) skipped".
  - El prefijo "Warning: graph is PARTIAL:" (matches consumer greps).

Si en el futuro alguien decide centralizar, los snapshot tests
fallarán con un diff claro (la línea "Warning: graph is PARTIAL"
se mueve de la position N al final del bloque) y la decisión
queda explícita en el diff.

## 4. Follow-up opcional (T1-5.5+)

Si la JSON dispatch (T1-5.5) necesita que los 5 sub-commands
parciales lleven el status en su data side, el cambio es:
  1. Añadir `partial_files: Vec<String>` (o `status: String`) a
     los 5 variants.
  2. Cada sub-command rellena el campo cuando
     `build_full_graph_report` devuelve `Partial { skipped }`.
  3. NO se cambia la signature; NO se mueve el eprintln!.

Esto se decide en T1-5.5 con criterio similar.
