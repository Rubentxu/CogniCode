# Capabilities Matrix (MCP tools × language × precision)

**MUST:** "cada operación stable declara soporte por lenguaje y precisión
semántica (p. ej. AST/LSP/heurística), incompletitud, límites y
exclusiones del corpus. `Unsupported` no es grafo vacío válido."

**Status:** Borrador inicial generado automáticamente desde
`crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs::cognicode_meta()`
y los handlers reales. Primer pase — no es declaración contractual
todavía; las decisiones de "qué tools son stable" las mantiene el
operador.

**Generado:** 2026-09-23 desde HEAD `ad975525` (sesión 4, AUTO).
**Pineado por:** `test_capabilities_matrix_for_stable_tools` en
`crates/cognicode-core/src/interface/mcp/capabilities.rs` (a añadir;
cierra PRF-ANA-01 cuando el test pase y la matriz esté firmada por
el operador).

---

## Lenguajes con parser tree-sitter

Este documento **no declara la lista**. La lista se deriva del registro
de parsers —`LanguageConfig::all()`, que es lo que liga un lenguaje a su
gramática compilada— y se publica en `product/languages.json`, que la
genera `scripts/product/generate_support_matrix.py` desde el mismo
registro.

Enumerarla aquí era una tercera fuente de verdad. Cuando lo estaba, las
tres se separaron: la lista de este documento y la constante del crate
coincidían entre sí —por eso el lint de drift daba verde— y ambas
mentían frente al registro. La lista anunciaba `jsx`, `tsx` y `luau`,
que son extensiones de JavaScript, TypeScript y Lua y no lenguajes que
el producto lea; y callaba doce que sí lee, con gramática compilada y
config en el registro.

Un consumidor que necesite la lista la lee del catálogo publicado, que
es la misma que genera el producto y la que acompaña al binario.

Cualquier archivo con extensión no reconocida por el registro cae en
**unsupported** (U15, `SkipReason::UnsupportedExtension`, §96). Las
extensiones `kt`/`kts` son una excepción tipada: a partir de K0 (línea
Kotlin, veredicto `2026-10-17-k0-honest-coverage.md`) viven en
`per_file_graph::KNOWN_UNSUPPORTED_EXTENSIONS` con la clasificación
explícita `WalkClassification::KnownUnsupportedGap`, no en
`SkipReason::UnsupportedExtension` — son un gap anunciado del que el
walk da cuenta, no un fallo silencioso. La línea Kotlin K1 los
re-añade a esta matriz cuando cablee `tree-sitter-kotlin` en el
registro `Language::from_extension`.

## Lenguajes con provider LSP (override de tree-sitter para navigation)

`CompositeProvider` en `crates/cognicode-core/src/infrastructure/lsp/providers/composite.rs`
intenta LSP server-side y cae a tree-sitter fallback. Lenguajes con
LSP config nativo en `lsp.rs::LspIntelligenceProvider`:

```
Python, Rust, JavaScript, TypeScript (4 lenguajes)
```

(El resto usa tree-sitter puro o — si no hay parser en absoluto —
`SkipReason::UnsupportedExtension`.)

## Tabla de capabilities por tool category

| Tool category        | Stability        | Lenguajes efectivos          | Precisión por defecto | Override LSP si disponible |
|----------------------|------------------|------------------------------|-----------------------|----------------------------|
| `graph`              | `stable`         | Todos los del parser (21)    | AST (call graph)      | n/a (graph no usa LSP)     |
| `graph`              | `experimental`    | Mismos 21                    | AST (call graph)      | n/a                        |
| `navigation`         | `stable`         | Solo con LSP: Python, Rust, JS, TypeScript | LSP + AST fallback | Sí (LSP si server, AST si no) |
| `search`             | `stable`         | Mismos 21 (sobre call graph) | Heuristic ranking     | n/a (heuristic sobre AST)  |
| `file`               | `stable`         | Mismos 21                    | AST (tree-sitter)     | No                         |
| `quality`            | `stable`         | Mismos 21                    | AST (métricas)        | No                         |
| `quality`            | `experimental`    | Mismos 21                    | AST                   | No                         |
| `refactor`           | `stable`         | Mismos 21                    | AST (rename)          | No                         |
| `view`               | `stable`         | language-agnostic (lee modelo) | n/a (no es parser) | n/a                        |
| `composite`          | `stable`         | Hereda de las tools subordinadas | Compuesto          | Hereda                     |
| `composite`          | `experimental`    | Hereda                       | Compuesto             | Hereda                     |
| `composite`          | `gated`          | Hereda                       | Compuesto             | Hereda                     |
| `aix`                | `experimental`    | language-agnostic (NL+heuristic) | Heuristic NL       | n/a                        |
| `infra`              | `experimental`    | language-agnostic             | n/a                  | n/a                        |

