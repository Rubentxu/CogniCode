# Equivalencia `full` vs `per_file` — Oráculo de caracterización (R4)

## Objetivo

Caracterizar — **sin forzar equivalencia** — las divergencias legítimas
y los bugs reales entre `FullGraphStrategy::build_full_graph` y
`PerFileStrategy::build_full_graph` (o `build_full_graph_report`).

Las dos estrategias tienen propósitos distintos:

- `FullGraphStrategy`: una sola pasada sobre el árbol de directorios.
  Construye un `PetGraphStore` ad-hoc. **Traga todos los errores** de
  walk / read / parse / language detect (`continue` en cada rama).
- `PerFileStrategy`: cache incremental por archivo. Construye un
  `PerFileGraphCache` que preserva los `CallGraph` ya analizados.
  **Antes de F2.W2** también tragaba errores (mismo bug); **después
  de F2.W2**, el método `build_full_graph_report` reporta los
  archivos omitidos en `BuildStatus::Partial`.

## Oráculo por escenario

Este corpus NO pretende que las dos estrategias devuelvan exactamente
el mismo `CallGraph`. Pretende que el oráculo sea **explícito y
reproducible** sobre cada caso.

| Escenario | Archivo(s) | Lo que `full` y `per_file` deberían hacer | Lo que la matriz recoge |
|---|---|---|---|
| Símbolo único simple | `src/lib.rs::hello` | Ambos deben encontrar `hello` (1 símbolo, 0 edges) | Recuento de símbolos, listado de nombres |
| Mismo símbolo en varios archivos | `src/lib.rs::shared`, `src/nested/mod.rs::shared` | Ambos deben encontrar 2 instancias con `qualified_name` distintos | Recuento de símbolos por qualified name |
| Función duplicada en el mismo archivo | `src/dup.rs` con dos `pub fn same_name` | Ambos deben encontrar 2 instancias (mismo nombre, distinta Location) | Recuento de símbolos por name + check de Location única |
| Archivo vacío | `src/empty.rs` | Ambos deben encontrar 0 símbolos para ese archivo | Recuento agregado |
| Archivo solo con comentarios | `src/comments_only.rs` | Ambos deben encontrar 0 símbolos | Recuento agregado |
| Cross-file call | `src/lib.rs::caller` llama a `src/nested/mod.rs::callee` | El símbolo `caller` debe tener un edge al símbolo `callee` | Recuento de edges entre símbolos específicos |
| Archivo con sintaxis rota | `src/broken.rs` con `pub fn oops(` | Después de F2.W2, `per_file` debe **reportarlo en `skipped`**; `full` lo sigue ignorando silenciosamente | Lista de `SkippedFile`s (per_file) vs 0 reporte (full) |
| Recorrido anidado profundo | `src/nested/deeply/deep.rs` | Ambos deben encontrar el símbolo | Recuento de símbolos en el subdirectorio |
| Diferentes firmas misma función | `src/lib.rs::compute(x: u32)` y `src/nested/mod.rs::compute(s: &str)` | Ambos deben encontrar 2 instancias con `qualified_name` distintos | Recuento de símbolos por qualified name |

## Lo que NO es objetivo de F2.W3

- **No forzar equivalencia bit-a-bit**: las dos estrategias
  indexan de forma diferente (full construye sobre la marcha;
  per_file tiene un cache con su propio lifecycle). Las divergencias
  legítimas (p. ej. orden de inserción en el grafo) se documentan
  como comportamiento esperado, no como bug.
- **No corregir `FullGraphStrategy`**: el mismo patrón R3 existe ahí
  (`_ => continue` en cada rama). Ese arreglo es scope de F2.W4
  (cerrar huecos) y NO de F2.W3.
- **No cambiar APIs públicas**: la caracterización es aditiva
  (`tests::test_*`); los call sites existentes no cambian.

## Estructura del corpus

```text
docs/prf/fixtures/equivalence_full_vs_perfile/
├── CORPUS.md                              (este archivo)
└── src/
    ├── lib.rs                             (hello, shared, caller)
    ├── dup.rs                             (dos funciones con mismo nombre)
    ├── empty.rs                           (archivo vacío)
    ├── comments_only.rs                   (solo comentarios)
    ├── broken.rs                          (sintaxis rota)
    └── nested/
        ├── mod.rs                         (shared, callee, compute(s))
        └── deeply/
            └── deep.rs                    (deep_symbol)
```
