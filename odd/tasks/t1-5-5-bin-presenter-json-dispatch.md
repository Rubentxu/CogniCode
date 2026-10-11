# T1-5.5 — Mover el presenter al bin + dispatch `--format text|json`

> **Estado**: T1-5.5 DONE. SHAs: 810396a (5a+b) · 9456136 (5c) ·
> 1b8add5 (5f-prep) · 0e51d7a (5d) · 7bb8c3b (5e) · c2383a9 (5f).
> 49 snapshot tests verdes + 22 JSON dispatch tests verdes.
> Funcionalmente verificado: `cogh --format json index build .`
> emite `{ "kind": "build", "path": ".", ... }` con el `kind`
> discriminator que T1-5.4 diseñó.

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `odd/tasks/t1-5-presenter-move.md` | goal doc T1-5 (T1-5.5 cierra T1-5.5..5.6 del goal) |
| `odd/tasks/t1-5-3d-index-presenter-snapshot.md` | predecesor: snapshot del text path de IndexResult |
| `odd/tasks/t1-5-4-decision-partial-warning.md` | decisión heredada: partial-warn `eprintln!` queda inline en 6 arms de Graph |
| `crates/cognicode-core/src/interface/cli/result.rs` | nuevo `CommandOutput` enum + derives |
| `crates/cognicode-core/src/interface/cli/commands.rs` | return type de `execute` cambia a `Result<CommandOutput, _>` |
| `crates/cognicode-core/src/interface/cli/presenter.rs` | **borrado** (35K, 13 `pub fn` + test_helpers) |
| `crates/cognicode-cli/src/lib.rs` | **nuevo** — `[lib]` target para que `cognicode_cli::presenter` sea importable desde tests |
| `crates/cognicode-cli/src/presenter.rs` | **nuevo** — text (copiado byte-for-byte desde core) + JSON wrappers + dispatch top-level |
| `crates/cognicode-cli/src/main.rs` | dispatch `presenter::present_output(&output, cli.format, &mut stdout)` |
| `crates/cognicode-cli/tests/t1_5_5_json_dispatch.rs` | **nuevo** — 22 tests del JSON path |

## 1. El problema

Tras T1-5.3c (execute_index → IndexResult) y T1-5.4a..l (execute_graph → GraphResult, 11 sub-commands), el siguiente item del goal doc era T1-5.5: el bin consume los `*Result` y dispatcha `--format text|json`. El estado pre-T1-5.5 era una arquitectura a medias:

1. El bin **no podía emitir JSON** aunque los datos ya eran `Serialize`. La única ruta JSON ad-hoc era `cogh graph full --format json`, que vivía paralela al presenter en `commands.rs::execute_graph::Full` con un envelope `FullGraphJson { schema_version: "cognicode.graph.full/v1", ... }` hardcoded.
2. La librería **seguía formateando** — 13 call sites (`presenter::present_index_result` + 12 `crate::interface::cli::presenter::present_graph_*`) inline en `execute_index` y `execute_graph`. Violación de arquitectura hexagonal: la lib no debería tener presenter.
3. El módulo `presenter` estaba acoplado a `cognicode-core` aunque el responsable del output es el bin.
4. El `let _result = Self::execute_graph(command).await?;` (línea 603) descartaba el `GraphResult` calculado, dejando claro que el bin no podía consumirlo.

## 2. La solución

**Mover el presenter al bin y propagar `CommandOutput` por la frontera library → bin.**

### 2.1 Tipo de retorno

`CommandExecutor::execute` pasa de `Result<(), _>` a `Result<CommandOutput, _>`:

```rust
pub enum CommandOutput {
    Index(IndexResult),
    Graph(GraphResult),
    Other,
}
```

- `Index(r)`: `cogh index ...` migrado (T1-5.3c).
- `Graph(r)`: `cogh graph ...` migrado (T1-5.4).
- `Other`: marker para los 11 sub-comandos restantes (Navigate, Analyze, Doctor, FindUsages, Capabilities, ExportAssurance, Refactor, Evidence, DocsIngest, IssuesIngest) que aún emiten texto inline.

### 2.2 Top-level `--format text|json`

```rust
#[arg(long, value_enum, default_value_t = Format::Text)]
pub format: Format,
```

Default `text` preserva byte-identical con el pre-T1-5.5. Sub-comandos con `--format` per-variant propio (Capabilities, Doctor, FindUsages, Refactor, Graph::Full, Graph::Mermaid) lo mantienen y **ignoran** el top-level — el top-level es no-op para ellos. Esto se documenta en el help text.

