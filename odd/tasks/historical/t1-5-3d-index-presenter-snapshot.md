# T1-5.3d — Snapshot test del presenter `IndexResult` (text format)

> **Estado**: T1-5.3d DONE. SHA: 6365ee0. 6/6 snapshot tests
> pass. El text output del `cogh index` subcommand queda pin
> en `crates/cognicode-core/tests/t1_5_3d_index_presenter_snapshot.rs`.

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `odd/tasks/t1-5-presenter-move.md` | goal doc T1-5 |
| `odd/tasks/t1-5-2-classification.md` | output de T1-5.2: ejecut_index tiene ~12 `println!` y 2 `eprintln!` |
| `odd/tasks/t1-5-3a-index-design.md` | design del data side (IndexResult / IndexError) |
| `odd/tasks/t1-5-3b-index-result-serde.md` | skeleton del data side + 7/7 serde tests verdes |
| T1-5.3c (commit 6bb5cb7) | migración de `execute_index` a `Result<IndexResult, IndexError>` |
| T1-5.3d (este doc, commit 6365ee0) | extract presenter a módulo público + snapshot tests |
| T1-5.5 (siguiente) | move presenter to bin + add JSON dispatch |

## 1. El problema

`present_index_result` quedó privada en `commands.rs` durante
T1-5.3c. La función emite text byte-identical al pre-migration
println!, pero al ser privada **no es alcanzable desde un test
de integración** en `tests/`. Una regresión en el shape del
texto (off-by-one en la indentación, un newline que falta, un
label que cambia) solo se detectaría cuando un humano leyera
la salida de `cogh index` y se quejara.

## 2. La solución

Extraer la función a un módulo público `pub mod presenter` que
acepta `&mut dyn Write` en vez de hardcodear `println!`. Eso
permite:

  - El bin (cognicode-cli) sigue llamándola igual:
    `presenter::present_index_result(&result, &mut std::io::stdout())`.
  - El test la llama con un `Vec<u8>` (que implementa `Write`)
    y compara el byte stream contra un expected literal.

La firma `&mut dyn Write` es la estándar de stdlib para
"formateador puro sobre un Writer" — reconocible al instante,
sin necesidad de `Cursor` ni wrappers.

## 3. Cambios

| Archivo | Cambio |
|---|---|
| `crates/cognicode-core/src/interface/cli/presenter.rs` (NEW) | Aloja `present_index_result` + `write_outline_tree` (recursivo) con `&mut dyn Write`. |
| `crates/cognicode-core/src/interface/cli/mod.rs` | `pub mod presenter;` |
| `crates/cognicode-core/src/interface/cli/commands.rs` | Borra las dos funciones privadas; el call site en `execute()` queda en 3 líneas. Quita el `use OutlineNode` que queda unused. |
| `crates/cognicode-core/tests/t1_5_3d_index_presenter_snapshot.rs` (NEW) | 6 snapshot tests (uno por variant + un sub-caso de `doc=None` para SymbolCode). |

## 4. Lo que el test pinea

Cada variant de `IndexResult` tiene un test que asserta el byte
stream exacto:

| Variant | Test | Expected |
|---|---|---|
| `Build` | `snapshot_build` | `Index built successfully in 1234ms using lightweight strategy\n` |
| `Query` | `snapshot_query_with_locations` | `Found 2 location(s):\n  /tmp/proj/src/main.rs:1:1 (Function)\n  /tmp/proj/src/lib.rs:42:0 (Method)\n` |
| `QueryEmpty` | `snapshot_query_empty_uses_dedicated_message` | `No locations found for symbol 'missing_xyz'\n` (NO `Found 0 location(s):`) |
| `Outline` | `snapshot_outline_two_level_recursion` | `Found 1 top-level symbols:\nouter (Class)\n└── inner (Function)\n` |
| `SymbolCode` (con doc) | `snapshot_symbol_code_with_doc` | `\n/// Docstring:\n{...}\n\n/// Symbol code (lines 10 - 15):\n{...}\n` |
| `SymbolCode` (sin doc) | `snapshot_symbol_code_without_doc_skips_doc_block` | Sin bloque `/// Docstring:`; assertion belt-and-braces `!out.contains("Docstring:")` |

## 5. TDD walkthrough

| Step | Resultado |
|---|---|
| RED inicial | `snapshot_outline_two_level_recursion` falla: el test esperaba `"  └── inner"` (2 espacios) pero el código emite `"└── inner"` (0 espacios a indent=1; la branch character es el separator visual, los "  " se suman a partir de indent=2). |
| Fix | El código estaba bien; el test estaba mal. Expected actualizado al byte stream real. |
| GREEN | 6/6 verde. |
| Verificación cruzada | `cargo test -p cognicode-core --test t1_5_3b_index_result_serde` → 7/7 verde (no se rompió la migración). |
| Build | `cargo build -p cognicode-core --lib` → 0 errors. `cargo build -p cognicode-cli` → 0 errors (link OK). |

## 6. Decisión: por qué NO insta / snapshot crate

`cargo tree` confirma que el proyecto no tiene snapshot deps.
Añadir `insta` (o similar) solo para este test es desproporcionado.
Las strings son cortas, viven inline en el assert, y un reviewer
las verifica leyéndolas directamente. El failure mode (diff de
expected vs actual) es exactamente lo que queremos.

## 7. Gotcha: `#[cfg(test)]` en módulos públicos

`pub mod test_helpers` (que expone `outline_node` para construir
el árbol del snapshot) **NO** está gateado por `#[cfg(test)]`.
Razón: integration tests en `tests/` se compilan como crate
separado, y no ven los items `#[cfg(test)]` de la lib de la que
dependen. El módulo se marca con `#[doc(hidden)]` para mantenerlo
fuera de rustdoc (no es API pública; solo lo usa el test de
snapshot). Coste: ~50 bytes de binary; beneficio: el test de
integration test puede importar `outline_node` sin ceremonias.

## 8. Recibo

| WU | SHA | Estado |
|---|---|---|
| T1-5.3a (design) | 2bef2763 | DONE |
| T1-5.3b (skeleton + serde) | ed116f1e | DONE |
| T1-5.3c (migration) | 6bb5cb72 | DONE |
| T1-5.3d (snapshot test) | 6365ee0 | DONE |

Siguiente: T1-5.4..6 — replica el patrón sobre `execute_graph`
(~50 println!/eprintln!), `execute_navigate` (~25),
`execute_analyze` (~20), y el resto hasta vaciar el contador.
El mismo flujo: skeleton del result enum → migrate → snapshot.