El conjunto de tools registradas, con su estabilidad y su categoría, vive
en `crates/cognicode-core/src/interface/mcp/capabilities.rs` y en
`product/tools.json`.

## Incompletitud y exclusiones del corpus (cubierto por U15/§96)

Cada tool `read` o `mutating` que recibe un `directory` o `file_path`
debe pasar por `AnalysisService::build_project_graph` (o equivalente).
El método ahora reporta `BuildReport { status, skipped }`:

- `status = Complete`: TODOS los archivos del walk se procesaron (sin
  skips ni read errors).
- `status = Partial { skipped }`: walk descubrió N archivos pero M
  fueron omitidos (parse fail, IO error, extensión no soportada).
- `status = Complete + skipped`: imposible por invariante (ver test
  `test_u15_unsupported_files_must_appear_in_build_report_skipped`).

Cada entrada de `skipped` lleva `reason` con uno de:

| Reason                          | Significa                                            |
|---------------------------------|------------------------------------------------------|
| `Read(String)`                  | IO error (permisos, not found, vanished)             |
| `Parse(String)`                 | tree-sitter falló en parsear el archivo              |
| `UnsupportedExtension(String)` | extensión sin parser asignado (e.g. `.cob`, `.txt`) |
| `Other(String)`                 | Otro error no clasificado                            |

## Test que pinea la matriz

`test_capabilities_matrix_for_stable_tools`, en
`crates/cognicode-core/src/interface/mcp/capabilities.rs`: recorre
`build_all_tools()` y exige que toda tool estable declare capacidades con
precisión no vacía. Una tool nueva que olvide declararlas rompe el test.

`c30_capabilities_declare_the_languages_the_registry_reads`, en
`crates/cognicode-core/tests/`: exige que los lenguajes que las
capacidades anuncien sean los que el registro lee, en los dos sentidos,
y que ningún nombre anunciado sea una extensión en vez de un lenguaje.

## Cómo actualizar esta matriz

1. Modificar `cognicode_meta(...)` en `rmcp_adapter.rs` para incluir el
   nuevo parámetro `capabilities: ToolCapabilities` (estructura con
   `langs: &[&'static str]`, `precision: &'static str`).
2. **No hay nada que actualizar a mano.** Un lenguaje nuevo entra
   añadiendo su config al registro de parsers, y de ahí sale solo: la
   lista de las capacidades se deriva, y el catálogo publicado se genera.
   `sandbox/scripts/capabilities_drift_lint.py --strict` falla si el
   documento vuelve a declarar una lista propia, o si la derivación no
   coincide con el catálogo publicado.
3. El test de arriba pinea que el cambio no rompa backward-compat
   (tool con capabilities vacías debe empezar a devolver `Some(non_empty)`).
4. La tabla por categoría no se actualiza: no tiene recuento.

## Decisiones operator-gated (no cerradas en este pase)

- **¿Qué tools pasan de `experimental` a `stable`?** Esto requiere
  UAT contra binario real; las 7 `experimental` graph + 3 composite +
  2 aix + 1 quality + 1 infra siguen experimentales hasta nuevo aviso.
- **¿Las `gated` tools deben ser visibles en `tools/list`?** La 1
  `gated` actual queda como `gated` y un futuro flag decidirá si
  es private o public.
- **¿Añadir tools `lua`/`zig`/`dart` con provider LSP?** Hoy no
  tienen LSP; si el operador quiere soporte LSP para esos, hay que
  añadir el provider en `infrastructure/lsp/providers/`.

**Refs:** U15/§96, herramienta:
`crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs`. Contrato de
capacidades: `crates/cognicode-core/src/interface/mcp/capabilities.rs`.
