# T1-5.3a — Design: `IndexResult` + tracing scaffolding para `execute_index`

> **Estado**: T1-5.3a DONE (design). El código de este doc es
> la **especificación** que T1-5.3c implementa. No es código
> que se commitea directamente — primero se valida el patrón
> con `execute_index`, luego se replica a los otros 9 subcomandos
> en T1-5.4..6.

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `odd/tasks/t1-5-presenter-move.md` | goal doc T1-5 |
| `odd/tasks/t1-5-2-classification.md` | output de T1-5.2: ejecut_index tiene ~12 `println!` y 2 `eprintln!` (no 30 como estimé, los demás sub-comandos tienen más) |
| `odd/tasks/t1-5-1-classification.md` | output de T1-5.1: snapshot baseline |
| `crates/cognicode-core/src/interface/cli/commands.rs:643..748` | **fuente** — la función `execute_index` actual, con los 4 sub-commands (Build/Query/Outline/SymbolCode) |
| T1-5.3c (próximo) | implementación basada en este design |

## 1. El patrón de migración (resumen)

**Antes (estado actual):**
```rust
async fn execute_index(command: &IndexCommand)
    -> Result<(), Box<dyn std::error::Error>>
{
    match command {
        IndexCommand::Build { path, strategy } => {
            println!("Building {} index at: {}", strategy, path);
            // ...
            println!("Index built successfully in {}ms using {} strategy",
                     elapsed, strategy_box.name());
            // ...
            eprintln!("Error building index: {}", e);
        }
        // ... 3 variantes más, todas con println!/eprintln!
    }
    Ok(())
}
```

**Después (diseño):**
```rust
async fn execute_index(command: &IndexCommand)
    -> Result<IndexResult, IndexError>
{
    match command {
        IndexCommand::Build { path, strategy } => {
            tracing::info!(target: "cli.index.build", strategy = %strategy, path = %path, "building index");
            let start = Instant::now();
            let mut strategy_box = GraphStrategyFactory::create(strategy);
            let dir = PathBuf::from(path);
            strategy_box.build_index(&dir).map_err(|e| IndexError::Build {
                path: dir.clone(),
                strategy: strategy.to_string(),
                source: e.into(),
            })?;
            let elapsed_ms = start.elapsed().as_millis();
            Ok(IndexResult::Build {
                path: dir,
                strategy: strategy.to_string(),
                elapsed_ms,
            })
        }
        IndexCommand::Query { symbol, path } => {
            tracing::info!(target: "cli.index.query", symbol, path, "querying symbol");
            let mut strategy = LightweightStrategy::new();
            let dir = PathBuf::from(path);
            strategy.build_index(&dir).map_err(|e| IndexError::Query {
                symbol: symbol.clone(),
                path: dir.clone(),
                source: e.into(),
            })?;
            let locations = strategy.query_symbols(symbol);
            if locations.is_empty() {
                Ok(IndexResult::QueryEmpty {
                    symbol: symbol.clone(),
                    path: dir,
                })
            } else {
                Ok(IndexResult::Query {
                    symbol: symbol.clone(),
                    path: dir,
                    locations,
                })
            }
        }
        IndexCommand::Outline { file, include_private, include_tests } => {
            tracing::info!(target: "cli.index.outline", file, "getting outline");
            let source = std::fs::read_to_string(file).map_err(|e| IndexError::Outline {
                file: file.clone(),
                source: e.into(),
            })?;
            let language = Language::from_extension(...).unwrap_or(Language::Rust);
            let outline = crate::infrastructure::semantic::build_outline(
                &source, file, language, *include_private, *include_tests,
            );
            Ok(IndexResult::Outline { file: file.clone(), symbols: outline })
        }
        IndexCommand::SymbolCode { file, line, column, include_doc: _ } => {
            tracing::info!(target: "cli.index.symbol_code",
                file, line, column, "getting symbol code");
            let service = SymbolCodeService::new();
            let code = service.get_symbol_code(file, *line, *column).map_err(|e| {
                IndexError::SymbolCode { file: file.clone(), line: *line, column: *column, source: e.into() }
            })?;
            Ok(IndexResult::SymbolCode {
                file: file.clone(),
                line: *line,
                column: *column,
                start_line: code.start_line,
                end_line: code.end_line,
                doc: code.docstring,
                code: code.code,
            })
        }
    }
}
```