### 2.3 Presenter en el bin

`crates/cognicode-cli/src/presenter.rs` (1109 líneas) tiene:

- **Text path** (copiado byte-for-byte de core): `present_index_result` + 12 `present_graph_*` + `pub mod test_helpers`.
- **JSON wrappers** (nuevos): `present_index_result_json` y `present_graph_result_json` — `serde_json::to_writer_pretty(&mut *w, result)` + `\n`.
- **Dispatch interno** (nuevo): `present_graph_result(&GraphResult, w)` matchea 12 variants → función text correcta.
- **Top-level dispatcher** (nuevo): `present_output(&CommandOutput, Format, w)` — entry point que llama `main.rs`.

### 2.4 Wiring en main.rs

```rust
let cli_format = cli.format;
let output = CommandExecutor::execute(cli).await?;

use cognicode_core::interface::cli::result::CommandOutput;
if !matches!(output, CommandOutput::Other) {
    let mut stdout = std::io::stdout();
    cognicode_cli::presenter::present_output(&output, cli_format, &mut stdout)?;
}
```

`CommandOutput::Other` → bin no-op (el lib ya emitió el texto).
`CommandOutput::Index(r)` o `Graph(r)` → bin dispatcha según `cli_format`.

### 2.5 Consolidación del JSON de `GraphCommand::Full`

Se borra el `FullGraphJson` ad-hoc (35 líneas con `schema_version: "cognicode.graph.full/v1"`, `SkippedJson` anidado, branch `json_mode`). El arm `Full` ahora siempre construye `Ok(GR::Full { ... })` y el bin dispatcha. Cambio de wire format intencional:

**Antes (T1-5.4k):**
```json
{
  "schema_version": "cognicode.graph.full/v1",
  "path": "/repo",
  ...
}
```

**Después (T1-5.5):**
```json
{
  "kind": "full",
  "path": "/repo",
  "rebuild": false,
  ...
}
```

El `kind: "full"` reemplaza al `schema_version`. Consumidores que dependan del campo antiguo deben migrar.

## 3. Cambios

| Archivo | Cambio |
|---|---|
| `crates/cognicode-core/src/interface/cli/result.rs` | + `CommandOutput` enum. Imports `serde::{Deserialize, Serialize}` añadidos pero `Deserialize` se removió tras descubrir incompatibilidad con `u128` (ver §6). |
| `crates/cognicode-core/src/interface/cli/mod.rs` | − `pub mod presenter;`. + re-export `CommandOutput` y `Format`. |
| `crates/cognicode-core/src/interface/cli/commands.rs` | + `Format` clap value enum. + campo `format: Format` en `Cli`. `execute()` return → `Result<CommandOutput, _>`. **13 call sites al presenter eliminados**. `Full` arm pierde `FullGraphJson` + branch `json_mode` (consolidado). `_format` per-variant de `Full` se ignora (top-level canónico). 8 sites de `Cli { ... }` en tests internos actualizados con `format: Format::Text`. |
| `crates/cognicode-core/src/interface/cli/presenter.rs` | **BORRADO** (35K, 1005 líneas). |
| `crates/cognicode-cli/Cargo.toml` | + `[lib]` section (`name = "cognicode_cli"`, `path = "src/lib.rs"`). |
| `crates/cognicode-cli/src/lib.rs` | **NUEVO** (13 líneas). `pub use cognicode_core; pub mod presenter;`. |
| `crates/cognicode-cli/src/presenter.rs` | **NUEVO** (1109 líneas). Text + JSON + dispatch. |
| `crates/cognicode-cli/src/main.rs` | + `let cli_format = cli.format; let output = execute(cli).await?; + present_output(&output, cli_format, &mut stdout)?` (cuando `output` no es `Other`). |
| 11 snapshot tests `crates/cognicode-core/tests/t1_5_4*` + `t1_5_3d` | **MOVIDOS** vía `git mv` a `crates/cognicode-cli/tests/`. `use` paths reescritos `cognicode_core::interface::cli::presenter::` → `cognicode_cli::presenter::`. |
| `crates/cognicode-cli/tests/t1_5_5_json_dispatch.rs` | **NUEVO** (450 líneas). 22 tests del JSON path. |

## 4. Lo que los tests pinean

### Snapshot tests (migrados sin cambios en asserts)

11 suites graph + 1 index + 1 binary smoke = **49 tests verdes** byte-exact:

