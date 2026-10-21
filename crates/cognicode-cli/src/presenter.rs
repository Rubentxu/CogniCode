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

use std::io::{self, Write};

use cognicode_core::architecture::diagnostic::ArchitectureDiagnostic;
use cognicode_core::infrastructure::semantic::outline::OutlineNode;
use cognicode_core::interface::cli::commands::Format as CliFormat;
use cognicode_core::interface::cli::result::{
    CommandOutput, GraphResult, HoverResult, IndexResult, NavigateResult, ReferenceEntry,
};

/// Render an `IndexResult` as text into `w`.
///
/// The output is byte-identical to the pre-migration `println!`
/// calls in `execute_index` (T1-5.1's snapshot and any downstream
/// grep-based UAT depend on this stability). The T1-5.3d snapshot
/// test pins the exact bytes for every variant.
pub fn present_index_result(result: &IndexResult, w: &mut dyn Write) -> std::io::Result<()> {
    use cognicode_core::interface::cli::result::IndexResult as R;
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
            writeln!(
                w,
                "\n/// Symbol code (lines {} - {}):",
                start_line, end_line
            )?;
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
fn write_outline_tree(
    nodes: &[OutlineNode],
    indent: usize,
    w: &mut dyn Write,
) -> std::io::Result<()> {
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let prefix = if indent == 0 {
            String::new()
        } else {
            format!(
                "{}{}",
                "  ".repeat(indent - 1),
                if is_last { "└── " } else { "├── " }
            )
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
    use cognicode_core::domain::value_objects::Location;

    /// Build an `OutlineNode` from public fields. Exists only so the
    /// snapshot test in `tests/t1_5_3d_index_presenter_snapshot.rs`
    /// can construct a tree without depending on `Location`'s private
    /// fields.
    pub fn outline_node(
        name: &str,
        kind: cognicode_core::domain::value_objects::SymbolKind,
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

// ===================================================================
// T1-5.4c — Presenter for `GraphResult` (text format).
// ===================================================================
//
// Estado pre-T1-5.4c:
//   - T1-5.4b migrated execute_graph to return GraphResult,
//     but the in-function println! calls still emit text. The
//     data side and the text side are produced together; the
//     bin currently discards the GraphResult via `let _result =
//     ...` because the text already happened inside the
//     function.
//   - T1-5.5 will move the text formatting to the bin (where
//     the JSON dispatch lives). For now the function emits text
//     AND returns the data; the snapshot tests pin the text so
//     a future presenter change in the bin is forced to match
//     the existing shape.
//
// Política:
//   - One function per GraphResult variant. Single-shot: takes
//     the data, writes the text. No progress lines (those are
//     tracing::info! candidates, not user-facing text).
//   - `&mut dyn Write` for testability (matches the index
//     presenter signature). The function callsite uses
//     `&mut std::io::stdout()`.
//
// T1-5.4c adds the first of the 11 presenters: `present_graph_perfile`.
// The other 10 are added in T1-5.4d..m, one per commit, each
// following the same flow:
//   1. Add `present_graph_<variant>(result, &mut dyn Write)` here.
//   2. Replace the println! block in execute_graph with a call
//      to the presenter (stdout).
//   3. Add a snapshot test in tests/t1_5_4_<x>_presenter_snapshot.rs.

// (no import needed: the presenter uses the fully-qualified
// `cognicode_core::interface::cli::result::GraphResult` path in its
// signature. A `use ... as _` import would also work but adds
// noise; the fully-qualified path is short enough.)

/// T1-5.4c pilot — text format for `GraphResult::PerFile`.
///
/// Pre-migration text (commands.rs:851 + 860..862):
///   Getting per-file graph for: {file}
///   Per-file graph for {file}:
///     Symbols: {n}
///     Dependencies: {n}
///
/// Four lines: one progress line (preserved as stdout, NOT
/// moved to tracing::info!, because the snapshot test pins the
/// user-visible byte stream) and a three-line result block.
/// No trailing blank line. The header has no indent; the two
/// counts are indented with two spaces (matches the other
/// graph sub-commands' bullet-style output).
pub fn present_graph_perfile(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::PerFile {
        file,
        symbols,
        dependencies,
        // The references the file states and nobody declared do not go
        // here: stdout is the pinned three-line block, and the command
        // writes them to stderr the way the full graph does. The JSON
        // presenter carries them, because there the whole result is the
        // answer.
        pending_references: _,
    } = result
    {
        // Progress line — same text as the pre-migration
        // println! at commands.rs:851.
        writeln!(w, "Getting per-file graph for: {}", file)?;
        // Result block — same three lines as commands.rs:860..862.
        writeln!(w, "Per-file graph for {}:", file)?;
        writeln!(w, "  Symbols: {}", symbols)?;
        writeln!(w, "  Dependencies: {}", dependencies)?;
    } else {
        // The presenter is paired with a specific variant. A
        // mismatch is a programming error, not a runtime error;
        // the panic surfaces in tests, not in user code.
        panic!(
            "present_graph_perfile called with non-PerFile GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4d — text format for `GraphResult::EntryPoints`.
///
/// Pre-migration text (commands.rs:1021, 1047..1058, 1060):
///   Finding entry points in: {path}
///   [blank line]
///   Entry points (no incoming edges):
///     {name} at {file}:{line}:{column}    (max 20)
///   [blank line]
///   Found {total} entry points in {elapsed_ms}ms
///
/// The 'Warning: graph is PARTIAL' eprintln! at commands.rs:1034
/// goes to stderr and is NOT part of this presenter's output
/// (snapshot tests pin stdout only; the warning stays inline
/// per the T1-5.4 decision in odd/tasks/t1-5-4-decision-partial-warning.md).
///
/// `total` is the un-truncated count (the data side carries
/// the FULL list even though the loop takes 20). The summary
/// line uses `total`, not `symbols.len()`, so the user sees
/// "Found 47 entry points" even when only 20 are listed.
pub fn present_graph_entrypoints(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::EntryPoints {
        path,
        symbols,
        total,
        elapsed_ms,
    } = result
    {
        // Progress line — same text as commands.rs:1021.
        writeln!(w, "Finding entry points in: {}", path)?;
        // Blank line + header — commands.rs:1047.
        writeln!(w)?;
        writeln!(w, "Entry points (no incoming edges):")?;
        // Data lines (up to 20) — commands.rs:1048..1058. The
        // data side already has at most 20 entries (built with
        // iter().take(20) at commands.rs:1116), so we just
        // iterate `symbols` without a slice.
        for s in symbols {
            writeln!(w, "  {} at {}:{}:{}", s.name, s.file, s.line, s.column)?;
        }
        // Summary line — commands.rs:1060.
        writeln!(w)?;
        writeln!(w, "Found {} entry points in {}ms", total, elapsed_ms)?;
    } else {
        panic!(
            "present_graph_entrypoints called with non-EntryPoints GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4d — text format for `GraphResult::LeafFunctions`.
///
/// Identical shape to EntryPoints; the only differences are
/// the progress label ("leaf functions" vs "entry points")
/// and the section header ("no outgoing edges" vs "no
/// incoming edges"). One presenter per variant keeps the
/// `match` in the caller a one-liner and lets each presenter's
/// snapshot test pin its exact text independently.
pub fn present_graph_leaffunctions(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::LeafFunctions {
        path,
        symbols,
        total,
        elapsed_ms,
    } = result
    {
        writeln!(w, "Finding leaf functions in: {}", path)?;
        writeln!(w)?;
        writeln!(w, "Leaf functions (no outgoing edges):")?;
        for s in symbols {
            writeln!(w, "  {} at {}:{}:{}", s.name, s.file, s.line, s.column)?;
        }
        writeln!(w)?;
        writeln!(w, "Found {} leaf functions in {}ms", total, elapsed_ms)?;
    } else {
        panic!(
            "present_graph_leaffunctions called with non-LeafFunctions GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4e — text format for `GraphResult::HotPaths`.
///
/// Pre-migration text (commands.rs:1031..1082):
///   Finding hot paths in: {path} (limit={limit}, min_fan_in={min_fan_in})
///   [Warning: graph is PARTIAL: N file(s) skipped (...)]   ← stderr, no snapshot
///   [blank line]
///   Hot paths (most called functions):
///   {Function:<40} {Fan-in:>8} {Fan-out:>8}  Location
///   --------------------------------------------------------------------------------
///   {name:<40} {fan_in:>8} {fan_out:>8}  {file}:{line}
///   ...
///   [blank line]
///   Found {N} hot paths in {elapsed_ms}ms
///
/// The column widths (40 / 8 / 8) and the 80-dash separator
/// are part of the contract — downstream consumers (notably
/// `cogh | column -t` and similar) depend on the alignment.
/// Pinning them in the snapshot catches a refactor that
/// changes the column widths.
///
/// The data is the `entries` Vec (already filtered by
/// min_fan_in in execute_graph). The presenter iterates
/// without further filtering. `limit` is shown in the
/// progress line but is not used in the table (the analyzer
/// returns at most `limit` rows; the data side already has
/// at most that many).
pub fn present_graph_hotpaths(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::HotPaths {
        path,
        limit,
        min_fan_in,
        entries,
        elapsed_ms,
    } = result
    {
        // Progress line — commands.rs:1031.
        writeln!(
            w,
            "Finding hot paths in: {} (limit={}, min_fan_in={})",
            path, limit, min_fan_in
        )?;
        // Blank + header — commands.rs:1067.
        writeln!(w)?;
        writeln!(w, "Hot paths (most called functions):")?;
        // Column header — commands.rs:1068..1071. The widths
        // are part of the contract; pin them in the snapshot.
        write!(w, "{:<40} {:>8} {:>8}  ", "Function", "Fan-in", "Fan-out")?;
        writeln!(w, "Location")?;
        // 80-dash separator — commands.rs:1072.
        writeln!(w, "{}", "-".repeat(80))?;
        // Data rows — commands.rs:1074..1079.
        for hp in entries {
            writeln!(
                w,
                "{:<40} {:>8} {:>8}  {}:{}",
                hp.name, hp.fan_in, hp.fan_out, hp.file, hp.line
            )?;
        }
        // Summary — commands.rs:1082.
        writeln!(w)?;
        writeln!(w, "Found {} hot paths in {}ms", entries.len(), elapsed_ms)?;
    } else {
        panic!(
            "present_graph_hotpaths called with non-HotPaths GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4f — text format for `GraphResult::Complexity`.
///
/// Pre-migration text (commands.rs:1304..1322):
///   Calculating complexity metrics for: {path}
///   [Warning: graph is PARTIAL: ...]   ← stderr, no snapshot
///   [blank line]
///   Complexity Metrics:
///     Total symbols: {n}
///     Total edges: {n}
///     Max depth: {n}
///     Cyclomatic complexity: {n}
///     High fan-out (>=10): {n}
///     Medium fan-out (5-9): {n}
///     Low fan-out (<5): {n}
///     Entry points: {n}
///     Leaf functions: {n}
///   [blank line]
///   Analysis completed in {elapsed}ms
///
/// All nine metrics are pinned. The fan-out buckets
/// (>=10, 5-9, <5) are easy to swap in a refactor (e.g. someone
/// renames '>=10' to '>=8' because the threshold was wrong);
/// the snapshot catches that.
///
/// The progress line is the only one without a leading blank
/// line; the section header 'Complexity Metrics:' is preceded
/// by a blank line (after the warning goes to stderr).
pub fn present_graph_complexity(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::Complexity {
        path,
        metrics,
        elapsed_ms,
    } = result
    {
        // Progress line — commands.rs:1276.
        writeln!(w, "Calculating complexity metrics for: {}", path)?;
        // Blank + section header — commands.rs:1304.
        writeln!(w)?;
        writeln!(w, "Complexity Metrics:")?;
        // 9 metric lines — commands.rs:1305..1319. Two-space
        // indent matches the pre-migration output exactly.
        writeln!(w, "  Total symbols: {}", metrics.total_symbols)?;
        writeln!(w, "  Total edges: {}", metrics.total_edges)?;
        writeln!(w, "  Max depth: {}", metrics.max_depth)?;
        writeln!(
            w,
            "  Cyclomatic complexity: {}",
            metrics.cyclomatic_complexity
        )?;
        writeln!(w, "  High fan-out (>=10): {}", metrics.high_fan_out_count)?;
        writeln!(
            w,
            "  Medium fan-out (5-9): {}",
            metrics.medium_fan_out_count
        )?;
        writeln!(w, "  Low fan-out (<5): {}", metrics.low_fan_out_count)?;
        writeln!(w, "  Entry points: {}", metrics.entry_point_count)?;
        writeln!(w, "  Leaf functions: {}", metrics.leaf_function_count)?;
        // Summary — commands.rs:1322.
        writeln!(w)?;
        writeln!(w, "Analysis completed in {}ms", elapsed_ms)?;
    } else {
        panic!(
            "present_graph_complexity called with non-Complexity GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4g — text format for `GraphResult::TracePathFound`.
///
/// Pre-migration text (commands.rs:1107, 1131..1148, 1155):
///   Tracing path from '{from}' to '{to}' in: {path}
///   [Warning: graph is PARTIAL: ...]   ← stderr, no snapshot
///   [blank line]
///   Path found ({N} hops):
///     1. {name} at {file}:{line}
///     2. {name} at {file}:{line}
///     ...
///   [blank line]
///   Trace completed in {elapsed}ms
///
/// The hop list is 1-indexed (the pre-migration code does
/// `i + 1` in the format string; the data side carries
/// `position: (i+1) as u32` so consumers don't have to
/// re-compute). The path has no leading '→ ' or similar
/// arrow; just a number prefix.
pub fn present_graph_tracepath_found(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::TracePathFound {
        from: _,
        to: _,
        hops,
        elapsed_ms,
    } = result
    {
        // The 'Tracing path from X to Y in: PATH' progress
        // line is NOT emitted by the presenter. The
        // TracePathFound variant doesn't carry the project
        // path (it's a side-channel in the function; the
        // data side cares only about from/to/hops), so the
        // presenter can't reproduce it. The progress line
        // stays in commands.rs as a println! (T1-5.5 will
        // move it to the bin's text path or a tracing event).
        //
        // The presenter emits only the result block + summary.
        writeln!(w)?;
        writeln!(w, "Path found ({} hops):", hops.len())?;
        for hop in hops {
            writeln!(
                w,
                "  {}. {} at {}:{}",
                hop.position, hop.name, hop.file, hop.line
            )?;
        }
        writeln!(w)?;
        writeln!(w, "Trace completed in {}ms", elapsed_ms)?;
    } else {
        panic!(
            "present_graph_tracepath_found called with non-TracePathFound GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4g — text format for `GraphResult::TracePathNotFound`.
///
/// Pre-migration text (commands.rs:1151, 1155):
///   [blank line]
///   No path found between '{from}' and '{to}'
///   [blank line]
///   Trace completed in {elapsed}ms
///
/// The progress line 'Tracing path from ...' stays in the
/// function (same reason as TracePathFound: the project
/// path is not in the data side's variant). The presenter
/// emits the result block + summary.
pub fn present_graph_tracepath_notfound(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::TracePathNotFound {
        from,
        to,
        elapsed_ms,
    } = result
    {
        writeln!(w)?;
        writeln!(w, "No path found between '{}' and '{}'", from, to)?;
        writeln!(w)?;
        writeln!(w, "Trace completed in {}ms", elapsed_ms)?;
    } else {
        panic!(
            "present_graph_tracepath_notfound called with non-TracePathNotFound GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4h — text format for `GraphResult::Mermaid`.
///
/// Pre-migration text (commands.rs:1159, 1197..1212):
///   Exporting to Mermaid format from: {path}
///   [Warning: graph is PARTIAL: ...]   ← stderr, no snapshot
///   [blank line]
///   Mermaid diagram ({N} chars):
///   [blank line]
///   {format-specific block}
///   [blank line]
///   Export completed in {elapsed}ms
///
/// Three format cases:
///   - 'svg' / 'png': emits a 'requires mermaid-cli' hint
///     and a 'Run: cat << EOF | mermaid -s' shell snippet.
///   - 'txt' (default) with diagram_len <= 2000: emits the
///     full diagram.
///   - 'txt' with diagram_len > 2000: emits the first 2000
///     chars + a '... (truncated, full output in file)'
///     line.
///
/// The 'truncated' field in the data side is computed in
/// commands.rs (not in the presenter) so the data side
/// carries the binary decision. The presenter uses the
/// flag to choose between the full-diagram and truncated
/// branches.
///
/// The 'Mermaid diagram (N chars):' header uses the full
/// diagram length (NOT the truncated length); the user
/// always sees the real size.
pub fn present_graph_mermaid(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::Mermaid {
        path: _,
        diagram,
        diagram_len,
        format,
        truncated,
        elapsed_ms,
    } = result
    {
        // Progress line stays in the function: the Mermaid
        // variant doesn't carry the project path (same
        // pattern as TracePath, T1-5.4g).
        //
        // Result block header.
        writeln!(w)?;
        writeln!(w, "Mermaid diagram ({} chars):", diagram_len)?;
        writeln!(w)?;
        // Format-specific block.
        match format.as_str() {
            "svg" | "png" => {
                writeln!(w, "Note: SVG/PNG export requires mermaid-cli")?;
                writeln!(w, "Run: cat << 'EOF' | mermaid -s\n{}\nEOF", diagram)?;
            }
            _ => {
                // The 'truncated' flag drives whether we
                // show the full diagram or the
                // 2000-char + 'truncated' notice. The data
                // side has already computed the flag based
                // on diagram_len > 2000.
                if *truncated {
                    let slice_end = diagram.len().min(2000);
                    writeln!(w, "{}", &diagram[..slice_end])?;
                    writeln!(w)?;
                    writeln!(w, "... (truncated, full output in file)")?;
                } else {
                    writeln!(w, "{}", diagram)?;
                }
            }
        }
        // Summary.
        writeln!(w)?;
        writeln!(w, "Export completed in {}ms", elapsed_ms)?;
    } else {
        panic!(
            "present_graph_mermaid called with non-Mermaid GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4i — text format for `GraphResult::OnDemand`.
///
/// Pre-migration text (commands.rs:795..826):
///   Building on-demand subgraph for '{symbol}' (depth={depth}, direction={direction})
///   [blank line]
///   Subgraph built in {elapsed}ms
///   Root: {root_name} ({file}:{line}:{column})
///   Entries: {entries_len}
///
/// The progress line includes the depth and direction
/// (pre-migration uses '{symbol}' with single quotes — pin
/// that in the snapshot). The result block has 3 lines: a
/// timing line, a 'Root:' line with the symbol's name and
/// 3-tuple location, and an 'Entries:' line with the
/// count.
///
/// The 'Root:' line uses 'name (file:line:column)' — note
/// the space-separated format (not 'name@file:line:col' or
/// 'name at file:line:col' as in other sub-commands). This
/// is the only sub-command that uses the parenthesized
/// 3-tuple form; the snapshot pins it.
pub fn present_graph_ondemand(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::OnDemand {
        symbol,
        depth,
        direction,
        elapsed_ms,
        root_name,
        root_file,
        root_line,
        root_column,
        entries_len,
    } = result
    {
        // Progress line — commands.rs:795.
        writeln!(
            w,
            "Building on-demand subgraph for '{}' (depth={}, direction={})",
            symbol, depth, direction
        )?;
        // Blank + result block — commands.rs:817..825.
        writeln!(w)?;
        writeln!(w, "Subgraph built in {}ms", elapsed_ms)?;
        writeln!(
            w,
            "Root: {} ({}:{}:{})",
            root_name, root_file, root_line, root_column
        )?;
        writeln!(w, "Entries: {}", entries_len)?;
    } else {
        panic!(
            "present_graph_ondemand called with non-OnDemand GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4j — text format for `GraphResult::Hierarchy`.
///
/// Pre-migration text (commands.rs:1221..1272):
///   Getting call hierarchy for 'SYMBOL' (depth=N, direction=DIR) in: PATH
///   [blank line]
///   Call hierarchy for 'SYMBOL':
///   Root: NAME at FILE:LINE:COL
///   [blank line]
///   Entries by depth:
///     Depth 1: N entries
///       - NAME (callers|callees) at FILE:LINE
///       - ...
///     Depth 2: N entries
///       - ...
///   [blank line]
///   Total entries: N in Xms
///
/// The 'Root:' line uses 'name at file:line:col' (4-tuple
/// form, with the trailing column), NOT the 'name (file:line:column)'
/// 3-tuple form used by OnDemand. Pin that distinction.
///
/// The per-depth entry lines are 2-space indented under the
/// '  Depth N: M entries' line. The 2-space indent is part
/// of the contract (a downstream consumer like 'cogh |
/// grep' depends on it).
///
/// The recursion is bounded by `depth` (the user-supplied
/// arg), and each depth's entries are bounded to 5
/// (matches the pre-migration iter().take(5) slice). The
/// data side already has these slices applied; the
/// presenter just iterates `buckets`.
pub fn present_graph_hierarchy(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::Hierarchy {
        symbol,
        depth: _,
        direction: _,
        root_file,
        root_line,
        root_column,
        buckets,
        total_entries,
        elapsed_ms,
    } = result
    {
        // The 'Getting call hierarchy for X (depth=N, direction=DIR)
        // in: PATH' progress line is NOT emitted by the
        // presenter. The Hierarchy variant doesn't carry the
        // project path (same pattern as TracePath/Mermaid);
        // the function emits the progress line. The presenter
        // emits the result block + summary.
        //
        // Result block + per-depth entries.
        writeln!(w)?;
        writeln!(w, "Call hierarchy for '{}':", symbol)?;
        writeln!(
            w,
            "Root: {} at {}:{}:{}",
            symbol, root_file, root_line, root_column
        )?;
        writeln!(w)?;
        writeln!(w, "Entries by depth:")?;
        for bucket in buckets {
            writeln!(
                w,
                "  Depth {}: {} entries",
                bucket.depth,
                bucket.entries.len()
            )?;
            for entry in &bucket.entries {
                writeln!(
                    w,
                    "    - {} ({}) at {}:{}",
                    entry.name, entry.direction, entry.file, entry.line
                )?;
            }
        }
        // Summary.
        writeln!(w)?;
        writeln!(w, "Total entries: {} in {}ms", total_entries, elapsed_ms)?;
    } else {
        panic!(
            "present_graph_hierarchy called with non-Hierarchy GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4k — text format for `GraphResult::Full` (stdout only).
///
/// Pre-migration stdout (commands.rs:898, 985..987):
///   Building full project graph at: {path}{ (rebuild) ? }
///   Full graph built in {elapsed}ms
///     Total symbols: {n}
///     Total dependencies: {n}
///
/// The 'Skipped: {file}: {reason}' lines go to STDERR
/// (pre-migration eprintln! at commands.rs:989). They are
/// NOT part of this presenter's output (same pattern as
/// the 'Warning: graph is PARTIAL' eprintln! in the other
/// sub-commands: stderr side-channel, not data-side output).
/// The data side still carries `skipped_files` for the
/// JSON dispatch (T1-5.5); the eprintln! stays in
/// commands.rs.
///
/// The json branch in commands.rs uses a `FullGraphJson`
/// ad-hoc struct that the bin serializes separately; T1-5.5
/// will consolidate.
pub fn present_graph_full(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::Full {
        path,
        rebuild,
        elapsed_ms,
        symbols,
        dependencies,
        status: _,
        skipped_files: _,
        pending_references: _,
    } = result
    {
        // Progress line (stdout in text mode; in json
        // mode this is suppressed and goes to stderr —
        // the dispatch in commands.rs handles that).
        writeln!(
            w,
            "Building full project graph at: {}{}",
            path,
            if *rebuild { " (rebuild)" } else { "" }
        )?;
        // Result block.
        writeln!(w, "Full graph built in {}ms", elapsed_ms)?;
        writeln!(w, "  Total symbols: {}", symbols)?;
        writeln!(w, "  Total dependencies: {}", dependencies)?;
        // Skipped lines NOT emitted here — see the doc
        // comment above.
    } else {
        panic!(
            "present_graph_full called with non-Full GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.4l — text format for `GraphResult::Impact`.
///
/// Two cases, branched on `direct_dependents == 0`:
///
/// Case A (no matching symbols): pre-migration text
/// (commands.rs:1603):
///   [blank line]
///   No symbols found matching '{symbol}'
///   [blank line]
///   Analysis completed in {elapsed}ms
///
/// Case B (matches found): pre-migration text
/// (commands.rs:1616..1679):
///   Analyzing impact of changing '{symbol}' in: {path}
///   [blank line]
///   Found {N} symbol(s) matching '{symbol}'
///   [blank line]
///   Impact Analysis for '{symbol}':
///     Risk Level: {RISK}
///     Direct dependents: {N}
///     Total impacted symbols: {N}
///     Impacted files: {N}
///   [blank line]
///   Impacted symbols (first 20):
///     1. {name}
///     ...
///   ... and {N} more
///   [blank line]
///   Impacted files (first 10):
///     1. {path}
///     ...
///   ... and {N} more
///   [blank line]
///   Analysis completed in {elapsed}ms
///
/// The progress line 'Analyzing impact of changing X in: PATH'
/// is NOT emitted by the presenter (the Impact variant
/// doesn't carry the project path; same pattern as
/// TracePath, Mermaid, Hierarchy).
///
/// The data side carries the FULL impacted_symbols/files
/// lists (not truncated to 20/10). The presenter applies
/// the truncation in the rendering pass: show first 20
/// symbols + '... and N more' if N > 20, first 10 files +
/// '... and N more' if N > 10.
pub fn present_graph_impact(
    result: &cognicode_core::interface::cli::result::GraphResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::GraphResult::Impact {
        symbol,
        risk_level,
        direct_dependents,
        total_impacted_symbols,
        impacted_files_count,
        impacted_symbols,
        impacted_files,
        elapsed_ms,
    } = result
    {
        if *direct_dependents == 0 {
            // Case A: no matching symbols.
            writeln!(w)?;
            writeln!(w, "No symbols found matching '{}'", symbol)?;
            writeln!(w)?;
            writeln!(w, "Analysis completed in {}ms", elapsed_ms)?;
        } else {
            // Case B: matches found.
            //
            // The 'Found N symbol(s) matching X' line is
            // pre-migration output that the data side can
            // reconstruct from direct_dependents.
            writeln!(w)?;
            writeln!(
                w,
                "Found {} symbol(s) matching '{}'",
                direct_dependents, symbol
            )?;
            writeln!(w)?;
            writeln!(w, "Impact Analysis for '{}':", symbol)?;
            writeln!(w, "  Risk Level: {}", risk_level)?;
            writeln!(w, "  Direct dependents: {}", direct_dependents)?;
            writeln!(w, "  Total impacted symbols: {}", total_impacted_symbols)?;
            writeln!(w, "  Impacted files: {}", impacted_files_count)?;
            if !impacted_symbols.is_empty() {
                writeln!(w)?;
                writeln!(w, "Impacted symbols (first 20):")?;
                for (i, s) in impacted_symbols.iter().enumerate().take(20) {
                    writeln!(w, "  {}. {}", i + 1, s)?;
                }
                if impacted_symbols.len() > 20 {
                    writeln!(w, "  ... and {} more", impacted_symbols.len() - 20)?;
                }
            }
            if !impacted_files.is_empty() {
                writeln!(w)?;
                writeln!(w, "Impacted files (first 10):")?;
                for (i, f) in impacted_files.iter().enumerate().take(10) {
                    writeln!(w, "  {}. {}", i + 1, f)?;
                }
                if impacted_files.len() > 10 {
                    writeln!(w, "  ... and {} more", impacted_files.len() - 10)?;
                }
            }
            writeln!(w)?;
            writeln!(w, "Analysis completed in {}ms", elapsed_ms)?;
        }
    } else {
        panic!(
            "present_graph_impact called with non-Impact GraphResult: {:?}",
            result
        );
    }
    Ok(())
}

// ===================================================================
// T1-5.6a — Presenter for `NavigateResult` (text format).
// ===================================================================
//
// Estado pre-T1-5.6a:
//   - `cognicode navigate ...` emitió ~25 `println!` y ~3 `eprintln!`
//     calls dentro de `execute_navigate` (commands.rs:1606..1714). El
//     bin no podía cambiar la salida a JSON: el library formateaba
//     directamente y descartaba datos en `let _ = ...`.
//   - T1-5.5 ya consolidó el patrón "data side en core, presenter
//     en bin" para Index y Graph. T1-5.6a replica ese patrón para
//     Navigate: tres variants (Definition / Hover / References),
//     tres funciones `present_navigate_*` aquí, una rama en
//     `present_output`, y el JSON dispatch vía `present_navigate_result_json`.
//
// Política T1-5.3 (sin cambios):
//   - Text byte-identical con el pre-migration `commands.rs` execute_navigate
//     (los snapshot tests en `tests/t1_5_6a_navigate_presenter_snapshot.rs`
//     pinean cada variant). Si cambia el wording, la indentación, las
//     anchuras de columna o el formato Debug de `[KIND]`, esos tests
//     fallan y fuerzan una actualización consciente.
//
// Detalles preservados:
//   - Las progress lines ("Go to definition: {file}:{line}:{col}
//     (workspace: {workspace})" + "Connecting to LSP server...")
//     se emiten en el presenter (eran inline en el pre-migration).
//     El data side lleva `file/line/column/workspace` ya parseados
//     para reconstruir esa línea.
//   - Las references se imprimen como
//     `  {file}:{line}:{col} [{kind:?}] in {container}` — el Debug
//     format de `ReferenceKind` da PascalCase ("Read" / "Call" /
//     "Import" / "Write" / "Type"); la substitución `(unknown)` se
//     aplica ya en `execute_navigate` antes de guardar el entry.

/// T1-5.6a — text format for `NavigateResult::Definition`.
///
/// Pre-migration text (commands.rs:1614..1632):
///   Go to definition: {file}:{line}:{column} (workspace: {workspace})
///   Connecting to LSP server...
///   Definition found:
///     File:   {def.file}
///     Line:   {def.line}  (1-indexed; pre-migration did def.line() + 1)
///     Column: {def.column}
/// — or — when the LSP returns None:
///   Go to definition: {file}:{line}:{column} (workspace: {workspace})
///   Connecting to LSP server...
///   No definition found for {file}:{line}
///
/// The `  File:   ` / `  Line:   ` / `  Column: ` two-space indent
/// and column widths (4 / 4 / 6) are part of the contract (downstream
/// `column -t` consumers depend on the alignment).
pub fn present_navigate_definition(
    result: &cognicode_core::interface::cli::result::NavigateResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::NavigateResult::Definition {
        file,
        line,
        column,
        workspace,
        location,
    } = result
    {
        // Progress lines — same as commands.rs:1614..1618.
        writeln!(
            w,
            "Go to definition: {}:{}:{} (workspace: {})",
            file, line, column, workspace
        )?;
        writeln!(w, "Connecting to LSP server...")?;
        // Result block — either the 4-line "found" block or the
        // 1-line "not found" message.
        match location {
            Some(def) => {
                writeln!(w, "Definition found:")?;
                writeln!(w, "  File:   {}", def.file)?;
                writeln!(w, "  Line:   {}", def.line)?;
                writeln!(w, "  Column: {}", def.column)?;
            }
            None => {
                writeln!(w, "No definition found for {}:{}", file, line)?;
            }
        }
    } else {
        panic!(
            "present_navigate_definition called with non-Definition NavigateResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.6a — text format for `NavigateResult::Hover`.
///
/// Pre-migration text (commands.rs:1642..1661):
///   Hover info: {file}:{line}:{column} (workspace: {workspace})
///   Connecting to LSP server...
///   Hover information:
///     Type:    {type_info}
///     [Docs:    {documentation}]   ← solo si documentation.is_some()
/// — or — when the LSP returns None:
///   Hover info: {file}:{line}:{column} (workspace: {workspace})
///   Connecting to LSP server...
///   No hover information found for {file}:{line}
///
/// The 4-space gap after `Type:` / `Docs:` is part of the contract
/// (visually aligns the two field labels despite "Docs" being
/// shorter than "Type").
pub fn present_navigate_hover(
    result: &cognicode_core::interface::cli::result::NavigateResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::NavigateResult::Hover {
        file,
        line,
        column,
        workspace,
        info,
    } = result
    {
        writeln!(
            w,
            "Hover info: {}:{}:{} (workspace: {})",
            file, line, column, workspace
        )?;
        writeln!(w, "Connecting to LSP server...")?;
        match info {
            Some(HoverResult {
                type_info,
                documentation,
            }) => {
                writeln!(w, "Hover information:")?;
                writeln!(w, "  Type:    {}", type_info)?;
                // Documentation optional: None => no "Docs:" line.
                // The data side already serializes documentation as
                // `Option<String>`; the JSON path makes the absence
                // explicit as `null`, the text path just omits the
                // line entirely.
                if let Some(doc) = documentation {
                    writeln!(w, "  Docs:    {}", doc)?;
                }
            }
            None => {
                writeln!(w, "No hover information found for {}:{}", file, line)?;
            }
        }
    } else {
        panic!(
            "present_navigate_hover called with non-Hover NavigateResult: {:?}",
            result
        );
    }
    Ok(())
}

/// T1-5.6a — text format for `NavigateResult::References`.
///
/// Pre-migration text (commands.rs:1675..1704):
///   Find references: {file}:{line}:{column} (workspace: {workspace})
///   Connecting to LSP server...
///   Found {n} reference(s):
///     {file}:{line}:{col} [{KIND}] in {container}
///     ...
/// — or — when the LSP returns an empty Vec:
///   Find references: {file}:{line}:{column} (workspace: {workspace})
///   Connecting to LSP server...
///   No references found for {file}:{line}
///
/// `{KIND}` uses the Debug format of `ReferenceKind` (PascalCase:
/// "Read" / "Call" / "Import" / "Write" / "Type"). The `(unknown)`
/// substitution happens in `execute_navigate` so the data side
/// carries a `String`, not `Option<String>`.
pub fn present_navigate_references(
    result: &cognicode_core::interface::cli::result::NavigateResult,
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    if let cognicode_core::interface::cli::result::NavigateResult::References {
        file,
        line,
        column,
        workspace,
        include_declaration: _,
        references,
    } = result
    {
        writeln!(
            w,
            "Find references: {}:{}:{} (workspace: {})",
            file, line, column, workspace
        )?;
        writeln!(w, "Connecting to LSP server...")?;
        if references.is_empty() {
            writeln!(w, "No references found for {}:{}", file, line)?;
        } else {
            writeln!(w, "Found {} reference(s):", references.len())?;
            for ReferenceEntry {
                file: rfile,
                line: rline,
                column: rcol,
                kind,
                container,
            } in references
            {
                writeln!(
                    w,
                    "  {}:{}:{} [{:?}] in {}",
                    rfile, rline, rcol, kind, container
                )?;
            }
        }
    } else {
        panic!(
            "present_navigate_references called with non-References NavigateResult: {:?}",
            result
        );
    }
    Ok(())
}

// ===================================================================
// T1-5.5 — JSON dispatch + top-level output dispatcher
// ===================================================================
//
// Estado pre-T1-5.5:
//   - El presenter solo emitía text (12 funciones `present_*` con
//     `&mut dyn Write`).
//   - El JSON de `cogh graph full --format json` vivía paralelo como
//     un `FullGraphJson` ad-hoc en `commands.rs::execute_graph::Full`.
//   - El bin no podía dispatchar `--format text|json` sobre los
//     `*Result` aunque ya eran `Serialize`.
//
// Lo que este bloque aporta:
//   - `present_index_result_json` y `present_graph_result_json`:
//     wrappers de `serde_json::to_writer_pretty` con newline final
//     (para que `cogh --format json index ... | jq` consuma bien).
//   - `present_graph_result`: dispatch interno de `&GraphResult` a
//     la función text correcta (sustituye a los 12 `match` arms
//     inline que vivían en `commands.rs` durante T1-5.4).
//   - `present_output`: el entry point consumido por
//     `cognicode-cli/src/main.rs`. Mapea `(&CommandOutput, Format)`
//     a la combinación text/JSON adecuada.
//
// Política T1-5.5:
//   - El text shape de `present_*` se queda byte-identical con
//     T1-5.3d/T1-5.4*. Los snapshot tests preexistentes siguen
//     pasando sin cambios.
//   - El JSON es `serde_json::to_writer_pretty(&result)` con un
//     `\n` final. El envelope (`#[serde(tag = "kind")]`) está
//     declarado en `result.rs`; el `kind` discriminador reemplaza
//     al `schema_version` ad-hoc del antiguo `FullGraphJson`.

/// Emit `IndexResult` as JSON (pretty-printed, newline-terminated).
///
/// `cognicode_core::interface::cli::result::IndexResult` carries
/// `#[serde(tag = "kind", rename_all = "snake_case")]`, so the JSON
/// has a top-level `"kind": "<variant>"|"` field that distinguishes
/// `Build` / `Query` / `QueryEmpty` / `Outline` / `SymbolCode`.
///
/// Trailing newline so `cogh --format json index ... | jq .kind`
/// works out of the box (jq tolerates trailing whitespace but the
/// line-based consumers in CI scripts do not).
pub fn present_index_result_json(result: &IndexResult, w: &mut dyn Write) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *w, result)?;
    w.write_all(b"\n")
}

/// Emit `GraphResult` as JSON (pretty-printed, newline-terminated).
///
/// `#[serde(tag = "kind", rename_all = "snake_case")]` gives each of
/// the 11 variants its own `"kind": "<variant>"|"` discriminator.
pub fn present_graph_result_json(result: &GraphResult, w: &mut dyn Write) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *w, result)?;
    w.write_all(b"\n")
}

// ===================================================================
// T1-5.6a — JSON dispatch + dispatch top-level for `NavigateResult`
// ===================================================================
//
// Estado pre-T1-5.6a:
//   - `cognicode navigate ...` no tenía JSON dispatch (T1-5.5 lo
//     consolidó para Index y Graph pero Navigate se quedó en
//     `CommandOutput::Other`). El bin ignoraba `--format json` para
//     navigate silenciosamente.
//
// Lo que este bloque aporta:
//   - `present_navigate_result_json`: wrapper de `serde_json::to_writer_pretty`
//     con newline final, paralelo a `present_index_result_json`.
//   - `present_navigate_result`: dispatch interno de `&NavigateResult`
//     a la función text correcta (sustituye a los 3 brazos del
//     `match` inline que vivía en `commands.rs::execute_navigate`).
//   - Rama `CommandOutput::Navigate(r)` en `present_output`.

/// Emit `NavigateResult` as JSON (pretty-printed, newline-terminated).
///
/// `#[serde(tag = "kind", rename_all = "snake_case")]` gives each of
/// the 3 variants (`"definition"` / `"hover"` / `"references"`) its
/// own `"kind": "<variant>"` discriminator. `DefinitionLocation`,
/// `HoverResult`, and `ReferenceEntry` are serialized via their
/// own `Serialize` derives — `ReferenceKind` uses its Debug-derived
/// PascalCase variant name in the JSON (matching the text path's
/// `[KIND]` form, kept consistent so a single consumer can switch
/// between text and JSON without surprises).
pub fn present_navigate_result_json(result: &NavigateResult, w: &mut dyn Write) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *w, result)?;
    w.write_all(b"\n")
}

/// Dispatch a `&NavigateResult` to the text presenter for its variant.
/// Replaces the 3 inline `match` arms that lived in
/// `commands.rs::execute_navigate` pre-T1-5.6a.
fn present_navigate_result(result: &NavigateResult, w: &mut dyn Write) -> io::Result<()> {
    match result {
        NavigateResult::Definition { .. } => present_navigate_definition(result, w),
        NavigateResult::Hover { .. } => present_navigate_hover(result, w),
        NavigateResult::References { .. } => present_navigate_references(result, w),
    }
}

/// Dispatch a `&GraphResult` to the text presenter for its variant.
/// Replaces the 12 inline `match` arms that lived in
/// `commands.rs::execute_graph` during T1-5.4.
fn present_graph_result(result: &GraphResult, w: &mut dyn Write) -> io::Result<()> {
    match result {
        GraphResult::OnDemand { .. } => present_graph_ondemand(result, w),
        GraphResult::PerFile { .. } => present_graph_perfile(result, w),
        GraphResult::Full { .. } => present_graph_full(result, w),
        GraphResult::HotPaths { .. } => present_graph_hotpaths(result, w),
        GraphResult::EntryPoints { .. } => present_graph_entrypoints(result, w),
        GraphResult::LeafFunctions { .. } => present_graph_leaffunctions(result, w),
        GraphResult::TracePathFound { .. } => present_graph_tracepath_found(result, w),
        GraphResult::TracePathNotFound { .. } => present_graph_tracepath_notfound(result, w),
        GraphResult::Mermaid { .. } => present_graph_mermaid(result, w),
        GraphResult::Hierarchy { .. } => present_graph_hierarchy(result, w),
        GraphResult::Complexity { .. } => present_graph_complexity(result, w),
        GraphResult::Impact { .. } => present_graph_impact(result, w),
    }
}

/// Top-level output dispatcher — the entry point the bin calls.
///
/// The bin (cognicode-cli/src/main.rs) calls this once after
/// `CommandExecutor::execute` returns. `CommandOutput::Other` is
/// the marker for sub-commands not yet migrated to a structured
/// `*Result`; those already emitted their text internally so
/// the presenter is a no-op.
pub fn present_output(
    output: &CommandOutput,
    format: CliFormat,
    w: &mut dyn Write,
) -> io::Result<()> {
    match (output, format) {
        (CommandOutput::Index(r), CliFormat::Text) => present_index_result(r, w),
        (CommandOutput::Index(r), CliFormat::Json) => present_index_result_json(r, w),
        (CommandOutput::Graph(r), CliFormat::Text) => present_graph_result(r, w),
        (CommandOutput::Graph(r), CliFormat::Json) => present_graph_result_json(r, w),
        (CommandOutput::Navigate(r), CliFormat::Text) => present_navigate_result(r, w),
        (CommandOutput::Navigate(r), CliFormat::Json) => present_navigate_result_json(r, w),
        (CommandOutput::Architecture(r), CliFormat::Text) => present_architecture_diagnostic(r, w),
        (CommandOutput::Architecture(r), CliFormat::Json) => {
            present_architecture_diagnostic_json(r, w)
        }
        (CommandOutput::Other, _) => Ok(()),
    }
}

/// B4.W2 — Architecture diagnostic text output. Diagnostic is read-only
/// observations + evidence; the presenter summarizes them as bullet points.
pub fn present_architecture_diagnostic(
    diag: &ArchitectureDiagnostic,
    w: &mut dyn Write,
) -> io::Result<()> {
    writeln!(
        w,
        "architecture diagnostic: {} observation(s)",
        diag.observations.len()
    )?;
    writeln!(
        w,
        "  rules evaluated: {}  skipped: {}",
        diag.total_rules_evaluated, diag.total_rules_skipped
    )?;
    for obs in &diag.observations {
        writeln!(
            w,
            "  - [{}] {}: {}",
            match obs.severity {
                cognicode_core::architecture::diagnostic::Severity::Error => "error",
                cognicode_core::architecture::diagnostic::Severity::Warning => "warning",
                cognicode_core::architecture::diagnostic::Severity::Info => "info",
                cognicode_core::architecture::diagnostic::Severity::Hint => "hint",
            },
            obs.rule_id,
            obs.message
        )?;
        for ev in &obs.evidence {
            writeln!(
                w,
                "      @ {} [{}..{}]",
                ev.file.display(),
                ev.byte_range.start,
                ev.byte_range.end
            )?;
        }
    }
    Ok(())
}

/// B4.W2 — JSON dispatch for architecture diagnostics.
pub fn present_architecture_diagnostic_json(
    diag: &ArchitectureDiagnostic,
    w: &mut dyn Write,
) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *w, diag)?;
    w.write_all(b"\n")
}