## 2. El `IndexResult` (lo que el presenter recibe)

```rust
// crates/cognicode-core/src/interface/cli/result.rs (nuevo módulo)
use std::path::PathBuf;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IndexResult {
    /// `cognicode index build <path> <strategy>` — index built OK.
    Build {
        path: PathBuf,
        strategy: String,
        elapsed_ms: u128,
    },
    /// `cognicode index query <symbol> <path>` — symbol found at N locations.
    Query {
        symbol: String,
        path: PathBuf,
        locations: Vec<SymbolLocation>,
    },
    /// `cognicode index query <symbol> <path>` — symbol not found.
    QueryEmpty {
        symbol: String,
        path: PathBuf,
    },
    /// `cognicode index outline <file>` — outline extracted.
    Outline {
        file: PathBuf,
        symbols: Vec<OutlineSymbol>,
    },
    /// `cognicode index symbol-code <file>:<line>:<col>` — symbol source extracted.
    SymbolCode {
        file: PathBuf,
        line: u32,
        column: u32,
        start_line: u32,
        end_line: u32,
        doc: Option<String>,
        code: String,
    },
}

/// Re-exported from the existing infrastructure::semantic module.
#[derive(Debug, Serialize)]
pub struct SymbolLocation {
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
    pub symbol_kind: String, // canonical SymbolKind's display form
}

/// Re-exported from the existing infrastructure::semantic module.
#[derive(Debug, Serialize)]
pub struct OutlineSymbol {
    pub name: String,
    pub kind: String,
    pub line: u32,
    pub column: u32,
    pub children: Vec<OutlineSymbol>,
}
```

## 3. El `IndexError` (errors tipados, no strings)

```rust
// crates/cognicode-core/src/interface/cli/result.rs (mismo archivo)
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("Error building index at {path} using {strategy} strategy: {source}")]
    Build {
        path: PathBuf,
        strategy: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Error querying symbol '{symbol}' in {path}: {source}")]
    Query {
        symbol: String,
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Error reading outline source {file}: {source}")]
    Outline {
        file: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Error reading symbol code at {file}:{line}:{column}: {source}")]
    SymbolCode {
        file: PathBuf,
        line: u32,
        column: u32,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
```

**Por qué `#[source]`**: el consumer del error (el presenter) puede matchear sobre la variante Y acceder al source chain completo si necesita detalle (e.g. para logging estructurado). El `eprintln!("Error building index: {}", e)` actual pierde el chain (es un `Display`).

**Por qué `Box<dyn Error + Send + Sync>`**: el `GraphStrategy` y otros backends devuelven errores que no conocemos estáticamente; el `Send + Sync` permite al error cruzar el async boundary.

## 4. El presenter (en `cognicode-cli/src/bin/cogh.rs` o un nuevo módulo)

```rust
// crates/cognicode-cli/src/presenter/index.rs (nuevo)
use cognicode_core::interface::cli::result::{IndexResult, IndexError};

pub fn present(result: &IndexResult, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => present_text(result),
        OutputFormat::Json => present_json(result),
    }
}

fn present_text(result: &IndexResult) -> String {
    match result {
        IndexResult::Build { path, strategy, elapsed_ms } => {
            format!("Index built successfully in {}ms using {} strategy\n", elapsed_ms, strategy)
                // Note: NO incluimos "Building X index at: Y" aquí — eso es
                // un tracing event, no output del comando. El usuario que
                // quiera ver progreso usa `RUST_LOG=cli.index.build=info`.
        }
        IndexResult::Query { symbol, path, locations } => {
            let mut s = format!("Found {} location(s) for '{}' in {}:\n",
                locations.len(), symbol, path.display());
            for loc in locations {
                s.push_str(&format!("  {}:{}:{} ({})\n",
                    loc.file.display(), loc.line, loc.column, loc.symbol_kind));
            }
            s
        }
        IndexResult::QueryEmpty { symbol, path } => {
            format!("No locations found for '{}' in {}\n", symbol, path.display())
        }
        IndexResult::Outline { file, symbols } => {
            let mut s = format!("Found {} top-level symbols in {}:\n",
                symbols.len(), file.display());
            for sym in symbols {
                s.push_str(&present_outline_symbol(sym, 0));
            }
            s
        }
        IndexResult::SymbolCode { file, line, column, start_line, end_line, doc, code } => {
            let mut s = String::new();
            if let Some(d) = doc {
                s.push_str(&format!("\n/// Docstring:\n{}\n", d));
            }
            s.push_str(&format!("\n/// Symbol code (lines {} - {}):\n", start_line, end_line));
            s.push_str(code);
            s
        }
    }
}

fn present_json(result: &IndexResult) -> String {
    serde_json::to_string_pretty(result).expect("IndexResult serializes to JSON")
}
```

