//! M5.1 — AST lifting from tree-sitter extraction to [`FunctionLocalView`].
//!
//! Per design D1 (m5-1-ast-lifting), this module is a pure value transform
//! over one or more [`ExtractionResult`]s. It produces the same shape the
//! M5 algorithms already accept (see `cognicode-graph-algos::algorithms::FunctionLocalView`),
//! so the conformance harness can swap synthetic flat slices for real
//! source-derived views without changing the algorithm layer.
//!
//! ## M5.1b — Statement extraction
//!
//! Since M5.1b, `FunctionLocalView.statements` is populated from the
//! `statements_by_function` map in each `ExtractionResult`. Statement-aware
//! algorithms (`cfg_per_function`, `dominators_cfg`, `slice_forward`,
//! `slice_backward`, `taint_flow`) are now served from lifted real source.
//!
//! ## Known limitations
//!
//! - **Cross-file callees are unresolved.** When the source has
//!   `TargetRef::Unresolved` callees, the lift records them as opaque
//!   indices in a separate vector; the M5 algorithms see only resolved
//!   indices for now.
//!
//! ## Feature gating
//!
//! The module compiles without the `program-analysis-server` feature, but the
//! test module that exercises real source IS gated on the feature (it requires
//! the tree-sitter Rust parser, which lives behind the same feature in the
//! workspace).

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use cognicode_graph_algos::algorithms::FunctionLocalView;

use crate::application::ingest::types::ExtractionResult;
use crate::domain::aggregates::generic_graph::GraphNode;
use crate::domain::value_objects::{NodeKind, SymbolKind};

/// Lift one or more extraction results into a function-local view + call-graph
/// adjacency pair.
///
/// Returns `(functions, call_graph)` where `call_graph[i]` is the list of
/// `functions` indices that `functions[i]` calls (directly, file-local).
///
/// The function is total: empty input → `(vec![], vec![])`; failed
/// extractions are skipped (REQ-LIFT-06).
///
/// **M5.1b:** `FunctionLocalView.statements` is populated from each
/// `ExtractionResult.statements_by_function` map keyed by the function's
/// `node.id`. Functions with no extracted statements get `Vec::new()`.
pub fn lift(extractions: &[ExtractionResult]) -> (Vec<FunctionLocalView>, Vec<Vec<usize>>) {
    let mut functions: Vec<FunctionLocalView> = Vec::new();
    // Map of `node.id` (FQN string) → index in `functions`.
    let mut function_index: HashMap<String, usize> = HashMap::new();

    // Pass 1: collect function symbols. We walk each extraction result in
    // order and append functions; the per-file source order is preserved
    // within a single file but multiple files are interleaved by their
    // order in `extractions`.
    for result in extractions {
        if result.error.is_some() {
            continue;
        }
        for node in &result.nodes {
            if let Some(mut view) = node_to_function_view(node) {
                let idx = functions.len();
                view.function_id = idx;
                function_index.insert(node.id.to_string(), idx);
                functions.push(view);
            }
        }
    }

    // Pass 2: walk `dependency.calls` edges and build the call adjacency.
    // For each edge where the source is a known function and the target
    // resolves to a known function in the same lift (target_ref is a
    // `Resolved(node_id)`), append the target index to the source's
    // `calls` list. Duplicates are de-duplicated but the input order is
    // preserved.
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); functions.len()];
    for result in extractions {
        if result.error.is_some() {
            continue;
        }
        for edge in &result.edges {
            if edge.kind != "dependency.calls" {
                continue;
            }
            let Some(&src_idx) = function_index.get(&edge.source) else {
                continue;
            };
            let target_idx = match &edge.target_ref {
                crate::application::ingest::types::TargetRef::Resolved(t) => {
                    function_index.get(t).copied()
                }
                crate::application::ingest::types::TargetRef::Unresolved(_) => None,
            };
            if let Some(tgt_idx) = target_idx
                && tgt_idx != src_idx
                && !adjacency[src_idx].contains(&tgt_idx)
            {
                adjacency[src_idx].push(tgt_idx);
            }
        }
    }

    // Pass 3: each view's `calls` field reflects its adjacency row.
    for (idx, view) in functions.iter_mut().enumerate() {
        view.calls.clone_from(&adjacency[idx]);
    }

    // Pass 4 (M5.1b): inject statements from statements_by_function map.
    // We need the node.id → statements mapping per result.
    for result in extractions {
        if result.error.is_some() {
            continue;
        }
        for node in &result.nodes {
            let node_id = node.id.to_string();
            if let (Some(&fn_idx), Some(stmts)) = (
                function_index.get(&node_id),
                result.statements_by_function.get(&node_id),
            ) {
                functions[fn_idx].statements.clone_from(stmts);
            }
        }
    }

    (functions, adjacency)
}

/// Convert one `GraphNode` into a `FunctionLocalView` if the node represents
/// a function. Returns `None` for any other kind (file, class, variable).
fn node_to_function_view(node: &GraphNode) -> Option<FunctionLocalView> {
    let NodeKind::Symbol(kind) = node.kind else {
        return None;
    };
    if !is_function_kind(kind) {
        return None;
    }
    // M5.1b: statements are injected post-construction in Pass 4 of `lift`.
    Some(FunctionLocalView {
        function_id: 0, // overwritten by `lift` after the index is known
        statements: Vec::new(),
        calls: Vec::new(),
    })
}

/// Returns `true` for kinds we treat as "function" for M5's purposes.
///
/// Function and Method are the obvious ones; we also include Trait (for
/// languages where traits are first-class callables) and Closure / Lambda
/// when present in the SymbolKind enum (currently absent, but the predicate
/// is forward-compatible).
fn is_function_kind(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::Function | SymbolKind::Method)
        || format!("{kind:?}") == "Closure"
        || format!("{kind:?}") == "Lambda"
}

/// Helper for tests: invoke `extract_file` for a small in-memory Rust source
/// without touching the filesystem.
///
/// Wraps the result in a one-element slice so it composes with `lift`.
#[cfg(feature = "program-analysis-server")]
pub fn lift_rust_source(path: &Path, source: &str) -> (Vec<FunctionLocalView>, Vec<Vec<usize>>) {
    use crate::application::ingest::extractor::extract_file;
    use crate::infrastructure::parser::language_config::RUST_CONFIG;
    let hash = "test-hash";
    let result = extract_file(&RUST_CONFIG, path, source, hash);
    lift(std::slice::from_ref(&result))
}

// ============================================================================
// Tests

#[cfg(test)]
#[path = "ast_lift_tests.rs"]
mod tests;