| Suite | Tests | Pin |
|---|---|---|
| `t1_5_3d_index_presenter_snapshot` | 6 | `IndexResult` text shape (Build, Query, QueryEmpty, Outline, SymbolCode con/sin doc) |
| `t1_5_4c_graph_perfile_presenter_snapshot` | 4 | `GraphResult::PerFile` text shape |
| `t1_5_4d_graph_entrypoints_leaffunctions_snapshot` | 4 | `EntryPoints` + `LeafFunctions` (≤20 rows, summary) |
| `t1-5-4e_graph_hotpaths_snapshot` | 4 | `HotPaths` con column widths `<40/8/8` + 80-dash separator |
| `t1_5_4f_graph_complexity_snapshot` | 3 | `Complexity` (9 metrics con fan-out labels) |
| `t1_5_4g_graph_tracepath_snapshot` | 4 | `TracePathFound` + `TracePathNotFound` |
| `t1_5_4h_graph_mermaid_snapshot` | 4 | `Mermaid` (txt no-truncated, txt truncated >2000, svg/png shell) |
| `t1_5_4i_graph_ondemand_snapshot` | 3 | `OnDemand` (Root 3-tuple) |
| `t1_5_4j_graph_hierarchy_snapshot` | 4 | `Hierarchy` (per-depth truncation at 5) |
| `t1_5_4k_graph_full_snapshot` | 3 | `Full` text shape (progress + 2 count lines) |
| `t1_5_4l_graph_impact_snapshot` | 4 | `Impact` (empty + full branches, 20/10 truncations) |
| `t1_5_snapshot` (binary smoke) | 4 | `cogh --version`, `cogh current`, `cogh doctor`, `cogh --help` byte stream |

### JSON dispatch tests (nuevos)

22 tests en `t1_5_5_json_dispatch.rs`:

| Test | Pin |
|---|---|
| `json_index_{build,query,query_empty,outline,symbol_code}_shape` (5) | `kind` discriminator presente; JSON parseable; campos clave |
| `json_index_has_trailing_newline` | Trailing `\n` para line-based consumers |
| `json_graph_{on_demand,per_file,full,hot_paths,entry_points,leaf_functions,trace_path_found,trace_path_not_found,mermaid,hierarchy,complexity,impact}_shape` (11) | `kind` discriminator presente; legacy `schema_version` ausente (test específico para `full`) |
| `present_output_text_index_uses_text_presenter` | Text path emite text byte stream (no JSON leak) |
| `present_output_json_index_uses_json_presenter` | Json path emite JSON con `kind: "build"` |
| `present_output_json_graph_uses_json_presenter` | Json path emite JSON con `kind: "per_file"` |
| `present_output_other_is_noop_for_either_format` | `CommandOutput::Other` no escribe a stdout (text ni json) |

## 5. Verificación end-to-end (pre-PR)

```bash
bash scripts/test-fast.sh

# Snapshot byte streams
cargo build --release --bin cognicode
diff <(/var/home/rubentxu/cargo-targets/release/cognicode index build /tmp/cog-test 2>/dev/null) \
     <(echo -n "Index built successfully in 0ms using lightweight strategy
")
# (manual; el snapshot inline ya pineó el byte stream)

# JSON dispatch
/var/home/rubentxu/cargo-targets/release/cognicode --format json index build /tmp/cog-test | head
# {
#   "kind": "build",
#   "path": "/tmp/cog-test",
#   "strategy": "lightweight",
#   "elapsed_ms": 0
# }

/var/home/rubentxu/cargo-targets/release/cognicode --format json graph per-file /tmp/cog-test/src/main.rs | head
# {
#   "kind": "per_file",
#   "file": "/tmp/cog-test/src/main.rs",
#   "symbols": 1,
#   "dependencies": 0
# }

# Format flag visible
/var/home/rubentxu/cargo-targets/release/cognicode --help | grep -- '--format'
#       --format <FORMAT>  Output format for structured commands ...

# Suite completa antes de push
bash scripts/test-full.sh    # fmt + clippy + cargo test --workspace + doctests + baseline
```

## 6. Gotcha: u128 en `elapsed_ms`

`IndexResult::Build.elapsed_ms` y 8 variantes de `GraphResult` usan `u128` (de `Instant::elapsed().as_millis()`). serde_json's default deserializer **no soporta u128** porque JSON no tiene tipo nativo de 128-bit integer.