## 5. Tracing scaffolding

El `tracing` ya está en el workspace (`Cargo.toml:131`). El bin `cogh` necesita configurar el subscriber para que las trazas vayan a stderr (NO stdout — eso es AGENTS.md §17).

```rust
// crates/cognicode-cli/src/main.rs (modificar el init)
fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("cognicode_core=info,cognicode_cli=info"));
    fmt().with_env_filter(filter).with_writer(std::io::stderr).init();
}
```

**Por qué stderr**: stdout es la salida del comando; stderr es para logs. Mezclar ambos en stdout rompe el parsing JSON y la captura por pipe (AGENTS.md §17).

**Por qué `EnvFilter`**: el usuario puede subir la verbosidad con `RUST_LOG=cli.index.build=debug` sin recompilar.

## 6. El snapshot test (T1-5.3b, próximo)

Una vez implementado el migration, este test RED-detecta cualquier cambio de output:

```rust
// crates/cognicode-cli/tests/t1_5_index_snapshot.rs (nuevo)
use std::process::Command;

#[test]
fn snapshot_cognicode_index_query_no_locations() {
    let output = Command::new(env!("CARGO_BIN_EXE_cogh"))
        .args(["index", "query", "nonexistent_symbol_xyz", "."])
        .output()
        .expect("cogh must run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "No locations found for 'nonexistent_symbol_xyz' in .\n",
        "IndexResult::QueryEmpty must render this exact text"
    );
}

#[test]
fn snapshot_cognicode_index_query_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_cogh"))
        .args(["index", "query", "nonexistent_symbol_xyz", ".", "--format", "json"])
        .output()
        .expect("cogh must run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .expect("IndexResult::QueryEmpty must emit valid JSON");
    assert_eq!(json["kind"], "query_empty");
    assert_eq!(json["symbol"], "nonexistent_symbol_xyz");
    // ...
}
```

## 7. Riesgos identificados

- **R-1**: el `SymbolCodeService::get_symbol_code` devuelve `Result<_, String>` (no `Result<_, std::error::Error>`). El `String` se boxed en el error actual. La migración a `IndexError::SymbolCode` requiere un mapeo explícito `e.into()` que NO preserva el tipo (es una String). Si se necesita preservar el source chain, hace falta un `From<String>` para el error variant. Mitigación: empezar con `String` y migrar a `Box<dyn Error>` después.
- **R-2**: el presenter introduce un `--format json` mode. Si los consumers existentes (instaladores, scripts) parsean el text output, el cambio a `--format text` explícito puede romper. Mitigación: `--format text` es el default, así que el comportamiento sin `--format` no cambia.
- **R-3**: agregar `tracing` requiere inicializar el subscriber en el bin. Si el bin ya tiene un subscriber para otra cosa (e.g. `cognicode-mcp`), hay colisión. Mitigación: usar un guard "init once" con `OnceLock`.

## 8. Tareas siguientes (T1-5.3b..d)

- **T1-5.3b**: implementar el `IndexResult` + `IndexError` + `present()` (sin tocar `execute_index`). Compile-only, sin cambiar output.
- **T1-5.3c**: migrar `execute_index` al nuevo patrón. Wired en el test snapshot (T1-5.3d).
- **T1-5.3d**: añadir el test snapshot (T1-5.3b/d) y verificar que el output del bin `cogh` queda byte-a-byte igual.

## 9. Recibo (al cierre de T1-5.3a)

| Fecha | WU | Notas |
|---|---|---|
| 2026-10-10 | T1-5.3a | Design completo. `IndexResult` enum (5 variantes, `#[serde(tag = "kind")]`), `IndexError` enum (4 variantes con `#[source]`), `present()` para text + JSON, tracing scaffolding con `EnvFilter` + stderr writer, snapshot test design. Sin código commiteado — la implementación es T1-5.3b..d. |