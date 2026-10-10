//! T1-5.3b — Structured result types for the CLI subcommands.
//!
//! Estado pre-T1-5.3b:
//!   - `execute_index` (y los otros 9 `execute_*`) devolvían
//!     `Result<(), Box<dyn Error>>` con la información del éxito
//!     comunicada exclusivamente vía `println!`/`eprintln!`.
//!   - La librería no tenía cómo decirle al bin qué había
//!     pasado: el bin solo podía re-imprimir lo que la librería
//!     había decidido poner en stdout.
//!   - El bin no podía formatear diferente (text vs JSON) sin
//!     cambiar el código de la librería.
//!
//! Lo que este módulo aporta:
//!   - `IndexResult`: el resultado exitoso de cada variante
//!     de `IndexCommand` como dato estructurado. El bin lo
//!     formatea (text o JSON) sin que la librería sepa nada
//!     del transporte.
//!   - `IndexError`: el error tipado con `#[source]` para
//!     preservar la chain. El bin decide el formato del
//!     error (text line, JSON envelope, etc.).
//!   - `Serialize` en ambos: el JSON es canónico; el text es
//!     decisión del presenter.
//!
//! Política T1-5.3:
//!   - La librería NO formatea. Solo emite datos.
//!   - El bin (cognicode-cli) tiene el presenter.
//!   - El cambio de format es un commit de "format change"
//!     separado, no un side-effect del refactor de T1-5.3c.
//!
//! Ver `odd/tasks/t1-5-3a-index-design.md` para el design
//! completo (antes/después, riesgos, y plan de réplica a
//! los otros 9 sub-comandos en T1-5.4..6).

use serde::Serialize;
use std::path::PathBuf;

pub use crate::infrastructure::graph::SymbolLocation;
pub use crate::infrastructure::semantic::outline::OutlineNode;

/// Resultado exitoso de un `cognicode index <subcommand>`.
///
/// `#[serde(tag = "kind")]` permite serializar a JSON con un
/// campo discriminador (`"build"`, `"query"`, etc.) sin
/// anidamiento extra. El bin formatea text o JSON desde esta
/// misma estructura.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IndexResult {
    /// `cognicode index build <path> <strategy>` — index built OK.
    Build {
        path: PathBuf,
        strategy: String,
        elapsed_ms: u128,
    },
    /// `cognicode index query <symbol> <path>` — symbol found at N
    /// locations. El presenter text emite
    /// "Found {n} location(s):\n  FILE:LINE:COL (KIND)\n  ...".
    Query {
        symbol: String,
        path: PathBuf,
        locations: Vec<SymbolLocation>,
    },
    /// `cognicode index query <symbol> <path>` — symbol not found.
    /// Variante separada de `Query { locations: vec![] }` para
    /// que el JSON distinga entre "found zero" (raro pero
    /// posible) y "didn't find" (lo normal cuando no hay match).
    QueryEmpty {
        symbol: String,
        path: PathBuf,
    },
    /// `cognicode index outline <file>` — outline extracted.
    Outline {
        file: PathBuf,
        symbols: Vec<OutlineNode>,
    },
    /// `cognicode index symbol-code <file>:<line>:<col>` — source
    /// extracted for a symbol.
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

/// Error tipado de un `cognicode index <subcommand>`.
///
/// `#[source]` preserva la chain para que el presenter pueda
/// mostrar el mensaje del error original (no solo el wrapper).
/// `thiserror` deriva `Display` y `Error` por nosotros; el
/// `Box<dyn Error + Send + Sync>` permite al error cruzar
/// el async boundary y downcast-earse en el caller.
#[derive(Debug, thiserror::Error)]
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

impl IndexResult {
    /// `true` if the result represents a successful operation
    /// that the presenter should emit. Used by the bin to
    /// decide between `present(&result)` (success) and
    /// `present_error(&err)` (failure).
    pub fn is_empty_query(&self) -> bool {
        matches!(self, IndexResult::QueryEmpty { .. })
    }
}
