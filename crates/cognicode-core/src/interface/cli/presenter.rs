//! T1-5.3d — Presenter for `IndexResult` (text format).
//!
//! Estado pre-T1-5.3d:
//!   - T1-5.3c landed `present_index_result` as a private function
//!     in `commands.rs` to keep T1-5.3c's commit minimal (data-side
//!     only). The private function emitted byte-identical text to
//!     the pre-migration `println!` calls in `execute_index`.
//!   - A private function can't be exercised by an integration test
//!     in `tests/`, so a regression in the text shape (an off-by-one
//!     in the indentation, a missing newline, a changed label) would
//!     only surface when a user reads the CLI and complains.
//!
//! What this module fixes:
//!   - The presenter lives here, takes `&mut dyn Write` instead of
//!     hardcoding `stdout`, and is `pub` so the test in
//!     `tests/t1_5_3d_index_presenter_snapshot.rs` can drive it
//!     with a `Vec<u8>` and pin the byte stream.
//!   - The bin (`cognicode-cli/src/main.rs` -> `CommandExecutor::execute`)
//!     still calls it the same way it did before; the call site
//!     just got one more argument (`&mut std::io::stdout()`).
//!
//! Política T1-5.3 (sin cambios):
//!   - La librería NO decide el transporte (text vs JSON).
//!   - Esta función decide "text" porque el caller le pasa un
//!     `Write`; cuando T1-5.5 añada JSON dispatch, ese path vive
//!     en el bin y formatea via `serde_json::to_string_pretty`.
//!     Aquí no se añade nada relacionado con JSON.
//!
//! Por qué `&mut dyn Write` y no `impl Write`:
//!   - `dyn Write` es lo que `std::io::stdout()` ya implementa
//!     (vía `Stdout`'s `Write` impl), así que el bin no cambia
//!     su ergonomía.
//!   - El test usa `Vec<u8>` (que también implementa `Write`) sin
//!     tener que envolver en `Cursor` o similar.
//!   - Es la firma estándar de la stdlib para "formateador puro
//!     sobre un Writer", lo que cualquier reviewer reconoce al
//!     instante.
//!
//! T1-5.5 will move this to `cognicode-cli` and add JSON dispatch.
//! That move is a path change, not a logic change: the text path
//! stays byte-identical.

use std::io::Write;

use crate::infrastructure::semantic::outline::OutlineNode;
use crate::interface::cli::result::IndexResult;

/// Render an `IndexResult` as text into `w`.
///
/// The output is byte-identical to the pre-migration `println!`
/// calls in `execute_index` (T1-5.1's snapshot and any downstream
/// grep-based UAT depend on this stability). The T1-5.3d snapshot
/// test pins the exact bytes for every variant.
pub fn present_index_result(result: &IndexResult, w: &mut dyn Write) -> std::io::Result<()> {
    use crate::interface::cli::result::IndexResult as R;
    match result {
        R::Build {
            path: _,
            strategy,
            elapsed_ms,
        } => {
            writeln!(
                w,
                "Index built successfully in {}ms using {} strategy",
                elapsed_ms, strategy
            )?;
        }
        R::Query {
            symbol: _,
            path: _,
            locations,
        } => {
            writeln!(w, "Found {} location(s):", locations.len())?;
            for loc in locations {
                writeln!(
                    w,
                    "  {}:{}:{} ({:?})",
                    loc.file, loc.line, loc.column, loc.symbol_kind
                )?;
            }
        }
        R::QueryEmpty { symbol, path: _ } => {
            writeln!(w, "No locations found for symbol '{}'", symbol)?;
        }
        R::Outline { file: _, symbols } => {
            writeln!(w, "Found {} top-level symbols:", symbols.len())?;
            write_outline_tree(symbols, 0, w)?;
        }
        R::SymbolCode {
            file: _,
            line: _,
            column: _,
            start_line,
            end_line,
            doc,
            code,
        } => {
            if let Some(d) = doc {
                writeln!(w, "\n/// Docstring:\n{}", d)?;
            }
            writeln!(w, "\n/// Symbol code (lines {} - {}):", start_line, end_line)?;
            writeln!(w, "{}", code)?;
        }
    }
    Ok(())
}

/// Recursive outline printer (extracted from the original
/// `print_outline_tree` in `commands.rs` to keep the call signature
/// of the test entry point short). The shape of the output
/// (indent unit, branch characters, kind/signature formatting)
/// matches the pre-migration version byte-for-byte; the T1-5.3d
/// test pins the `Outline` variant end-to-end.
fn write_outline_tree(nodes: &[OutlineNode], indent: usize, w: &mut dyn Write) -> std::io::Result<()> {
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let prefix = if indent == 0 {
            String::new()
        } else {
            format!("{}{}", "  ".repeat(indent - 1), if is_last { "└── " } else { "├── " })
        };

        let sig_info = node
            .signature
            .as_ref()
            .map(|s| format!(": {}", s))
            .unwrap_or_default();

        writeln!(w, "{}{} ({:?}){}", prefix, node.name, node.kind, sig_info)?;

        if !node.children.is_empty() {
            write_outline_tree(&node.children, indent + 1, w)?;
        }
    }
    Ok(())
}

// Allow the test to construct a `Location` for the `Outline` snapshot
// without depending on private fields. `Location::new` is the only
// public constructor and that's what we use here.
//
// `#[doc(hidden)]` keeps the module out of rustdoc (the only
// consumer is the integration test in `tests/`, and we don't want
// to advertise `outline_node` as a stable public API). The module
// is NOT gated by `#[cfg(test)]` because integration tests in
// `tests/` are compiled as a separate crate and don't see
// `cfg(test)` items from the lib they depend on.
#[doc(hidden)]
pub mod test_helpers {
    use super::*;
    use crate::domain::value_objects::Location;

    /// Build an `OutlineNode` from public fields. Exists only so the
    /// snapshot test in `tests/t1_5_3d_index_presenter_snapshot.rs`
    /// can construct a tree without depending on `Location`'s private
    /// fields.
    pub fn outline_node(
        name: &str,
        kind: crate::domain::value_objects::SymbolKind,
        line: u32,
        children: Vec<OutlineNode>,
    ) -> OutlineNode {
        OutlineNode {
            name: name.to_string(),
            kind,
            location: Location::new("/lib/foo.rs", line, 1),
            signature: None,
            children,
            is_private: false,
        }
    }
}
