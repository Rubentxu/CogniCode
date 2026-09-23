# PRF-ANA-01 — Capabilities Matrix (MCP tools × language × precision)

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

Lista oficial inferida de `crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs::from_extension`
(18 lenguajes):

```
Python, Rust, JavaScript, TypeScript, JSX, TSX, Go, Java,
C, Cpp, CSharp, Hcl (Terraform), Yaml, Ruby, Php, Swift,
Scala, Lua, Luau, Zig, Dart, Kotlin (todos via tree-sitter)
```

Cualquier archivo con extensión distinta a la lista cae en
**unsupported** (U15, `SkipReason::UnsupportedExtension`, §96).

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

| Tool category        | Stability        | # tools | Lenguajes efectivos          | Precisión por defecto | Override LSP si disponible |
|----------------------|------------------|---------|------------------------------|-----------------------|----------------------------|
| `graph`              | `stable` (29)    | 29      | Todos los del parser (18)    | AST (call graph)      | n/a (graph no usa LSP)     |
| `graph`              | `experimental` (7) | 7     | Mismos 18                    | AST (call graph)      | n/a                        |
| `navigation`         | `stable` (3)     | 3       | Solo con LSP: Python, Rust, JS, TypeScript | LSP + AST fallback | Sí (LSP si server, AST si no) |
| `search`             | `stable` (6)     | 6       | Mismos 18 (sobre call graph) | Heuristic ranking     | n/a (heuristic sobre AST)  |
| `file`               | `stable` (6)     | 6       | Mismos 18                    | AST (tree-sitter)     | No                         |
| `quality`            | `stable` (4)     | 4       | Mismos 18                    | AST (métricas)        | No                         |
| `quality`            | `experimental` (1) | 1     | Mismos 18                    | AST                   | No                         |
| `refactor`           | `stable` (1)     | 1       | Mismos 18                    | AST (rename)          | No                         |
| `view`               | `stable` (2)     | 2       | language-agnostic (lee modelo) | n/a (no es parser) | n/a                        |
| `composite`          | `stable` (8)     | 8       | Hereda de las tools subordinadas | Compuesto          | Hereda                     |
| `composite`          | `experimental` (3) | 3     | Hereda                       | Compuesto             | Hereda                     |
| `composite`          | `gated` (1)      | 1       | Hereda                       | Compuesto             | Hereda                     |
| `aix`                | `experimental` (2) | 2     | language-agnostic (NL+heuristic) | Heuristic NL       | n/a                        |
| `infra`              | `experimental` (1) | 1     | language-agnostic             | n/a                  | n/a                        |

**Totales:** 74 tool-meta invocations.

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

## Test RED→GREEN pineando la matriz

En `crates/cognicode-core/src/interface/mcp/capabilities.rs` (a crear):

```rust
#[test]
fn test_capabilities_matrix_for_stable_tools() {
    use crate::interface::mcp::rmcp_adapter::{build_all_tools, list_tool_capabilities};
    for tool in build_all_tools() {
        let caps = list_tool_capabilities(&tool.name.to_string())
            .unwrap_or_else(|| panic!("missing capabilities for stable tool '{}'", tool.name));
        // Toda tool estable debe declarar al menos un provider (no cadena vacía)
        assert!(
            !caps.precision.is_empty(),
            "tool estable '{}' no declara precisión semántica",
            tool.name
        );
    }
}
```

## Cómo actualizar esta matriz

1. Modificar `cognicode_meta(...)` en `rmcp_adapter.rs` para incluir el
   nuevo parámetro `capabilities: ToolCapabilities` (estructura con
   `langs: &[&'static str]`, `precision: &'static str`).
2. Re-correr el algoritmo de actualización (script en
   `sandbox/scripts/generate_capabilities_matrix.sh`).
3. El test arriba pinea que el cambio no rompa backward-compat
   (tool con capabilities vacías debe empezar a devolver `Some(non_empty)`).
4. La matriz de este documento se actualiza como docs(p) separado.

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

**Refs:** U15/§96, PRF-ANA-01, SPEC-ANALYSIS.md, herramienta:
`crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs`.