Por eso los nuevos tests JSON parsean a `serde_json::Value` (no a la struct). Roundtrip completo requeriría `#[serde(with = "u128_as_string")]` o migrar el tipo a `u64` (cabe cualquier runtime realista, ~580 millones de años). Decisión: parse a `Value` para los tests; los consumers downstream que necesiten round-trip Rust puro pueden añadir el adapter ellos mismos o proponer la migración a `u64` como un follow-up.

Originalmente añadí `Deserialize` a `IndexResult` y `GraphResult` (todos los structs auxiliares), pero el round-trip fallaba con `Error("u128 is not supported")`. Revertí los derives y los tests usan `Value`.

## 7. Decisión: por qué `--format` top-level y no per-subcommand

Patrón existente (Capabilities, Doctor, FindUsages, Refactor, Graph::Full, Graph::Mermaid) tenía `--format text|json` per-subcommand. T1-5.5 introduce top-level `--format` con la semántica:

- **Aplica** a sub-comandos sin `--format` propio: Index (5 variants), Graph (9 de 11 — OnDemand, PerFile, HotPaths, EntryPoints, LeafFunctions, TracePath, Hierarchy, Complexity, Impact).
- **No aplica** a sub-comandos con `--format` propio (mantienen su flag, ignoran el top-level).
- **Graph::Full** y **Graph::Mermaid** entran en la segunda categoría. Su `--format` per-variant se queda declarado en clap (no breaking change), pero el `_format` que llega a `execute_graph::Full` se ignora (top-level canónico, consolidado con el JSON unified dispatch).

Si el usuario hace `cogh --format json graph full`, el top-level decide. Si hace `cogh graph full --format json` (sin top-level), el top-level es `Text` por default → emite text. **Esto es un cambio sutil** respecto a T1-5.4k (donde `cogh graph full --format json` emitía el `FullGraphJson` ad-hoc). Documentado en el help text del flag.

## 8. Recibo

| WU | SHA | Commit | Notas |
|---|---|---|---|
| T1-5.5a+b | `810396a` | `feat(cli): T1-5.5a+b — add CommandOutput + Format + propagate return type` | Añade `CommandOutput` enum + `Format` clap value enum + cambia return de `execute` a `Result<CommandOutput, _>`. Inline presenter calls STAY (los borra 5d). 8 sitios `Cli { ... }` actualizados con `format: Format::Text`. |
| T1-5.5c | `9456136` | `feat(cli): T1-5.5c — create cognicode_cli::presenter with text + JSON + dispatch` | Crea `cognicode-cli/src/lib.rs` + `presenter.rs` con text (movido byte-for-byte) + JSON wrappers + dispatch. |
| T1-5.5f-prep | `1b8add5` | `test(cli): T1-5.5f-prep — move 12 presenter snapshot tests to cognicode-cli` | `git mv` 11 graph + 1 index snapshot de core/tests a cli/tests. `use` paths reescritos. |
| T1-5.5d | `0e51d7a` | `refactor(core): T1-5.5d — remove inline presenter calls + delete core presenter module` | 13 call sites eliminados (Index arm + 12 de execute_graph). `FullGraphJson` ad-hoc borrado. `presenter.rs` borrado de core. `mod.rs` actualizado. |
| T1-5.5e | `7bb8c3b` | `feat(cli): T1-5.5e — wire main.rs to cognicode_cli::presenter::present_output` | main.rs dispatcha con `cli_format` y `output` (Other → no-op). |
| T1-5.5f | `c2383a9` | `test(cli): T1-5.5f — add JSON dispatch tests for presenter + CommandOutput` | 22 tests nuevos en `t1_5_5_json_dispatch.rs`. Parse a `serde_json::Value` (no `Deserialize` derive por incompatibilidad u128). |

Verificación:
- 49 snapshot tests verdes (byte-identical text path).
- 22 JSON dispatch tests verdes.
- 7 serde tests verdes (`t1_5_3b_index_result_serde`).
- 12 serde tests verdes (`t1_5_4_graph_result_serde`).
- 4 binary smoke tests verdes (`t1_5_snapshot`).
- `cargo build --workspace` exit 0 (16 crates).
- Smoke E2E: `cognicode --format json index build /tmp/cog-test` emite `{ "kind": "build", ... }`. `cognicode --format json graph per-file <file>` emite `{ "kind": "per_file", ... }`.

Siguiente: **T1-5.6** — replicar el patrón para `execute_navigate`, `execute_analyze`, y los otros 8 `execute_*` aún con `println!` inline (~117 println! restantes). Multi-día. Fuera del scope de esta sesión.